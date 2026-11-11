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
    let confirm = params.get("confirm").and_then(Value::as_bool).unwrap_or(false) || settings.confirm_large;
    let no_limit = params.get("nolimit").and_then(Value::as_bool).unwrap_or(false);
    let cap = if no_limit {
        usize::MAX
    } else if confirm {
        wfs::HARD_TOTAL
    } else {
        wfs::MAX_TOTAL
    };
    // The request's own number wins, then the saved limit; zero means no byte limit.
    let asked_mb = crate::number::of(&params, "limit_mb").unwrap_or(settings.limit_mb);
    let limit = if asked_mb <= 0 {
        u64::MAX
    } else {
        (asked_mb * 1_000_000) as u64
    };
    // The scope and the name go to the service as one filter, so no page window cuts the answer.
    let wanted_name = params
        .get("filtr")
        .or_else(|| params.get("filter"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let filter = register_filter(wanted_level.as_str(), (!whole_country).then_some(scope.as_str()), wanted_name);
    // Warn before the first page when the scope holds more than one run may take.
    if !no_limit && !layer.unfiltered {
        if let Some(wanted) = crate::tool::counted(layer, bbox, filter.as_deref(), lang) {
            if wanted > cap {
                host::progress_line(&text::over_budget(
                    lang, &label, wanted, cap, confirm, &layer.name, true, true,
                ));
            }
        }
    }
    host::progress_line(&text::levels_reading(lang, label));
    let mut report = |line: &str| host::progress_line(line);
    let download = match filter {
        Some(filter) => wfs::fetch(layer, bbox, None, Some(&filter), &mut report, lang, limit, cap)?,
        None => {
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
            wfs::fetch(layer, bbox, Some(&area_box), None, &mut report, lang, limit, cap)?
        }
    };
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

/// The tool behind the form: the same options, but a Tools pane dialog instead of a script.
///
/// A tool gets no host, so the unit's boundary comes from the register here and the layer returns as
/// GeoJSON. The scope filter is the TERYT code, so no clip is needed for exactness.
pub fn tool(request: &Value, lang: Lang) -> Result<Value, String> {
    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
    let level = params
        .get("poziom")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_lowercase();
    let (label, _) = match level.as_str() {
        "wojewodztwa" | "voivodeships" => ("Granice województw", LEVELS.0),
        "powiaty" | "counties" => ("Granice powiatów", LEVELS.1),
        "gminy" | "municipalities" => ("Granice gmin", LEVELS.2),
        other => return Err(text::levels_unknown_level(lang, other)),
    };
    let layer = catalog::wfs_by_label(label).ok_or_else(|| text::unknown_dataset(lang, label))?;

    // The dropdown picks a voivodeship; the code field narrows it to a powiat or a gmina.
    let picked_voivodeship = params.get("wojewodztwo").and_then(Value::as_str).map(str::trim);
    // "cały kraj" is a choice, so it means the country: only a missing field repeats the last run.
    let country_chosen = picked_voivodeship
        .map(|value| value.starts_with("cały") || value.starts_with("whole"))
        .unwrap_or(false);
    let voivodeship = picked_voivodeship
        .filter(|value| !value.is_empty() && !value.starts_with("cały") && !value.starts_with("whole"))
        .map(str::to_string);
    // The field wins; empty falls back to the unit chosen in the panel's tree.
    let narrower = params
        .get("zakres")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            request
                .get("panel")
                .and_then(|panel| panel.get("values"))
                .and_then(|values| values.get("unit"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        });
    let remembered = crate::cache::last_scope();
    let scope = narrower
        .clone()
        .or_else(|| {
            voivodeship
                .as_deref()
                .and_then(crate::jednostki::code_of_name)
        })
        .or_else(|| {
            // An untouched form repeats the last run; a chosen voivodeship or the country overrides it.
            if country_chosen || picked_voivodeship.is_some() {
                None
            } else {
                remembered.as_ref().map(|(_, scope, _)| scope.clone())
            }
        })
        .unwrap_or_else(|| "kraj".to_string());
    let whole_country = matches!(scope.to_lowercase().as_str(), "kraj" | "country" | "pl" | "");
    let bbox = POLAND;
    let scope_code = if whole_country {
        None
    } else {
        // A name or a code both work, and a name resolves through the shipped table.
        let code = match crate::jednostki::resolve(&scope) {
            Some(_) => scope.clone(),
            None if narrower.is_none() => {
                // A dropdown answer is an exact name, so it never reaches the fragment path.
                return Err(text::unit_unknown(lang, &scope));
            }
            None => {
                // A fragment may name several units, so the form answers with the matches.
                let matches = crate::jednostki::matching(&scope, 12);
                match matches.len() {
                    0 => return Err(text::unit_unknown(lang, &scope)),
                    1 => matches[0].0.clone(),
                    _ => return Err(text::levels_choose(lang, &match_list(&matches))),
                }
            }
        };
        Some(code)
    };

    let settings = crate::cache::settings();
    let cap = if settings.confirm_large { wfs::HARD_TOTAL } else { wfs::MAX_TOTAL };
    let asked_mb = crate::number::of(&params, "limit_mb").unwrap_or(settings.limit_mb);
    let limit = if asked_mb <= 0 {
        u64::MAX
    } else {
        (asked_mb * 1_000_000) as u64
    };
    // The scope code and the typed name go to the service as one filter, so the answer is complete.
    let wanted_name = params
        .get("filtr")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let filter = register_filter(&level, scope_code.as_deref(), wanted_name);
    let country = json!({
        "type": "FeatureCollection",
        "features": [{
            "type": "Feature",
            "properties": {},
            "geometry": {
                "type": "Polygon",
                "coordinates": [[[14.0, 49.0], [24.2, 49.0], [24.2, 54.9], [14.0, 54.9], [14.0, 49.0]]]
            }
        }]
    });
    let area = filter.is_none().then_some(&country);
    let mut report = |line: &str| host::progress_line(line);
    let download = wfs::fetch(layer, bbox, area, filter.as_deref(), &mut report, lang, limit, cap)?;

    let filter = params
        .get("filtr")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            // The filter repeats too, unless this form carried its own (even an empty one).
            if params.get("filtr").is_some() {
                None
            } else {
                remembered
                    .as_ref()
                    .map(|(_, _, filter)| filter.clone())
                    .filter(|filter| !filter.trim().is_empty())
            }
        })
        .map(|filter| fold(&filter));
    let arrived = download.features.len();
    let kept: Vec<Value> = download
        .features
        .into_iter()
        .filter(|feature| match &scope_code {
            Some(code) => code_of(feature)
                .map(|row| row.starts_with(code.as_str()) || code.starts_with(row.as_str()))
                .unwrap_or(false),
            None => true,
        })
        .filter(|feature| match &filter {
            Some(pattern) => name_of(feature)
                .map(|name| fold(&name).contains(pattern.as_str()))
                .unwrap_or(false),
            None => true,
        })
        .collect();
    if kept.is_empty() {
        return Err(text::levels_no_match(lang, &scope));
    }
    let (unit_name, _) = match crate::jednostki::resolve(&scope) {
        Some((unit, level)) => (unit, level),
        None => (text::levels_country(lang).to_string(), "kraj"),
    };
    let name = match &filter {
        Some(pattern) => text::typed_layer(lang, label, pattern, "poziom"),
        None => text::typed_layer(lang, label, &unit_name, "poziom"),
    };
    crate::cache::remember_scope(&level, &scope, &filter.clone().unwrap_or_default());
    let mut messages = vec![text::count_size(lang, &name, kept.len(), download.bytes)];
    // The cap message only when the cap cut the run; a slow or failed cell is not a cap stop.
    if download.truncated && arrived >= cap {
        messages.push(text::truncated_at(lang, &name, cap, settings.confirm_large));
    }
    Ok(json!({
        "layers": [{ "name": name, "geojson": json!({ "type": "FeatureCollection", "features": kept }) }],
        "messages": messages
    }))
}

/// A register filter for the level, the scope code and the name, so the service sends only matches.
///
/// The register compares names as written, so the typed word goes in three spellings; a code keeps its
/// prefix, since a powiat's gminy all start with it.
pub fn register_filter(level: &str, scope: Option<&str>, name: Option<&str>) -> Option<String> {
    let length = match level {
        "wojewodztwa" => 2,
        "powiaty" => 4,
        _ => 7,
    };
    let mut parts: Vec<String> = Vec::new();
    if let Some(code) = scope {
        let code = code.trim();
        let literal = if code.chars().count() >= length {
            code.chars().take(length).collect::<String>()
        } else {
            format!("{code}")
        };
        let literal = if code.chars().count() == length {
            literal
        } else {
            format!("{literal}*")
        };
        parts.push(like("JPT_KOD_JE", &literal));
    }
    if let Some(word) = name {
        let word = word.trim();
        if !word.is_empty() {
            let probes = name_probes(word);
            let mut alternatives: Vec<String> = Vec::new();
            for probe in probes {
                alternatives.push(like("JPT_NAZWA_", &format!("*{}*", escape_xml(&probe))));
            }
            parts.push(if alternatives.len() == 1 {
                alternatives.remove(0)
            } else {
                format!("<Or>{}</Or>", alternatives.join(""))
            });
        }
    }
    if parts.is_empty() {
        return None;
    }
    let body = if parts.len() == 1 {
        parts.remove(0)
    } else {
        format!("<And>{}</And>", parts.join(""))
    };
    let xml = format!("<Filter xmlns:ogc=\"http://www.opengis.net/ogc\">{body}</Filter>");
    Some(percent_encode(&xml))
}

/// The spellings worth asking the register for: four cases, and the accented forms of plain letters.
///
/// The service compares names as written, so "miedzy" needs the ę and "MIĘDZY" needs Między.
pub fn name_probes(word: &str) -> Vec<String> {
    let cases = [
        word.to_string(),
        title_case(word),
        word.to_lowercase(),
        word.to_uppercase(),
    ];
    let mut out: Vec<String> = Vec::new();
    for probe in cases {
        if !probe.is_empty() && !out.contains(&probe) {
            out.push(probe);
        }
    }
    // Plain letters expand to the accented ones a store may carry; the cap keeps the address short.
    for expanded in accents(word) {
        for probe in [expanded.clone(), title_case(&expanded)] {
            if !out.contains(&probe) && out.len() < 12 {
                out.push(probe);
            }
        }
    }
    out
}

/// First letter upper, the rest lower, which is how the register writes a gmina.
fn title_case(word: &str) -> String {
    match word.chars().next() {
        Some(first) => first.to_uppercase().collect::<String>() + &word[first.len_utf8()..].to_lowercase(),
        None => String::new(),
    }
}

/// The accented readings of a plain word, at most eight of them.
fn accents(word: &str) -> Vec<String> {
    const PAIRS: [(char, [char; 3]); 9] = [
        ('a', ['a', 'ą', 'a']),
        ('c', ['c', 'ć', 'c']),
        ('e', ['e', 'ę', 'e']),
        ('l', ['l', 'ł', 'l']),
        ('n', ['n', 'ń', 'n']),
        ('o', ['o', 'ó', 'o']),
        ('s', ['s', 'ś', 's']),
        ('z', ['z', 'ź', 'ż']),
        ('x', ['x', 'x', 'x']),
    ];
    let mut out = vec![String::new()];
    for letter in word.chars() {
        let lower = letter.to_lowercase().next().unwrap_or(letter);
        let options: Vec<char> = match PAIRS.iter().find(|(plain, _)| *plain == lower) {
            Some((_, forms)) => forms.to_vec(),
            None => vec![letter],
        };
        let mut next: Vec<String> = Vec::new();
        for prefix in &out {
            for option in &options {
                if next.len() < 8 {
                    let mut candidate = prefix.clone();
                    candidate.push(*option);
                    next.push(candidate);
                }
            }
        }
        out = next;
    }
    out.retain(|candidate| !candidate.is_empty());
    out
}

/// One pattern test on a property, with the register's own wildcard and escape.
fn like(property: &str, literal: &str) -> String {
    format!(
        "<PropertyIsLike wildCard=\"*\" singleChar=\"?\" escapeChar=\"!\">\
<PropertyName>{property}</PropertyName><Literal>{literal}</Literal></PropertyIsLike>"
    )
}

/// The characters a filter's literal cannot carry as they are.
fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// The filter travels in the address, so its XML is encoded here.
fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// The matches as "name (code)" lines, so a refusal is a list to choose from.
fn match_list(matches: &[(String, String, &'static str)]) -> String {
    matches
        .iter()
        .take(12)
        .map(|(code, name, _)| format!("{name} ({code})"))
        .collect::<Vec<String>>()
        .join(", ")
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
