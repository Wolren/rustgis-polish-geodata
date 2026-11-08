//! The area layer: a rectangle taken from the map view, or a shape drawn on the map.
//!
//! RustGIS draws editable features point by point, and its rectangle tool draws a graphic, so a
//! rectangle comes from the view in one click or from a drag moved into the area by
//! [`take_rectangle`]. A shape that is drawn stays as drawn; the elevation model, which can only
//! fetch a rectangle, says which rectangle it is covering.

use crate::cache;
use crate::gk92;
use crate::host::Host;
use crate::text::{self, Lang};
use serde_json::{json, Value};

/// How the area layer was made. A model snaps a drawn shape to its box and leaves a rectangle be.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// A rectangle copied from the map view.
    View,
    /// A shape drawn on the map, kept as it was drawn.
    Drawn,
}

impl Mode {
    fn key(self) -> &'static str {
        match self {
            Mode::View => "widok",
            Mode::Drawn => "rysowany",
        }
    }

    /// The mode of the last area command, for a layer whose geometry says nothing.
    pub fn stored() -> Mode {
        match std::fs::read_to_string(cache::area_mode_file()) {
            Ok(text) if text.trim() == Mode::View.key() => Mode::View,
            _ => Mode::Drawn,
        }
    }

    /// The mode the layer itself shows: one closed rectangle ring was copied from the view.
    pub fn of(features: &[Value]) -> Option<Mode> {
        if features.is_empty() {
            return None;
        }
        Some(match features {
            [feature] if rectangle(feature) => Mode::View,
            _ => Mode::Drawn,
        })
    }

    pub fn remember(self) {
        std::fs::write(cache::area_mode_file(), self.key()).ok();
    }

    /// Drops the stored mode, so another project's area does not inherit it.
    fn forget() {
        std::fs::remove_file(cache::area_mode_file()).ok();
    }
}

/// Whether a feature is one closed ring of five corners on an axis-aligned rectangle.
fn rectangle(feature: &Value) -> bool {
    let Some(geometry) = feature.get("geometry") else {
        return false;
    };
    if geometry.get("type").and_then(Value::as_str) != Some("Polygon") {
        return false;
    }
    let Some(rings) = geometry.get("coordinates").and_then(Value::as_array) else {
        return false;
    };
    let [ring] = rings.as_slice() else {
        return false;
    };
    let Some(points) = ring.as_array() else {
        return false;
    };
    if points.len() != 5 || points.first() != points.last() {
        return false;
    }
    let corner = |index: usize| {
        let point = points.get(index)?;
        Some((point.get(0)?.as_f64()?, point.get(1)?.as_f64()?))
    };
    let (Some(a), Some(b), Some(c), Some(d)) = (corner(0), corner(1), corner(2), corner(3)) else {
        return false;
    };
    a.1 == b.1 && b.0 == c.0 && c.1 == d.1 && d.0 == a.0
}

/// Fills the area layer with the current view, so a rectangle needs no drawing at all.
pub fn rectangle_from_view(host: &mut Host, lang: Lang) -> Result<Value, String> {
    // A rectangle dragged but not taken goes, so it cannot overwrite this one later.
    drop_drawing(host);
    let extent = host.extent()?;
    if extent[2] <= extent[0] || extent[3] <= extent[1] {
        return Err(text::area_no_view(lang));
    }
    let name = area_layer(host, lang)?;
    let drawn = present(host, &name)?.unwrap_or(0) > 0;
    refill(host, &name, &ring(extent), lang)?;
    host.call("layer.select", json!({ "layer": name }))?;
    host.call("tool.set", json!({ "tool": "explore" }))?;
    Mode::View.remember();
    // A shape drawn in the layer goes with the rectangle, and says so.
    if drawn {
        host.notify(&text::area_replaced(lang, &name));
    }
    host.notify(&text::area_from_view(lang, &name, size_km(extent)));
    Ok(json!({ "layer": name, "mode": Mode::View.key(), "box": extent }))
}

/// Puts the drawing tool on the map, with the area layer active.
///
/// An area layer that is already there is kept and drawn into, so a second shape widens the area
/// instead of throwing the first one away. A drawn layer that is in the way can be removed with
/// Clear area.
pub fn draw(host: &mut Host, lang: Lang) -> Result<Value, String> {
    let name = area_layer(host, lang)?;
    let layers = host.call("host.layers", json!({}))?;
    let list = layers.as_array().cloned().unwrap_or_default();
    let there = list
        .iter()
        .any(|layer| layer.get("name").and_then(Value::as_str) == Some(name.as_str()));
    if !there {
        host.call(
            "layer.template",
            json!({ "template": "areas", "name": name.clone() }),
        )?;
    }
    host.call("layer.select", json!({ "layer": name }))?;
    host.call("edit.enable", json!({ "on": true }))?;
    host.call("tool.set", json!({ "tool": "createPolygon" }))?;
    Mode::Drawn.remember();
    host.notify(&text::area_armed(lang, &name));
    Ok(
        json!({ "layer": name, "mode": Mode::Drawn.key(), "tool": "createPolygon", "editing": true }),
    )
}

/// Drops the drawing tool. The app also cancels a half-drawn ring on Escape.
pub fn abort(host: &mut Host, lang: Lang) -> Result<Value, String> {
    drop_drawing(host);
    host.call("tool.set", json!({ "tool": "explore" }))?;
    host.notify(&text::area_aborted(lang));
    Ok(json!({ "tool": "explore" }))
}

/// Puts the base app's rectangle tool on the map, drawing into a layer of its own.
///
/// That tool draws a graphic and the area layer holds features, so [`take_rectangle`] moves the
/// drag into the area layer; the next command does the same on its own.
pub fn draw_rectangle(host: &mut Host, lang: Lang) -> Result<Value, String> {
    // A rectangle drawn before becomes the area now, so a second drag replaces the first.
    take_rectangle(host, lang)?;
    drop_drawing(host);
    let wanted = text::draw_layer(lang);
    host.call("graphics.newLayer", json!({ "name": wanted }))?;
    let name = made_layer(host, wanted).unwrap_or_else(|| wanted.to_string());
    cache::remember_drawing(&name);
    host.call("tool.set", json!({ "tool": "graphicRectangle" }))?;
    host.notify(&text::area_rect_armed(lang));
    Ok(json!({ "layer": name, "tool": "graphicRectangle", "drawing": true }))
}

/// Moves the rectangle dragged on the map into the area layer.
///
/// Nothing is taken while nothing was drawn, so a pending drawing waits for the next command.
pub fn take_rectangle(host: &mut Host, lang: Lang) -> Result<Option<Value>, String> {
    let Some(drawing) = cache::pending_drawing() else {
        return Ok(None);
    };
    let features = match host.call("host.features", json!({ "layer": drawing.clone() })) {
        Ok(f) => f,
        Err(_) => {
            // The layer went: the pending drawing goes with it.
            cache::forget_drawing();
            return Ok(None);
        }
    };
    let ring = features
        .get("features")
        .and_then(Value::as_array)
        .and_then(|list| list.iter().rev().find_map(rectangle_ring));
    let Some(ring) = ring else {
        return Ok(None);
    };
    let name = area_layer(host, lang)?;
    let replaced = present(host, &name)?.unwrap_or(0) > 0;
    refill(host, &name, &ring, lang)?;
    host.call("layer.remove", json!({ "layer": [drawing] }))?;
    cache::forget_drawing();
    Mode::Drawn.remember();
    if replaced {
        host.notify(&text::area_replaced(lang, &name));
    }
    let box_lon = bounds(&ring);
    host.notify(&text::area_from_rectangle(lang, &name, size_km(box_lon)));
    Ok(Some(json!({ "layer": name, "mode": Mode::Drawn.key(), "box": box_lon })))
}

/// Fills the area layer from a box or a polygon a script hands in, with no drawing at all.
///
/// `bbox` takes `[west, south, east, north]` in lon and lat; `geojson` takes any Feature, Feature
/// Collection or geometry and uses its bounds, which is what a WFS filter needs.
pub fn from_json(host: &mut Host, request: &Value, lang: Lang) -> Result<Value, String> {
    let params = request.get("params").cloned().unwrap_or(Value::Null);
    let box_lon = read_box(&params).ok_or_else(|| text::area_needs_box(lang))?;
    drop_drawing(host);
    let name = area_layer(host, lang)?;
    let replaced = present(host, &name)?.unwrap_or(0) > 0;
    refill(host, &name, &ring(box_lon), lang)?;
    host.call("layer.select", json!({ "layer": name }))?;
    Mode::View.remember();
    if replaced {
        host.notify(&text::area_replaced(lang, &name));
    }
    host.notify(&text::area_from_box(lang, &name, size_km(box_lon)));
    Ok(json!({ "layer": name, "mode": Mode::View.key(), "box": box_lon }))
}

/// The box a request names, from `bbox` numbers or from a GeoJSON shape's bounds.
fn read_box(params: &Value) -> Option<[f64; 4]> {
    if let Some(values) = params.get("bbox").and_then(Value::as_array) {
        let numbers: Vec<f64> = values.iter().filter_map(Value::as_f64).collect();
        let box_lon = [*numbers.first()?, *numbers.get(1)?, *numbers.get(2)?, *numbers.get(3)?];
        return (box_lon[2] > box_lon[0] && box_lon[3] > box_lon[1]).then_some(box_lon);
    }
    let shape = params.get("geojson").or_else(|| params.get("geometry"))?;
    let mut points: Vec<[f64; 2]> = Vec::new();
    collect_points(shape, &mut points, 0);
    if points.is_empty() {
        return None;
    }
    let box_lon = bounds(&points);
    (box_lon[2] > box_lon[0] && box_lon[3] > box_lon[1]).then_some(box_lon)
}

/// Every coordinate pair under a geometry, nested as deep as GeoJSON puts them.
pub(crate) fn collect_points(value: &Value, out: &mut Vec<[f64; 2]>, depth: usize) {
    if depth > 8 || out.len() >= 200_000 {
        return;
    }
    match value {
        Value::Array(items) => {
            if items.len() == 2 && items.iter().all(Value::is_number) {
                if let (Some(lon), Some(lat)) = (items[0].as_f64(), items[1].as_f64()) {
                    out.push([lon, lat]);
                }
                return;
            }
            for item in items {
                collect_points(item, out, depth + 1);
            }
        }
        Value::Object(map) => {
            for key in ["geometry", "features", "coordinates"] {
                if let Some(inner) = map.get(key) {
                    collect_points(inner, out, depth + 1);
                }
            }
        }
        _ => {}
    }
}

/// Removes the layer a drawn rectangle waits in, so the next drag starts empty.
pub fn drop_drawing(host: &mut Host) {
    if let Some(name) = cache::pending_drawing() {
        host.call("layer.remove", json!({ "layer": [name] })).ok();
        cache::forget_drawing();
    }
}

/// The layer `graphics.newLayer` really made: a taken name gets " (2)" appended.
fn made_layer(host: &mut Host, wanted: &str) -> Option<String> {
    let layers = host.call("host.layers", json!({})).ok()?;
    let suffix = format!("{wanted} (");
    layers
        .as_array()?
        .iter()
        .filter_map(|layer| layer.get("name").and_then(Value::as_str))
        .find(|name| *name == wanted || name.starts_with(&suffix))
        .map(str::to_string)
}

/// The ring of a rectangle graphic, in lon and lat, when the feature is one.
fn rectangle_ring(feature: &Value) -> Option<Vec<[f64; 2]>> {
    if feature.get("properties")?.get("kind")?.as_str()? != "rectangle" {
        return None;
    }
    let geometry = feature.get("geometry")?;
    if geometry.get("type")?.as_str()? != "Polygon" {
        return None;
    }
    let mut ring: Vec<[f64; 2]> = geometry
        .get("coordinates")?
        .get(0)?
        .as_array()?
        .iter()
        .map(|point| Some([point.get(0)?.as_f64()?, point.get(1)?.as_f64()?]))
        .collect::<Option<Vec<_>>>()?;
    if ring.len() < 4 {
        return None;
    }
    // The area layer's own check wants the first corner last too.
    if ring.first() != ring.last() {
        let first = *ring.first()?;
        ring.push(first);
    }
    Some(ring)
}

/// The box around a ring, as `[west, south, east, north]`.
fn bounds(ring: &[[f64; 2]]) -> [f64; 4] {
    let mut extent = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for [lon, lat] in ring {
        extent[0] = extent[0].min(*lon);
        extent[1] = extent[1].min(*lat);
        extent[2] = extent[2].max(*lon);
        extent[3] = extent[3].max(*lat);
    }
    extent
}

/// Removes the area layer, so the next rectangle or shape starts empty.
pub fn clear(host: &mut Host, lang: Lang) -> Result<Value, String> {
    drop_drawing(host);
    let layers = host.call("host.layers", json!({}))?;
    let list = layers.as_array().cloned().unwrap_or_default();
    let found = list
        .iter()
        .filter_map(|layer| layer.get("name").and_then(Value::as_str))
        .find(|name| text::AREA_LAYERS.contains(name))
        .map(str::to_string);
    let Some(name) = found else {
        return Err(text::area_none(lang));
    };
    host.call("layer.remove", json!({ "layer": [name.clone()] }))?;
    // The mode goes with the layer, so no later model reads a stale one.
    Mode::forget();
    host.notify(&text::area_cleared(lang, &name));
    Ok(json!({ "removed": name }))
}

/// The area layer's name: the one on the map, in either language, or a fresh one in this one.
fn area_layer(host: &mut Host, lang: Lang) -> Result<String, String> {
    let layers = host.call("host.layers", json!({}))?;
    let list = layers.as_array().cloned().unwrap_or_default();
    // A layer made in the other language is the same layer, so it is reused and never doubled.
    let existing = list.iter().find_map(|layer| {
        let name = layer.get("name").and_then(Value::as_str)?;
        text::AREA_LAYERS.contains(&name).then(|| name.to_string())
    });
    Ok(existing.unwrap_or_else(|| text::area_layer(lang).to_string()))
}

/// The area layer's feature count on the map, when it is there at all.
fn present(host: &mut Host, name: &str) -> Result<Option<i64>, String> {
    let layers = host.call("host.layers", json!({}))?;
    Ok(layers
        .as_array()
        .into_iter()
        .flatten()
        .find(|layer| layer.get("name").and_then(Value::as_str) == Some(name))
        .map(|layer| layer.get("features").and_then(Value::as_i64).unwrap_or(0)))
}

/// The area layer's name, for a command that fills it from its own geometry.
pub fn area_layer_name(host: &mut Host, lang: Lang) -> Result<String, String> {
    area_layer(host, lang)
}

/// Fills the area layer with one feature per polygon, so a unit with exclaves keeps them all.
pub fn fill_polygons(
    host: &mut Host,
    name: &str,
    rings: &[Vec<[f64; 2]>],
    lang: Lang,
) -> Result<(), String> {
    let answer = host.call(
        "layer.template",
        json!({ "template": "areas", "name": format!("{name} (new)") }),
    )?;
    let made = answer
        .get("layer")
        .cloned()
        .ok_or_else(|| text::area_failed(lang))?;
    host.call("edit.enable", json!({ "on": true }))?;
    for ring in rings {
        host.call(
            "edit.addFeature",
            json!({ "layer": made.clone(), "geometry": { "type": "Polygon", "coordinates": [ring] } }),
        )?;
    }
    // The old layer goes last: a failure above leaves the area as it was.
    if present(host, name)?.is_some() {
        host.call("layer.remove", json!({ "layer": [name] }))?;
    }
    host.call("layer.rename", json!({ "layer": made, "name": name }))?;
    Ok(())
}

/// The box around every ring of a shape.
pub fn bounds_of(rings: &[Vec<[f64; 2]>]) -> [f64; 4] {
    let mut extent = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for ring in rings {
        let box_lon = bounds(ring);
        extent[0] = extent[0].min(box_lon[0]);
        extent[1] = extent[1].min(box_lon[1]);
        extent[2] = extent[2].max(box_lon[2]);
        extent[3] = extent[3].max(box_lon[3]);
    }
    extent
}

/// Makes the area layer hold `ring`, building the new layer before the old one goes.
fn refill(host: &mut Host, name: &str, ring: &[[f64; 2]], lang: Lang) -> Result<(), String> {
    let answer = host.call(
        "layer.template",
        json!({ "template": "areas", "name": format!("{name} (new)") }),
    )?;
    let made = answer
        .get("layer")
        .cloned()
        .ok_or_else(|| text::area_failed(lang))?;
    host.call("edit.enable", json!({ "on": true }))?;
    host.call(
        "edit.addFeature",
        json!({ "layer": made.clone(), "geometry": { "type": "Polygon", "coordinates": [ring] } }),
    )?;
    // The old layer goes last: a failure above leaves the area as it was.
    if present(host, name)?.is_some() {
        host.call("layer.remove", json!({ "layer": [name] }))?;
    }
    host.call("layer.rename", json!({ "layer": made, "name": name }))?;
    Ok(())
}

/// Replaces the drawn shape with its bounding box, so the layer shows the ground the model covers.
pub fn snap(host: &mut Host, name: &str, box_lon: [f64; 4], lang: Lang) -> Result<(), String> {
    refill(host, name, &ring(box_lon), lang)
}

/// A closed ring around a `[west, south, east, north]` box.
fn ring(extent: [f64; 4]) -> Vec<[f64; 2]> {
    let [w, s, e, n] = extent;
    vec![[w, s], [e, s], [e, n], [w, n], [w, s]]
}

/// The box in kilometres, measured on the projection the model uses.
fn size_km(extent: [f64; 4]) -> (f64, f64) {
    let [w, s, e, n] = extent;
    let (x0, y0) = gk92::forward(w, s);
    let (x1, y1) = gk92::forward(e, n);
    ((x1 - x0).abs() / 1000.0, (y1 - y0).abs() / 1000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

/// A small deterministic generator, so a fuzz case failing here fails everywhere.
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Lcg(seed | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound.max(1)
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u64) as usize]
    }
}

    fn feature(ring: &[(f64, f64)]) -> Value {
        let coordinates: Vec<Value> = ring.iter().map(|(x, y)| json!([x, y])).collect();
        json!({
            "type": "Feature",
            "properties": {},
            "geometry": { "type": "Polygon", "coordinates": [coordinates] }
        })
    }

    #[test]
    fn every_mode_key_reads_back() {
        for mode in [Mode::View, Mode::Drawn] {
            assert_eq!(
                match mode.key() {
                    "widok" => Mode::View,
                    _ => Mode::Drawn,
                },
                mode
            );
        }
    }

    #[test]
    fn the_ring_closes_on_the_first_corner() {
        let ring = ring([16.9, 52.4, 17.0, 52.5]);
        assert_eq!(ring.len(), 5);
        assert_eq!(ring[0], ring[4]);
        assert_eq!(ring[0], [16.9, 52.4]);
        assert_eq!(ring[2], [17.0, 52.5]);
    }

    #[test]
    fn a_box_of_one_degree_reads_in_kilometres() {
        let (width, height) = size_km([16.0, 52.0, 17.0, 53.0]);
        assert!(width > 40.0 && width < 120.0, "width {width}");
        assert!(height > 100.0 && height < 120.0, "height {height}");
    }

    #[test]
    fn a_drawn_rectangle_gives_a_closed_ring_and_its_box() {
        let drawn = json!({
            "type": "Feature",
            "properties": { "kind": "rectangle" },
            "geometry": {
                "type": "Polygon",
                "coordinates": [[[16.9, 52.4], [17.0, 52.4], [17.0, 52.5], [16.9, 52.5], [16.9, 52.4]]]
            }
        });
        let ring = rectangle_ring(&drawn).expect("a rectangle graphic");
        assert_eq!(ring.len(), 5);
        assert_eq!(ring[0], ring[4]);
        assert_eq!(bounds(&ring), [16.9, 52.4, 17.0, 52.5]);
        // The app draws the ring closed already, and a second corner never reopens it.
        assert_eq!(Mode::of(&[drawn.clone()]), Some(Mode::View));
        // Another graphic, and a rectangle without a shape, are not an area.
        let mut circle = drawn.clone();
        circle["properties"]["kind"] = json!("circle");
        assert_eq!(rectangle_ring(&circle), None);
        let mut empty = drawn.clone();
        empty["geometry"] = Value::Null;
        assert_eq!(rectangle_ring(&empty), None);
    }

    #[test]
    fn a_view_rectangle_is_read_from_the_geometry() {
        let view = [(16.9, 52.4), (17.0, 52.4), (17.0, 52.5), (16.9, 52.5), (16.9, 52.4)];
        assert_eq!(Mode::of(&[feature(&view)]), Some(Mode::View));
        // A triangle, and a shape drawn twice, are not the view's rectangle.
        let triangle = [(16.9, 52.4), (17.0, 52.4), (17.0, 52.5), (16.9, 52.4)];
        assert_eq!(Mode::of(&[feature(&triangle)]), Some(Mode::Drawn));
        assert_eq!(Mode::of(&[feature(&view), feature(&view)]), Some(Mode::Drawn));
        assert_eq!(Mode::of(&[]), None, "an empty layer says nothing");
    }
    /// Boxes a script can send: wrong lengths, inverted corners, huge numbers, non-numbers.
    #[test]
    fn a_box_from_a_script_is_read_or_refused_but_never_a_panic() {
        let mut rng = Lcg::new(7);
        let pool = [0.0_f64, -1.0, 16.9, 52.4, 180.0, 360.0, 1e300, f64::NAN, f64::INFINITY];
        let bad = json!({"bbox": "16.9,52.4"});
        assert!(read_box(&bad).is_none());
        assert!(read_box(&json!({"bbox": [16.9, 52.4, 17.0]})).is_none());
        assert!(read_box(&json!({"bbox": [17.0, 52.4, 16.9, 52.5]})).is_none());
        for _ in 0..500 {
            let count = rng.below(6) as usize;
            let values: Vec<Value> = (0..count)
                .map(|_| json!(rng.pick(&pool)))
                .collect();
            let params = json!({ "bbox": values });
            // Only a sane box comes back, and a bad one is a refusal rather than a panic.
            if let Some(box_lon) = read_box(&params) {
                assert!(box_lon[2] > box_lon[0] && box_lon[3] > box_lon[1], "{box_lon:?}");
            }
        }
    }

    /// Geometry a script can send: deep nesting, mixed types, a stray number pair.
    #[test]
    fn a_shape_from_a_script_is_walked_to_a_stop() {
        let mut rng = Lcg::new(11);
        for _ in 0..200 {
            let depth = rng.below(12) as usize;
            let mut value = json!([[16.9, 52.4], [17.0, 52.5]]);
            for _ in 0..depth {
                value = json!([value, "x", 3]);
            }
            let mut points = Vec::new();
            collect_points(&value, &mut points, 0);
            assert!(points.len() <= 200_000);
        }
    }

}
