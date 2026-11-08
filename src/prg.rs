//! The area from an administrative unit: a gmina, a powiat or a voivodeship.
//!
//! PRG (`AdministrativeBoundaries`) answers one feature per unit, filtered by its name in an OGC
//! `FILTER`, so the shape comes back without the whole country. The polygons land in the area
//! layer, so a download uses their box and a clip follows the real boundary.

use crate::gml;
use crate::host::Host;
use crate::http;
use crate::obszar;
use crate::text::{self, Lang};
use serde_json::{json, Value};

const URL: &str = "https://mapy.geoportal.gov.pl/wss/service/PZGIK/PRG/WFS/AdministrativeBoundaries";

/// The three levels the command knows, with the WFS type each one reads.
const LEVELS: &[(&str, &str)] = &[
    ("gmina", "A03_Granice_gmin"),
    ("powiat", "A02_Granice_powiatow"),
    ("wojewodztwo", "A01_Granice_wojewodztw"),
];

/// Sets the area from a unit the request names, and cuts a layer to it when asked.
pub fn area(host: &mut Host, params: &Value, lang: Lang) -> Result<Value, String> {
    let name = params
        .get("nazwa")
        .or_else(|| params.get("name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| text::prg_needs_name(lang))?;
    let level = params
        .get("poziom")
        .or_else(|| params.get("level"))
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("gmina")
        .to_lowercase();
    let type_name = level_type(&level).ok_or_else(|| text::prg_levels(lang))?;

    let features = fetch(name, type_name, &level, lang)?;
    let rings = rings_of(&features);
    if rings.is_empty() {
        return Err(text::prg_no_unit(lang, name, &level));
    }
    obszar::drop_drawing(host);
    let layer = obszar::area_layer_name(host, lang)?;
    obszar::fill_polygons(host, &layer, &rings, lang)?;
    host.call("layer.select", json!({ "layer": layer.clone() }))?;
    obszar::Mode::View.remember();
    let box_lon = obszar::bounds_of(&rings);
    host.notify(&text::prg_area_set(lang, &layer, name, rings.len()));

    // A refused cut leaves the area set and comes back as clip_error, not as a failure.
    let mut cut = Value::Null;
    let mut clip_error = Value::Null;
    if let Some(wanted) = params.get("przytnij").or_else(|| params.get("clip")) {
        let target = match wanted.as_str() {
            Some(name) if !name.trim().is_empty() && name != "true" => name.trim().to_string(),
            _ => match crate::clip::layers(host, lang) {
                Ok(list) => crate::clip::target(&list, &layer)
                    .ok_or_else(|| text::nothing_to_clip(lang))
                    .unwrap_or_default(),
                Err(error) => error,
            },
        };
        match crate::clip::to_area(host, &target, &layer, lang) {
            Ok(value) => cut = value,
            Err(error) => {
                host.notify(&error);
                clip_error = json!(error);
            }
        }
    }
    Ok(json!({
        "layer": layer,
        "unit": name,
        "level": level,
        "parts": rings.len(),
        "box": box_lon,
        "clip": cut,
        "clip_error": clip_error,
    }))
}

/// The WFS type a level names: the Polish word, `gm`, `pow` or `woj`, and the type itself.
fn level_type(level: &str) -> Option<&'static str> {
    let level = plain(level);
    LEVELS
        .iter()
        .find(|(word, type_name)| level == *word || level == *type_name || level.starts_with(word))
        .map(|(_, type_name)| *type_name)
}

/// The level word without its diacritics, so "województwo" and "wojewodztwo" both match.
fn plain(word: &str) -> String {
    word.chars()
        .map(|c| match c {
            'ą' => 'a',
            'ć' => 'c',
            'ę' => 'e',
            'ł' => 'l',
            'ń' => 'n',
            'ó' => 'o',
            'ś' => 's',
            'ź' | 'ż' => 'z',
            other => other,
        })
        .collect()
}

/// One `GetFeature` filtered by the unit's name, in lon and lat.
///
/// PRG writes a powiat as "powiat ostrowski", so the spellings are tried in turn: the name as
/// given, its capitalised form, and the prefixed one the register uses.
fn fetch(name: &str, type_name: &str, level: &str, lang: Lang) -> Result<Vec<Value>, String> {
    for literal in spellings(name, level) {
        let filter = format!(
            "<Filter><PropertyIsEqualTo><PropertyName>JPT_NAZWA_</PropertyName><Literal>{}</Literal></PropertyIsEqualTo></Filter>",
            escape(&literal)
        );
        let url = format!(
            "{URL}?SERVICE=WFS&VERSION=2.0.0&REQUEST=GetFeature&TYPENAMES={type_name}&SRSNAME=EPSG:4326&COUNT=8&FILTER={}",
            encode(&filter)
        );
        // The answer is small and names one unit, so it is not worth keeping for a week.
        let body = http::fetch(&url, 90, lang)?;
        let features = gml::parse_features(&body, false, lang)?;
        if !features.is_empty() {
            return Ok(features);
        }
    }
    Ok(Vec::new())
}

/// The spellings a unit's name takes in PRG.
fn spellings(name: &str, level: &str) -> Vec<String> {
    let name = name.trim();
    let lower = name.to_lowercase();
    let mut out = vec![name.to_string()];
    if let Some(rest) = lower.strip_prefix("powiat ").or_else(|| lower.strip_prefix("województwo ")) {
        out.push(rest.to_string());
    }
    if level.starts_with("powiat") {
        out.push(format!("powiat {lower}"));
        out.push(format!("powiat {}", lower.trim_start_matches("powiat ")));
    }
    let mut first = name.chars();
    if let Some(letter) = first.next() {
        let capitalised = format!("{}{}", letter.to_uppercase(), first.as_str());
        out.push(capitalised);
    }
    out.push(lower);
    out.sort();
    out.dedup();
    out
}

/// Every outer ring of every polygon a feature holds, in lon and lat.
fn rings_of(features: &[Value]) -> Vec<Vec<[f64; 2]>> {
    let mut rings = Vec::new();
    for feature in features {
        let Some(geometry) = feature.get("geometry") else {
            continue;
        };
        match geometry.get("type").and_then(Value::as_str) {
            Some("Polygon") => {
                if let Some(ring) = ring_at(geometry.get("coordinates"), 0) {
                    rings.push(ring);
                }
            }
            Some("MultiPolygon") => {
                if let Some(parts) = geometry.get("coordinates").and_then(Value::as_array) {
                    for part in parts {
                        if let Some(ring) = ring_at(Some(part), 0) {
                            rings.push(ring);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    rings
}

/// The ring at `index` of a coordinates array, as points.
fn ring_at(coordinates: Option<&Value>, index: usize) -> Option<Vec<[f64; 2]>> {
    let ring = coordinates?.get(index)?.as_array()?;
    let mut points: Vec<[f64; 2]> = ring
        .iter()
        .filter_map(|point| Some([point.get(0)?.as_f64()?, point.get(1)?.as_f64()?]))
        .collect();
    if points.len() < 4 {
        return None;
    }
    if points.first() != points.last() {
        let first = *points.first()?;
        points.push(first);
    }
    Some(points)
}

/// The unit's name as an XML literal.
fn escape(name: &str) -> String {
    name.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Percent-encoding for the query string, so a name with a space or a diacritic survives.
fn encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_level_names_its_type() {
        assert_eq!(level_type("gmina"), Some("A03_Granice_gmin"));
        assert_eq!(level_type("powiat"), Some("A02_Granice_powiatow"));
        assert_eq!(level_type("województwo"), Some("A01_Granice_wojewodztw"));
        assert_eq!(level_type("powiaty"), Some("A02_Granice_powiatow"));
        assert_eq!(level_type("dzielnica"), None);
    }

    #[test]
    fn a_name_is_encoded_for_the_query_and_escaped_for_the_filter() {
        assert_eq!(encode("Poznań"), "Pozna%C5%84");
        assert_eq!(encode("Nowy Sącz"), "Nowy%20S%C4%85cz");
        assert_eq!(escape("A&B"), "A&amp;B");
    }

    #[test]
    fn a_polygon_gives_its_ring() {
        let features = vec![json!({
            "type": "Feature",
            "geometry": {"type": "Polygon", "coordinates": [[[16.9, 52.4], [17.0, 52.4], [17.0, 52.5], [16.9, 52.5], [16.9, 52.4]]]},
        })];
        let rings = rings_of(&features);
        assert_eq!(rings.len(), 1);
        assert_eq!(rings[0].len(), 5);
    }
}
