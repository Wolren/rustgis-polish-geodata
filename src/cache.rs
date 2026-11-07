//! A small on-disk cache for WFS pages.
//!
//! GUGiK's answers are big (a country's counties is 42 MB) and change slowly. Caching each
//! exact request for a week turns a repeated download into a file read. The key is the request
//! URL, hashed with FNV-1a; the cache is per user, under the app's data folder.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// How long an answer is reused.
pub const TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);

pub fn dir() -> PathBuf {
    root().join("cache")
}

/// Where a downloaded ASCII model is kept: named by its box, so a second click costs no download.
pub fn dem_file(name: &str) -> PathBuf {
    dem_dir().join(name)
}

/// The graphics layer a rectangle is dragged into, until a command takes it as the area.
pub fn area_draw_file() -> PathBuf {
    root().join("area.draw")
}

/// The drawing layer a drawn rectangle waits in, when one does.
pub fn pending_drawing() -> Option<String> {
    let name = fs::read_to_string(area_draw_file()).ok()?;
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_string())
}

pub fn remember_drawing(name: &str) {
    let _ = fs::write(area_draw_file(), name);
}

pub fn forget_drawing() {
    let _ = fs::remove_file(area_draw_file());
}

/// The mode of the last draw command, kept next to the cache so a model reads it in any session.
pub fn area_mode_file() -> PathBuf {
    root().join("area.mode")
}

/// The register the update tool wrote, if it wrote one; the plugin falls back to its own copy.
pub fn register_file() -> PathBuf {
    root().join("eziudp.json")
}

/// The panel's two download settings, kept beside the cache so they outlast a session.
pub struct Settings {
    pub confirm_large: bool,
    pub limit_mb: i64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { confirm_large: false, limit_mb: 25 }
    }
}

pub fn settings_file() -> PathBuf {
    root().join("settings.json")
}

/// The stored settings: the file's own values, the defaults for anything it lacks or garbles.
pub fn settings() -> Settings {
    let Ok(text) = fs::read_to_string(settings_file()) else {
        return Settings::default();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Settings::default();
    };
    Settings {
        confirm_large: value
            .get("confirm_large")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        limit_mb: value
            .get("limit_mb")
            .and_then(serde_json::Value::as_i64)
            .filter(|mb| (5..=500).contains(mb))
            .unwrap_or(25),
    }
}

pub fn remember_settings(settings: &Settings) {
    let body = format!(
        "{{\"confirm_large\": {}, \"limit_mb\": {}}}\n",
        settings.confirm_large, settings.limit_mb
    );
    let _ = fs::write(settings_file(), body);
}

pub fn dem_dir() -> PathBuf {
    root().join("dem")
}

fn root() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    base.join("RustGIS").join("dane-pl")
}

pub fn key(url: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in url.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// The cached answer for `url`, when it is younger than [`TTL`].
pub fn get(url: &str) -> Option<String> {
    let path = dir().join(key(url));
    let modified = fs::metadata(&path).ok()?.modified().ok()?;
    let age = SystemTime::now().duration_since(modified).ok()?;
    if age > TTL {
        return None;
    }
    fs::read_to_string(path).ok()
}

pub fn put(url: &str, body: &str) {
    let dir = dir();
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    let _ = fs::write(dir.join(key(url)), body);
}

/// Removes every cached answer; returns how many and how many bytes.
pub fn clear() -> Result<(usize, u64), String> {
    clear_in(&dir())
}

/// Removes the downloaded models; returns how many and how many bytes.
pub fn dem_clear() -> (usize, u64) {
    clear_in(&dem_dir()).unwrap_or((0, 0))
}

fn clear_in(dir: &std::path::Path) -> Result<(usize, u64), String> {
    let dir = dir.to_path_buf();
    if !dir.exists() {
        return Ok((0, 0));
    }
    let mut count = 0usize;
    let mut bytes = 0u64;
    for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if let Ok(meta) = entry.metadata() {
            bytes += meta.len();
        }
        fs::remove_file(entry.path()).map_err(|e| e.to_string())?;
        count += 1;
    }
    Ok((count, bytes))
}

/// How many answers are stored and how many bytes they take.
pub fn stats() -> (usize, u64) {
    stats_in(&dir())
}

/// How many model files are stored and how many bytes they take.
pub fn dem_stats() -> (usize, u64) {
    stats_in(&dem_dir())
}

/// Whether a file of this name is a finished answer, not a download's leftover.
fn finished(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    !name.ends_with(".part") && !name.starts_with('.')
}

fn stats_in(dir: &std::path::Path) -> (usize, u64) {
    let mut count = 0;
    let mut bytes = 0;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if !finished(&entry.file_name().to_string_lossy()) {
                continue;
            }
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    bytes += meta.len();
                    count += 1;
                }
            }
        }
    }
    (count, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_stable_and_distinct() {
        assert_eq!(key("https://a"), key("https://a"));
        assert_ne!(key("https://a"), key("https://b"));
        assert_eq!(key("https://a").len(), 16);
    }

    #[test]
    fn a_leftover_is_not_a_stored_answer() {
        assert!(finished("0123456789abcdef"));
        assert!(finished("DTM_PL-KRON86-NH_348129_497807_2317x2193.asc"));
        assert!(!finished("DTM_PL-KRON86-NH_348129_497807_2317x2193.part"));
        assert!(!finished(".DS_Store"));
    }
}
