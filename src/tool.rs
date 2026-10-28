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
use crate::host;
use crate::text::{self, Lang};
use crate::wfs;
use serde_json::{json, Value};

pub fn run(request: &Value, lang: Lang) -> Result<Value, String> {
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

    // The panel's checkbox asks for the stored answers to go once the data is in.
    let clear = request
        .get("panel")
        .and_then(|panel| panel.get("values"))
        .and_then(|values| values.get("clear"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let (features, truncated) = {
        let mut report = |text: &str| host::progress_line(text);
        wfs::fetch(layer, bbox, Some(features_of_area), &mut report, lang)?
    };
    if features.is_empty() {
        return Err(text::no_features(lang, layer.name));
    }

    let total = features.len();
    let collection = json!({ "type": "FeatureCollection", "features": features });
    let mut messages = vec![text::count(lang, layer.name, total)];
    if truncated {
        messages.push(text::truncated(lang).to_string());
    }
    if clear {
        if let Ok((files, bytes)) = cache::clear() {
            messages.push(text::cache_cleared(lang, files, bytes as f64 / 1e6));
        }
    }
    Ok(json!({
        "layers": [{ "name": layer.name, "geojson": collection }],
        "messages": messages
    }))
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
        assert!(polish.contains("nie otrzymałem"), "{polish}");
    }
}
