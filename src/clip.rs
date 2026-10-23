//! Cutting features to the area boundary.
//!
//! A WFS spatial filter returns whole features that touch the area, never cut ones. Tools
//! cannot make host requests, so the cut is a separate command: it runs after a download,
//! calls RustGIS's own Clip, and replaces the layer with the cut result.

use crate::host::Host;
use serde_json::{json, Value};

/// The layer a download produces, and the area it was cut to.
pub const AREA_LAYER: &str = "Obszar pobierania";

/// The newest vector layer that is not the area, so a caller need not name the layer it just
/// downloaded. Newest is the highest id, which RustGIS assigns in creation order.
pub fn newest_layer(host: &mut Host, area: &str) -> Result<String, String> {
    let layers = host.call("host.layers", json!({}))?;
    let list = layers
        .as_array()
        .ok_or_else(|| "RustGIS podało warstwy w nieoczekiwanej postaci".to_string())?;
    let mut best: Option<(&str, i64)> = None;
    for layer in list {
        let name = layer.get("name").and_then(Value::as_str).unwrap_or_default();
        if name.is_empty() || name == area {
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
        .ok_or_else(|| "nie ma warstwy do przycięcia — najpierw pobierz dane".to_string())
}

/// Clips `layer` to `area`, replacing the layer in place.
pub fn to_area(host: &mut Host, layer: &str, area: &str) -> Result<Value, String> {
    let layers = host.call("host.layers", json!({}))?;
    let exists = |name: &str| {
        layers
            .as_array()
            .map(|all| all.iter().any(|l| l.get("name").and_then(Value::as_str) == Some(name)))
            .unwrap_or(false)
    };
    if !exists(layer) {
        return Err(format!("nie ma warstwy „{layer}” do przycięcia"));
    }
    if !exists(area) {
        return Err(format!("nie ma warstwy obszaru „{area}”"));
    }

    let cut = format!("{layer} (przycięte)");
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

    host.notify(&format!("Przycięto „{layer}” do „{area}”: {kept} obiektów"));
    Ok(json!({ "layer": layer, "area": area, "features": kept }))
}
