//! Cutting features to a boundary layer.
//!
//! A WFS spatial filter returns whole features that touch a boundary, never cut ones. Tools
//! cannot make host requests, so the cut is a separate command: it runs after a download,
//! calls RustGIS's own Clip, and replaces the layer with the cut result.

use crate::host::Host;
use crate::text::{self, Lang};
use serde_json::{json, Value};

/// The map's layers, as `host.layers` gives them.
pub fn layers(host: &mut Host, lang: Lang) -> Result<Vec<Value>, String> {
    let answer = host.call("host.layers", json!({}))?;
    answer
        .as_array()
        .cloned()
        .ok_or_else(|| text::layers_unexpected(lang))
}

/// Whether `layer` can be cut: a vector layer with features, and not the boundary itself.
fn cuttable(layer: &Value, boundary: &str) -> bool {
    let name = layer.get("name").and_then(Value::as_str).unwrap_or_default();
    !name.is_empty()
        && name != boundary
        && !text::AREA_LAYERS.contains(&name)
        && layer.get("raster").and_then(Value::as_bool) != Some(true)
        && layer.get("features").and_then(Value::as_i64).unwrap_or(0) > 0
}

/// The area layer's name on this map, which may carry the other language's wording.
pub fn area_in(list: &[Value]) -> Option<String> {
    list.iter().find_map(|layer| {
        let name = layer.get("name").and_then(Value::as_str)?;
        text::AREA_LAYERS.contains(&name).then(|| name.to_string())
    })
}

/// The boundary chosen in the panel's tree, while the map still holds it.
pub fn chosen(list: &[Value], request: &Value) -> Option<String> {
    let name = request
        .get("panel")
        .and_then(|panel| panel.get("values"))
        .and_then(|values| values.get("cel"))
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty())?;
    let held = list
        .iter()
        .any(|layer| layer.get("name").and_then(Value::as_str) == Some(name));
    held.then(|| name.to_string())
}

/// The layer a caller naming none means: the Contents pane's active layer, else the newest one.
pub fn target(list: &[Value], boundary: &str) -> Option<String> {
    let usable = || list.iter().filter(|layer| cuttable(layer, boundary));
    let active = usable().find(|layer| layer.get("active").and_then(Value::as_bool) == Some(true));
    let newest = usable().max_by_key(|layer| layer.get("id").and_then(Value::as_i64).unwrap_or(0));
    active
        .or(newest)
        .and_then(|layer| layer.get("name").and_then(Value::as_str).map(str::to_string))
}

/// Clips `layer` to `area`, replacing the layer in place.
pub fn to_area(host: &mut Host, layer: &str, area: &str, lang: Lang) -> Result<Value, String> {
    let list = layers(host, lang)?;
    let exists = |name: &str| {
        list.iter()
            .any(|l| l.get("name").and_then(Value::as_str) == Some(name))
    };
    if !exists(layer) {
        return Err(text::no_layer(lang, layer));
    }
    if !exists(area) {
        return Err(text::no_area_layer(lang, area));
    }

    let cut = format!("{layer}{}", text::clipped_suffix(lang));
    host.call(
        "gp.run",
        json!({
            "tool": "clip",
            "params": { "input": layer, "clip": area, "output": cut }
        }),
    )?;

    // Keep one layer, not two: the uncut original obscures what the cut actually kept.
    let kept = host
        .call("host.features", json!({ "layer": cut }))
        .ok()
        .and_then(|f| f.get("features")?.as_array().map(|a| a.len()))
        .unwrap_or(0);
    let _ = host.call("layer.remove", json!({ "layer": layer }));
    let _ = host.call("layer.rename", json!({ "layer": cut, "name": layer }));

    host.notify(&text::clipped(lang, layer, area, kept));
    Ok(json!({ "layer": layer, "area": area, "features": kept }))
}
