//! The panel's state: what the pane shows when it opens, and which of its buttons apply.

use crate::cache;
use crate::clip;
use crate::host::Host;
use crate::text::{self, Lang};
use serde_json::{json, Map, Value};

/// Answers the panel: the view, the area, the layers a clip can cut, and the stored answers.
pub fn state(host: &mut Host, lang: Lang, request: &Value) -> Result<Value, String> {
    let extent = host.extent().unwrap_or([0.0; 4]);
    let scale = host
        .call("host.view", json!({}))
        .ok()
        .and_then(|view| view.get("scale").and_then(Value::as_f64))
        .unwrap_or(0.0);
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

    let area_layer = list
        .iter()
        .find(|layer| text::AREA_LAYERS.contains(&named(layer).as_str()));
    let area = area_layer.map(named);
    let boundary = clip::chosen(&list, request).or_else(|| area.clone());
    let target = clip::target(&list, boundary.as_deref().unwrap_or_default());

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
    items.insert(
        "view".into(),
        json!({ "text": text::view_text(lang, extent, scale) }),
    );
    items.insert(
        "area".into(),
        json!({
            "text": match area_layer {
                Some(layer) => text::area_present(lang, features_of(layer) as usize),
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
