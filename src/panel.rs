//! The panel's state: what the pane shows when it opens, and which of its buttons apply.

use crate::cache;
use crate::clip;
use crate::host::Host;
use crate::text::{self, Lang};
use serde_json::{json, Map, Value};

/// Copies the panel's state into a command's answer, so the pane shows what the command changed.
pub fn attach(answer: &mut Value, state: Value) {
    let (Some(fields), Some(panel)) = (answer.as_object_mut(), state.get("panel")) else {
        return;
    };
    fields.insert("panel".to_string(), panel.clone());
}

/// Answers the panel: the view, the area, the layers a clip can cut, and the stored answers.
pub fn state(host: &mut Host, lang: Lang, request: &Value) -> Result<Value, String> {
    // A missing extent is hidden, not shown as [0, 0, 0, 0]: that box looks like a real view.
    let view = match host.extent() {
        Ok(extent) => json!({ "text": text::view_text(lang, extent, scale(host)), "visible": true }),
        Err(_) => json!({ "text": text::view_missing(lang), "visible": false }),
    };
    let list = clip::layers(host, lang)?;

    let named = |layer: &Value| {
        layer
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let features_of = |layer: &Value| layer.get("features").and_then(Value::as_i64).unwrap_or(0);
    let polygon = |layer: &Value| {
        layer
            .get("geometry")
            .and_then(Value::as_str)
            .is_some_and(|shape| shape.contains("Polygon"))
    };

    // The panel and the cutting command share one reading of the request.
    let (target, boundary) = clip::plan(&list, request);
    let area = clip::area_in(&list);
    let area_features = area
        .as_deref()
        .and_then(|name| list.iter().find(|layer| named(layer) == name))
        .map(|layer| usize::try_from(features_of(layer)).unwrap_or(0))
        .unwrap_or(0);

    // A polygon layer is the only boundary; the area layer is listed even while it is empty.
    let mut nodes: Vec<Value> = list
        .iter()
        .filter(|layer| polygon(layer))
        .filter(|layer| features_of(layer) > 0 || area.as_deref() == Some(named(layer).as_str()))
        .map(|layer| json!({ "id": named(layer), "label": named(layer), "count": features_of(layer) }))
        .collect();
    nodes.sort_by_key(|node| node["label"].as_str().unwrap_or_default().to_lowercase());
    let has_nodes = !nodes.is_empty();

    let others = list
        .iter()
        .filter(|layer| !text::AREA_LAYERS.contains(&named(layer).as_str()))
        .filter(|layer| layer.get("raster").and_then(Value::as_bool) != Some(true))
        .filter(|layer| features_of(layer) > 0)
        .count();

    let (files, bytes) = cache::stats();
    let (models, model_bytes) = cache::dem_stats();
    let (files, bytes) = (files + models, bytes + model_bytes);
    let mut items = Map::new();
    items.insert("view".into(), view);
    items.insert(
        "area".into(),
        json!({
            "text": match &area {
                Some(_) => text::area_present(lang, area_features),
                None => text::area_missing(lang),
            }
        }),
    );
    items.insert(
        "clip".into(),
        json!({
            "text": match (&target, &boundary) {
                (Some(layer), Some(boundary)) => text::clip_text(layer, boundary),
                (None, _) => text::clip_missing(lang),
                (_, None) => text::clip_no_boundary(lang),
            }
        }),
    );
    items.insert(
        "cel".into(),
        json!({ "nodes": nodes, "enabled": has_nodes }),
    );
    items.insert(
        "cache".into(),
        json!({ "text": text::cache_text(lang, files, bytes) }),
    );
    items.insert(
        "catalogue".into(),
        json!({
            "text": text::catalogue_text(
                lang,
                crate::catalog::offered().count(),
                crate::catalog::wfs_layers().len()
            )
        }),
    );
    items.insert(
        "przytnij".into(),
        json!({ "enabled": target.is_some() && boundary.is_some() }),
    );
    items.insert("wyczysc-cache".into(), json!({ "enabled": files > 0 }));

    Ok(json!({
        "panel": { "items": items },
        "area": area.is_some(),
        "layers": others,
        "clip": { "layer": target, "boundary": boundary },
    }))
}

/// The map's scale, or 0 when the app does not report it.
fn scale(host: &mut Host) -> f64 {
    host.call("host.view", json!({}))
        .ok()
        .and_then(|view| view.get("scale").and_then(Value::as_f64))
        .unwrap_or(0.0)
}
