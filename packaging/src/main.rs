//! Packages the plugin: the dev folder, the `.rgplugin` and the registry entry.
//!
//! The archive is byte-identical between runs: entries are sorted, timestamps are fixed at
//! 1980-01-01 and modes are set by hand, so a rebuild of the same binaries hashes the same.
//! The registry pins that hash, so a package must change only when its contents change.
//!
//!     cargo run --release -p package                  strict: every platform in `run` is built
//!     cargo run --release -p package -- --allow-missing  package what is built, for a try
//!     cargo run --release -p package -- --source-url URL  the address the entry points at

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

const MAX_PACKAGE: u64 = 64 * 1024 * 1024;
const MAX_ENTRIES: usize = 4096;
/// Where the releases of this plugin live, for the registry entry a local package writes.
/// `--source-url` overrides it: CI names the release it is building.
const REPOSITORY: &str = "Wolren/rustgis-dane-pl";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("package: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut allow_missing = false;
    let mut source_url: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--allow-missing" => allow_missing = true,
            "--source-url" => source_url = args.next(),
            other => return Err(format!("unknown argument '{other}'")),
        }
    }

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or("the packaging crate has no parent folder")?;
    let dist = root.join("dist");
    let manifest_path = root.join("plugin.json");
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("plugin.json: {e}"))?;
    let id = text(&manifest, "id")?;
    let version = text(&manifest, "version")?;

    let binaries = collect(&root, &dist, &manifest, allow_missing)?;

    // The dev folder the running app can point at, rebuilt from scratch each time.
    let staging = dist.join(&id);
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|e| format!("{}: {e}", staging.display()))?;
    }
    fs::create_dir_all(staging.join("bin")).map_err(|e| e.to_string())?;
    fs::copy(&manifest_path, staging.join("plugin.json")).map_err(|e| e.to_string())?;
    for (name, path) in &binaries {
        let target = staging.join("bin").join(name);
        fs::copy(path, &target).map_err(|e| format!("{}: {e}", path.display()))?;
        set_executable(&target)?;
    }

    let package = dist.join(format!("{id}-{version}.rgplugin"));
    if package.exists() {
        fs::remove_file(&package).map_err(|e| e.to_string())?;
    }
    let entries = write_archive(&package, &staging)?;

    let digest = sha256(&package)?;
    let url = source_url.unwrap_or_else(|| {
        format!(
            "https://github.com/{REPOSITORY}/releases/download/v{version}/{id}-{version}.rgplugin"
        )
    });
    let entry = json!({
        "id": id,
        "source": { "url": url, "sha256": digest },
        "categories": ["Data"],
    });
    let entry_path = dist.join(format!("{id}.json"));
    fs::write(
        &entry_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&entry).map_err(|e| e.to_string())?
        ),
    )
    .map_err(|e| e.to_string())?;

    let size = fs::metadata(&package).map_err(|e| e.to_string())?.len();
    println!(
        "package: {id} {version}, {entries} files, {}",
        platforms(&binaries)
    );
    println!("package: dist/{id}-{version}.rgplugin {size} bytes sha256 {digest}");
    println!("package: entry at dist/{id}.json");
    Ok(())
}

/// Every binary the manifest runs, by its path inside the package.
fn collect(
    root: &Path,
    dist: &Path,
    manifest: &Value,
    allow_missing: bool,
) -> Result<Vec<(String, PathBuf)>, String> {
    let mut wanted: Vec<(String, String)> = Vec::new();
    match manifest.get("run") {
        Some(Value::Object(platforms)) => {
            for (platform, argv) in platforms {
                if let Some(first) = argv.get(0).and_then(Value::as_str) {
                    if let Some(name) = first.strip_prefix("bin/") {
                        wanted.push((platform.clone(), name.to_string()));
                    }
                }
            }
        }
        Some(Value::Array(argv)) => {
            if let Some(name) = argv
                .get(0)
                .and_then(Value::as_str)
                .and_then(|a| a.strip_prefix("bin/"))
            {
                wanted.push(("default".into(), name.to_string()));
            }
        }
        _ => {}
    }
    if wanted.is_empty() {
        return Err("the manifest's run does not name a binary in bin/".into());
    }

    let mut found = Vec::new();
    let mut missing = Vec::new();
    for (platform, name) in wanted {
        let built = dist.join("bin").join(&name);
        let candidate = if built.is_file() {
            built
        } else {
            let fallback = root.join("target").join("release").join(&name);
            if fallback.is_file() {
                fallback
            } else {
                missing.push(format!("{platform} ({name})"));
                continue;
            }
        };
        found.push((name, candidate));
    }
    if found.is_empty() {
        return Err(format!("no binaries at all: {}", missing.join(", ")));
    }
    if !missing.is_empty() && !allow_missing {
        return Err(format!(
            "no binary for {}: build it or pass --allow-missing",
            missing.join(", ")
        ));
    }
    for gap in &missing {
        println!("package: no binary for {gap}, left out");
    }
    Ok(found)
}

/// Writes the archive and returns how many files went in.
fn write_archive(package: &Path, staging: &Path) -> Result<usize, String> {
    let mut files = Vec::new();
    walk(staging, staging, &mut files)?;
    files.sort();
    if files.len() > MAX_ENTRIES {
        return Err(format!(
            "{} entries, the limit is {MAX_ENTRIES}",
            files.len()
        ));
    }

    let file = fs::File::create(package).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipWriter::new(file);
    let stamp = zip::DateTime::from_date_and_time(1980, 1, 1, 0, 0, 0)
        .map_err(|e| format!("the fixed timestamp: {e}"))?;
    for inside in &files {
        let mode = if inside.starts_with("bin/") {
            0o755
        } else {
            0o644
        };
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .compression_level(Some(9))
            .last_modified_time(stamp)
            .unix_permissions(mode);
        archive
            .start_file(inside, options)
            .map_err(|e| format!("{inside}: {e}"))?;
        let bytes = fs::read(staging.join(inside)).map_err(|e| format!("{inside}: {e}"))?;
        archive
            .write_all(&bytes)
            .map_err(|e| format!("{inside}: {e}"))?;
    }
    archive.finish().map_err(|e| e.to_string())?;

    let size = fs::metadata(package).map_err(|e| e.to_string())?.len();
    if size > MAX_PACKAGE {
        return Err(format!("{size} bytes, the limit is {MAX_PACKAGE}"));
    }
    Ok(files.len())
}

/// Collects the file paths inside `dir`, relative to `root`, with forward slashes.
fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, out)?;
            continue;
        }
        let inside = path
            .strip_prefix(root)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        out.push(inside);
    }
    Ok(())
}

fn sha256(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let digest = Sha256::digest(&bytes);
    Ok(digest.iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

fn text(manifest: &Value, key: &str) -> Result<String, String> {
    manifest
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("plugin.json has no '{key}'"))
}

fn platforms(found: &[(String, PathBuf)]) -> String {
    let mut names: Vec<&str> = found.iter().map(|(name, _)| name.as_str()).collect();
    names.sort();
    names.join(", ")
}
