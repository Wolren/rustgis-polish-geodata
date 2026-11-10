//! The county's own databases: BDOT500 and GESUT from the EZiUDP register, for the unit under the
//! view. One command per dataset. It finds the gmina from ULDK, takes the most specific register
//! WMS (gmina, then county, then national) and adds the whole service as one overlay. A unit that
//! publishes the dataset only as a WFS gets its id named, because a command cannot start a tool.

use crate::egib;
use crate::host::Host;
use crate::http;
use crate::text::{self, Lang};
use crate::zrodla::{self, Registered};
use quick_xml::events::Event;
use quick_xml::Reader;
use serde_json::{json, Value};

/// The BDOT500 dataset, as the register names it.
pub const BDOT500: &str = "BDOT500";
/// The GESUT dataset, as the register names it.
pub const GESUT: &str = "GESUT";

/// Adds the unit's own dataset for the view: a WMS on the map, or the id of its WFS.
pub fn run(host: &mut Host, lang: Lang, dataset: &str) -> Result<Value, String> {
    let (lon, lat) = host.centre()?;
    let unit = unit_at(lon, lat, lang)?;
    let found = matches(dataset, &unit);
    let entry = *found
        .first()
        .ok_or_else(|| text::county_no_dataset(lang, dataset, &unit.name))?;
    let candidates: Vec<Value> = found
        .iter()
        .map(|entry| {
            json!({
                "id": entry.id,
                "name": entry.name,
                "kind": entry.kind,
                "teryt": entry.teryt,
                "url": entry.url,
            })
        })
        .collect();
    if entry.kind != "wms" {
        let note = text::county_wfs(lang, dataset, &unit.name, &entry.id);
        host.notify(&note);
        return Ok(json!({
            "dataset": dataset,
            "unit": unit.name,
            "teryt": unit.teryt,
            "kind": entry.kind,
            "service": entry.id,
            "name": entry.name,
            "url": entry.url,
            "count": candidates.len(),
            "candidates": candidates,
            "note": note,
        }));
    }
    let layers = layers_of(entry, lang);
    zrodla::add_wms(host, entry, &layers, lang)?;
    host.notify(&text::county_added(lang, dataset, &unit.name, &text::layers(lang, layer_count(&layers))));
    Ok(json!({
        "dataset": dataset,
        "unit": unit.name,
        "teryt": unit.teryt,
        "kind": "wms",
        "service": entry.id,
        "name": entry.name,
        "url": entry.url,
        "layers": layers,
        "count": candidates.len(),
        "candidates": candidates,
    }))
}

/// The unit the register matches against: the gmina under the point, or the county when ULDK
/// names no gmina.
struct Unit {
    /// The gmina's TERYT without its type suffix, empty when only the county is known.
    gmina: String,
    county: String,
    teryt: String,
    name: String,
}

fn unit_at(lon: f64, lat: f64, lang: Lang) -> Result<Unit, String> {
    for (x, y) in [(lon, lat), (lat, lon)] {
        if let Ok((code, name)) = egib::commune_at(x, y, lang) {
            let gmina = stem(&code).to_string();
            let county = gmina.chars().take(4).collect();
            return Ok(Unit { gmina, county, teryt: code, name });
        }
    }
    for (x, y) in [(lon, lat), (lat, lon)] {
        if let Ok((code, name)) = egib::county_at(x, y, lang) {
            return Ok(Unit { gmina: String::new(), county: code.clone(), teryt: code, name });
        }
    }
    Err(text::county_view_outside(lang).to_string())
}

/// The TERYT code without the gmina type suffix: `306401_1` becomes `306401`.
fn stem(code: &str) -> &str {
    code.split('_').next().unwrap_or(code)
}

/// How close a service is to the unit: a WMS before a WFS, then gmina, county and country.
fn rank(entry: &Registered, unit: &Unit) -> Option<u8> {
    let code = stem(&entry.teryt);
    let territory = if !unit.gmina.is_empty() && code == unit.gmina {
        0
    } else if code == unit.county {
        1
    } else if entry.teryt.eq_ignore_ascii_case("PL") {
        2
    } else {
        return None;
    };
    Some(u8::from(entry.kind != "wms") * 4 + territory)
}

/// Every service of the dataset that names the unit's own gmina, county or country, best first.
fn matches(dataset: &str, unit: &Unit) -> Vec<&'static Registered> {
    matches_in(dataset, unit, zrodla::all())
}

fn matches_in<'a>(dataset: &str, unit: &Unit, entries: &'a [Registered]) -> Vec<&'a Registered> {
    let mut found: Vec<(u8, &'a Registered)> = entries
        .iter()
        .filter(|entry| entry.name.to_ascii_uppercase().contains(dataset))
        .filter_map(|entry| rank(entry, unit).map(|rank| (rank, entry)))
        .collect();
    found.sort_by_key(|(rank, _)| *rank);
    found.into_iter().map(|(_, entry)| entry).collect()
}

fn layer_count(layers: &str) -> usize {
    layers.split(',').filter(|name| !name.trim().is_empty()).count()
}

/// The layer names of the whole service: the capabilities' root layer, or its direct children.
fn layers_of(entry: &Registered, lang: Lang) -> String {
    let separator = if entry.url.contains('?') { '&' } else { '?' };
    let url = format!("{}{separator}SERVICE=WMS&VERSION=1.3.0&REQUEST=GetCapabilities", entry.url);
    let names = http::get(&url, 60, lang).map(|caps| top_layers(&caps)).unwrap_or_default();
    if names.is_empty() {
        entry.layer.clone()
    } else {
        names.join(",")
    }
}

/// The layer names a WMS draws as a whole: the root layer's name, or the direct children's.
pub fn top_layers(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    let mut depth = 0usize;
    let mut naming = 0usize;
    let mut text = String::new();
    let mut root: Option<String> = None;
    let mut children: Vec<String> = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => match element.local_name().as_ref() {
                b"Layer" => depth += 1,
                b"Name" if depth == 1 || depth == 2 => {
                    naming = depth;
                    text.clear();
                }
                _ => {}
            },
            Ok(Event::Text(content)) if naming > 0 => {
                let value = content
                    .unescape()
                    .map(|value| value.into_owned())
                    .unwrap_or_else(|_| String::from_utf8_lossy(&content).into_owned());
                text.push_str(&value);
            }
            Ok(Event::CData(content)) if naming > 0 => {
                text.push_str(&String::from_utf8_lossy(&content));
            }
            Ok(Event::End(element)) => match element.local_name().as_ref() {
                b"Layer" => depth = depth.saturating_sub(1),
                b"Name" if naming > 0 => {
                    let name = text.trim().to_string();
                    if naming == 1 {
                        root = (!name.is_empty()).then_some(name);
                    } else if !name.is_empty() {
                        children.push(name);
                    }
                    naming = 0;
                    text.clear();
                }
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    match root {
        Some(name) => vec![name],
        None => children,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, name: &str, kind: &str, teryt: &str) -> Registered {
        Registered {
            id: id.to_string(),
            url: format!("https://example.test/{id}"),
            kind: kind.to_string(),
            name: name.to_string(),
            institution: "Gmina Test".into(),
            unit: String::new(),
            teryt: teryt.to_string(),
            datasets: 0,
            layer: "layer".into(),
        }
    }

    fn unit(gmina: &str, county: &str) -> Unit {
        Unit {
            gmina: gmina.to_string(),
            county: county.to_string(),
            teryt: String::new(),
            name: "Poznań (miasto)".into(),
        }
    }

    #[test]
    fn the_gmina_service_beats_the_county_and_the_country() {
        let entries = vec![
            entry("k-gesut", "Krajowa baza (K-GESUT)", "wms", "PL"),
            entry("county", "GESUT powiatu", "wms", "3064"),
            entry("gmina", "GESUT gminy", "wms", "306401_1"),
        ];
        let found: Vec<&str> = matches_in("GESUT", &unit("306401", "3064"), &entries).iter().map(|e| e.id.as_str()).collect();
        assert_eq!(found, ["gmina", "county", "k-gesut"]);
    }

    #[test]
    fn a_wms_comes_before_a_wfs_at_the_same_unit() {
        let entries = vec![
            entry("wfs", "BDOT500 powiatu", "wfs", "3064"),
            entry("wms", "BDOT500 miasta", "wms", "3064"),
        ];
        let found = matches_in("BDOT500", &unit("", "3064"), &entries);
        assert_eq!(found[0].id, "wms");
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn another_gmina_service_does_not_name_the_unit() {
        let entries = vec![entry("other", "BDOT500 obcej gminy", "wms", "240204_3")];
        assert!(matches_in("BDOT500", &unit("306401", "3064"), &entries).is_empty());
    }

    #[test]
    fn the_country_counts_only_when_no_local_service_exists() {
        let entries = vec![entry("k-gesut", "K-GESUT", "wms", "PL")];
        assert_eq!(matches_in("GESUT", &unit("306401", "3064"), &entries)[0].id, "k-gesut");
    }

    #[test]
    fn the_root_layer_names_the_whole_service() {
        let xml = r#"
            <WMS_Capabilities><Capability><Layer>
              <Title>BDOT500</Title>
              <Layer><Name>bdot_rzezba_terenu</Name><Title>Rzeźba</Title></Layer>
              <Layer><Name>bdot_wody</Name><Title>Wody</Title>
                <Layer><Name>wody_powierzchnie</Name></Layer>
              </Layer>
            </Layer></Capability></WMS_Capabilities>"#;
        assert_eq!(top_layers(xml), ["bdot_rzezba_terenu", "bdot_wody"]);
    }

    #[test]
    fn a_named_root_layer_takes_every_layer_with_it() {
        let xml = r#"
            <WMS_Capabilities><Capability><Layer>
              <Name>k-gesut</Name>
              <Layer><Name>hydranty</Name></Layer>
              <Layer><Name>sieci</Name></Layer>
            </Layer></Capability></WMS_Capabilities>"#;
        assert_eq!(top_layers(xml), ["k-gesut"]);
    }
}
