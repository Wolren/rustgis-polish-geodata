//! The static EZiUDP register: every Polish WMS and WFS that answered a real request.
//!
//! `eziudp` queries the register live and refuses its unfiltered nine megabytes; this catalogue is
//! the offline copy, proved once by `tools/eziudp.py` or rebuilt at run time by `aktualizuj-rejestr`,
//! which keeps its file beside the cache and wins over the copy in the binary.

use std::collections::HashSet;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::cache;
use crate::host;
use crate::host::Host;
use crate::http;
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
/// The six register services the ribbon offers, as `data/ribbon.json` names them.
const RIBBON: &str = include_str!("../data/ribbon.json");

/// One ribbon entry: the command the tab runs and the register service it adds.
#[derive(Deserialize)]
struct RibbonEntry {
    command: String,
    id: String,
}

/// The service a ribbon command adds, when the command is one of the register's.
pub fn ribbon_target(command: &str) -> Option<&'static Registered> {
    let entries: Vec<RibbonEntry> = serde_json::from_str(RIBBON).ok()?;
    let entry = entries.into_iter().find(|entry| entry.command == command)?;
    find(&entry.id)
}

/// Whether every ribbon command names a WMS the register holds, for the manifest test.
#[cfg(test)]
pub fn ribbon_ids() -> Vec<(String, String)> {
    serde_json::from_str::<Vec<RibbonEntry>>(RIBBON)
        .unwrap_or_default()
        .into_iter()
        .map(|entry| (entry.command, entry.id))
        .collect()
}

/// The register this process uses: the update tool's file when it holds one, else the binary's.
pub fn all() -> &'static [Registered] {
    static PARSED: std::sync::OnceLock<Vec<Registered>> = std::sync::OnceLock::new();
    PARSED.get_or_init(|| {
        let updated = fs::read_to_string(cache::register_file())
            .ok()
            .and_then(|text| serde_json::from_str::<Vec<Registered>>(&text).ok())
            .filter(|entries: &Vec<Registered>| !entries.is_empty());
        updated.unwrap_or_else(|| serde_json::from_str(RAW).unwrap_or_default())
    })
}

pub fn find(id: &str) -> Option<&'static Registered> {
    all().iter().find(|entry| entry.id == id)
}

/// Searches the offline register: `szukaj` over the dataset and institution names, `instytucja`,
/// `teryt`, `rodzaj` (`wms` or `wfs`), `limit`, or `dodaj` with an id to add the WMS.
pub fn for_view(host: &mut Host, params: &Value, lang: Lang) -> Result<Value, String> {
    let (lon, lat) = host.centre()?;
    let mut county = None;
    for (x, y) in [(lon, lat), (lat, lon)] {
        if let Ok(found) = crate::egib::county_at(x, y, lang) {
            county = Some(found);
            break;
        }
    }
    let (code, name) = county.ok_or_else(|| text::register_off_county(lang))?;
    let kind = params
        .get("rodzaj")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|kind| matches!(*kind, "wms" | "wfs"))
        .unwrap_or("wms");
    let entries: Vec<Value> = all()
        .iter()
        .filter(|entry| {
            entry.kind == kind && (entry.teryt.starts_with(&code) || code.starts_with(&entry.teryt))
        })
        .map(|entry| {
            json!({
                "id": entry.id,
                "name": entry.name,
                "institution": entry.institution,
                "url": entry.url,
                "kind": entry.kind,
            })
        })
        .collect();
    host.notify(&text::register_county(lang, &name, &code, kind, entries.len()));
    Ok(json!({ "county": name, "teryt": code, "kind": kind, "count": entries.len(), "services": entries }))
}

pub fn search(host: &mut Host, params: &Value, lang: Lang) -> Result<Value, String> {
    if let Some(id) = wanted(params, "dodaj") {
        let entry = find(&id).ok_or_else(|| text::register_unknown(lang, &id))?;
        return add_service(host, entry, lang);
    }
    query(params, lang)
}

/// The part of a search that needs no host: what a filter matches.
pub fn query(params: &Value, lang: Lang) -> Result<Value, String> {
    let word = wanted(params, "szukaj").map(|value| value.to_lowercase());
    let institution = wanted(params, "instytucja").map(|value| value.to_lowercase());
    let teryt = wanted(params, "teryt").map(|value| value.to_lowercase());
    let kind = wanted(params, "rodzaj").map(|value| value.to_lowercase());
    let limit = params
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(50)
        .min(500) as usize;
    let _ = lang;

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
        "how": "run_command  plugin:polish-geodata/zrodla  {\"dodaj\": \"<id>\"} adds a WMS as an overlay",
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
    let separator = if entry.url.contains('?') { '&' } else { '?' };
    let url = format!(
        "{}{separator}SERVICE=WMS&VERSION=1.3.0&REQUEST=GetMap&LAYERS={}&STYLES=&CRS=EPSG:3857&\
         BBOX={{bbox-epsg-3857}}&WIDTH={{width}}&HEIGHT={{height}}&FORMAT=image/png&TRANSPARENT=TRUE",
        entry.url, entry.layer
    );
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
    host.notify(&text::added(lang, &name));
    Ok(json!({ "added": name }))
}

/// What one address needs to be kept: a WMS layer that draws a tile, or a WFS capabilities document.
fn prove(url: &str, kind: &str, lang: Lang) -> Result<String, String> {
    let joined = |extra: &str| {
        let separator = if url.contains('?') { '&' } else { '?' };
        format!("{url}{separator}{extra}")
    };
    if kind == "wfs" {
        let caps = http::fetch(&joined("SERVICE=WFS&VERSION=2.0.0&REQUEST=GetCapabilities"), 30, lang)?;
        if caps.contains("WFS_Capabilities") {
            return Ok(String::new());
        }
        return Err("not a WFS".into());
    }
    let caps = http::fetch(&joined("SERVICE=WMS&VERSION=1.3.0&REQUEST=GetCapabilities"), 30, lang)?;
    let layer = first_layer(&caps).ok_or("no layer")?;
    for area in ["1883782,6873571,1884382,6874171", "2075809,7236423,2076409,7237023"] {
        let tile = joined(&format!(
            "SERVICE=WMS&VERSION=1.3.0&REQUEST=GetMap&LAYERS={layer}&STYLES=&CRS=EPSG:3857\
             &BBOX={area}&WIDTH=256&HEIGHT=256&FORMAT=image/png&TRANSPARENT=TRUE"
        ));
        if http::fetch(&tile, 30, lang).is_ok() {
            return Ok(layer);
        }
    }
    Err("no tile".into())
}

/// The first layer name a GetMap can ask for.
fn first_layer(caps: &str) -> Option<String> {
    let mut rest = caps;
    while let Some(start) = rest.find("<Layer") {
        rest = &rest[start..];
        let end = rest.find("</Layer>")?;
        let block = &rest[..end];
        if let Some(name) = block.find("<Name>") {
            let text = &block[name + 6..];
            let text = text.strip_prefix("<![CDATA[").unwrap_or(text);
            let stop = text.find(['<', ']']).unwrap_or(0);
            let name = text[..stop].trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
        rest = &rest[1..];
    }
    None
}

/// The update tool: reads the register, proves the addresses the file lacks, and keeps the result.
pub fn update(params: &Value, lang: Lang) -> Result<Value, String> {
    let full = params.get("pelna").and_then(Value::as_bool).unwrap_or(false);
    let limit = params.get("limit").and_then(Value::as_u64).unwrap_or(0) as usize;
    host::progress_line(&text::register_reading(lang));

    let body = http::fetch("https://integracja.gugik.gov.pl/eziudp/api.php?format=json", 120, lang)?;
    let register: Value = serde_json::from_str(&body).map_err(|_| text::eziudp_unreadable(lang))?;
    let rows: Vec<Value> = register
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    host::progress_line(&text::register_rows(lang, rows.len()));

    let mut entries = group(&rows);
    let known: HashSet<String> = if full {
        HashSet::new()
    } else {
        all().iter().map(|entry| entry.url.clone()).collect()
    };
    let mut fresh: Vec<&mut Entry> = entries
        .iter_mut()
        .filter(|entry| !known.contains(&entry.url) && (entry.kind == "wms" || entry.kind == "wfs"))
        .collect();
    if limit > 0 {
        fresh.truncate(limit);
    }
    let total = fresh.len();
    host::progress_line(&text::register_checking(lang, total));

    let done = AtomicUsize::new(0);
    let proven = AtomicUsize::new(0);
    let queue = std::sync::Mutex::new(fresh);
    std::thread::scope(|scope| {
        for _ in 0..6 {
            scope.spawn(|| loop {
                let entry = {
                    let mut queue = queue.lock().unwrap_or_else(|e| e.into_inner());
                    match queue.pop() {
                        Some(entry) => entry,
                        None => break,
                    }
                };
                let kind = entry.kind.clone();
                let url = entry.url.clone();
                if let Ok(layer) = prove(&url, &kind, lang) {
                    entry.layer = layer;
                    entry.proven = true;
                    proven.fetch_add(1, Ordering::Relaxed);
                }
                let count = done.fetch_add(1, Ordering::Relaxed) + 1;
                host::progress_line(&text::register_progress(lang, count, total));
            });
        }
    });

    let mut kept: Vec<Value> = all().iter().map(entry_to_value).collect();
    kept.extend(entries.iter().filter(|entry| entry.proven).map(entry_value));
    kept.sort_by(|a, b| {
        let key = |value: &Value| {
            (
                value["institution"].as_str().unwrap_or("").to_lowercase(),
                value["name"].as_str().unwrap_or("").to_lowercase(),
            )
        };
        key(a).cmp(&key(b))
    });
    kept.dedup_by(|a, b| a["url"] == b["url"] && a["layer"] == b["layer"]);

    // A cancelled run leaves the old file alone: the new one appears only once it is whole.
    let path = cache::register_file();
    let temp = path.with_extension("json.tmp");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string(&kept).map_err(|e| e.to_string())?;
    fs::write(&temp, text).map_err(|e| e.to_string())?;
    fs::rename(&temp, &path).map_err(|e| e.to_string())?;

    let answered = proven.load(Ordering::Relaxed);
    let note = text::register_updated(lang, kept.len(), answered, total.saturating_sub(answered));
    Ok(json!({ "messages": [note] }))
}

/// The register as the panel's map list shows it: institution branches with their WMS leaves.
pub fn branches(lang: Lang, limit: usize) -> Vec<Value> {
    let other = pick_text(lang);
    let mut groups: Vec<(String, Vec<Value>)> = Vec::new();
    let mut leaves = 0usize;
    for entry in all() {
        if entry.kind != "wms" || leaves >= limit {
            continue;
        }
        let leaf = json!({ "id": format!("e:{}", entry.id), "label": entry.name });
        match groups.last_mut() {
            Some((name, list)) if *name == entry.institution => list.push(leaf),
            _ => groups.push((entry.institution.clone(), vec![leaf])),
        }
        leaves += 1;
    }
    groups
        .into_iter()
        .map(|(name, children)| {
            let label = if name.trim().is_empty() { other.to_string() } else { name };
            json!({ "id": format!("eziudp:{label}"), "label": label, "children": children })
        })
        .collect()
}

fn pick_text(lang: Lang) -> &'static str {
    text::register_other(lang)
}

/// One address the register reports, before it is proved.
struct Entry {
    id: String,
    url: String,
    kind: String,
    name: String,
    institution: String,
    unit: String,
    teryt: String,
    datasets: u64,
    layer: String,
    proven: bool,
}

fn entry_value(entry: &Entry) -> Value {
    json!({
        "id": entry.id,
        "url": entry.url,
        "kind": entry.kind,
        "name": entry.name,
        "institution": entry.institution,
        "unit": entry.unit,
        "teryt": entry.teryt,
        "datasets": entry.datasets,
        "layer": entry.layer,
    })
}

fn entry_to_value(entry: &Registered) -> Value {
    json!({
        "id": entry.id,
        "url": entry.url,
        "kind": entry.kind,
        "name": entry.name,
        "institution": entry.institution,
        "unit": entry.unit,
        "teryt": entry.teryt,
        "datasets": entry.datasets,
        "layer": entry.layer,
    })
}

/// Groups register rows into one entry per address, naming the first dataset that reports it.
fn group(rows: &[Value]) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for row in rows {
        let services = row.get("uslugi").cloned().unwrap_or(Value::Null);
        for key in ["przegladania", "pobierania", "wyszukiwania"] {
            for raw in services.get(key).and_then(Value::as_array).into_iter().flatten() {
                let Some(url) = address(raw.as_str().unwrap_or_default()) else {
                    continue;
                };
                if !seen.insert(url.clone()) {
                    continue;
                }
                let low = url.to_lowercase();
                let kind = if low.contains("wfs") {
                    "wfs"
                } else if low.contains("wms") || low.contains("mapserver") || low.contains("geoserver") {
                    "wms"
                } else {
                    continue;
                };
                entries.push(Entry {
                    id: id_of(&url),
                    url,
                    kind: kind.to_string(),
                    name: row.get("zbior").and_then(Value::as_str).unwrap_or_default().to_string(),
                    institution: row.get("organ").and_then(Value::as_str).unwrap_or_default().to_string(),
                    unit: row.get("jednostka").and_then(Value::as_str).unwrap_or_default().to_string(),
                    teryt: row.get("teryt").and_then(Value::as_str).unwrap_or_default().to_string(),
                    datasets: 1,
                    layer: String::new(),
                    proven: false,
                });
            }
        }
    }
    entries
}

/// The address itself: the register writes some of them as `WMS=http://…`.
fn address(raw: &str) -> Option<String> {
    let mut text = raw.trim();
    if let Some((head, rest)) = text.split_once('=') {
        if head.len() <= 4 && !head.contains('/') {
            text = rest.trim();
        }
    }
    if !text.to_lowercase().starts_with("http") {
        return None;
    }
    Some(text.trim_end_matches('&').trim_end_matches('/').to_string())
}

/// An id that survives a re-read: the address, folded to lower-case letters and digits.
fn id_of(url: &str) -> String {
    let folded: String = url
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let trimmed = folded.trim_matches('-');
    let tail: String = trimmed.chars().rev().take(48).collect::<Vec<_>>().into_iter().rev().collect();
    format!("eziudp-{}", tail.trim_matches('-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The file in the binary parses whole, so an update cannot lose an entry to a missing field.
    #[test]
    fn the_file_is_read_whole() {
        let rows: Vec<Value> =
            serde_json::from_str(RAW).expect("data/eziudp.json is not a JSON array");
        let entries: Vec<Registered> =
            serde_json::from_str(RAW).expect("data/eziudp.json has an entry a field is missing from");
        assert!(!rows.is_empty(), "data/eziudp.json lists nothing");
        assert_eq!(rows.len(), entries.len());
        assert!(!all().is_empty());
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

    /// The ribbon names services by id, so an id that drifts from the register leaves a dead button.
    #[test]
    fn every_ribbon_command_names_a_wms() {
        let ids = ribbon_ids();
        assert!(!ids.is_empty(), "data/ribbon.json lists nothing");
        for (command, id) in ids {
            let entry = find(&id).unwrap_or_else(|| panic!("{command}: no register entry {id}"));
            assert_eq!(entry.kind, "wms", "{command}: {}", entry.url);
            assert!(!entry.layer.is_empty(), "{command}: {}", entry.url);
        }
    }

    #[test]
    fn groups_one_entry_per_address() {
        let rows = json!([
            {"zbior": "A", "organ": "X", "uslugi": {"przegladania": ["WMS=https://a/wms"]}},
            {"zbior": "B", "organ": "X", "uslugi": {"przegladania": ["https://a/wms"]}},
            {"zbior": "C", "organ": "Y", "uslugi": {"pobierania": ["https://b/wfs"]}},
        ]);
        let entries = group(rows.as_array().unwrap());
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "A");
        assert_eq!(entries[1].kind, "wfs");
    }
}
