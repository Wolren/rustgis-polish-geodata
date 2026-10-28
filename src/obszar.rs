//! "Draw area": the drawing tool, the area layer it fills, and the modes that shape an area.
//!
//! A grid request honours a rectangle only, so rectangle mode stores the bounding box of the drawn
//! shape and polygon mode keeps the shape as drawn.

use crate::cache;
use crate::gk92;
use crate::host::Host;
use crate::text::{self, Lang};
use serde_json::{json, Value};

/// How the drawn shape becomes an area.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// The bounding box of the drawn shape, the only shape a grid request can honour.
    Rectangle,
    /// The shape as drawn, kept for the record.
    Polygon,
    /// Two opposite corners, joined as the diagonal of a rectangle.
    Line,
    /// The current map view.
    View,
}

impl Mode {
    pub fn parse(key: &str) -> Mode {
        match key {
            "poligon" => Mode::Polygon,
            "linia" => Mode::Line,
            "widok" => Mode::View,
            _ => Mode::Rectangle,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Mode::Rectangle => "prostokat",
            Mode::Polygon => "poligon",
            Mode::Line => "linia",
            Mode::View => "widok",
        }
    }

    /// Two clicks make a diagonal, three or more make a ring.
    fn tool(self) -> &'static str {
        match self {
            Mode::Line => "createLine",
            _ => "createPolygon",
        }
    }

    /// The mode of the last draw command, so a model knows how to read the shape.
    pub fn stored() -> Mode {
        std::fs::read_to_string(cache::area_mode_file())
            .map(|text| Mode::parse(text.trim()))
            .unwrap_or(Mode::Rectangle)
    }

    fn remember(self) {
        std::fs::write(cache::area_mode_file(), self.key()).ok();
    }
}

/// Puts the drawing tool on the map with an empty area layer active.
pub fn draw(host: &mut Host, lang: Lang, mode: Mode) -> Result<Value, String> {
    // The app keeps geometry types per layer, so a diagonal needs a line layer, not a polygon one.
    let template = if mode == Mode::Line { "lines" } else { "areas" };
    let name = fresh(host, lang, template)?;
    host.call("layer.select", json!({ "layer": name }))?;
    host.call("edit.enable", json!({ "on": true }))?;
    host.call("tool.set", json!({ "tool": mode.tool() }))?;
    mode.remember();
    host.notify(&text::area_armed(lang, mode.key(), &name));
    Ok(json!({ "layer": name, "mode": mode.key(), "tool": mode.tool(), "editing": true }))
}

/// Fills the area layer with the current view, so a rectangle needs no drawing at all.
pub fn from_view(host: &mut Host, lang: Lang) -> Result<Value, String> {
    let extent = host.extent()?;
    if extent[2] <= extent[0] || extent[3] <= extent[1] {
        return Err(text::area_no_view(lang));
    }
    let name = fresh(host, lang, "areas")?;
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

/// Drops the drawing tool. The app also cancels a half-drawn ring on Escape.
pub fn abort(host: &mut Host, lang: Lang) -> Result<Value, String> {
    host.call("tool.set", json!({ "tool": "explore" }))?;
    host.notify(&text::area_aborted(lang));
    Ok(json!({ "tool": "explore" }))
}

/// Replaces the drawn shape with its bounding box, so the layer shows the ground the model covers.
pub fn snap(host: &mut Host, name: &str, box_lon: [f64; 4]) -> Result<(), String> {
    host.call("layer.remove", json!({ "layer": [name] }))?;
    host.call("layer.template", json!({ "template": "areas", "name": name }))?;
    host.call("edit.enable", json!({ "on": true }))?;
    host.call(
        "edit.addFeature",
        json!({ "layer": name, "geometry": { "type": "Polygon", "coordinates": [ring(box_lon)] } }),
    )?;
    Ok(())
}

/// The area layer with no features: the drawn one emptied, or a new one in the interface language.
fn fresh(host: &mut Host, lang: Lang, template: &str) -> Result<String, String> {
    let layers = host.call("host.layers", json!({}))?;
    let list = layers.as_array().cloned().unwrap_or_default();
    // A layer drawn in the other language is the same layer, so it is reused and never doubled.
    let existing = list.iter().find_map(|layer| {
        let name = layer.get("name").and_then(Value::as_str)?;
        text::AREA_LAYERS.contains(&name).then(|| name.to_string())
    });
    let name = existing.unwrap_or_else(|| text::area_layer(lang).to_string());
    if name.is_empty() {
        return Err(text::area_no_geometry(lang, &name));
    }
    host.call("layer.remove", json!({ "layer": [name.clone()] })).ok();
    host.call("layer.template", json!({ "template": template, "name": name.clone() }))?;
    Ok(name)
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
    fn every_mode_key_parses_back() {
        for mode in [Mode::Rectangle, Mode::Polygon, Mode::Line, Mode::View] {
            assert_eq!(Mode::parse(mode.key()), mode);
        }
        assert_eq!(Mode::parse("unknown"), Mode::Rectangle);
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
