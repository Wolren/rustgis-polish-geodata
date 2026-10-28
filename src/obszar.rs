//! The area layer: a rectangle taken from the map view, or a shape drawn on the map.
//!
//! RustGIS draws editable features point by point and has no rectangle tool, so a rectangle is
//! taken from the view instead of drawn: one click, exact, and visible as a rectangle at once.
//! A shape that is drawn stays as drawn; the elevation model, which can only fetch a rectangle,
//! says which rectangle it is covering.

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

    /// The mode of the last area command, so a model knows whether to snap the shape.
    pub fn stored() -> Mode {
        match std::fs::read_to_string(cache::area_mode_file()) {
            Ok(text) if text.trim() == Mode::View.key() => Mode::View,
            _ => Mode::Drawn,
        }
    }

    fn remember(self) {
        std::fs::write(cache::area_mode_file(), self.key()).ok();
    }
}

/// Fills the area layer with the current view, so a rectangle needs no drawing at all.
pub fn rectangle_from_view(host: &mut Host, lang: Lang) -> Result<Value, String> {
    let extent = host.extent()?;
    if extent[2] <= extent[0] || extent[3] <= extent[1] {
        return Err(text::area_no_view(lang));
    }
    let name = area_layer(host, lang)?;
    // The layer is emptied, not dropped: its renderer and place in the layer list stay.
    host.call("layer.remove", json!({ "layer": [name.clone()] }))
        .ok();
    host.call(
        "layer.template",
        json!({ "template": "areas", "name": name.clone() }),
    )?;
    host.call("edit.enable", json!({ "on": true }))?;
    host.call(
        "edit.addFeature",
        json!({ "layer": name, "geometry": { "type": "Polygon", "coordinates": [ring(extent)] } }),
    )?;
    host.call("layer.select", json!({ "layer": name }))?;
    host.call("tool.set", json!({ "tool": "explore" }))?;
    Mode::View.remember();
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
    let present = list
        .iter()
        .any(|layer| layer.get("name").and_then(Value::as_str) == Some(name.as_str()));
    if !present {
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
    host.call("tool.set", json!({ "tool": "explore" }))?;
    host.notify(&text::area_aborted(lang));
    Ok(json!({ "tool": "explore" }))
}

/// Removes the area layer, so the next rectangle or shape starts empty.
pub fn clear(host: &mut Host, lang: Lang) -> Result<Value, String> {
    let layers = host.call("host.layers", json!({}))?;
    let list = layers.as_array().cloned().unwrap_or_default();
    let present = list
        .iter()
        .filter_map(|layer| layer.get("name").and_then(Value::as_str))
        .find(|name| text::AREA_LAYERS.contains(name))
        .map(str::to_string);
    let Some(name) = present else {
        return Err(text::area_none(lang));
    };
    host.call("layer.remove", json!({ "layer": [name.clone()] }))?;
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

/// Replaces the drawn shape with its bounding box, so the layer shows the ground the model covers.
pub fn snap(host: &mut Host, name: &str, box_lon: [f64; 4]) -> Result<(), String> {
    host.call("layer.remove", json!({ "layer": [name] }))?;
    host.call(
        "layer.template",
        json!({ "template": "areas", "name": name }),
    )?;
    host.call("edit.enable", json!({ "on": true }))?;
    host.call(
        "edit.addFeature",
        json!({ "layer": name, "geometry": { "type": "Polygon", "coordinates": [ring(box_lon)] } }),
    )?;
    Ok(())
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
}
