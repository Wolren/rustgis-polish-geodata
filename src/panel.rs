//! The panel's state: what the pane shows when it opens, and which of its buttons apply.

use crate::cache;
use crate::catalog;
use crate::clip;
use crate::host::Host;
use crate::services;
use crate::text::{self, Lang};
use serde_json::{json, Map, Value};

/// The curated overlays the map list offers, in the order the ribbon shows them.
const CURATED: &[&str] = &["orto", "bdot", "nmt", "dzialki-wms", "adresy", "prg-wms", "prng"];

/// The WFS download tools, in the order the map list shows them: id, English label, Polish label.
const DOWNLOAD_TOOLS: &[(&str, &str, &str)] = &[
    ("pobierz-adresy", "Addresses and buildings", "Adresy i budynki"),
    ("pobierz-granice", "Boundaries and units", "Granice i jednostki"),
    ("pobierz-osnowy", "Control points", "Osnowy i pomiary"),
    ("pobierz-urzedy", "Offices and courts", "Urzędy i sądy"),
    ("pobierz-pozostale", "Other data", "Pozostałe dane"),
    ("pobierz-gdos", "Protected areas (GDOŚ)", "Ochrona przyrody (GDOŚ)"),
];

/// The manifest's id: the host prefixes a tool id with it.

/// Copies the panel's state into a command's answer, so the pane shows what the command changed.
pub fn attach(answer: &mut Value, state: Value) {
    let (Some(fields), Some(panel)) = (answer.as_object_mut(), state.get("panel")) else {
        return;
    };
    fields.insert("panel".to_string(), panel.clone());
}

/// The panel's two download settings, as the widgets name them.
const CONFIRM_ID: &str = "potwierdz-duze";
const LIMIT_ID: &str = "limit-mb";
/// The checkbox that cuts a typed download to the unit boundary.
const CLIP_ID: &str = "przytnij-jednostke";
/// A slider's ends, and the limit a stored value must fall between.
const LIMIT_MIN: i64 = 5;
const LIMIT_MAX: i64 = 500;

/// Stores what the panel's checkbox and slider say, then lets the panel answer with them.
pub fn preferences(host: &mut Host, request: &Value, lang: Lang) -> Result<Value, String> {
    let values = request
        .get("panel")
        .and_then(|panel| panel.get("values"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let mut settings = cache::settings();
    if let Some(confirm) = values.get(CONFIRM_ID).and_then(Value::as_bool) {
        settings.confirm_large = confirm;
    }
    if let Some(limit) = values.get(LIMIT_ID).and_then(Value::as_i64) {
        settings.limit_mb = limit.clamp(LIMIT_MIN, LIMIT_MAX);
    }
    if let Some(clip) = values.get(CLIP_ID).and_then(Value::as_bool) {
        settings.clip_unit = clip;
    }
    cache::remember_settings(&settings);
    let note = text::settings_saved(lang, settings.limit_mb, settings.confirm_large);
    host.notify(&note);
    Ok(json!({ "note": note }))
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
        .filter(|layer| !text::DRAW_LAYERS.contains(&named(layer).as_str()))
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
    items.insert("mapa".into(), json!({ "nodes": map_nodes(lang) }));
    items.insert("unit".into(), json!({ "nodes": crate::jednostki::nodes(lang) }));
    let chosen_unit = request
        .get("panel")
        .and_then(|panel| panel.get("values"))
        .and_then(|values| values.get("unit"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|code| crate::jednostki::resolve(code).map(|(name, level)| (name, level)));
    let boundaries = crate::granice::nodes(lang);
    let mut granice_item = json!({ "nodes": boundaries });
    if let Some((name, level)) = chosen_unit {
        granice_item["label"] = json!(text::boundaries_of(lang, &name, level));
    }
    items.insert("granice".into(), granice_item);
    items.insert(
        "elevacja-5m".into(),
        json!({ "value": request.get("panel").and_then(|panel| panel.get("values"))
            .and_then(|values| values.get("elevacja-5m")).and_then(Value::as_bool).unwrap_or(false) }),
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

    // The stored settings go back with the state, so the widgets show what a run will use.
    let settings = cache::settings();
    Ok(json!({
        "panel": {
            "items": items,
            "values": { CONFIRM_ID: settings.confirm_large, LIMIT_ID: settings.limit_mb, CLIP_ID: settings.clip_unit },
        },
        "area": area.is_some(),
        "layers": others,
        "clip": { "layer": target, "boundary": boundary },
    }))
}

/// Adds the map or the dataset the panel's list holds, as `v:` and `d:` rows name them.
pub fn mapa(host: &mut Host, request: &Value, lang: Lang) -> Result<Value, String> {
    let row = picked(request).ok_or_else(|| text::map_missing(lang))?;
    // A folder is a row too, and choosing one adds nothing: say what to pick instead of failing.
    if let Some(label) = folder_of(&row, lang) {
        let note = text::map_folder(lang, &label);
        host.notify(&note);
        return Ok(json!({ "folder": row, "note": note }));
    }
    if let Some(id) = row.strip_prefix("v:") {
        if !is_map(id) {
            return Err(text::map_unknown(lang, &row));
        }
        // The host refuses a plugin-prefixed id, so the plugin runs its own command here.
        let sub = json!({ "id": id, "params": {} });
        return crate::run(&sub, id, &json!({}), host, lang);
    }
    if let Some(id) = row.strip_prefix("e:") {
        let entry = crate::zrodla::find(id).ok_or_else(|| text::register_unknown(lang, id))?;
        return crate::zrodla::add_service(host, entry, lang);
    }
    let Some(rest) = row.strip_prefix("d:") else {
        return Err(text::map_unknown(lang, &row));
    };
    let (tool, dataset) = rest.split_once(':').ok_or_else(|| text::map_unknown(lang, &row))?;
    download(host, tool, dataset, lang)
}

/// The label of the folder a row names, when it names one of the list's folders.
fn folder_of(row: &str, lang: Lang) -> Option<String> {
    fn find(nodes: &[Value], row: &str) -> Option<String> {
        for node in nodes {
            let id = node.get("id").and_then(Value::as_str)?;
            let label = node.get("label").and_then(Value::as_str).unwrap_or_default();
            if id == row && node.get("children").is_some() {
                return Some(label.to_string());
            }
            let children = node.get("children").and_then(Value::as_array)?;
            if let Some(found) = find(children, row) {
                return Some(found);
            }
        }
        None
    }
    let nodes = map_nodes(lang);
    find(nodes.as_array()?, row)
}

/// The row chosen in the panel's map list, which is the command's only input.
fn picked(request: &Value) -> Option<String> {
    request
        .get("panel")
        .and_then(|panel| panel.get("values"))
        .and_then(|values| values.get("mapa"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|row| !row.is_empty())
        .map(str::to_string)
}

/// Whether the list offers this map command: a curated overlay, a service, a Wody service, a Cartomatic map.
fn is_map(id: &str) -> bool {
    CURATED.contains(&id)
        || services::all().iter().any(|service| service.cmd == id)
        || catalog::wody().iter().any(|service| service.id == id)
        || catalog::cartomatic().iter().any(|map| map.id == id)
}

/// Names the tool a `d:` row belongs to: a command cannot start a background run, so it points.
fn download(host: &mut Host, tool: &str, dataset: &str, lang: Lang) -> Result<Value, String> {
    let layer = catalog::wfs_layers()
        .iter()
        .find(|layer| !layer.index && layer.id == dataset && layer.tool == tool)
        .ok_or_else(|| text::map_unknown(lang, &format!("d:{tool}:{dataset}")))?;
    let note = text::map_download(lang, &layer.name);
    host.notify(&note);
    Ok(json!({
        "download": { "tool": tool, "dataset": layer.id, "title": layer.title },
        "note": note,
    }))
}


/// The panel's map list: every map under Podkłady, every downloadable dataset under Pobieranie.
fn map_nodes(lang: Lang) -> Value {
    let [maps, downloads, register] = text::map_roots(lang);
    json!([
        { "id": "podklady", "label": maps, "children": map_leaves(lang) },
        { "id": "pobieranie", "label": downloads, "children": download_leaves(lang) },
        { "id": "eziudp", "label": register, "children": crate::zrodla::branches(lang, 3000) },
    ])
}

/// Every map the plugin can show, as `v:<command id>` leaves in catalogue order.
fn map_leaves(lang: Lang) -> Vec<Value> {
    let mut leaves: Vec<Value> = CURATED
        .iter()
        .filter_map(|id| catalog::overlay(id))
        .map(|overlay| leaf(&format!("v:{}", overlay.id), overlay.name(lang)))
        .collect();
    leaves.extend(
        services::all()
            .iter()
            .map(|service| leaf(&format!("v:{}", service.cmd), service.display())),
    );
    leaves.extend(
        catalog::wody()
            .iter()
            .map(|service| leaf(&format!("v:{}", service.id), service.title)),
    );
    leaves.extend(
        catalog::cartomatic()
            .iter()
            .map(|map| leaf(&format!("v:{}", map.id), map.title)),
    );
    leaves
}

/// One folder per download tool, each leaf `d:<tool>:<dataset>` labelled as the tool's choice names it.
fn download_leaves(lang: Lang) -> Vec<Value> {
    DOWNLOAD_TOOLS
        .iter()
        .filter_map(|(tool, english, polish)| {
            let leaves: Vec<Value> = catalog::wfs_layers()
                .iter()
                .filter(|layer| !layer.index && layer.tool == *tool)
                .map(|layer| leaf(&format!("d:{tool}:{}", layer.id), layer.name))
                .collect();
            let label = match lang {
                Lang::En => english,
                Lang::Pl => polish,
            };
            (!leaves.is_empty()).then(|| {
                json!({ "id": format!("folder:{tool}"), "label": label, "children": leaves })
            })
        })
        .collect()
}

/// A tree leaf: its id and its label, with no count.
fn leaf(id: &str, label: &str) -> Value {
    json!({ "id": id, "label": label })
}

/// The map's scale, or 0 when the app does not report it.
fn scale(host: &mut Host) -> f64 {
    host.call("host.view", json!({}))
        .ok()
        .and_then(|view| view.get("scale").and_then(Value::as_f64))
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The nodes of a subtree, the node itself included.
    fn count_nodes(node: &Value) -> usize {
        1 + node["children"]
            .as_array()
            .map(|children| children.iter().map(count_nodes).sum())
            .unwrap_or(0)
    }

    /// Every row names something this build still has, and no id repeats.
    #[test]
    fn every_row_names_a_map_or_a_dataset() {
        let nodes = map_nodes(Lang::En);
        let maps = nodes[0]["children"].as_array().expect("map leaves");
        let folders = nodes[1]["children"].as_array().expect("download folders");
        assert_eq!(
            maps.len(),
            CURATED.len() + services::all().len() + catalog::wody().len() + catalog::cartomatic().len()
        );
        assert_eq!(folders.len(), DOWNLOAD_TOOLS.len());
        let total: usize = nodes.as_array().expect("nodes").iter().map(count_nodes).sum();
        // The host takes 5 000 nodes; the register brings the tree close to it, so keep the check.
        assert!(total <= 4800, "{total} nodes");

        let mut ids: Vec<String> = Vec::new();
        for leaf in maps {
            let id = leaf["id"].as_str().expect("an id");
            assert!(is_map(id.trim_start_matches("v:")), "{id}");
            assert!(!leaf["label"].as_str().unwrap_or_default().is_empty(), "{id}");
            ids.push(id.to_string());
        }
        for folder in folders {
            assert!(!folder["label"].as_str().unwrap_or_default().is_empty());
            for leaf in folder["children"].as_array().expect("dataset leaves") {
                let id = leaf["id"].as_str().expect("an id");
                let rest = id.strip_prefix("d:").unwrap_or_default();
                let found = rest.split_once(':').and_then(|(tool, dataset)| {
                    catalog::wfs_layers()
                        .iter()
                        .find(|layer| !layer.index && layer.tool == tool && layer.id == dataset)
                });
                assert!(found.is_some(), "{id}");
                ids.push(id.to_string());
            }
        }
        let before = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(before, ids.len(), "two rows share an id");
    }

    /// Every offered dataset sits in a folder of its own tool, so the list can start each download.
    #[test]
    fn every_offered_dataset_is_in_a_folder() {
        let nodes = map_nodes(Lang::Pl);
        let folders = nodes[1]["children"].as_array().expect("download folders");
        let mut listed: Vec<&str> = Vec::new();
        for folder in folders {
            for leaf in folder["children"].as_array().expect("dataset leaves") {
                listed.extend(leaf["id"].as_str());
            }
        }
        for layer in catalog::offered() {
            let wanted = format!("d:{}:{}", layer.tool, layer.id);
            assert!(listed.contains(&wanted.as_str()), "{} is not listed", layer.id);
        }
        assert_eq!(listed.len(), catalog::offered().count());
    }

    /// A `d:` row passes the dataset's title, so every offered title must resolve to its own entry.
    #[test]
    fn every_offered_title_resolves_to_its_dataset() {
        for layer in catalog::offered() {
            let found = catalog::wfs_by_label(layer.title);
            assert_eq!(found.map(|entry| entry.id), Some(layer.id), "{}", layer.id);
        }
    }

    /// The folders carry the tool names the manifest prints, so the panel and the ribbon agree.
    #[test]
    fn the_folders_carry_the_manifest_tool_names() {
        let manifest: Value =
            serde_json::from_str(include_str!("../plugin.json")).expect("plugin.json is not JSON");
        for (tool, english, _) in DOWNLOAD_TOOLS {
            let found = manifest["tools"]
                .as_array()
                .expect("tools")
                .iter()
                .find(|decl| decl["id"] == *tool)
                .and_then(|decl| decl["name"].as_str());
            assert_eq!(found, Some(*english), "{tool}");
        }
    }

    /// A press without a row, or with a row no map defines, fails before the map is touched.
    #[test]
    fn a_press_without_a_row_says_so() {
        let mut host = Host::new(Lang::En);
        let empty = json!({ "params": {} });
        assert!(mapa(&mut host, &empty, Lang::En).unwrap_err().contains("panel list"));
        let unknown = json!({ "panel": { "values": { "mapa": "v:nothing" } } });
        assert!(mapa(&mut host, &unknown, Lang::Pl).unwrap_err().contains("nie ma mapy"));
    }
}
