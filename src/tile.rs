//! Adding (and taking away) a catalogue layer: a GUGiK overlay, a Wody service, a Cartomatic map.

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
        return remove(host, &name, lang);
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

/// A catalogue WMS, added as one picture of the whole view instead of a tile grid.
struct Picture<'a> {
    /// The layer's name on the map: the catalogue's title.
    name: &'a str,
    /// The service address, without its query string.
    url: &'a str,
    /// The `LAYERS` value: the comma-joined layer list.
    layers: &'a str,
    attribution: &'a str,
    max_zoom: u8,
}

impl Picture<'_> {
    /// The GetMap address RustGIS fetches, with the view left for the host to fill in.
    fn address(&self) -> String {
        wms_template(self.url, self.layers)
    }
}

/// Adds a Wody Polskie service, or takes it away when the same layer is already on the map.
pub fn toggle_wody(host: &mut Host, id: &str, lang: Lang) -> Result<Value, String> {
    let service = catalog::find_wody(id).ok_or_else(|| text::unknown_command(lang, id))?;
    let family = catalog::wody_family();
    toggle_wms(
        host,
        &Picture {
            name: service.title,
            url: service.url,
            layers: &service.layers_csv(),
            attribution: family.attribution,
            max_zoom: family.zoom,
        },
        lang,
    )
}

/// Adds a Cartomatic map, or takes it away when the same layer is already on the map.
pub fn toggle_carto(host: &mut Host, id: &str, lang: Lang) -> Result<Value, String> {
    let map = catalog::find_carto(id).ok_or_else(|| text::unknown_command(lang, id))?;
    let family = catalog::carto_family();
    toggle_wms(
        host,
        &Picture {
            name: map.title,
            url: catalog::CARTO_URL,
            layers: map.layer,
            attribution: family.attribution,
            max_zoom: family.zoom,
        },
        lang,
    )
}

/// Adds a single-image WMS as an overlay, or takes the layer away when it is already on the map.
fn toggle_wms(host: &mut Host, picture: &Picture, lang: Lang) -> Result<Value, String> {
    let tiles = host.call("map.tiles", json!({}))?;
    if let Some(name) = on_map(&tiles, picture.name, &picture.address()) {
        return remove(host, &name, lang);
    }
    host.call("map.addTiles", wms_layer(picture))?;
    host.notify(&text::added(lang, picture.name));
    Ok(json!({ "added": picture.name }))
}

/// The GetMap template of a single-image WMS: one picture of the view, all layers in it.
fn wms_template(url: &str, layers: &str) -> String {
    format!(
        "{url}?SERVICE=WMS&VERSION=1.3.0&REQUEST=GetMap&LAYERS={layers}&STYLES=&CRS=EPSG:3857&\
BBOX={{bbox-epsg-3857}}&WIDTH={{width}}&HEIGHT={{height}}&FORMAT=image/png&TRANSPARENT=TRUE"
    )
}

/// The `map.addTiles` request for a single-image WMS: no `tileSize`, so the host asks for one picture.
fn wms_layer(picture: &Picture) -> Value {
    json!({
        "url": picture.address(),
        "kind": "raster",
        "name": picture.name,
        "attribution": picture.attribution,
        "overlay": true,
        "maxZoom": picture.max_zoom,
    })
}

/// The layer already on the map that is this picture: the same name and the same address.
fn on_map(tiles: &Value, name: &str, url: &str) -> Option<String> {
    tiles.as_array()?.iter().find_map(|tile| {
        let layer = tile.get("name")?.as_str()?;
        let address = tile.get("url")?.as_str()?;
        (layer == name && address.trim() == url.trim()).then(|| layer.to_string())
    })
}

/// Takes the layer off the map and answers the command with what it did.
fn remove(host: &mut Host, name: &str, lang: Lang) -> Result<Value, String> {
    host.call("map.removeTiles", json!({ "name": name }))?;
    host.notify(&text::removed(lang, name));
    Ok(json!({ "removed": name }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture<'a>(url: &'a str, layers: &'a str) -> Picture<'a> {
        Picture {
            name: "Wody Polskie",
            url,
            layers,
            attribution: "PGW Wody Polskie (ISOK)",
            max_zoom: 18,
        }
    }

    /// One request draws every layer: the template names the list and leaves the view to RustGIS.
    #[test]
    fn the_getmap_template_carries_the_layer_list_and_the_view() {
        let template = wms_template("https://wody.isok.gov.pl/gpservices/x/WMSServer", "0,1,2");
        assert!(template.contains("LAYERS=0,1,2"), "{template}");
        assert!(template.contains("CRS=EPSG:3857"), "{template}");
        assert!(template.contains("{bbox-epsg-3857}"), "{template}");
        assert!(
            template.contains("{width}") && template.contains("{height}"),
            "{template}"
        );
        assert_eq!(template.matches("SERVICE=WMS").count(), 1, "{template}");
        assert!(!template.contains("tileSize"), "{template}");
    }

    /// A tile grid of a WMS would ask for the same layers thousands of times: one picture is enough.
    #[test]
    fn a_catalogue_wms_asks_for_no_tile_size() {
        let layer = wms_layer(&picture("https://example.test/wms", "0,1"));
        assert!(layer.get("tileSize").is_none(), "{layer}");
        assert_eq!(layer["overlay"], true, "{layer}");
        assert_eq!(layer["kind"], "raster", "{layer}");
        assert_eq!(layer["maxZoom"], 18, "{layer}");
        assert_eq!(layer["attribution"], "PGW Wody Polskie (ISOK)", "{layer}");
        assert!(
            layer["url"].as_str().unwrap_or_default().contains("LAYERS=0,1"),
            "{layer}"
        );
    }

    /// Every catalogue entry becomes a template that carries its own layers and the view.
    #[test]
    fn every_catalogue_entry_builds_its_own_getmap() {
        for service in catalog::wody() {
            let template = wms_template(service.url, &service.layers_csv());
            let expected = format!("LAYERS={}&STYLES=", service.layers_csv());
            assert!(template.contains(&expected), "{}: {template}", service.id);
            assert!(template.contains("{bbox-epsg-3857}"), "{}", service.id);
        }
        for map in catalog::cartomatic() {
            let template = wms_template(catalog::CARTO_URL, map.layer);
            let expected = format!("LAYERS={}&STYLES=", map.layer);
            assert!(template.contains(&expected), "{}: {template}", map.id);
            assert!(template.contains("{bbox-epsg-3857}"), "{}", map.id);
        }
    }

    /// A layer of the same name from another server is not this picture, so it stays untouched.
    #[test]
    fn a_layer_with_the_name_but_another_address_is_not_the_picture() {
        let url = picture("https://example.test/wms", "0").address();
        let ours = json!([{ "name": "Wody Polskie", "url": url }]);
        assert_eq!(
            on_map(&ours, "Wody Polskie", &url),
            Some("Wody Polskie".to_string())
        );
        let foreign = json!([{ "name": "Wody Polskie", "url": "https://example.test/other" }]);
        assert_eq!(on_map(&foreign, "Wody Polskie", &url), None);
        let other = json!([{ "name": "Inne", "url": url }]);
        assert_eq!(on_map(&other, "Wody Polskie", &url), None);
    }
}
