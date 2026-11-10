//! Parcels and buildings straight from the county: EGiB as vectors for the area on screen.
//!
//! GUGiK's national KIEG service only rasterises parcels, so nothing in it can be clipped. Every
//! county runs its own WFS EGiB and reports it to EZiUDP, and that WFS answers GML for a bounding
//! box. The chain: ULDK names the county under the area, the register gives its WFS address, the
//! capabilities name the types, and one bounded `GetFeature` per type becomes a layer.

use crate::catalog;
use crate::gml;
use crate::host::{self, Host};
use crate::http;
use crate::text::{self, Lang};
use crate::wfs;
use crate::zrodla;
use serde_json::{json, Value};

const ULDK: &str = "https://uldk.gugik.gov.pl/";

/// How many features one run may collect: a city's parcels run to thousands.
const MAX_FEATURES: usize = 20_000;
/// How many features one page asks for.
const PAGE: usize = 1000;
/// A clipped feature smaller than this in square degrees is a line on the boundary, not a shape.
const MIN_AREA: f64 = 1e-12;

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
    let mut grid_warned = false;
    for type_name in &types {
        let mut start = 0usize;
        let mut lat_first = false;
        loop {
            let body = fetch_page(&service.url, type_name, box_lon, PAGE, start, lang, lat_first)?;
            let mut rows = gml::parse_features(&body, false, lang)?;
            let got = rows.len();
            // A WFS-compliant server wants the box as latitude then longitude; ask again that way.
            if got == 0 && start == 0 && !lat_first {
                lat_first = true;
                continue;
            }
            if !gml::normalize_lon_lat(&mut rows) && !grid_warned {
                grid_warned = true;
                host.notify(&text::register_grid_answer(lang, &service.name));
            }
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

/// Downloads one typed dataset for a unit named by its code, then cuts it to the unit.
///
/// This is the one-call query: a gmina and a dataset label answer a layer of that type, clipped to
/// the unit's boundary, with no area layer and no second command.
pub fn typed_for_unit(host: &mut Host, params: &Value, lang: Lang) -> Result<Value, String> {
    let label = params
        .get("dane")
        .or_else(|| params.get("dataset"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| text::typed_needs_dataset(lang))?;
    let code = params
        .get("unit")
        .or_else(|| params.get("jednostka"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| text::unit_not_chosen(lang))?;
    let (unit, level) = crate::jednostki::resolve(code)
        .ok_or_else(|| text::unit_unknown(lang, code))?;
    let layer = catalog::wfs_by_label(label).ok_or_else(|| text::unknown_dataset(lang, label))?;

    // The checkbox in the panel decides whether the layer is cut; it is on unless turned off.
    let clip = crate::cache::settings().clip_unit;
    let area = crate::prg::area(host, &json!({ "nazwa": unit, "poziom": level, "kod": code }), lang)?;
    let boundary = area
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
    let bbox = [box_lon[0], box_lon[1], box_lon[2], box_lon[3]];

    let mut notes = Vec::new();
    let mut report = |line: &str| notes.push(line.to_string());
    // The fetch budget counts bytes, as the download tools count it: the saved limit in megabytes.
    let limit = (crate::cache::settings().limit_mb.max(1) * 1_000_000) as u64;
    // The typed download takes the panel's tick too, so a big unit is one decision, not a refusal.
    let cap = if crate::cache::settings().confirm_large {
        wfs::HARD_TOTAL
    } else {
        wfs::MAX_TOTAL
    };
    let download = wfs::fetch(layer, bbox, None, None, &mut report, lang, limit, cap)?;
    if download.features.is_empty() {
        return Err(text::typed_empty(lang, label, &unit));
    }
    if download.truncated {
        host.notify(&text::typed_capped(lang, download.features.len()));
    }

    // The unit's ring cuts the box result, unless the panel's checkbox was turned off.
    let name = text::typed_layer(lang, label, &unit, &level);
    let fetched = download.features.len() as u64;
    let geojson = json!({ "type": "FeatureCollection", "features": download.features });
    host.add_layer(&name, geojson)?;
    let mut kept = fetched;
    if clip {
        kept = match crate::clip::to_area(host, &name, &boundary, lang) {
            Ok(answer) => answer
                .get("features")
                .and_then(Value::as_u64)
                .unwrap_or(fetched),
            Err(error) => {
                host.notify(&text::typed_clip_failed(lang, &error));
                fetched
            }
        };
        // A feature that only touches the boundary comes out of the clip with no area, so it goes.
        let solid = drop_shavings(host, &name, lang).unwrap_or(kept);
        if solid < kept {
            host.notify(&text::typed_shavings(lang, kept - solid));
        }
        kept = solid;
    } else {
        host.notify(&text::typed_uncut(lang));
    }
    host.notify(&text::typed_added(lang, &name, kept, &unit, &level));
    Ok(json!({
        "layer": name,
        "unit": unit,
        "level": level,
        "dataset": label,
        "features": kept,
        "clipped": clip,
    }))
}

/// Rewrites the layer without features whose clipped geometry has no area, and answers the count.
fn drop_shavings(host: &mut Host, layer: &str, lang: Lang) -> Result<u64, String> {
    let answer = host.call("host.features", json!({ "layer": layer }))?;
    let features = answer
        .get("features")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let total = features.len() as u64;
    let solid: Vec<Value> = features
        .into_iter()
        .filter(|feature| match feature.get("geometry") {
            // A point or a line has no area and always stays: only a cut polygon can become a sliver.
            Some(geometry) => match geometry.get("type").and_then(Value::as_str) {
                Some("Polygon") | Some("MultiPolygon") => ring_area(geometry) > MIN_AREA,
                _ => true,
            },
            None => true,
        })
        .collect();
    if solid.len() as u64 == total {
        return Ok(total);
    }
    host.call("layer.remove", json!({ "layer": [layer] }))?;
    let collection = json!({ "type": "FeatureCollection", "features": solid });
    host.add_layer(layer, collection)?;
    let _ = lang;
    Ok(solid_count(&solid))
}

/// The count of the features that stayed, for the message.
fn solid_count(features: &[Value]) -> u64 {
    features.len() as u64
}

/// A geometry's area in square degrees, from the shoelace over its exterior rings.
fn ring_area(geometry: &Value) -> f64 {
    let kind = geometry.get("type").and_then(Value::as_str).unwrap_or_default();
    let coordinates = geometry.get("coordinates").cloned().unwrap_or(Value::Null);
    let polygons: Vec<&Value> = match kind {
        "Polygon" => vec![&coordinates],
        "MultiPolygon" => coordinates
            .as_array()
            .map(|list| list.iter().collect())
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    let mut area = 0.0;
    for polygon in polygons {
        let Some(ring) = polygon.get(0).and_then(Value::as_array) else {
            continue;
        };
        let mut points: Vec<[f64; 2]> = Vec::with_capacity(ring.len());
        for point in ring {
            let Some(pair) = point.as_array() else { continue };
            if let (Some(x), Some(y)) = (pair.first().and_then(Value::as_f64), pair.get(1).and_then(Value::as_f64)) {
                points.push([x, y]);
            }
        }
        area += shoelace(&points).abs();
    }
    area
}

/// The signed area of a ring, halved, which is zero for a line that ran along the boundary.
fn shoelace(points: &[[f64; 2]]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    for index in 0..points.len() {
        let [x0, y0] = points[index];
        let [x1, y1] = points[(index + 1) % points.len()];
        sum += x0 * y1 - x1 * y0;
    }
    sum / 2.0
}

/// Downloads features from any WFS the register lists, by type, for the area.
///
/// The county command is this one with the service picked for you: here the caller names the
/// register id, so the 1 554 WFS addresses the register proves can be read as layers too.
pub fn from_register(host: &mut Host, params: &Value, lang: Lang) -> Result<Value, String> {
    let wanted = params
        .get("usluga")
        .or_else(|| params.get("service"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| text::register_download_needs_service(lang))?;
    let service = zrodla::find(wanted)
        .or_else(|| {
            zrodla::all()
                .iter()
                .find(|entry| entry.kind == "wfs" && entry.url == wanted)
        })
        .ok_or_else(|| text::register_unknown(lang, wanted))?;
    if service.kind != "wfs" {
        return Err(text::register_not_a_download(lang, &service.name));
    }
    let (area, features) = area_features(host, params, lang)?;
    let box_lon = lon_lat_bounds(&features).ok_or_else(|| text::area_no_geometry(lang, &area))?;
    let wanted_types = params
        .get("typy")
        .or_else(|| params.get("types"))
        .and_then(Value::as_str)
        .map(|list| {
            list.split(',')
                .map(|part| part.trim().to_lowercase())
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    // A stored type answers without capabilities, so the long tail of municipal services is cheap.
    let stored = if wanted_types.is_empty() {
        zrodla::stored_type(&service.url)
    } else {
        None
    };
    let mut types: Vec<String> = match stored {
        Some(type_name) => vec![type_name],
        None => {
            let body = http::fetch(
                &format!("{}?SERVICE=WFS&REQUEST=GetCapabilities", service.url),
                90,
                lang,
            )?;
            capability_types(&body)
        }
    };
    if !wanted_types.is_empty() {
        types.retain(|name| {
            let lower = name.to_lowercase();
            wanted_types.iter().any(|want| lower.contains(want.as_str()))
        });
    }
    types.truncate(8);
    if types.is_empty() {
        return Err(text::register_no_types(lang, &service.name));
    }

    let cap = params
        .get("limit")
        .and_then(Value::as_u64)
        .map(|limit| (limit as usize).clamp(1, MAX_FEATURES))
        .unwrap_or(MAX_FEATURES);
    let mut collection: Vec<Value> = Vec::new();
    let mut grid_warned = false;
    for type_name in &types {
        let mut start = 0usize;
        let mut lat_first = false;
        loop {
            let body = fetch_page(&service.url, type_name, box_lon, PAGE, start, lang, lat_first)?;
            let mut rows = gml::parse_features(&body, false, lang)?;
            let got = rows.len();
            // A WFS-compliant server wants the box as latitude then longitude; ask again that way.
            if got == 0 && start == 0 && !lat_first {
                lat_first = true;
                continue;
            }
            if !gml::normalize_lon_lat(&mut rows) && !grid_warned {
                grid_warned = true;
                host.notify(&text::register_grid_answer(lang, &service.name));
            }
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
        return Err(text::register_no_features(lang, &service.name));
    }
    let name = text::register_layer(lang, &service.name);
    let capped = collection.len() >= cap;
    let geojson = json!({ "type": "FeatureCollection", "features": collection });
    host.add_layer(&name, geojson)?;
    host.notify(&text::register_added(lang, &name, collection.len()));
    if capped {
        host.notify(&text::egib_capped(lang, cap));
    }
    Ok(json!({
        "layer": name,
        "service": service.url,
        "types": types,
        "features": collection.len(),
        "capped": capped,
    }))
}

/// The area layer the request names, else the one the map holds.
pub(crate) fn area_features(host: &mut Host, params: &Value, lang: Lang) -> Result<(String, Vec<Value>), String> {
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
pub(crate) fn county_at(lon: f64, lat: f64, lang: Lang) -> Result<(String, String), String> {
    unit_at("GetCountyByXY", "teryt,county", lon, lat, lang)
}

/// The gmina a point falls in, as ULDK gives it: the TERYT with its type suffix and the name.
pub(crate) fn commune_at(lon: f64, lat: f64, lang: Lang) -> Result<(String, String), String> {
    unit_at("GetCommuneByXY", "teryt,commune", lon, lat, lang)
}

fn unit_at(
    request: &str,
    result: &str,
    lon: f64,
    lat: f64,
    lang: Lang,
) -> Result<(String, String), String> {
    let url = format!("{ULDK}?request={request}&xy={lon:.6},{lat:.6},4326&result={result}");
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
pub(crate) fn capability_types(body: &str) -> Vec<String> {
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

/// The same page asked in WFS 1.1.0, for the servers that refuse 2.0.0.
fn page_url_v1(
    url: &str,
    type_name: &str,
    box_lon: [f64; 4],
    count: usize,
    start: usize,
    lat_first: bool,
) -> String {
    let box_text = box_text(box_lon, lat_first);
    format!(
        "{url}?SERVICE=WFS&VERSION=1.1.0&REQUEST=GetFeature&TYPENAME={}&SRSNAME=EPSG:4326&MAXFEATURES={count}&STARTINDEX={start}&BBOX={box_text}",
        encode(type_name)
    )
}

/// A page of one type: WFS 2.0.0 first, then 1.1.0, because register servers vary.
fn fetch_page(
    url: &str,
    type_name: &str,
    box_lon: [f64; 4],
    count: usize,
    start: usize,
    lang: Lang,
    lat_first: bool,
) -> Result<String, String> {
    match http::fetch(&page_url(url, type_name, box_lon, count, start, lat_first), 110, lang) {
        Ok(body) => Ok(body),
        Err(_) => http::fetch(&page_url_v1(url, type_name, box_lon, count, start, lat_first), 110, lang),
    }
}

/// A box in the order the request wants: longitude first, or latitude first for a WFS-compliant server.
fn box_text(box_lon: [f64; 4], lat_first: bool) -> String {
    let [west, south, east, north] = box_lon;
    if lat_first {
        format!("{south:.6},{west:.6},{north:.6},{east:.6},EPSG:4326")
    } else {
        format!("{west:.6},{south:.6},{east:.6},{north:.6},EPSG:4326")
    }
}

/// The `GetFeature` address for one type and page, bounded by the area.
pub(crate) fn page_url(
    url: &str,
    type_name: &str,
    box_lon: [f64; 4],
    count: usize,
    start: usize,
    lat_first: bool,
) -> String {
    let box_text = box_text(box_lon, lat_first);
    format!(
        "{url}?SERVICE=WFS&VERSION=2.0.0&REQUEST=GetFeature&TYPENAMES={}&SRSNAME=EPSG:4326&COUNT={count}&STARTINDEX={start}&BBOX={box_text}",
        encode(type_name)
    )
}

/// The box around every feature, in lon and lat.
pub(crate) fn lon_lat_bounds(features: &[Value]) -> Option<[f64; 4]> {
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
pub(crate) fn encode(text: &str) -> String {
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
        let url = page_url("https://x/wfs", "gmgml:dzialki", [16.9, 52.4, 17.0, 52.5], 1000, 2000, false);
        assert!(url.contains("TYPENAMES=gmgml:dzialki"), "{url}");
        assert!(url.contains("BBOX=16.900000,52.400000,17.000000,52.500000,EPSG:4326"));
        assert!(url.contains("COUNT=1000"));
        assert!(url.contains("STARTINDEX=2000"));
        let swapped = page_url("https://x/wfs", "gmgml:dzialki", [16.9, 52.4, 17.0, 52.5], 1000, 2000, true);
        assert!(swapped.contains("BBOX=52.400000,16.900000,52.500000,17.000000,EPSG:4326"), "{swapped}");
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

#[cfg(test)]
mod typed_tests {
    use super::{ring_area, shoelace};

    #[test]
    fn a_touching_polygon_has_no_area() {
        let line = serde_json::json!({"type": "Polygon", "coordinates": [[[0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [0.0, 0.0]]]});
        assert!(ring_area(&line) < 1e-12);
        let square = serde_json::json!({"type": "Polygon", "coordinates": [[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.0, 0.0]]]});
        assert!((ring_area(&square) - 1.0).abs() < 1e-9);
        assert!(shoelace(&[]).abs() < 1e-12);
    }
}
