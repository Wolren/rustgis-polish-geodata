//! The static EZiUDP register: every Polish WMS and WFS that answered a real request when
//! `tools/eziudp.py` built the file.
//!
//! The register itself is queried live by `eziudp`, which refuses the unfiltered nine megabytes.
//! This catalogue is the offline copy: 2 600 services that a tile or a capabilities document
//! proved, searchable by dataset, institution or TERYT, and addable as an overlay by id.

use crate::host::Host;
use crate::text::{self, Lang};
use serde::Deserialize;
use serde_json::{json, Value};

/// One service the register reports, as `data/eziudp.json` stores it.
#[derive(Deserialize)]
pub struct Registered {
    pub id: String,
    pub url: String,
    /// `wms` or `wfs`.
    pub kind: String,
    pub name: String,
    pub institution: String,
    #[serde(default)]
    pub unit: String,
    #[serde(default)]
    pub teryt: String,
    #[serde(default)]
    pub datasets: u64,
    /// The layer a GetMap asked for, empty for a WFS.
    #[serde(default)]
    pub layer: String,
}

const RAW: &str = include_str!("../data/eziudp.json");

/// The register's services, read once per process.
pub fn all() -> &'static [Registered] {
    static PARSED: std::sync::OnceLock<Vec<Registered>> = std::sync::OnceLock::new();
    PARSED.get_or_init(|| serde_json::from_str(RAW).unwrap_or_default())
}

pub fn find(id: &str) -> Option<&'static Registered> {
    all().iter().find(|entry| entry.id == id)
}

/// Searches the offline register: `szukaj` over the dataset and institution names, `instytucja`,
/// `teryt`, `rodzaj` (`wms` or `wfs`), and `limit`.
pub fn search(host: &mut Host, params: &Value, lang: Lang) -> Result<Value, String> {
    if let Some(id) = wanted(params, "dodaj") {
        let entry = find(&id).ok_or_else(|| text::register_unknown(lang, &id))?;
        return add_service(host, entry, lang);
    }
    query(params, lang)
}

/// The part of a search that needs no host: what an id names and what a filter matches.
pub fn query(params: &Value, lang: Lang) -> Result<Value, String> {
    let text_of = |key: &str| {
        params
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_lowercase())
    };

    let word = text_of("szukaj");
    let institution = text_of("instytucja");
    let teryt = text_of("teryt");
    let kind = text_of("rodzaj");
    let limit = params
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(50)
        .min(500) as usize;

    let matches = |entry: &Registered| {
        let name = format!("{} {} {}", entry.name, entry.institution, entry.unit).to_lowercase();
        let mut keep = true;
        if let Some(word) = &word {
            keep &= name.contains(word.as_str());
        }
        if let Some(institution) = &institution {
            keep &= entry.institution.to_lowercase().contains(institution.as_str());
        }
        if let Some(teryt) = &teryt {
            keep &= entry.teryt.to_lowercase().starts_with(teryt.as_str());
        }
        if let Some(kind) = &kind {
            keep &= entry.kind == *kind;
        }
        keep
    };

    let found: Vec<&Registered> = all().iter().filter(|entry| matches(entry)).collect();
    let entries: Vec<Value> = found
        .iter()
        .take(limit)
        .map(|entry| {
            json!({
                "id": entry.id,
                "dataset": entry.name,
                "institution": entry.institution,
                "unit": entry.unit,
                "teryt": entry.teryt,
                "kind": entry.kind,
                "url": entry.url,
                "layer": entry.layer,
                "datasets": entry.datasets,
            })
        })
        .collect();
    Ok(json!({
        "total": found.len(),
        "returned": entries.len(),
        "entries": entries,
        "how": "run_command  plugin:dane-pl/zrodla  {\"dodaj\": \"<id>\"} adds a WMS as an overlay",
    }))
}

/// A parameter's text, trimmed and kept only when it holds something.
fn wanted(params: &Value, key: &str) -> Option<String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// Adds one registered WMS as an overlay; a WFS is reported as not addable this way.
pub fn add_service(host: &mut Host, entry: &Registered, lang: Lang) -> Result<Value, String> {
    if entry.kind != "wms" || entry.layer.is_empty() {
        return Err(text::register_not_a_map(lang, &entry.name));
    }
    let mut url = format!(
        "{}?SERVICE=WMS&VERSION=1.3.0&REQUEST=GetMap&LAYERS={}&STYLES=&CRS=EPSG:3857&\
         BBOX={{bbox-epsg-3857}}&WIDTH={{width}}&HEIGHT={{height}}&FORMAT=image/png&TRANSPARENT=TRUE",
        entry.url, entry.layer
    );
    if entry.url.contains('?') {
        url = url.replacen('?', "&", 1);
    }
    let name = entry.name.clone();
    host.call(
        "map.addTiles",
        json!({
            "url": url,
            "kind": "raster",
            "name": name,
            "attribution": entry.institution,
            "overlay": true,
            "minZoom": 0,
            "maxZoom": 19,
        }),
    )?;
    let note = text::added(lang, &name);
    host.notify(&note);
    Ok(json!({ "added": name }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_is_read_whole() {
        assert!(!all().is_empty(), "data/eziudp.json lists nothing");
        let raw: Vec<Value> =
            serde_json::from_str(RAW).expect("data/eziudp.json is not a JSON array");
        assert_eq!(all().len(), raw.len(), "an entry was skipped: a field is missing");
    }

    #[test]
    fn every_entry_has_what_it_needs() {
        for entry in all() {
            assert!(!entry.id.is_empty() && !entry.url.is_empty(), "{}", entry.id);
            assert!(entry.kind == "wms" || entry.kind == "wfs", "{}", entry.id);
            assert!(entry.url.starts_with("http"), "{}", entry.id);
            if entry.kind == "wms" {
                assert!(!entry.layer.is_empty(), "{}", entry.id);
            }
        }
    }

    #[test]
    fn a_search_finds_a_warsaw_service() {
        let found = query(&json!({"instytucja": "Warszawy"}), Lang::En).unwrap();
        assert!(found["total"].as_u64().unwrap() > 0, "{found}");
    }
}
