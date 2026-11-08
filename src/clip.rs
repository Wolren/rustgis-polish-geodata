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

/// The boundary named in the panel's tree, whether or not the map still holds it.
pub fn asked(request: &Value) -> Option<String> {
    request
        .get("panel")
        .and_then(|panel| panel.get("values"))
        .and_then(|values| values.get("cel"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

/// Whether the map holds a layer of this name.
fn held(list: &[Value], name: &str) -> bool {
    list.iter()
        .any(|layer| layer.get("name").and_then(Value::as_str) == Some(name))
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

/// The clip a request asks for, as the panel shows it and `cut` runs it.
pub fn plan(list: &[Value], request: &Value) -> (Option<String>, Option<String>) {
    let params = request.get("params").cloned().unwrap_or(Value::Null);
    let boundary = params
        .get("obszar")
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .map(str::to_string)
        .or_else(|| match asked(request) {
            // A name the map no longer holds is still the pick: no press switches to another.
            Some(name) => held(list, &name).then_some(name),
            None => area_in(list),
        });
    let named = params
        .get("layer")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let layer = if named.trim().is_empty() {
        target(list, boundary.as_deref().unwrap_or_default())
    } else {
        Some(named.to_string())
    };
    (layer, boundary)
}

/// The layer `gp.run` reports as added: its id, or the first of several.
fn added(answer: &Value) -> Option<Value> {
    answer
        .get("layer")
        .cloned()
        .or_else(|| answer.get("layers")?.as_array()?.first().cloned())
}

/// The boundary a clip cuts with, and whether it is a temporary copy of a selection.
pub struct Boundary {
    pub layer: String,
    /// True when the boundary is a copy of the selected shapes, so it goes after the cut.
    pub temporary: bool,
    pub from_view: bool,
}

/// Resolves the boundary: the named layer, else the area layer.
///
/// A polygon layer with a selection is copied into a temporary layer first, so the cut follows
/// the shapes the user picked rather than everything the layer holds.
pub fn boundary(host: &mut Host, named: Option<&str>, lang: Lang) -> Result<Boundary, String> {
    let layer = match named.map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) => name.to_string(),
        None => {
            let list = layers(host, lang)?;
            area_in(&list).ok_or_else(|| text::clip_no_boundary(lang))?
        }
    };
    let list = layers(host, lang)?;
    let found = list
        .iter()
        .find(|entry| entry.get("name").and_then(Value::as_str) == Some(layer.as_str()))
        .ok_or_else(|| text::no_area_layer(lang, &layer))?;
    let from_view = crate::text::AREA_LAYERS.contains(&layer.as_str())
        && crate::obszar::Mode::stored() == crate::obszar::Mode::View;
    let selected = found.get("selected").and_then(Value::as_i64).unwrap_or(0);
    if selected > 0 {
        if let Ok(answer) = host.call("host.features", json!({ "layer": layer, "selected": true })) {
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

/// Cuts a layer to the boundary the request names, following a selection when there is one.
pub fn cut(host: &mut Host, request: &Value, lang: Lang) -> Result<Value, String> {
    let list = layers(host, lang)?;
    let (layer, named) = plan(&list, request);
    let boundary = boundary(host, named.as_deref(), lang)?;
    let layer = layer.ok_or_else(|| text::nothing_to_clip(lang))?;
    if layer == boundary.layer {
        return Err(text::clip_same(lang, &layer));
    }
    let answer = to_area(host, &layer, &boundary.layer, lang);
    if boundary.temporary {
        host.call("layer.remove", json!({ "layer": [boundary.layer] })).ok();
    }
    let mut answer = answer?;
    if let Some(object) = answer.as_object_mut() {
        object.insert("from_view".into(), json!(boundary.from_view));
    }
    Ok(answer)
}

/// Clips `layer` to `area`, replacing the layer in place.
pub fn to_area(host: &mut Host, layer: &str, area: &str, lang: Lang) -> Result<Value, String> {
    let list = layers(host, lang)?;
    if !held(&list, layer) {
        return Err(text::no_layer(lang, layer));
    }
    if !held(&list, area) {
        return Err(text::no_area_layer(lang, area));
    }

    let cut = format!("{layer}{}", text::clipped_suffix(lang));
    let answer = host.call(
        "gp.run",
        json!({
            "tool": "clip",
            "params": { "input": layer, "clip": area, "output": cut }
        }),
    )?;
    // The host renames a taken output, so the answer's layer id is the only safe handle.
    let made = added(&answer).ok_or_else(|| text::clipped_unreported(lang))?;
    let after = layers(host, lang)?;
    let produced = after
        .iter()
        .find(|found| found.get("id") == Some(&made))
        .ok_or_else(|| text::clipped_unreported(lang))?;
    let kept = produced.get("features").and_then(Value::as_i64).unwrap_or(0);

    if kept <= 0 {
        // Nothing kept: the empty result goes, and the original stays exactly as it was.
        host.call("layer.remove", json!({ "layer": [made] }))?;
        return Err(text::clipped_empty(lang, layer, area));
    }
    let kept = usize::try_from(kept).unwrap_or(0);

    // Keep one layer, not two: the uncut original obscures what the cut actually kept.
    host.call("layer.remove", json!({ "layer": layer }))?;
    host.call("layer.rename", json!({ "layer": made, "name": layer }))?;

    host.notify(&text::clipped(lang, layer, area, kept));
    Ok(json!({ "layer": layer, "area": area, "features": kept }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(name: &str, id: i64, features: i64, active: bool) -> Value {
        json!({ "name": name, "id": id, "features": features, "active": active, "raster": false })
    }

    fn map() -> Vec<Value> {
        vec![
            layer("Area to download", 1, 1, false),
            layer("parcels", 2, 40, false),
            layer("roads", 3, 7, true),
        ]
    }

    fn request(cel: Value) -> Value {
        json!({ "params": {}, "panel": { "values": { "cel": cel } } })
    }

    #[test]
    fn a_panel_pick_is_the_boundary_and_the_active_layer_is_cut() {
        let (layer, boundary) = plan(&map(), &request(json!("parcels")));
        assert_eq!(boundary, Some("parcels".to_string()));
        assert_eq!(layer, Some("roads".to_string()));
    }

    #[test]
    fn nothing_chosen_falls_back_to_the_area_layer() {
        let (layer, boundary) = plan(&map(), &request(Value::Null));
        assert_eq!(boundary, Some("Area to download".to_string()));
        assert_eq!(layer, Some("roads".to_string()));
    }

    #[test]
    fn a_pick_the_map_lost_is_no_boundary_and_no_switch() {
        let (_, boundary) = plan(&map(), &request(json!("gone")));
        assert_eq!(boundary, None, "the area layer must not stand in for a lost pick");
        assert_eq!(asked(&request(json!("gone"))), Some("gone".to_string()));
    }

    #[test]
    fn parameters_name_both_sides_of_the_cut() {
        let asked = json!({ "params": { "obszar": "roads", "layer": "parcels" } });
        let (layer, boundary) = plan(&map(), &asked);
        assert_eq!(layer, Some("parcels".to_string()));
        assert_eq!(boundary, Some("roads".to_string()));
    }

    #[test]
    fn the_boundary_is_never_the_layer_to_cut() {
        let (layer, _) = plan(&map(), &request(json!("parcels")));
        assert_ne!(layer, Some("parcels".to_string()));
    }
}
