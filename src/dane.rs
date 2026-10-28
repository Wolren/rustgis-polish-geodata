//! "Data" commands: one national WFS type, downloaded for the area and added as a layer.
//!
//! The area comes from the area layer's own extent, so a download works even when the layer was
//! read from a file and its features never reach the plugin. The polygon filter is a bonus: when
//! the features do arrive and form one polygon, the server filters by it instead of the box.

use crate::catalog;
use crate::host::Host;
use crate::nmt;
use crate::text::{self, Lang};
use crate::wfs;
use serde_json::{json, Value};

/// Downloads `id` (a catalogue id or label) over the area and adds the features.
pub fn add(host: &mut Host, lang: Lang, request: &Value, id: &str) -> Result<Value, String> {
    let layer = catalog::wfs_by_label(id).ok_or_else(|| text::unknown_dataset(lang, id))?;
    let (source, features) = nmt::area(host, lang, request, text::data_needs_area(lang))?;
    let boxes = nmt::envelopes(&features).ok_or_else(|| text::area_no_geometry(lang, &source))?;
    let bbox = boxes.lon;
    if wfs::too_large(layer, bbox) {
        return Err(wfs::span_error(layer, bbox, lang));
    }

    let area = json!({ "type": "FeatureCollection", "features": features });
    let name = layer.name(lang).to_string();
    let (found, truncated) = {
        let mut report = |line: &str| crate::host::progress_line(line);
        wfs::fetch(layer, bbox, Some(&area), &mut report, lang)?
    };
    if found.is_empty() {
        return Err(text::no_features(lang, &name));
    }
    let count = found.len();
    host.add_layer(&name, json!({ "type": "FeatureCollection", "features": found }))?;
    let mut message = text::data_added(lang, &name, count);
    if truncated {
        message.push(' ');
        message.push_str(text::truncated(lang));
    }
    host.notify(&message);
    Ok(json!({
        "added": name,
        "features": count,
        "area": source,
        "notices": [message],
    }))
}
