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

/// The cap any confirmed run stops at, whatever the panel's slider says.
const HARD_MB: f64 = 256.0;
/// The size limit the tool form offers, in megabytes; zero means no byte limit at all.
const MIN_MB: i64 = 0;
const MAX_MB: i64 = 1_000_000;

pub fn run(request: &Value, lang: Lang) -> Result<Value, String> {
    // The sheet downloader and the register update name no dataset, so they run before `dane`.
    match request.get("id").and_then(Value::as_str) {
        Some("pobierz-arkusze") => return sheets::run(request, lang),
        Some("granice") => return crate::granice::tool(request, lang),
        Some("geokoduj-csv") => return crate::uug::geocode_csv(request, lang),
        Some("dzialki-csv") => return crate::uldk::parcels_csv(request, lang),
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

    // The no-ceiling tick takes every feature; the cache and the byte limit still apply.
    let no_limit = params.get("nolimit").and_then(Value::as_bool).unwrap_or(false);
    let cap = feature_cap(no_limit, confirm);
    // Warn before the first page when the area holds more than one run may take.
    let has_confirm = params.get("confirm").is_some();
    let has_nolimit = params.get("nolimit").is_some();
    pre_check(
        layer, area, features_of_area, bbox, lang, cap, confirm, no_limit, has_confirm, has_nolimit,
    )?;

    // The form's own number wins, then the panel's slider, so the pane that refuses can raise it.
    let asked_mb = crate::number::of(&params, "limit_mb")
        .map(|mb| mb.clamp(MIN_MB, MAX_MB) as f64)
        .unwrap_or(settings.limit_mb as f64);
    // A zero asks for no byte limit; the feature cap still guards the layer the tool returns.
    let limit = if asked_mb == 0.0 {
        u64::MAX
    } else if confirm {
        (HARD_MB.max(asked_mb) * 1e6) as u64
    } else {
        (asked_mb * 1e6) as u64
    };
    let download = {
        let mut report = |text: &str| host::progress_line(text);
        wfs::fetch(layer, bbox, Some(features_of_area), None, &mut report, lang, limit, cap)?
    };
    let megabytes = download.bytes as f64 / 1e6;
    if download.limit_hit && !confirm {
        return Err(text::download_too_big(
            lang,
            layer.name,
            megabytes,
            asked_mb,
            HARD_MB.max(asked_mb),
        ));
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
        messages.push(text::download_stopped_at(
            lang,
            layer.name,
            megabytes,
            limit as f64 / 1e6,
            HARD_MB.max(asked_mb),
        ));
    }
    if download.truncated {
        messages.push(text::truncated_at(lang, layer.name, cap, confirm));
    }
    if download.failed > 0 {
        messages.push(text::shortfall(lang, download.failed));
    }
    if let Some(problem) = download.fallback {
        messages.push(text::bbox_used(lang, problem));
    }
    if download.unfiltered {
        messages.push(text::service_ignores_area(lang, &layer.name));
    }
    let collection = json!({ "type": "FeatureCollection", "features": download.features });
    Ok(json!({
        "layers": [{ "name": layer.name, "geojson": collection }],
        "messages": messages
    }))
}

/// The features one run may return: 10 000, 50 000 when confirmed, every one when asked.
fn feature_cap(no_limit: bool, confirm: bool) -> usize {
    if no_limit {
        usize::MAX
    } else if confirm {
        wfs::HARD_TOTAL
    } else {
        wfs::MAX_TOTAL
    }
}

/// The count the service reports for the area, before the first page.
pub(crate) fn counted(layer: &catalog::WfsLayer, bbox: [f64; 4], filter: Option<&str>, lang: Lang) -> Option<usize> {
    let body = http::fetch(&hits_url(layer, bbox, filter), 60, lang).ok()?;
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
        None => {
            url.push_str(&format!(
                "&BBOX={:.6},{:.6},{:.6},{:.6}",
                bbox[0], bbox[1], bbox[2], bbox[3]
            ));
            // The KINR address service counts nothing without the CRS on the box.
            if layer.bbox_crs {
                url.push_str(",EPSG:4326");
            }
        }
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

/// Refuses a run that would stop at the feature cap, and names the tick that lifts it.
fn pre_check(
    layer: &catalog::WfsLayer,
    area: &str,
    collection: &Value,
    bbox: [f64; 4],
    lang: Lang,
    cap: usize,
    confirm: bool,
    no_limit: bool,
    has_confirm: bool,
    has_nolimit: bool,
) -> Result<(), String> {
    // A service that ignores the area has no count to trust: the fetch cuts it.
    if no_limit || layer.unfiltered {
        return Ok(());
    }
    let filter = if layer.bbox_crs {
        None
    } else {
        filter::polygon_filter(layer, collection).ok()
    };
    let Some(wanted) = counted(layer, bbox, filter.as_deref(), lang) else {
        return Ok(());
    };
    if wanted > cap {
        return Err(text::over_budget(
            lang, area, wanted, cap, confirm, &layer.name, has_confirm, has_nolimit,
        ));
    }
    Ok(())
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
    fn a_count_over_the_cap_names_the_tick() {
        let msg = crate::text::over_budget(Lang::En, "Area", 11_058, 10_000, false, "Punkty adresowe", true, true);
        assert!(msg.contains("Download even if large"), "{msg}");
        assert!(msg.contains("No feature ceiling"), "{msg}");
        let over = crate::text::over_budget(Lang::En, "Area", 60_000, 50_000, true, "Punkty adresowe", true, true);
        assert!(over.contains("No feature ceiling"), "{over}");
        assert!(!over.contains("Download even if large"), "{over}");
    }

    #[test]
    fn a_hits_answer_carries_the_count() {
        let body = "<wfs:FeatureCollection numberOfFeatures=\"54267\"></wfs:FeatureCollection>";
        assert_eq!(hits_of(body), Some(54_267));
        assert_eq!(hits_of("<ServiceException/>"), None);
    }

    #[test]
    fn a_no_ceiling_run_takes_every_feature() {
        assert_eq!(feature_cap(true, false), usize::MAX);
        assert_eq!(feature_cap(true, true), usize::MAX);
        assert_eq!(feature_cap(false, false), wfs::MAX_TOTAL);
        assert_eq!(feature_cap(false, true), wfs::HARD_TOTAL);
    }

}
