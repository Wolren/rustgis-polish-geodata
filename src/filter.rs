//! Building a WFS spatial filter from the area layer.
//!
//! A rectangular bbox is the fallback; when the area is one polygon, the server can filter by
//! it, so selecting a single commune out of a country-wide layer downloads that commune's
//! features instead of every feature in its bounding box.

use crate::catalog::WfsLayer;
use serde_json::Value;

/// The filter is sent in the URL, so it must stay well under a typical 8 KB request line.
const MAX_FILTER_CHARS: usize = 6000;
/// Fewer points than this and the polygon is too coarse to filter usefully.
const MAX_POINTS: usize = 300;

/// A percent-encoded `<Filter>` for the area, or `None` when the area is not a single polygon
/// (several polygons, or none) — the caller then uses the bounding box.
pub fn polygon_filter(layer: &WfsLayer, collection: &Value) -> Option<String> {
    let rings = exterior_rings(collection);
    if rings.len() != 1 {
        return None;
    }
    let ring = rings.into_iter().next()?;
    if ring.len() < 4 {
        return None;
    }
    let ring = decimate(ring, MAX_POINTS);
    let mut positions = String::new();
    for [lon, lat] in &ring {
        positions.push_str(&format!("{lat:.7} {lon:.7} "));
    }
    let xml = format!(
        "<Filter><Intersects><PropertyName>{}</PropertyName><gml:Polygon><gml:exterior>\
<gml:LinearRing><gml:posList>{}</gml:posList></gml:LinearRing></gml:exterior></gml:Polygon>\
</Intersects></Filter>",
        layer.geom_field,
        positions.trim_end()
    );
    let encoded = percent_encode(&xml);
    (encoded.len() <= MAX_FILTER_CHARS).then_some(encoded)
}

/// The exterior rings of every polygon in a FeatureCollection.
fn exterior_rings(collection: &Value) -> Vec<Vec<[f64; 2]>> {
    let mut out = Vec::new();
    let Some(features) = collection.get("features").and_then(Value::as_array) else {
        return out;
    };
    for feature in features {
        let Some(geometry) = feature.get("geometry") else { continue };
        match geometry.get("type").and_then(Value::as_str) {
            Some("Polygon") => {
                if let Some(ring) = geometry.get("coordinates").and_then(|c| c.get(0)) {
                    if let Some(ring) = ring_of(ring) {
                        out.push(ring);
                    }
                }
            }
            Some("MultiPolygon") => {
                if let Some(polygons) = geometry.get("coordinates").and_then(Value::as_array) {
                    for polygon in polygons {
                        if let Some(ring) = polygon.get(0).and_then(ring_of) {
                            out.push(ring);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn ring_of(value: &Value) -> Option<Vec<[f64; 2]>> {
    let points = value.as_array()?;
    let mut ring = Vec::with_capacity(points.len());
    for point in points {
        let pair = point.as_array()?;
        if pair.len() < 2 {
            return None;
        }
        ring.push([pair[0].as_f64()?, pair[1].as_f64()?]);
    }
    (ring.len() >= 4).then_some(ring)
}

/// Keeps at most `max` points, evenly spaced, always including the first and the last.
fn decimate(mut ring: Vec<[f64; 2]>, max: usize) -> Vec<[f64; 2]> {
    if ring.len() <= max {
        return ring;
    }
    let step = ring.len().div_ceil(max);
    let last = *ring.last().unwrap_or(&ring[0]);
    let mut kept: Vec<[f64; 2]> = ring.drain(..).step_by(step).collect();
    if kept.last() != Some(&last) {
        kept.push(last);
    }
    kept
}

fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog;

    fn square(size: f64) -> Value {
        serde_json::json!({
            "type": "FeatureCollection",
            "features": [{
                "type": "Feature",
                "properties": {},
                "geometry": {"type": "Polygon", "coordinates": [[
                    [21.0, 52.2], [21.0 + size, 52.2], [21.0 + size, 52.2 + size], [21.0, 52.2 + size], [21.0, 52.2]
                ]]}
            }]
        })
    }

    #[test]
    fn filters_by_the_single_polygon() {
        let layer = catalog::wfs("gminy").unwrap();
        let filter = polygon_filter(layer, &square(0.01)).expect("a filter");
        assert!(filter.starts_with("%3CFilter%3E"));
        assert!(filter.contains("msGeometry"));
        // lat lon, as GML in EPSG:4326 wants it
        assert!(filter.contains("52.2000000"));
        assert!(filter.contains("21.0100000") || filter.contains("21.0000000"));
    }

    #[test]
    fn refuses_two_polygons() {
        let layer = catalog::wfs("gminy").unwrap();
        let mut area = square(0.01);
        let copy = area["features"][0].clone();
        area["features"].as_array_mut().unwrap().push(copy);
        assert!(polygon_filter(layer, &area).is_none());
    }

    #[test]
    fn decimates_and_keeps_a_closed_ring() {
        let ring: Vec<[f64; 2]> = (0..1000).map(|i| [21.0 + i as f64 * 1e-5, 52.2]).collect();
        let ring = decimate(ring, 300);
        assert!(ring.len() <= 301);
    }
}
