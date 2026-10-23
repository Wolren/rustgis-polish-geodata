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
    let base = std::env::var("LOCALAPPDATA")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    base.join("RustGIS").join("dane-pl").join("cache")
}

fn key(url: &str) -> String {
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
    let dir = dir();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_stable_and_distinct() {
        assert_eq!(key("https://a"), key("https://a"));
        assert_ne!(key("https://a"), key("https://b"));
        assert_eq!(key("https://a").len(), 16);
    }
}
