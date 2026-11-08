//! Parcels and buildings straight from the county: EGiB as vectors for the area on screen.
//!
//! GUGiK's national KIEG service only rasterises parcels, so nothing in it can be clipped. Every
//! county runs its own WFS EGiB and reports it to EZiUDP, and that WFS answers GML for a bounding
//! box. The chain: ULDK names the county under the area, the register gives its WFS address, the
//! capabilities name the types, and one bounded `GetFeature` per type becomes a layer.

use crate::gml;
use crate::host::{self, Host};
use crate::http;
use crate::text::{self, Lang};
use crate::zrodla;
use serde_json::{json, Value};

const ULDK: &str = "https://uldk.gugik.gov.pl/";

/// How many features one run may collect: a city's parcels run to thousands.
const MAX_FEATURES: usize = 20_000;
/// How many features one page asks for.
const PAGE: usize = 1000;

/// Adds the county's parcels and buildings for the area the request names.
pub fn from_area(host: &mut Host, params: &Value, lang: Lang) -> Result<Value, String> {
    let (area, features) = area_features(host, params, lang)?;
    let box_lon = lon_lat_bounds(&features).ok_or_else(|| text::area_no_geometry(lang, &area))?;
    let middle_x = (box_lon[0] + box_lon[2]) / 2.0;
    let middle_y = (box_lon[1] + box_lon[3]) / 2.0;
    let mut found = None;
    // The layer may hold lon and lat, or metres in the national grid: try each reading in turn.
    for (lon, lat) in [
        (middle_x, middle_y),
        (middle_y, middle_x),
        crate::gk92::inverse(middle_y, middle_x),
    ] {
        if let Ok(county) = county_at(lon, lat, lang) {
            found = Some(county);
            break;
        }
    }
    let (county_code, county) = found.ok_or_else(|| text::egib_no_county(lang))?;
    let services = services_for(&county_code);
    if services.is_empty() {
        return Err(text::egib_no_service(lang, &county, &county_code));
    }
    // A county may report several WFS addresses; the one that publishes parcels wins.
    let mut chosen = None;
    for service in &services {
        let types = wanted_types(&service.url, params, lang)?;
        if !types.is_empty() {
            chosen = Some((*service, types));
            break;
        }
    }
    let Some((service, types)) = chosen else {
        return Err(text::egib_no_types(lang, &county));
    };

    let cap = params
        .get("limit")
        .and_then(Value::as_u64)
        .map(|limit| (limit as usize).clamp(1, MAX_FEATURES))
        .unwrap_or(MAX_FEATURES);
    let mut collection: Vec<Value> = Vec::new();
    for type_name in &types {
        let mut start = 0usize;
        loop {
            let url = page_url(&service.url, type_name, box_lon, PAGE, start);
            let body = http::fetch(&url, 110, lang)?;
            let mut rows = gml::parse_features(&body, false, lang)?;
            let got = rows.len();
            rows.truncate(cap.saturating_sub(collection.len()));
            collection.extend(rows);
            if got < PAGE || collection.len() >= cap {
                break;
            }
            start += got;
            host::progress_line(&text::egib_progress(lang, collection.len()));
        }
        if collection.len() >= cap {
            break;
        }
    }
    if collection.is_empty() {
        return Err(text::egib_empty(lang, &county));
    }

    let name = text::egib_layer(lang, &county);
    let capped = collection.len() >= cap;
    let geojson = json!({ "type": "FeatureCollection", "features": collection });
    host.add_layer(&name, geojson)?;
    host.notify(&text::egib_added(lang, &name, collection.len(), &county));
    if capped {
        host.notify(&text::egib_capped(lang, cap));
    }
    Ok(json!({
        "layer": name,
        "county": county,
        "teryt": county_code,
        "types": types,
        "features": collection.len(),
        "capped": capped,
        "service": service.url,
    }))
}

/// The area layer the request names, else the one the map holds.
fn area_features(host: &mut Host, params: &Value, lang: Lang) -> Result<(String, Vec<Value>), String> {
    let named = params
        .get("obszar")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty());
    let answer = host.call("host.layers", json!({}))?;
    let layers = answer.as_array().cloned().unwrap_or_default();
    let name = match named {
        Some(name) => name.to_string(),
        None => layers
            .iter()
            .filter_map(|layer| layer.get("name").and_then(Value::as_str))
            .find(|name| text::AREA_LAYERS.contains(name))
            .map(str::to_string)
            .ok_or_else(|| text::egib_needs_area(lang))?,
    };
    let features = host.call("host.features", json!({ "layer": name.clone() }))?;
    let list = features
        .get("features")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if list.is_empty() {
        return Err(text::egib_empty_area(lang, &name));
    }
    Ok((name, list))
}

/// The county a point falls in, as ULDK gives it: the TERYT and the name.
fn county_at(lon: f64, lat: f64, lang: Lang) -> Result<(String, String), String> {
    let url = format!("{ULDK}?request=GetCountyByXY&xy={lon:.6},{lat:.6},4326&result=teryt,county");
    let body = http::fetch(&url, 60, lang)?;
    // ULDK answers a status line first, then the data line: "2464|powiat Częstochowa".
    let line = body
        .lines()
        .map(str::trim)
        .find(|line| line.contains('|'))
        .ok_or_else(|| text::egib_no_county(lang))?;
    let mut fields = line.split('|');
    let code = fields.next().unwrap_or_default().trim().to_string();
    let name = fields.next().unwrap_or_default().trim().to_string();
    if code.is_empty() {
        return Err(text::egib_no_county(lang));
    }
    Ok((code, name))
}

/// Every register WFS a county reports with an EGiB dataset, in register order.
fn services_for(county_code: &str) -> Vec<&'static zrodla::Registered> {
    let wanted = county_code.trim();
    zrodla::all()
        .iter()
        .filter(|entry| {
            entry.kind == "wfs"
                && entry.name.to_lowercase().contains("ewidencja grunt")
                && (entry.teryt.starts_with(wanted) || wanted.starts_with(&entry.teryt))
        })
        .collect()
}

/// The feature types to fetch: `dzialki` and `budynki` unless the request names `typy`.
fn wanted_types(url: &str, params: &Value, lang: Lang) -> Result<Vec<String>, String> {
    let asked = params
        .get("typy")
        .and_then(Value::as_str)
        .map(|list| {
            list.split(',')
                .map(|part| part.trim().to_lowercase())
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec!["dzialki".to_string(), "budynki".to_string()]);
    let body = http::fetch(
        &format!("{url}?SERVICE=WFS&REQUEST=GetCapabilities"),
        90,
        lang,
    )?;
    let names = capability_types(&body);
    let mut picked: Vec<String> = names
        .into_iter()
        .filter(|name| asked.iter().any(|want| name.to_lowercase().contains(want.as_str())))
        .collect();
    picked.dedup();
    Ok(picked)
}

/// The feature type names a capabilities document lists, whatever namespace its tags carry.
fn capability_types(body: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = body;
    while let Some(at) = rest.find("Name>") {
        rest = &rest[at + 5..];
        let Some(end) = rest.find('<') else {
            break;
        };
        let name = rest[..end].trim();
        rest = &rest[end..];
        // A type name is namespaced (`gmgml:dzialki`); a service or provider name is not.
        if name.contains(':') && !name.contains(' ') {
            names.push(name.to_string());
        }
    }
    names.dedup();
    names
}

/// The `GetFeature` address for one type and page, bounded by the area.
fn page_url(url: &str, type_name: &str, box_lon: [f64; 4], count: usize, start: usize) -> String {
    let box_text = format!(
        "{:.6},{:.6},{:.6},{:.6},EPSG:4326",
        box_lon[0], box_lon[1], box_lon[2], box_lon[3]
    );
    format!(
        "{url}?SERVICE=WFS&VERSION=2.0.0&REQUEST=GetFeature&TYPENAMES={}&SRSNAME=EPSG:4326&COUNT={count}&STARTINDEX={start}&BBOX={box_text}",
        encode(type_name)
    )
}

/// The box around every feature, in lon and lat.
fn lon_lat_bounds(features: &[Value]) -> Option<[f64; 4]> {
    let mut extent = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    let mut any = false;
    for feature in features {
        let geometry = feature.get("geometry")?;
        let mut points: Vec<[f64; 2]> = Vec::new();
        crate::obszar::collect_points(geometry, &mut points, 0);
        for [lon, lat] in points {
            extent[0] = extent[0].min(lon);
            extent[1] = extent[1].min(lat);
            extent[2] = extent[2].max(lon);
            extent[3] = extent[3].max(lat);
            any = true;
        }
    }
    (any && extent[2] > extent[0] && extent[3] > extent[1]).then_some(extent)
}

/// Percent-encoding for the query string.
fn encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b':' => {
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

/// A small deterministic generator, so a fuzz case failing here fails everywhere.
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Lcg(seed | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound.max(1)
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u64) as usize]
    }
}

    #[test]
    fn capabilities_name_their_types() {
        let body = "<WFS_Capabilities><FeatureTypeList><FeatureType><Name>gmgml:dzialki</Name></FeatureType><FeatureType><Name>gmgml:budynki</Name></FeatureType></FeatureTypeList></WFS_Capabilities>";
        assert_eq!(capability_types(body), vec!["gmgml:dzialki", "gmgml:budynki"]);
    }

    #[test]
    fn a_bounded_url_names_the_box_and_the_type() {
        let url = page_url("https://x/wfs", "gmgml:dzialki", [16.9, 52.4, 17.0, 52.5], 1000, 2000);
        assert!(url.contains("TYPENAMES=gmgml:dzialki"), "{url}");
        assert!(url.contains("BBOX=16.900000,52.400000,17.000000,52.500000,EPSG:4326"));
        assert!(url.contains("COUNT=1000"));
        assert!(url.contains("STARTINDEX=2000"));
    }
    /// Capabilities as real servers send them: namespaces, nesting, empty names, stray text.
    #[test]
    fn capabilities_are_read_from_any_shape() {
        let mut rng = Lcg::new(5);
        let chunks = [
            "<FeatureType><Name>gmgml:dzialki</Name></FeatureType>",
            "<wfs:FeatureType><wfs:Name>wfs:budynki</wfs:Name></wfs:FeatureType>",
            "<Name></Name>",
            "<Name>no-colon</Name>",
            "<Name>a b</Name>",
            "<FeatureType>",
            "plain text",
        ];
        for _ in 0..300 {
            let count = rng.below(6) as usize;
            let body: String = (0..count).map(|_| *rng.pick(&chunks)).collect();
            let names = capability_types(&body);
            for name in names {
                assert!(name.contains(':') && !name.contains(' '), "{name}");
            }
        }
    }

}
