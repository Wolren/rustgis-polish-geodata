//! The national services catalogue: WMS addresses that answered a real GetMap when it was built.
//!
//! `data/services.json` holds the verified subset of the geoportal catalogue. Every address is a
//! GetMap template, so no capability document is read at run time.

use crate::host::Host;
use crate::text::{self, Lang};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
pub struct Service {
    pub id: String,
    pub name: String,
    pub source: String,
    pub url: String,
    pub layer: String,
    pub zoom: u8,
    /// The short label the ribbon shows, kept apart from the full catalogue name.
    #[serde(default)]
    pub label: String,
    /// The plugin command that adds this service, one per catalogue entry.
    #[serde(default)]
    pub cmd: String,
    /// Who publishes the service, as the catalogue file records it.
    #[serde(default)]
    pub institution: String,
    /// The ribbon menu it sits in, when the institution spreads over more than one.
    #[serde(default, rename = "institutionMenu")]
    pub institution_menu: String,
}

impl Service {
    /// The short label when the catalogue sets one, the full name otherwise.
    pub fn display(&self) -> &str {
        if self.label.is_empty() {
            &self.name
        } else {
            &self.label
        }
    }
}

const RAW: &str = include_str!("../data/services.json");

/// The catalogue, parsed once per process.
pub fn all() -> &'static [Service] {
    static PARSED: std::sync::OnceLock<Vec<Service>> = std::sync::OnceLock::new();
    PARSED.get_or_init(|| serde_json::from_str(RAW).unwrap_or_default())
}

/// The service a request names, by id or by its catalogue name.
pub fn find(wanted: &str) -> Option<&'static Service> {
    let wanted = wanted.trim();
    all()
        .iter()
        .find(|s| s.id == wanted || s.cmd == wanted || s.name.eq_ignore_ascii_case(wanted))
}

/// The service a request names: the `service` parameter, or the command that carried it.
pub fn chosen(request: &Value) -> Option<&'static Service> {
    let named = request
        .get("params")
        .and_then(|params| params.get("service"))
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty());
    if let Some(wanted) = named {
        return find(wanted);
    }
    // A ribbon entry names its own command, so the command id is the service.
    if let Some(id) = request.get("id").and_then(Value::as_str) {
        if let Some(service) = all().iter().find(|s| s.cmd == id) {
            return Some(service);
        }
    }
    None
}

/// Adds a catalogue service as a tile overlay, or takes it away when it is already on the map.
pub fn toggle(host: &mut Host, lang: Lang, request: &Value) -> Result<Value, String> {
    let service = chosen(request).ok_or_else(|| {
        let wanted = request
            .get("params")
            .and_then(|params| params.get("service"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        text::service_unknown(lang, wanted, all().len())
    })?;

    let tiles = host.call("map.tiles", json!({}))?;
    let present = tiles.as_array().and_then(|list| {
        list.iter()
            .filter_map(|tile| tile.get("name").and_then(Value::as_str))
            .find(|name| *name == service.display())
            .map(str::to_string)
    });
    if let Some(name) = present {
        host.call("map.removeTiles", json!({ "name": name }))?;
        host.notify(&text::removed(lang, &name));
        return Ok(json!({ "removed": name }));
    }

    host.call(
        "map.addTiles",
        json!({
            "url": service.url,
            "kind": "raster",
            "name": service.display(),
            "attribution": service.source,
            "overlay": true,
            "minZoom": 0,
            "maxZoom": service.zoom,
            "tileSize": 256,
        }),
    )?;
    host.notify(&text::service_added(
        lang,
        service.display(),
        service.layer.as_str(),
    ));
    Ok(json!({
        "added": service.display(),
        "source": service.source,
        "layer": service.layer,
        "services": all().len(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalogue_parses_and_every_entry_can_be_asked_for_tiles() {
        let list = all();
        assert!(list.len() > 20, "only {} services", list.len());
        for service in list {
            assert!(!service.id.is_empty(), "{} has no id", service.name);
            assert!(!service.name.is_empty(), "{} has no name", service.id);
            assert!(
                service.url.contains("{bbox-epsg-3857}"),
                "{} is not a template",
                service.id
            );
            assert!(
                service.url.contains("{width}"),
                "{} has no width",
                service.id
            );
            assert!(!service.layer.is_empty(), "{} names no layer", service.id);
            assert!(
                service.zoom >= 14 && service.zoom <= 22,
                "{} zoom {}",
                service.id,
                service.zoom
            );
        }
    }

    #[test]
    fn unique_ids_and_names() {
        let mut ids: Vec<&str> = all().iter().map(|s| s.id.as_str()).collect();
        ids.sort();
        let before = ids.len();
        ids.dedup();
        assert_eq!(before, ids.len(), "duplicate id");
        let mut names: Vec<String> = all().iter().map(|s| s.name.to_lowercase()).collect();
        names.sort();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len(), "duplicate name");
    }

    #[test]
    fn a_service_is_found_by_id_and_by_name() {
        let first = &all()[0];
        assert!(find(&first.id).is_some());
        assert!(find(&first.name).is_some());
        assert!(find("no such service").is_none());
    }
}
