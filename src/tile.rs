//! Adding (and taking away) a GUGiK map layer as a web tile overlay.

use crate::catalog::{self, Overlay, ATTRIBUTION, POLAND};
use crate::host::Host;
use serde_json::{json, Value};

/// Overlays that GUGiK draws only at large scales: at country zoom their tiles are empty.
const DETAIL_ONLY: &[&str] = &["dzialki-wms", "adresy"];

pub fn toggle(host: &mut Host, overlay: &Overlay) -> Result<Value, String> {
    let tiles = host.call("map.tiles", json!({}))?;
    let present = tiles
        .as_array()
        .map(|list| list.iter().any(|t| t.get("name").and_then(Value::as_str) == Some(overlay.name)))
        .unwrap_or(false);

    if present {
        host.call("map.removeTiles", json!({ "name": overlay.name }))?;
        host.notify(&format!("Usunięto: {}", overlay.name));
        return Ok(json!({ "removed": overlay.name }));
    }

    // GUGiK serves Poland only: from a world view its tiles are blank, which reads as "broken".
    let view = host.extent().unwrap_or(POLAND);
    let mut zoomed = false;
    if !overlaps(view, POLAND) {
        host.call("map.zoomToExtent", json!({ "extent": POLAND }))?;
        zoomed = true;
    }

    host.call(
        "map.addTiles",
        json!({
            "url": catalog::url_for(overlay),
            "kind": "raster",
            "name": overlay.name,
            "attribution": ATTRIBUTION,
            "overlay": true,
            "minZoom": overlay.min_zoom,
            "maxZoom": overlay.max_zoom,
            "tileSize": overlay.tile_size,
        }),
    )?;

    let mut note = format!("Dodano: {}", overlay.name);
    if zoomed {
        note.push_str(" — przybliżono do Polski");
    }
    host.notify(&note);

    if DETAIL_ONLY.contains(&overlay.id) {
        let width = (view[2] - view[0]).abs();
        if zoomed || width > 0.05 {
            host.notify("Działki i adresy GUGiK rysuje dopiero od skali ok. 1:5000 — przybliż mapę");
        }
    }

    Ok(json!({ "added": overlay.name, "zoomedToPoland": zoomed }))
}

fn overlaps(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] <= b[2] && a[2] >= b[0] && a[1] <= b[3] && a[3] >= b[1]
}
