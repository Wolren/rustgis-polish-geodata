//! Cutting features to a boundary layer.
//!
//! A WFS spatial filter returns whole features that touch the area, never cut ones. Tools
//! cannot make host requests, so the cut is a separate command: it runs after a download, calls
//! RustGIS's own Clip, and replaces the layer with the cut result.
//!
//! The boundary is always a layer, never the map view: a named layer (`obszar`/`granica`), else
//! the area layer, and when that layer is a selection only the selected shapes count. A view
//! rectangle is a layer too, but the message says so instead of letting it pass as a drawn one.

use crate::host::Host;
use crate::text::{self, Lang};
use serde_json::{json, Value};

/// The layer a clip cuts with, and whether it is a temporary copy of a selection.
pub struct Boundary {
    pub layer: String,
    /// True when the boundary is a copy of the layer's selected features, to be removed after.
    pub temporary: bool,
    /// True when the boundary is the area layer made from the map view.
    pub from_view: bool,
}

/// Resolves the clip boundary: the named layer, else the area layer.
///
/// A polygon layer with a selection is copied into a temporary layer first, so the clip follows
/// the shapes the user picked rather than the whole country the layer may hold.
pub fn boundary(
    host: &mut Host,
    named: Option<&str>,
    lang: Lang,
) -> Result<Boundary, String> {
    let layer = match named.map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) => name.to_string(),
        None => existing_area(host, lang)?,
    };
    let layers = host.call("host.layers", json!({}))?;
    let list = layers.as_array().cloned().unwrap_or_default();
    let found = list
        .iter()
        .find(|entry| entry.get("name").and_then(Value::as_str) == Some(layer.as_str()))
        .ok_or_else(|| text::no_area_layer(lang, &layer))?;
    let from_view = text::AREA_LAYERS.contains(&layer.as_str())
        && crate::obszar::Mode::stored() == crate::obszar::Mode::View;
    let selected = found.get("selected").and_then(Value::as_i64).unwrap_or(0);

    if selected > 0 {
        if let Ok(answer) = host.call(
            "host.features",
            json!({ "layer": layer, "selected": true }),
        ) {
            if let Some(features) = answer.get("features") {
                let collection = json!({ "type": "FeatureCollection", "features": features });
                let temporary = text::clip_boundary_layer(lang);
                host.add_layer(&temporary, collection)?;
                return Ok(Boundary { layer: temporary, temporary: true, from_view });
            }
        }
    }

    Ok(Boundary { layer, temporary: false, from_view })
}

/// The newest vector layer that is not the area, so a caller need not name the layer it just
/// downloaded. Newest is the highest id, which RustGIS assigns in creation order.
pub fn newest_layer(host: &mut Host, area: &str, lang: Lang) -> Result<String, String> {
    let layers = host.call("host.layers", json!({}))?;
    let list = layers
        .as_array()
        .ok_or_else(|| text::layers_unexpected(lang))?;
    let mut best: Option<(&str, i64)> = None;
    for layer in list {
        let name = layer.get("name").and_then(Value::as_str).unwrap_or_default();
        if name.is_empty() || name == area || text::AREA_LAYERS.contains(&name) {
            continue;
        }
        if layer.get("raster").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        if layer.get("features").and_then(Value::as_i64) == Some(0) {
            continue;
        }
        let id = layer.get("id").and_then(Value::as_i64).unwrap_or(0);
        if best.map(|(_, b)| id > b).unwrap_or(true) {
            best = Some((name, id));
        }
    }
    best.map(|(name, _)| name.to_string())
        .ok_or_else(|| text::nothing_to_clip(lang))
}

/// The area layer's name on this map, which may carry the other language's wording.
pub fn existing_area(host: &mut Host, lang: Lang) -> Result<String, String> {
    let layers = host.call("host.layers", json!({}))?;
    let found = layers.as_array().and_then(|all| {
        all.iter().find_map(|layer| {
            let name = layer.get("name").and_then(Value::as_str)?;
            text::AREA_LAYERS.contains(&name).then(|| name.to_string())
        })
    });
    Ok(found.unwrap_or_else(|| text::area_layer(lang).to_string()))
}

/// Clips `layer` to `area`, replacing the layer in place.
pub fn to_area(
    host: &mut Host,
    layer: &str,
    boundary: &Boundary,
    lang: Lang,
) -> Result<Value, String> {
    let area = boundary.layer.as_str();
    let layers = host.call("host.layers", json!({}))?;
    let exists = |name: &str| {
        layers
            .as_array()
            .map(|all| all.iter().any(|l| l.get("name").and_then(Value::as_str) == Some(name)))
            .unwrap_or(false)
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
    if boundary.temporary {
        let _ = host.call("layer.remove", json!({ "layer": area }));
    }

    host.notify(&text::clipped(lang, layer, area, kept, boundary.from_view));
    Ok(json!({
        "layer": layer,
        "area": area,
        "fromView": boundary.from_view,
        "features": kept,
    }))
}
