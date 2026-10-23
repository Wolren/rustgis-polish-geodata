//! "Obszar z widoku": turn the current map view into a polygon layer.
//!
//! Downloads are a tool, and tools run in the background but cannot read the map. This one
//! command captures the view as a rectangle layer, which the tool then takes as its area —
//! visible on the map, editable, and reusable.

use crate::host::Host;
use serde_json::{json, Value};

pub const AREA_LAYER: &str = "Obszar pobierania";

pub fn from_view(host: &mut Host) -> Result<Value, String> {
    let [w, s, e, n] = host.extent()?;
    if e <= w || n <= s {
        return Err("widok mapy nie ma poprawnego zasięgu".into());
    }

    let layers = host.call("host.layers", json!({}))?;
    let exists = layers
        .as_array()
        .map(|all| all.iter().any(|l| l.get("name").and_then(Value::as_str) == Some(AREA_LAYER)))
        .unwrap_or(false);
    if exists {
        let _ = host.call("layer.remove", json!({ "layer": AREA_LAYER }));
    }

    let ring = json!([[w, s], [e, s], [e, n], [w, n], [w, s]]);
    let collection = json!({
        "type": "FeatureCollection",
        "features": [{
            "type": "Feature",
            "properties": {
                "zachod": w, "poludnie": s, "wschod": e, "polnoc": n,
                "szerokosc": e - w, "wysokosc": n - s
            },
            "geometry": { "type": "Polygon", "coordinates": [ring] }
        }]
    });
    host.add_layer(AREA_LAYER, collection)?;
    host.notify(&format!(
        "Obszar pobierania: {:.3}°–{:.3}° E, {:.3}°–{:.3}° N",
        w, e, s, n
    ));
    Ok(json!({ "layer": AREA_LAYER, "bbox": [w, s, e, n] }))
}
