//! The panel's state: what the pane shows when it opens, and which of its buttons apply.

use crate::cache;
use crate::host::Host;
use crate::text::{self, Lang};
use serde_json::{json, Map, Value};

/// Answers the panel: the view, the area layer, the stored answers, and which buttons apply.
pub fn state(host: &mut Host, lang: Lang, _request: &Value) -> Result<Value, String> {
    let extent = host.extent().unwrap_or([0.0; 4]);
    let scale = host
        .call("host.view", json!({}))
        .ok()
        .and_then(|view| view.get("scale").and_then(Value::as_f64))
        .unwrap_or(0.0);
    let layers = host.call("host.layers", json!({}))?;
    let list = layers.as_array().cloned().unwrap_or_default();

    let named = |layer: &Value| {
        layer
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_default()
    };
    let features_of = |layer: &Value| layer.get("features").and_then(Value::as_i64).unwrap_or(0);

    let area = list.iter().find(|l| text::AREA_LAYERS.contains(&named(l).as_str()));
    let others = list
        .iter()
        .filter(|l| !text::AREA_LAYERS.contains(&named(l).as_str()))
        .filter(|l| l.get("raster").and_then(Value::as_bool) != Some(true))
        .filter(|l| features_of(l) > 0)
        .count();

    let (files, bytes) = cache::stats();
    let (models, model_bytes) = cache::dem_stats();
    let (files, bytes) = (files + models, bytes + model_bytes);
    let mut items = Map::new();
    items.insert("view".into(), json!({ "text": text::view_text(lang, extent, scale) }));
    items.insert(
        "area".into(),
        json!({
            "text": match area {
                Some(layer) => text::area_present(lang, features_of(layer) as usize),
                None => text::area_missing(lang),
            }
        }),
    );
    items.insert("cache".into(), json!({ "text": text::cache_text(lang, files, bytes) }));
    items.insert("przytnij".into(), json!({ "enabled": area.is_some() && others > 0 }));
    items.insert("wyczysc-cache".into(), json!({ "enabled": files > 0 }));

    Ok(json!({
        "panel": { "items": items },
        "area": area.is_some(),
        "layers": others,
    }))
}
