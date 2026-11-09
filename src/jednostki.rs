//! The administrative units of Poland: every voivodeship, powiat and gmina with its TERYT code.
//!
//! The panel's tree reads them from the package, so a user picks a unit without a network round trip.
//! `tools/jednostki.py` rebuilds `data/jednostki.json` from PRG.

use serde_json::{json, Value};
use std::sync::OnceLock;

const DATA: &str = include_str!("../data/jednostki.json");

fn parsed() -> &'static Value {
    static UNITS: OnceLock<Value> = OnceLock::new();
    UNITS.get_or_init(|| serde_json::from_str(DATA).unwrap_or_else(|_| json!({ "wojewodztwa": [] })))
}

/// The tree the panel shows: voivodeships, their powiaty, their gminy.
pub fn nodes(lang: crate::text::Lang) -> Value {
    let empty: Vec<Value> = Vec::new();
    let voivodeships = parsed()
        .get("wojewodztwa")
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    let mut out = Vec::with_capacity(voivodeships.len());
    for voivodeship in voivodeships {
        let powiaty: Vec<Value> = voivodeship
            .get("powiaty")
            .and_then(Value::as_array)
            .unwrap_or(&empty)
            .iter()
            .map(|powiat| {
                let gminy: Vec<Value> = powiat
                    .get("gminy")
                    .and_then(Value::as_array)
                    .unwrap_or(&empty)
                    .iter()
                    .filter_map(|gmina| {
                        let code = gmina.get("kod")?.as_str()?;
                        let name = gmina.get("nazwa")?.as_str()?;
                        Some(json!({ "id": code, "label": name }))
                    })
                    .collect();
                let count = gminy.len();
                json!({
                    "id": powiat.get("kod").and_then(Value::as_str).unwrap_or_default(),
                    "label": powiat.get("nazwa").and_then(Value::as_str).unwrap_or_default(),
                    "count": count,
                    "children": gminy,
                })
            })
            .collect();
        let count = powiaty.len();
        out.push(json!({
            "id": voivodeship.get("kod").and_then(Value::as_str).unwrap_or_default(),
            "label": voivodeship.get("nazwa").and_then(Value::as_str).unwrap_or_default(),
            "count": count,
            "children": powiaty,
        }));
    }
    let _ = lang;
    Value::Array(out)
}

/// The name and PRG level behind a code: two digits mean a voivodeship, four a powiat, seven a gmina.
pub fn resolve(code: &str) -> Option<(String, &'static str)> {
    let code = code.trim();
    let level = match code.chars().count() {
        2 => "wojewodztwo",
        4 => "powiat",
        7 => "gmina",
        _ => return None,
    };
    let empty: Vec<Value> = Vec::new();
    for voivodeship in parsed().get("wojewodztwa")?.as_array().unwrap_or(&empty) {
        if level == "wojewodztwo" {
            if voivodeship.get("kod").and_then(Value::as_str) == Some(code) {
                return Some((
                    voivodeship.get("nazwa")?.as_str()?.to_string(),
                    level,
                ));
            }
            continue;
        }
        for powiat in voivodeship.get("powiaty").and_then(Value::as_array).unwrap_or(&empty) {
            if level == "powiat" {
                if powiat.get("kod").and_then(Value::as_str) == Some(code) {
                    return Some((powiat.get("nazwa")?.as_str()?.to_string(), level));
                }
                continue;
            }
            for gmina in powiat.get("gminy").and_then(Value::as_array).unwrap_or(&empty) {
                if gmina.get("kod").and_then(Value::as_str) == Some(code) {
                    return Some((gmina.get("nazwa")?.as_str()?.to_string(), level));
                }
            }
        }
    }
    None
}

/// The code a unit's name belongs to, so a form can take a name instead of a code.
pub fn code_of_name(name: &str) -> Option<String> {
    let wanted = crate::granice::fold(name.trim());
    if wanted.is_empty() {
        return None;
    }
    let empty: Vec<Value> = Vec::new();
    for voivodeship in parsed().get("wojewodztwa")?.as_array().unwrap_or(&empty) {
        let label = voivodeship.get("nazwa")?.as_str()?;
        if crate::granice::fold(label) == wanted {
            return voivodeship.get("kod")?.as_str().map(str::to_string);
        }
        for powiat in voivodeship.get("powiaty").and_then(Value::as_array).unwrap_or(&empty) {
            let label = powiat.get("nazwa")?.as_str()?;
            if crate::granice::fold(label) == wanted {
                return powiat.get("kod")?.as_str().map(str::to_string);
            }
            for gmina in powiat.get("gminy").and_then(Value::as_array).unwrap_or(&empty) {
                let label = gmina.get("nazwa")?.as_str()?;
                if crate::granice::fold(label) == wanted {
                    return gmina.get("kod")?.as_str().map(str::to_string);
                }
            }
        }
    }
    None
}

/// The units whose name holds a fragment, so a form can offer the matches instead of guessing.
pub fn matching(fragment: &str, limit: usize) -> Vec<(String, String, &'static str)> {
    let wanted = crate::granice::fold(fragment.trim());
    let mut out: Vec<(String, String, &'static str)> = Vec::new();
    if wanted.is_empty() {
        return out;
    }
    let empty: Vec<Value> = Vec::new();
    let Some(voivodeships) = parsed().get("wojewodztwa").and_then(Value::as_array) else {
        return out;
    };
    for voivodeship in voivodeships {
        let Some(code) = voivodeship.get("kod").and_then(Value::as_str) else { continue };
        let name = voivodeship.get("nazwa").and_then(Value::as_str).unwrap_or_default();
        if crate::granice::fold(name).contains(&wanted) {
            out.push((code.to_string(), name.to_string(), "wojewodztwo"));
        }
        for powiat in voivodeship.get("powiaty").and_then(Value::as_array).unwrap_or(&empty) {
            let Some(code) = powiat.get("kod").and_then(Value::as_str) else { continue };
            let name = powiat.get("nazwa").and_then(Value::as_str).unwrap_or_default();
            if crate::granice::fold(name).contains(&wanted) {
                out.push((code.to_string(), name.to_string(), "powiat"));
            }
            for gmina in powiat.get("gminy").and_then(Value::as_array).unwrap_or(&empty) {
                let Some(code) = gmina.get("kod").and_then(Value::as_str) else { continue };
                let name = gmina.get("nazwa").and_then(Value::as_str).unwrap_or_default();
                if crate::granice::fold(name).contains(&wanted) {
                    out.push((code.to_string(), name.to_string(), "gmina"));
                }
                if out.len() >= limit {
                    return out;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{nodes, resolve};

    #[test]
    fn every_level_resolves_by_code_length() {
        assert_eq!(resolve("14").map(|(name, level)| (name, level)), Some(("mazowieckie".to_string(), "wojewodztwo")));
        assert_eq!(resolve("1416").map(|(name, level)| (name, level)), Some(("ostrowski".to_string(), "powiat")));
        let (name, level) = resolve("1464011").expect("a gmina");
        assert_eq!(level, "gmina");
        assert!(!name.is_empty());
        assert!(resolve("141").is_none());
        assert!(resolve("9999").is_none());
    }

    #[test]
    fn the_tree_holds_the_whole_country() {
        let tree = nodes(crate::text::Lang::En);
        let voivodeships = tree.as_array().expect("a list");
        assert_eq!(voivodeships.len(), 16);
        let powiaty: usize = voivodeships
            .iter()
            .filter_map(|node| node.get("count").and_then(serde_json::Value::as_u64))
            .sum::<u64>() as usize;
        assert!(powiaty >= 300, "powiaty: {powiaty}");
        let path = std::path::Path::new("data/jednostki.json");
        assert!(path.is_file());
    }
}
