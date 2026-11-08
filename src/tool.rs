//! The download tools: a form in the Tools pane, with a dataset choice and an area layer.
//!
//! There is one tool per catalogue group, and nothing else downloads a WFS type: a command would
//! hold the interface for the length of the download. The tools share this code; the group only
//! decides which titles the form offers.
//!
//! Tools run without access to the map (no host requests), so the area is a layer the user names:
//! its features' bounding box, or only its selected features, which RustGIS hands over. Nothing is
//! inferred, so a click cannot take the whole country by surprise. What the tool returns is added
//! to the project by RustGIS, with undo.

use crate::cache;
use crate::catalog;
use crate::filter;
use crate::host;
use crate::http;
use crate::sheets;
use crate::text::{self, Lang};
use crate::wfs;
use serde_json::{json, Value};

/// The span the catalogue gives a dataset it sets no real limit for; those are counted instead.
const UNLIMITED_SPAN: f64 = 20.0;
/// The cap any confirmed run stops at, whatever the panel's slider says.
const HARD_MB: f64 = 256.0;

pub fn run(request: &Value, lang: Lang) -> Result<Value, String> {
    // The sheet downloader and the register update name no dataset, so they run before `dane`.
    match request.get("id").and_then(Value::as_str) {
        Some("pobierz-arkusze") => return sheets::run(request, lang),
        Some("geokoduj-csv") => return crate::uug::geocode_csv(request, lang),
        Some("aktualizuj-rejestr") => {
            let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
            return crate::zrodla::update(&params, lang);
        }
        _ => {}
    }
    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
    let label = params
        .get("dane")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if label.trim().is_empty() {
        return Err(text::pick_dataset(lang));
    }
    let layer = catalog::wfs_by_label(label).ok_or_else(|| text::unknown_dataset(lang, label))?;
    let area = params
        .get("obszar")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| text::pick_area(lang))?;
    // The box may have been drawn under the other language's name.
    let layers = request.get("layers");
    let (area, features_of_area) = match layers.and_then(|l| l.get(area)) {
        Some(found) => (area, found),
        None => {
            let other = text::AREA_LAYERS
                .iter()
                .find(|name| **name != area)
                .copied();
            match other.and_then(|name| layers.and_then(|l| l.get(name)).map(|found| (name, found)))
            {
                Some((name, found)) => (name, found),
                None => return Err(text::area_not_passed(lang, area)),
            }
        }
    };
    let bbox = bbox_of(features_of_area).ok_or_else(|| text::area_no_geometry(lang, area))?;

    // A tool request carries no panel: the form's checkboxes arrive as parameters.
    let clear = params.get("clear").and_then(Value::as_bool).unwrap_or(false);
    // The panel's checkbox and slider set the default; the form's tick covers one run.
    let settings = cache::settings();
    let confirm = params.get("confirm").and_then(Value::as_bool).unwrap_or(false) || settings.confirm_large;
    // The cache goes before the first request, so the click that asked for fresh data gets it.
    let cleared = if clear { cache::clear().ok() } else { None };

    gate(layer, area, features_of_area, bbox, lang)?;

    // Past the unconfirmed limit the run stops before anything is added, and names its size.
    let asked_mb = settings.limit_mb as f64;
    let limit = if confirm { (HARD_MB.max(asked_mb) * 1e6) as u64 } else { (asked_mb * 1e6) as u64 };
    let download = {
        let mut report = |text: &str| host::progress_line(text);
        wfs::fetch(layer, bbox, Some(features_of_area), &mut report, lang, limit)?
    };
    let megabytes = download.bytes as f64 / 1e6;
    if download.limit_hit && !confirm {
        return Err(text::download_too_big(lang, layer.name, megabytes, asked_mb));
    }
    if download.features.is_empty() {
        return Err(text::no_features(lang, layer.name));
    }

    let total = download.features.len();
    let mut messages = Vec::new();
    if let Some((files, bytes)) = cleared {
        messages.push(text::cache_cleared(lang, files, bytes as f64 / 1e6));
    }
    messages.push(text::count_size(lang, layer.name, total, download.bytes));
    if download.limit_hit {
        messages.push(text::download_stopped(lang, layer.name, megabytes));
    }
    if download.truncated {
        messages.push(text::truncated(lang).to_string());
    }
    if download.failed > 0 {
        messages.push(text::shortfall(lang, download.failed));
    }
    if let Some(problem) = download.fallback {
        messages.push(text::bbox_used(lang, problem));
    }
    let collection = json!({ "type": "FeatureCollection", "features": download.features });
    Ok(json!({
        "layers": [{ "name": layer.name, "geojson": collection }],
        "messages": messages
    }))
}

/// The one gate before a download: the layer's span where the catalogue sets a real limit, the
/// feature count of the area where it sets none.
fn gate(
    layer: &catalog::WfsLayer,
    area: &str,
    collection: &Value,
    bbox: [f64; 4],
    lang: Lang,
) -> Result<(), String> {
    if layer.max_span < UNLIMITED_SPAN {
        if wfs::too_large(layer, bbox) {
            return Err(text::area_too_large(
                lang,
                area,
                bbox[2] - bbox[0],
                bbox[3] - bbox[1],
                layer.name,
                layer.max_span,
            ));
        }
        return Ok(());
    }
    let filter = filter::polygon_filter(layer, collection).ok();
    host::progress_line(&text::counting(lang, layer.name, area));
    if let Some(wanted) = needed(layer, bbox, filter.as_deref(), lang) {
        if wanted > wfs::MAX_TOTAL {
            return Err(text::area_over_budget(
                lang,
                area,
                wanted,
                wfs::MAX_TOTAL,
                layer.name,
            ));
        }
    }
    Ok(())
}

/// The features the service counts in the area, when it answers with a number at all.
fn needed(layer: &catalog::WfsLayer, bbox: [f64; 4], filter: Option<&str>, lang: Lang) -> Option<usize> {
    let body = http::get_unstored(&hits_url(layer, bbox, filter), 60, lang).ok()?;
    hits_of(&body)
}

/// The GetFeature address that asks only how many features the area holds.
fn hits_url(layer: &catalog::WfsLayer, bbox: [f64; 4], filter: Option<&str>) -> String {
    let mut url = format!(
        "{}?SERVICE=WFS&VERSION=1.1.0&REQUEST=GetFeature&TYPENAME={}&SRSNAME=EPSG:4326&RESULTTYPE=hits",
        layer.url, layer.type_name
    );
    match filter {
        // GUGiK refuses BBOX and FILTER in one request, so the polygon replaces the box.
        Some(filter) => {
            url.push_str("&FILTER=");
            url.push_str(filter);
        }
        None => url.push_str(&format!(
            "&BBOX={:.6},{:.6},{:.6},{:.6}",
            bbox[0], bbox[1], bbox[2], bbox[3]
        )),
    }
    url
}

/// The count a `RESULTTYPE=hits` answer carries, when it carries one.
fn hits_of(body: &str) -> Option<usize> {
    let key = "numberOfFeatures=\"";
    let start = body.find(key)? + key.len();
    let rest = &body[start..];
    rest[..rest.find('"')?].parse().ok()
}

/// The bounding box of a FeatureCollection's geometry, `[west, south, east, north]`.
fn bbox_of(collection: &Value) -> Option<[f64; 4]> {
    let mut bbox = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    let mut found = false;
    if let Some(features) = collection.get("features").and_then(Value::as_array) {
        for feature in features {
            if let Some(geometry) = feature.get("geometry") {
                coordinates(geometry, &mut bbox, &mut found);
            }
        }
    }
    found.then_some(bbox)
}

fn coordinates(value: &Value, bbox: &mut [f64; 4], found: &mut bool) {
    if let Some(array) = value.as_array() {
        if array.len() >= 2 && array[0].is_number() && array[1].is_number() {
            let (lon, lat) = (
                array[0].as_f64().unwrap_or(0.0),
                array[1].as_f64().unwrap_or(0.0),
            );
            bbox[0] = bbox[0].min(lon);
            bbox[1] = bbox[1].min(lat);
            bbox[2] = bbox[2].max(lon);
            bbox[3] = bbox[3].max(lat);
            *found = true;
        } else {
            for item in array {
                coordinates(item, bbox, found);
            }
        }
    } else if let Some(object) = value.as_object() {
        if let Some(inner) = object.get("coordinates") {
            coordinates(inner, bbox, found);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sheet_tool_runs_before_the_dataset_lookup() {
        // No `dane`, and a blank folder: the route must reach the sheet tool, not the catalogue.
        let request = json!({
            "kind": "tool",
            "id": "pobierz-arkusze",
            "lang": "en",
            "params": { "skorowidz": "Skorowidz", "folder": "  " },
            "layers": {}
        });
        let error = run(&request, Lang::En).unwrap_err();
        assert!(error.contains("folder"), "{error}");
    }

    #[test]
    fn takes_the_bbox_of_a_collection() {
        let fc = json!({
            "type": "FeatureCollection",
            "features": [
                {"type": "Feature", "geometry": {"type": "Point", "coordinates": [21.0, 52.2]}, "properties": {}},
                {"type": "Feature", "geometry": {"type": "Polygon", "coordinates": [[[21.1, 52.1], [21.2, 52.3], [21.15, 52.25], [21.1, 52.1]]]}, "properties": {}}
            ]
        });
        assert_eq!(bbox_of(&fc), Some([21.0, 52.1, 21.2, 52.3]));
    }

    #[test]
    fn has_no_bbox_without_geometry() {
        assert_eq!(
            bbox_of(&json!({"type": "FeatureCollection", "features": []})),
            None
        );
    }

    #[test]
    fn answers_in_the_requested_language() {
        let request = |lang: &str| {
            json!({
                "kind": "tool",
                "id": "pobierz-adresy",
                "lang": lang,
                "params": { "dane": "Działki", "obszar": "Area to download" },
                "layers": {}
            })
        };
        let english = run(&request("en"), Lang::En).unwrap_err();
        let polish = run(&request("pl"), Lang::Pl).unwrap_err();
        assert!(english.contains("was not passed in"), "{english}");
        assert!(polish.contains("nie przekazano"), "{polish}");
    }

    #[test]
    fn asks_the_service_for_the_count_of_a_big_dataset() {
        let layer = catalog::wfs_by_label("Granice gmin").unwrap();
        let url = hits_url(layer, [16.8, 52.3, 16.9, 52.4], None);
        assert!(url.contains("RESULTTYPE=hits"), "{url}");
        assert!(url.contains("TYPENAME=A03_Granice_gmin"), "{url}");
        assert!(url.contains("BBOX=16.800000,52.300000,16.900000,52.400000"), "{url}");
        assert!(!url.contains("MAXFEATURES"), "a page limit caps the count too: {url}");
        // The polygon narrows the count as it narrows the download, and never rides with a bbox.
        let filtered = hits_url(layer, [16.8, 52.3, 16.9, 52.4], Some("%3CFilter%3E"));
        assert!(filtered.contains("&FILTER=%3CFilter%3E"), "{filtered}");
        assert!(!filtered.contains("BBOX="), "{filtered}");
        // The count is what GUGiK's answer carries, beside the empty collection.
        let body = "<wfs:FeatureCollection numberOfFeatures=\"54267\"></wfs:FeatureCollection>";
        assert_eq!(hits_of(body), Some(54_267));
        assert_eq!(hits_of("<ServiceException/>"), None);
        assert_eq!(hits_of("numberOfFeatures=\"unknown\""), None);
    }

    #[test]
    fn one_gate_refuses_a_wide_area_before_the_request() {
        let layer = catalog::wfs_by_label("Działki").unwrap();
        let area = json!({"type": "FeatureCollection", "features": []});
        let error = gate(layer, "Area to download", &area, [14.0, 49.0, 24.2, 54.9], Lang::En)
            .unwrap_err();
        assert!(error.contains("Area to download"), "{error}");
        assert!(error.contains("Działki"), "{error}");
        // The same gate lets a box inside the layer's own span through.
        assert!(gate(layer, "Area to download", &area, [21.0, 52.2, 21.01, 52.21], Lang::En).is_ok());
    }

    #[test]
    fn the_sentinel_span_is_left_to_the_feature_budget() {
        let gminy = catalog::wfs_by_label("Granice gmin").unwrap();
        let dzialki = catalog::wfs_by_label("Działki").unwrap();
        let country = [14.0, 49.0, 24.2, 54.9];
        assert_eq!(gminy.max_span, UNLIMITED_SPAN, "a dataset with no real limit");
        assert!(dzialki.max_span < UNLIMITED_SPAN, "a real span keeps the degree check");
        assert!(
            !wfs::too_large(gminy, country),
            "the sentinel alone lets the country through"
        );
        assert!(wfs::too_large(dzialki, country), "a real span still refuses it");
    }
}
