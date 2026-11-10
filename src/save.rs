//! Saves a vector layer as a GeoPackage file through the host's own export.

use crate::clip;
use crate::host::Host;
use crate::text::{self, Lang};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub fn run(host: &mut Host, request: &Value, lang: Lang) -> Result<Value, String> {
    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
    let list = clip::layers(host, lang)?;
    let asked = params
        .get("warstwa")
        .or_else(|| params.get("layer"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty() && !matches!(*name, "active" | "aktywna"));
    let layer = match asked {
        Some(name) => {
            let vector = list.iter().any(|entry| {
                entry.get("name").and_then(Value::as_str) == Some(name)
                    && entry.get("raster").and_then(Value::as_bool) != Some(true)
            });
            if !vector {
                return Err(text::save_not_vector(lang, name));
            }
            name.to_string()
        }
        None => clip::target(&list, "").ok_or_else(|| text::save_no_layer(lang))?,
    };
    let typed = params
        .get("plik")
        .or_else(|| params.get("path"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|path| !path.is_empty());
    let path = match typed {
        Some(typed) => {
            let path = place(typed);
            if path.exists() {
                return Err(text::save_exists(lang, &path.display().to_string()));
            }
            path
        }
        None => free(default_path(&layer)),
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    host.call(
        "share.exportLayer",
        json!({"layer": layer, "path": path.display().to_string()}),
    )?;
    let bytes = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
    host.notify(&text::saved_geopackage(
        lang,
        &layer,
        &path.display().to_string(),
        bytes,
    ));
    Ok(json!({"layer": layer, "path": path.display().to_string(), "bytes": bytes}))
}

/// The folder a bare name lands in: Documents on any OS, else the home folder.
fn documents() -> PathBuf {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let documents = home.join("Documents");
    if documents.is_dir() {
        documents
    } else {
        home
    }
}

fn default_path(layer: &str) -> PathBuf {
    documents().join(format!("{}.gpkg", safe(layer)))
}

/// A typed path: a bare name goes to Documents, and the extension is always `.gpkg`.
fn place(typed: &str) -> PathBuf {
    let path = Path::new(typed);
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        documents().join(path)
    };
    if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("gpkg")) {
        path
    } else {
        path.with_extension("gpkg")
    }
}

fn safe(layer: &str) -> String {
    let cleaned: String = layer
        .chars()
        .map(|c| if "<>:\"/\\|?*".contains(c) { '_' } else { c })
        .collect();
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() {
        "layer".to_string()
    } else {
        cleaned
    }
}

/// A path no file holds, so the host's export never replaces an existing GeoPackage.
fn free(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_else(|| "layer".to_string());
    let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
    for number in 2..10_000 {
        let candidate = parent.join(format!("{stem} ({number}).gpkg"));
        if !candidate.exists() {
            return candidate;
        }
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_name_lands_in_documents_as_a_geopackage() {
        let path = place("granice gmin.gpkg");
        assert!(path.is_absolute(), "{}", path.display());
        assert!(path.to_string_lossy().ends_with("granice gmin.gpkg"), "{}", path.display());
    }

    #[test]
    fn an_extension_other_than_gpkg_is_replaced() {
        let path = place("C:\\temp\\granice.geojson");
        assert!(path.to_string_lossy().ends_with("granice.gpkg"), "{}", path.display());
    }

    #[test]
    fn an_unsafe_layer_name_is_clean() {
        assert_eq!(safe("a/b:c*?"), "a_b_c__");
        assert_eq!(safe("   "), "layer");
    }

    #[test]
    fn a_taken_default_gets_a_number() {
        let dir = std::env::temp_dir().join(format!("save-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let first = dir.join("layer.gpkg");
        std::fs::write(&first, b"x").unwrap();
        let next = free(first.clone());
        assert_eq!(next.file_name().unwrap(), "layer (2).gpkg");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
