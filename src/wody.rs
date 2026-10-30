//! Wody Polskie: 32 WMS services and 904 layers behind one command each.
//!
//! These are the flood, drought and water-planning maps of Państwowe Gospodarstwo Wodne Wody
//! Polskie, on `wody.isok.gov.pl/gpservices`. The services are ArcGIS MapServers whose WMS
//! capabilities name every layer with a number and title it with that same number, so the
//! readable names come from the QGIS plugin's table (`tools/wody.py`, `data/wody.json`).
//!
//! One service is one command, because a command per layer would be 904 ribbon entries. The
//! layers of a service stay data: the command takes `warstwa`, and `warstwy` lists what a
//! service publishes, so a script or an agent can find the name before asking for it.

use crate::host::Host;
use crate::text::{self, Lang};
use serde::Deserialize;
use serde_json::{json, Value};

const RAW: &str = include_str!("../data/wody.json");
/// Who publishes the data, shown as the layer's attribution.
const ATTRIBUTION: &str = "PGW Wody Polskie · wody.isok.gov.pl";

#[derive(Deserialize)]
pub struct Layer {
    /// The MapServer layer id, which is what its WMS takes as `LAYERS`.
    pub id: String,
    pub name: String,
}

#[derive(Deserialize)]
pub struct Service {
    pub id: String,
    pub name: String,
    /// The WMSServer address, without its query string.
    pub url: String,
    /// The ribbon menu the command sits in.
    #[serde(default)]
    pub menu: String,
    /// The layer a click shows when `warstwa` is not given.
    pub default: String,
    pub layers: Vec<Layer>,
}

/// The catalogue, parsed once per process.
pub fn all() -> &'static [Service] {
    static PARSED: std::sync::OnceLock<Vec<Service>> = std::sync::OnceLock::new();
    PARSED.get_or_init(|| serde_json::from_str(RAW).unwrap_or_default())
}

/// The service a request names: the `usluga` parameter, the command that carried it, or its label.
pub fn chosen(request: &Value) -> Option<&'static Service> {
    let named = request
        .get("params")
        .and_then(|params| params.get("usluga"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty());
    if let Some(wanted) = named {
        return find(wanted);
    }
    let command = request.get("id").and_then(Value::as_str)?;
    all().iter().find(|service| service.id == command)
}

pub fn find(wanted: &str) -> Option<&'static Service> {
    let key = fold(wanted);
    all().iter().find(|service| {
        service.id == wanted
            || fold(&service.name) == key
            || fold(&service.name).contains(&key)
    })
}

impl Service {
    /// The layers a request asks for: the ones it names, else the service's default.
    pub fn layers_for(&self, wanted: &str) -> Vec<&Layer> {
        if wanted.trim().is_empty() {
            return self
                .layers
                .iter()
                .filter(|layer| layer.id == self.default)
                .collect();
        }
        let mut chosen: Vec<&Layer> = Vec::new();
        for token in wanted.split(',') {
            let token = token.trim();
            if token.is_empty() {
                continue;
            }
            if let Some(layer) = self.match_layer(token) {
                if !chosen.iter().any(|seen| seen.id == layer.id) {
                    chosen.push(layer);
                }
            }
        }
        chosen
    }

    fn match_layer(&self, token: &str) -> Option<&Layer> {
        let key = fold(token);
        self.layers
            .iter()
            .find(|layer| layer.id == token)
            .or_else(|| self.layers.iter().find(|layer| fold(&layer.name) == key))
            .or_else(|| {
                self.layers
                    .iter()
                    .find(|layer| fold(&layer.name).contains(&key))
            })
    }

    /// The layer the default id points at, by name.
    pub fn default_name(&self) -> &str {
        self.layers
            .iter()
            .find(|layer| layer.id == self.default)
            .map(|layer| layer.name.as_str())
            .unwrap_or("")
    }
}

/// Adds a service's layers as one tile overlay, or takes it away when it is already on the map.
pub fn toggle(host: &mut Host, lang: Lang, request: &Value) -> Result<Value, String> {
    let service = chosen(request).ok_or_else(|| text::wody_unknown(lang, all().len()))?;
    let wanted = request
        .get("params")
        .and_then(|params| params.get("warstwa"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let layers = service.layers_for(wanted);
    if layers.is_empty() {
        return Err(text::wody_layer_unknown(
            lang,
            &service.name,
            wanted,
            &sample(service),
        ));
    }

    let name = layer_name(service, &layers);
    let tiles = host.call("map.tiles", json!({}))?;
    let present = tiles.as_array().and_then(|list| {
        list.iter()
            .filter_map(|tile| tile.get("name").and_then(Value::as_str))
            .find(|on_map| *on_map == name)
            .map(str::to_string)
    });
    if let Some(on_map) = present {
        host.call("map.removeTiles", json!({ "name": on_map }))?;
        host.notify(&text::removed(lang, &on_map));
        return Ok(json!({ "removed": on_map }));
    }

    let ids: Vec<&str> = layers.iter().map(|layer| layer.id.as_str()).collect();
    host.call(
        "map.addTiles",
        json!({
            "url": getmap_url(service, &ids),
            "kind": "raster",
            "name": name,
            "attribution": ATTRIBUTION,
            "overlay": true,
            "minZoom": 0,
            "maxZoom": 22,
            "tileSize": 256,
        }),
    )?;
    host.notify(&text::wody_added(lang, &name, layers.len()));
    Ok(json!({
        "added": name,
        "service": service.name,
        "layers": layers.iter().map(|layer| json!({"id": layer.id, "name": layer.name})).collect::<Vec<_>>(),
    }))
}

/// The layer's name: the service, then what it holds when that is not the default.
fn layer_name(service: &Service, layers: &[&Layer]) -> String {
    if layers.len() == 1 && layers[0].id == service.default {
        return service.name.to_string();
    }
    let names: Vec<&str> = layers.iter().map(|layer| layer.name.as_str()).collect();
    let mut joined = names.join(", ");
    if joined.chars().count() > 60 {
        joined = joined.chars().take(57).collect::<String>() + "…";
    }
    format!("{} · {}", service.name, joined)
}

/// The GetMap request: one 3857 tile, the layers asked for, transparent where nothing is drawn.
pub fn getmap_url(service: &Service, ids: &[&str]) -> String {
    format!(
        "{}?SERVICE=WMS&VERSION=1.3.0&REQUEST=GetMap&LAYERS={}&STYLES=&CRS=EPSG:3857&\
BBOX={{bbox-epsg-3857}}&WIDTH={{width}}&HEIGHT={{height}}&FORMAT=image/png&TRANSPARENT=TRUE",
        service.url,
        ids.join(",")
    )
}

/// Lists the services, or the layers of one, or the layers whose name carries a word.
pub fn list(params: &Value, lang: Lang) -> Result<Value, String> {
    let text_of = |key: &str| {
        params
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };

    if let Some(wanted) = text_of("usluga") {
        let service = find(&wanted).ok_or_else(|| text::wody_unknown(lang, all().len()))?;
        let search = text_of("szukaj").map(|value| fold(&value));
        let layers: Vec<Value> = service
            .layers
            .iter()
            .filter(|layer| {
                search
                    .as_deref()
                    .map(|key| fold(&layer.name).contains(key))
                    .unwrap_or(true)
            })
            .map(|layer| {
                json!({
                    "id": layer.id,
                    "name": layer.name,
                    "default": layer.id == service.default,
                })
            })
            .collect();
        return Ok(json!({
            "service": service.name,
            "id": service.id,
            "layers": layers,
            "count": layers.len(),
            "how": format!(
                "run_command  plugin:dane-pl/{}  {{\"warstwa\": \"{}\"}}",
                service.id, service.default_name()
            ),
        }));
    }

    let search = text_of("szukaj").map(|value| fold(&value));
    let services: Vec<Value> = all()
        .iter()
        .filter(|service| {
            search
                .as_deref()
                .map(|key| {
                    fold(&service.name).contains(key)
                        || service
                            .layers
                            .iter()
                            .any(|layer| fold(&layer.name).contains(key))
                })
                .unwrap_or(true)
        })
        .map(|service| {
            json!({
                "id": service.id,
                "name": service.name,
                "menu": service.menu,
                "layers": service.layers.len(),
                "default": service.default_name(),
            })
        })
        .collect();
    Ok(json!({
        "services": services,
        "count": services.len(),
        "layers": all().iter().map(|service| service.layers.len()).sum::<usize>(),
        "how": "warstwy {\"usluga\": \"MZP · głębokość, 1%\"} lists one service's layers",
    }))
}

/// A few layer names, for an error that has to tell the user what the service does offer.
fn sample(service: &Service) -> String {
    service
        .layers
        .iter()
        .take(3)
        .map(|layer| layer.name.as_str())
        .collect::<Vec<&str>>()
        .join(", ")
}

/// Folds a name to what a person types: lower case, Polish letters to ascii, no punctuation.
fn fold(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        let plain = match character {
            'ą' | 'Ą' => 'a',
            'ć' | 'Ć' => 'c',
            'ę' | 'Ę' => 'e',
            'ł' | 'Ł' => 'l',
            'ń' | 'Ń' => 'n',
            'ó' | 'Ó' => 'o',
            'ś' | 'Ś' => 's',
            'ź' | 'Ź' => 'z',
            'ż' | 'Ż' => 'z',
            other => other,
        };
        for lower in plain.to_lowercase() {
            if lower.is_alphanumeric() || lower == ' ' {
                out.push(lower);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalogue_parses_and_every_service_has_a_default_layer() {
        let services = all();
        assert!(services.len() >= 30, "only {} services", services.len());
        let mut layers = 0;
        for service in services {
            layers += service.layers.len();
            assert!(!service.name.is_empty(), "{} has no name", service.id);
            assert!(service.url.starts_with("https://"), "{}", service.id);
            assert!(!service.menu.is_empty(), "{} names no menu", service.id);
            assert!(
                !service.default_name().is_empty(),
                "{} default {} is not one of its layers",
                service.id,
                service.default
            );
            assert!(service.layers.len() >= 1, "{}", service.id);
        }
        assert!(layers > 800, "only {layers} layers");
    }

    #[test]
    fn finds_a_service_by_id_label_or_a_word_of_it() {
        let service = all().iter().find(|s| s.id == "wody-apgw").unwrap();
        assert_eq!(find("wody-apgw").map(|s| s.id.as_str()), Some("wody-apgw"));
        assert_eq!(find(&service.name).map(|s| s.id.as_str()), Some("wody-apgw"));
        assert_eq!(find("plan gospodarowania").map(|s| s.id.as_str()), Some("wody-apgw"));
        assert!(find("nie ma takiej uslugi").is_none());
    }

    #[test]
    fn picks_layers_by_id_name_or_a_word_of_the_name() {
        let service = all().iter().find(|s| s.id == "wody-apgw").unwrap();
        assert_eq!(service.layers_for("").len(), 1, "the default layer alone");
        assert_eq!(service.layers_for("").first().unwrap().id, service.default);
        assert_eq!(service.layers_for("8")[0].name, "Główne rzeki");
        assert_eq!(service.layers_for("Główne rzeki")[0].id, "8");
        // Case and Polish letters do not have to match.
        assert_eq!(service.layers_for("glowne rzeki")[0].id, "8");
        assert_eq!(service.layers_for("8, JCWP Rzecznych").len(), 2);
        // A token that matches nothing adds nothing, and the caller reports it.
        assert!(service.layers_for("rowy melioracyjne").is_empty());
    }

    #[test]
    fn builds_a_3857_getmap_with_numeric_layers() {
        let service = all().iter().find(|s| s.id == "wody-apgw").unwrap();
        let url = getmap_url(service, &["8", "10"]);
        assert!(url.starts_with("https://wody.isok.gov.pl/gpservices/KZGW/ISOK_PGW/MapServer/WMSServer?"));
        assert!(url.contains("LAYERS=8,10"));
        assert!(url.contains("CRS=EPSG:3857"));
        assert!(url.contains("BBOX={bbox-epsg-3857}"));
        assert!(url.contains("WIDTH={width}"));
        assert!(url.contains("VERSION=1.3.0"));
    }

    #[test]
    fn names_the_layer_after_the_service_and_what_it_holds() {
        let service = all().iter().find(|s| s.id == "wody-apgw").unwrap();
        // The default layer keeps the plain service name, so a second click finds it again.
        assert_eq!(layer_name(service, &service.layers_for("")), service.name);
        assert_eq!(layer_name(service, &service.layers_for("8")), service.name);
        let named = layer_name(service, &service.layers_for("10"));
        assert!(named.starts_with(&service.name), "{named}");
        assert!(named.contains("JCWP Rzecznych"), "{named}");
    }

    #[test]
    fn lists_services_and_their_layers() {
        let services = list(&json!({}), Lang::En).unwrap();
        assert!(services["count"].as_u64().unwrap() >= 30);
        let one = list(&json!({"usluga": "wody-apgw"}), Lang::En).unwrap();
        assert!(one["layers"].as_array().unwrap().len() > 5);
        let searched = list(&json!({"usluga": "wody-apgw", "szukaj": "rzek"}), Lang::En).unwrap();
        assert!(searched["count"].as_u64().unwrap() < 5);
    }
}
