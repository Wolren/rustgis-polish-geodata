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
    let Some(typed) = typed else {
        return Err(text::save_needs_path(lang));
    };
    let path = place(typed);
    if !path.is_absolute() {
        return Err(text::save_needs_path(lang));
    }
    if path.exists() {
        return Err(text::save_exists(lang, &path.display().to_string()));
    }
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

fn place(typed: &str) -> PathBuf {
    let path = Path::new(typed);
    if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("gpkg")) {
        path.to_path_buf()
    } else {
        path.with_extension("gpkg")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_extension_other_than_gpkg_is_replaced() {
        let path = place("C:\\temp\\granice.geojson");
        assert!(path.to_string_lossy().ends_with("granice.gpkg"), "{}", path.display());
    }
}
