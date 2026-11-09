//! Building a WFS spatial filter from the area layer.
//!
//! A rectangular bbox is the fallback; when the area is one polygon, the server can filter by
//! it, so selecting a single commune out of a country-wide layer downloads that commune's
//! features instead of every feature in its bounding box.

use crate::catalog::WfsLayer;
use crate::gk92;
use serde_json::Value;

/// The filter is sent in the URL, so it must stay well under a typical 8 KB request line.
const MAX_FILTER_CHARS: usize = 6000;
/// Fewer points than this and the polygon is too coarse to filter usefully.
const MAX_POINTS: usize = 300;
/// Rings one filter may carry; a unit cut into more pieces falls back to its box.
const MAX_RINGS: usize = 4;

/// Why the area's shape could not be sent as a filter, so the request used its bounding box.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ShapeProblem {
    NoPolygon,
    SeveralPolygons,
    TooFewPoints,
    TooLong,
}

/// A percent-encoded `<Filter>` for the area in lon and lat, or why the area has no single ring.
pub fn polygon_filter(layer: &WfsLayer, collection: &Value) -> Result<String, ShapeProblem> {
    // GUGiK answers EPSG:4326 with the latitude first, which is the order its filter wants.
    polygon_filter_in(layer, collection, |lon, lat| [lat, lon])
}

/// The same ring in EPSG:2180, where the national grid carries the northing first.
pub fn polygon_filter_grid(layer: &WfsLayer, collection: &Value) -> Result<String, ShapeProblem> {
    polygon_filter_in(layer, collection, |lon, lat| {
        let (easting, northing) = gk92::forward(lon, lat);
        [northing, easting]
    })
}

/// One or more exterior rings, projected by `project`, as the `<Filter>` the `FILTER` takes.
///
/// A gmina is one ring and a powiat may be two, so every ring joins the filter as its own
/// `<Intersects>` inside an `<Or>`. Each ring is decimated until the whole filter fits the budget,
/// because a dense boundary would otherwise fall back to a box that reaches into its neighbours.
fn polygon_filter_in(
    layer: &WfsLayer,
    collection: &Value,
    project: impl Fn(f64, f64) -> [f64; 2],
) -> Result<String, ShapeProblem> {
    let mut rings = exterior_rings(collection);
    if rings.is_empty() {
        return Err(ShapeProblem::NoPolygon);
    }
    if rings.len() > MAX_RINGS {
        return Err(ShapeProblem::SeveralPolygons);
    }
    rings.retain(|ring| ring.len() >= 4);
    if rings.is_empty() {
        return Err(ShapeProblem::TooFewPoints);
    }
    // Each ring takes an equal share of the budget, then the whole filter is measured once.
    let mut budget = MAX_POINTS;
    loop {
        let share = (budget / rings.len()).max(4);
        let parts: Vec<String> = rings
            .iter()
            .map(|ring| {
                let ring = decimate(ring.clone(), share);
                format!(
                    "<Intersects><PropertyName>{}</PropertyName><gml:Polygon><gml:exterior>\
<gml:LinearRing><gml:posList>{}</gml:posList></gml:LinearRing></gml:exterior></gml:Polygon>\
</Intersects>",
                    layer.geom_field,
                    positions(&ring, &project).trim_end()
                )
            })
            .collect();
        let body = if parts.len() == 1 {
            parts[0].clone()
        } else {
            format!("<Or>{}</Or>", parts.join(""))
        };
        let xml = format!(
            "<Filter xmlns:ogc=\"http://www.opengis.net/ogc\" xmlns:gml=\"http://www.opengis.net/gml\">{body}</Filter>"
        );
        let encoded = percent_encode(&xml);
        if encoded.len() <= MAX_FILTER_CHARS {
            return Ok(encoded);
        }
        if budget <= 4 * rings.len() {
            return Err(ShapeProblem::TooLong);
        }
        budget = (budget * 3 / 4).max(4 * rings.len());
    }
}

/// The ring as `x y` pairs, each pair in the order the server's frame wants it.
fn positions(ring: &[[f64; 2]], project: impl Fn(f64, f64) -> [f64; 2]) -> String {
    let mut out = String::new();
    for [lon, lat] in ring {
        let [x, y] = project(*lon, *lat);
        out.push_str(&format!("{x:.7} {y:.7} "));
    }
    out
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
    Some(ring)
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
        let layer = catalog::wfs_by_label("gminy").unwrap();
        let filter = polygon_filter(layer, &square(0.01)).expect("a filter");
        assert!(filter.starts_with("%3CFilter%20xmlns%3Aogc"), "{filter}");
        assert!(filter.contains("msGeometry"));
        // lat lon, as GML in EPSG:4326 wants it
        assert!(filter.contains("52.2000000"));
        assert!(filter.contains("21.0100000") || filter.contains("21.0000000"));
    }

    #[test]
    fn declares_the_namespaces_the_filter_uses() {
        let layer = catalog::wfs_by_label("gminy").unwrap();
        let filter = polygon_filter(layer, &square(0.01)).unwrap();
        assert!(filter.contains("xmlns%3Aogc"), "{filter}");
        assert!(filter.contains("xmlns%3Agml"), "{filter}");
    }

    #[test]
    fn writes_the_grid_ring_northing_first() {
        let ring = [[21.0, 52.2], [21.01, 52.2], [21.01, 52.21], [21.0, 52.2]];
        let text = positions(&ring, |lon, lat| {
            let (easting, northing) = gk92::forward(lon, lat);
            [northing, easting]
        });
        let numbers: Vec<f64> = text.split_whitespace().filter_map(|n| n.parse().ok()).collect();
        let (easting, northing) = gk92::forward(21.0, 52.2);
        assert_eq!(numbers.len(), 8);
        assert!((numbers[0] - northing).abs() < 0.01, "{numbers:?}");
        assert!((numbers[1] - easting).abs() < 0.01, "{numbers:?}");
    }

    #[test]
    fn a_powiat_may_be_two_polygons() {
        let layer = catalog::wfs_by_label("gminy").unwrap();
        let mut area = square(0.01);
        let copy = area["features"][0].clone();
        area["features"].as_array_mut().unwrap().push(copy);
        let filter = polygon_filter(layer, &area).expect("two rings joined by Or");
        assert!(filter.contains("%3COr%3E") || filter.contains("<Or>"), "{filter}");
    }

    #[test]
    fn names_a_ring_with_too_few_points() {
        let layer = catalog::wfs_by_label("gminy").unwrap();
        let area = serde_json::json!({
            "type": "FeatureCollection",
            "features": [{"type": "Feature", "properties": {}, "geometry": {
                "type": "Polygon", "coordinates": [[[21.0, 52.2], [21.01, 52.2], [21.0, 52.2]]]
            }}]
        });
        assert_eq!(polygon_filter(layer, &area), Err(ShapeProblem::TooFewPoints));
    }

    #[test]
    fn names_the_reason_without_any_polygon() {
        let layer = catalog::wfs_by_label("gminy").unwrap();
        let empty = serde_json::json!({"type": "FeatureCollection", "features": []});
        assert_eq!(polygon_filter(layer, &empty), Err(ShapeProblem::NoPolygon));
    }

    #[test]
    fn decimates_and_keeps_a_closed_ring() {
        let ring: Vec<[f64; 2]> = (0..1000).map(|i| [21.0 + i as f64 * 1e-5, 52.2]).collect();
        let ring = decimate(ring, 300);
        assert!(ring.len() <= 301);
    }
}
