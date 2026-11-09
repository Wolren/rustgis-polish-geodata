//! Administrative boundaries at any level, downloaded for the country, a unit, or a name filter.
//!
//! This is the guided path the old desktop downloader offered: pick the level, pick the scope, done.

use crate::catalog;
use crate::host::{self, Host};
use crate::text::{self, Lang};
use crate::wfs;
use serde_json::{json, Value};

/// The layers of the national register of boundaries, by level.
const LEVELS: (&str, &str, &str) = ("wojewodztwa", "powiaty", "gminy");
/// The box that holds the whole country, used when no unit narrows it.
const POLAND: [f64; 4] = [14.0, 49.0, 24.2, 54.9];

/// Downloads one level of boundaries for a scope, then keeps only the names the filter matches.
pub fn levels(host: &mut Host, request: &Value, lang: Lang) -> Result<Value, String> {
    let params = request.get("params").cloned().unwrap_or(Value::Null);
    // A ribbon click carries no parameters, so the panel's tree and a default answer for it.
    let picked = params
        .get("poziom")
        .or_else(|| params.get("level"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| panel_pick(request));
    let picked = match picked {
        Some(spec) => spec,
        None => {
            host.notify(&text::levels_clicked(lang));
            "wojewodztwa|kraj".to_string()
        }
    };
    let (wanted_level, picked_scope) = parse_pick(&picked);
    let (label, _dataset) = match wanted_level.to_lowercase().as_str() {
        "wojewodztwa" | "voivodeships" => ("Granice województw", LEVELS.0),
        "powiaty" | "counties" => ("Granice powiatów", LEVELS.1),
        "gminy" | "municipalities" => ("Granice gmin", LEVELS.2),
        _ => return Err(text::levels_unknown_level(lang, &wanted_level)),
    };
    let layer = catalog::wfs_by_label(label).ok_or_else(|| text::unknown_dataset(lang, label))?;

    // The scope is either the whole country or a TERYT code, whose boundary becomes the area.
    let scope = params
        .get("zakres")
        .or_else(|| params.get("scope"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            if picked_scope.is_empty() {
                "kraj".to_string()
            } else {
                picked_scope.clone()
            }
        });
    // A tree node may point at the unit chosen in the panel, not at a code of its own.
    let scope = if scope == "unit" {
        request
            .get("panel")
            .and_then(|panel| panel.get("values"))
            .and_then(|values| values.get("unit"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .ok_or_else(|| text::levels_need_unit(lang))?
    } else {
        scope
    };
    let whole_country = matches!(scope.to_lowercase().as_str(), "kraj" | "country" | "pl");
    let mut area_name = String::new();
    let bbox = if whole_country {
        POLAND
    } else {
        let (unit, level) = crate::jednostki::resolve(&scope)
            .ok_or_else(|| text::unit_unknown(lang, &scope))?;
        let area = crate::prg::area(
            host,
            &json!({ "nazwa": unit, "poziom": level, "kod": scope }),
            lang,
        )?;
        area_name = area
            .get("layer")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let box_lon = area
            .get("box")
            .and_then(Value::as_array)
            .map(|values| values.iter().filter_map(Value::as_f64).collect::<Vec<f64>>())
            .filter(|values| values.len() == 4)
            .ok_or_else(|| text::area_no_geometry(lang, &unit))?;
        [box_lon[0], box_lon[1], box_lon[2], box_lon[3]]
    };

    let settings = crate::cache::settings();
    let confirm = settings.confirm_large;
    let cap = if confirm { wfs::HARD_TOTAL } else { wfs::MAX_TOTAL };
    // The request's own number wins, then the saved limit; zero means no byte limit.
    let asked_mb = params
        .get("limit_mb")
        .and_then(Value::as_i64)
        .unwrap_or(settings.limit_mb);
    let limit = if asked_mb <= 0 {
        u64::MAX
    } else {
        (asked_mb * 1_000_000) as u64
    };
    // The box goes as a polygon: a plain BBOX is read in the service's own axis order and misses.
    let [west, south, east, north] = bbox;
    let area_box = json!({
        "type": "FeatureCollection",
        "features": [{
            "type": "Feature",
            "properties": {},
            "geometry": {
                "type": "Polygon",
                "coordinates": [[[west, south], [east, south], [east, north], [west, north], [west, south]]]
            }
        }]
    });
    host::progress_line(&text::levels_reading(lang, label));
    let mut report = |line: &str| host::progress_line(line);
    let download = wfs::fetch(layer, bbox, Some(&area_box), &mut report, lang, limit, cap)?;
    if download.features.is_empty() {
        return Err(text::typed_empty(lang, label, &scope));
    }

    // A TERYT code names the parent, so a scope keeps the rows whose code nests under it.
    let scope_code = (!whole_country).then(|| scope.clone());
    let kept: Vec<Value> = match &scope_code {
        Some(code) => download
            .features
            .into_iter()
            .filter(|feature| {
                code_of(feature)
                    .map(|row| row.starts_with(code.as_str()) || code.starts_with(row.as_str()))
                    .unwrap_or(false)
            })
            .collect(),
        None => download.features,
    };
    if kept.is_empty() {
        return Err(text::typed_empty(lang, label, &scope));
    }

    // The name filter keeps the rows a person would look for, accents aside.
    let filter = params
        .get("filtr")
        .or_else(|| params.get("filter"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(fold);
    let kept: Vec<Value> = match &filter {
        Some(pattern) => kept
            .into_iter()
            .filter(|feature| name_of(feature).map(|name| fold(&name).contains(pattern.as_str())).unwrap_or(false))
            .collect(),
        None => kept,
    };
    if kept.is_empty() {
        return Err(text::levels_no_match(lang, &scope));
    }

    let name = match &filter {
        Some(pattern) => text::typed_layer(lang, label, pattern, "poziom"),
        None => text::typed_layer(lang, label, &scope, "poziom"),
    };
    host.add_layer(&name, json!({ "type": "FeatureCollection", "features": kept }))?;

    // A unit scope cuts the box result to the unit, as every guided download does.
    let mut total = kept_count(&name, host);
    if !whole_country && !area_name.is_empty() {
        if let Ok(answer) = crate::clip::to_area(host, &name, &area_name, lang) {
            total = answer
                .get("features")
                .and_then(Value::as_u64)
                .unwrap_or(total as u64) as usize;
        }
    }
    host.notify(&text::levels_added(lang, &name, total, label));
    Ok(json!({
        "layer": name,
        "level": wanted_level,
        "scope": scope,
        "features": total,
        "clipped": !whole_country,
    }))
}

/// The panel's tree: every option the guided downloader offers, the unit ones last.
pub fn nodes(lang: Lang) -> Value {
    let mut out = Vec::new();
    for (level, label) in [
        ("wojewodztwa", text::levels_node_country(lang, "voivodeships")),
        ("powiaty", text::levels_node_country(lang, "powiaty")),
        ("gminy", text::levels_node_country(lang, "gminy")),
    ] {
        out.push(json!({ "id": format!("{level}|kraj"), "label": label }));
    }
    for (level, label) in [
        ("wojewodztwa", text::levels_node_unit(lang, "voivodeships")),
        ("powiaty", text::levels_node_unit(lang, "powiaty")),
        ("gminy", text::levels_node_unit(lang, "gminy")),
    ] {
        out.push(json!({ "id": format!("{level}|unit"), "label": label }));
    }
    Value::Array(out)
}

/// "level|scope" from a tree node or a parameter, with a bare level asking for the country.
pub fn parse_pick(spec: &str) -> (String, String) {
    match spec.split_once('|') {
        Some((level, scope)) if !scope.trim().is_empty() => {
            (level.trim().to_string(), scope.trim().to_string())
        }
        Some((level, _)) => (level.trim().to_string(), "kraj".to_string()),
        None => (spec.trim().to_string(), "kraj".to_string()),
    }
}

/// The node chosen in the panel's boundaries tree, as "level|scope".
fn panel_pick(request: &Value) -> Option<String> {
    request
        .get("panel")
        .and_then(|panel| panel.get("values"))
        .and_then(|values| values.get("granice"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// The rows the layer holds now, for the message.
fn kept_count(name: &str, host: &mut Host) -> usize {
    host.call("host.layers", json!({}))
        .ok()
        .and_then(|answer| answer.as_array().cloned())
        .and_then(|layers| {
            layers
                .iter()
                .find(|layer| layer.get("name").and_then(Value::as_str) == Some(name))
                .and_then(|layer| layer.get("features").and_then(Value::as_u64))
        })
        .unwrap_or(0) as usize
}

/// The unit code a boundary row carries, which nests children under a parent.
fn code_of(feature: &Value) -> Option<String> {
    let properties = feature.get("properties")?.as_object()?;
    for key in ["JPT_KOD_JE", "jpt_kod_je", "KOD", "kod"] {
        if let Some(value) = properties.get(key).and_then(Value::as_str) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

/// A feature's name from the register's property, whichever spelling the service uses.
fn name_of(feature: &Value) -> Option<String> {
    let properties = feature.get("properties")?.as_object()?;
    for key in ["JPT_NAZWA_", "jpt_nazwa_", "NAZWA", "nazwa", "name", "NAME"] {
        if let Some(value) = properties.get(key).and_then(Value::as_str) {
            if !value.trim().is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Lower case with the Polish letters folded, so "zoliborz" finds "Żoliborz".
pub fn fold(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|letter| match letter {
            'ą' => 'a',
            'ć' => 'c',
            'ę' => 'e',
            'ł' => 'l',
            'ń' => 'n',
            'ó' => 'o',
            'ś' => 's',
            'ź' => 'z',
            'ż' => 'z',
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{fold, name_of};
    use serde_json::json;

    #[test]
    fn the_filter_ignores_case_and_diacritics() {
        assert_eq!(fold("Żoliborz"), "zoliborz");
        assert!(fold("Ostrów Wielkopolski").contains(&fold("ostrow")));
        assert!(fold("Łódź").contains(&fold("lodz")));
    }

    #[test]
    fn the_name_comes_from_the_register_property() {
        let feature = json!({"type": "Feature", "properties": {"JPT_NAZWA_": "powiat ostrowski"}});
        assert_eq!(name_of(&feature).as_deref(), Some("powiat ostrowski"));
        let empty = json!({"type": "Feature", "properties": {}});
        assert!(name_of(&empty).is_none());
    }

    #[test]
    fn the_level_names_the_register_layer() {
        assert_eq!(super::LEVELS, ("wojewodztwa", "powiaty", "gminy"));
    }

    #[test]
    fn a_tree_node_splits_into_a_level_and_a_scope() {
        assert_eq!(super::parse_pick("gminy|1416"), ("gminy".to_string(), "1416".to_string()));
        assert_eq!(super::parse_pick("gminy|unit"), ("gminy".to_string(), "unit".to_string()));
        assert_eq!(super::parse_pick("gminy"), ("gminy".to_string(), "kraj".to_string()));
        assert_eq!(super::parse_pick("gminy|"), ("gminy".to_string(), "kraj".to_string()));
    }

    #[test]
    fn the_tree_offers_every_option_of_the_guide() {
        let nodes = super::nodes(crate::text::Lang::En);
        let list = nodes.as_array().expect("a list");
        assert_eq!(list.len(), 6);
        let ids: Vec<&str> = list
            .iter()
            .filter_map(|node| node.get("id").and_then(serde_json::Value::as_str))
            .collect();
        assert!(ids.contains(&"wojewodztwa|kraj"));
        assert!(ids.contains(&"gminy|unit"));
    }
}
