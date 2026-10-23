//! The download tool: a form in the Tools pane, with a dataset choice and an area layer.
//!
//! Tools run without access to the map (no host requests), so the area comes from a layer the
//! user picks — its features' bounding box, or only its selected features, which RustGIS hands
//! over. What the tool returns is added to the project by RustGIS, with undo.

use crate::catalog;
use crate::host;
use crate::wfs;
use serde_json::{json, Value};

pub fn run(request: &Value) -> Result<Value, String> {
    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
    let label = params
        .get("dane")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let layer =
        catalog::wfs_by_label(label).ok_or_else(|| format!("nieznany zestaw danych: '{label}'"))?;
    let area = params
        .get("obszar")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or("wybierz warstwę obszaru — np. „Obszar pobierania” z przycisku „Obszar z widoku”")?;
    let features_of_area = request
        .get("layers")
        .and_then(|l| l.get(area))
        .ok_or_else(|| format!("nie otrzymałem warstwy obszaru „{area}”"))?;
    let bbox = bbox_of(features_of_area)
        .ok_or_else(|| format!("warstwa „{area}” nie ma geometrii do wyznaczenia obszaru"))?;

    if wfs::too_large(layer, bbox) {
        return Err(format!(
            "warstwa „{area}” obejmuje {:.1}° × {:.1}° — za dużo dla „{}” (maks. {:.2}°). \
             Zaznacz na mapie interesujące obiekty i uruchom narzędzie ponownie (zaznaczenie zawęża obszar) \
             albo wskaż mniejszą warstwę.",
            bbox[2] - bbox[0],
            bbox[3] - bbox[1],
            layer.name,
            layer.max_span
        ));
    }

    let (features, truncated) = {
        let mut report = |text: &str| host::progress_line(text);
        wfs::fetch(layer, bbox, Some(features_of_area), &mut report)?
    };
    if features.is_empty() {
        return Err(format!(
            "brak obiektów „{}” w wybranym obszarze",
            layer.name
        ));
    }

    let total = features.len();
    let collection = json!({ "type": "FeatureCollection", "features": features });
    let mut messages = vec![format!("{}: {total} obiektów", layer.name)];
    if truncated {
        messages.push("Pobieranie przerwane limitem — wybierz mniejszy obszar.".into());
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
}
