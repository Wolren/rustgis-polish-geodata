//! Adding (and taking away) a GUGiK map layer as a web tile overlay.

use crate::catalog::{self, Overlay, ATTRIBUTION, POLAND};
use crate::host::Host;
use crate::text::{self, Lang};
use serde_json::{json, Value};

/// Overlays that GUGiK draws only at large scales: at country zoom their tiles are empty.
const DETAIL_ONLY: &[&str] = &["dzialki-wms", "adresy"];

pub fn toggle(host: &mut Host, overlay: &Overlay, lang: Lang) -> Result<Value, String> {
    let tiles = host.call("map.tiles", json!({}))?;
    let present = tiles.as_array().and_then(|list| {
        list.iter()
            .filter_map(|t| t.get("name").and_then(Value::as_str))
            .find(|name| overlay.matches(name))
            .map(str::to_string)
    });

    if let Some(name) = present {
        host.call("map.removeTiles", json!({ "name": name }))?;
        host.notify(&text::removed(lang, &name));
        return Ok(json!({ "removed": name }));
    }

    // GUGiK serves Poland only: from a world view its tiles are blank, which reads as "broken".
    let view = host.extent().unwrap_or(POLAND);
    let mut zoomed = false;
    if !overlaps(view, POLAND) {
        host.call("map.zoomToExtent", json!({ "extent": POLAND }))?;
        zoomed = true;
    }

    let name = overlay.name(lang);
    host.call(
        "map.addTiles",
        json!({
            "url": catalog::url_for(overlay),
            "kind": "raster",
            "name": name,
            "attribution": ATTRIBUTION,
            "overlay": true,
            "minZoom": overlay.min_zoom,
            "maxZoom": overlay.max_zoom,
            "tileSize": overlay.tile_size,
        }),
    )?;

    let mut note = text::added(lang, name);
    if zoomed {
        note.push_str(text::zoomed_to_poland(lang));
    }
    host.notify(&note);

    if DETAIL_ONLY.contains(&overlay.id) {
        let width = (view[2] - view[0]).abs();
        if zoomed || width > 0.05 {
            host.notify(text::detail_hint(lang));
        }
    }

    Ok(json!({ "added": name, "zoomedToPoland": zoomed }))
}

fn overlaps(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] <= b[2] && a[2] >= b[0] && a[1] <= b[3] && a[3] >= b[1]
}
