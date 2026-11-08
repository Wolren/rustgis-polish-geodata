//! Two GUGiK point services: the address geocoder (UUG) and the terrain model's height.
//!
//! Both are asked in PL-1992 metres, so a point given as longitude and latitude is projected
//! first and the answer is projected back. Neither is cached: an address is looked up because the
//! user wants it now, and a height is a property of the terrain, not of a document.

use crate::catalog;
use crate::host::{self, Host};
use crate::http;
use crate::text::{self, Lang};
use crate::tm;
use serde_json::{json, Map, Value};

const UUG: &str = "https://services.gugik.gov.pl/uug/";
const NMT: &str = "https://services.gugik.gov.pl/nmt/";

/// The point a request asks about: `x`/`y` in PL-1992, else the map centre.
fn point(host: &mut Host, params: &Value) -> Result<(f64, f64), String> {
    match (
        params.get("x").and_then(Value::as_f64),
        params.get("y").and_then(Value::as_f64),
    ) {
        (Some(x), Some(y)) => Ok((x, y)),
        _ => {
            let (lon, lat) = host.centre()?;
            Ok(tm::to_2180(lon, lat))
        }
    }
}

/// One address as UUG answers it: the point in lon and lat, and the properties it names.
pub fn locate(address: &str, lang: Lang) -> Result<(f64, f64, Map<String, Value>), String> {
    let url = format!("{UUG}?request=GetAddress&address={}", encode(address));
    let body = http::fetch(&url, 25, lang)?;
    let answer: Value = serde_json::from_str(&body).map_err(|_| text::uug_unreadable(lang))?;
    let candidates: Vec<&Value> = answer
        .get("results")
        .and_then(Value::as_object)
        .map(|results| results.values().collect())
        .unwrap_or_default();
    if candidates.is_empty() {
        return Err(text::uug_no_address(lang, address));
    }
    let found = choose(&candidates, None);
    let x = found.get("x").and_then(number).ok_or_else(|| text::uug_unreadable(lang))?;
    let y = found.get("y").and_then(number).ok_or_else(|| text::uug_unreadable(lang))?;
    let [lon, lat] = tm::to_wgs84(x, y);
    let mut properties = Map::new();
    for (name, key) in [
        ("city", "city"),
        ("street", "street"),
        ("number", "number"),
        ("postcode", "code"),
        ("teryt", "teryt"),
        ("accuracy", "accuracy"),
        ("jednostka", "jednostka"),
    ] {
        if let Some(value) = found.get(key).cloned() {
            properties.insert(name.into(), value);
        }
    }
    properties.insert("query".into(), json!(address));
    properties.insert("pl1992".into(), json!([x, y]));
    Ok((lon, lat, properties))
}

/// Geocodes the addresses of a CSV file into one point layer, as a tool.
pub fn geocode_csv(request: &Value, lang: Lang) -> Result<Value, String> {
    let params = request.get("params").cloned().unwrap_or(Value::Null);
    let body = file_text(request, &params, lang)?;
    let name = params
        .get("file")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .unwrap_or("file")
        .to_string();
    let limit = params
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(500)
        .clamp(1, 5000) as usize;
    let column = params
        .get("kolumna")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty());

    let rows = read_csv(&body, column);
    if rows.is_empty() {
        return Err(text::csv_empty(lang, &name));
    }
    let total = rows.len().min(limit);
    let mut features = Vec::new();
    let mut failed = 0usize;
    for (index, (address, extra)) in rows.iter().take(total).enumerate() {
        match locate(address, lang) {
            Ok((lon, lat, mut properties)) => {
                properties.insert("adres".into(), json!(address));
                for (key, value) in extra {
                    properties.insert(key.clone(), value.clone());
                }
                features.push(json!({
                    "type": "Feature",
                    "geometry": { "type": "Point", "coordinates": [lon, lat] },
                    "properties": Value::Object(properties),
                }));
            }
            Err(_) => failed += 1,
        }
        if index % 25 == 24 {
            host::progress_line(&text::csv_progress(lang, index + 1, total));
        }
    }
    if features.is_empty() {
        return Err(text::csv_all_failed(lang, total));
    }
    let name = text::csv_layer(lang);
    let collection = json!({ "type": "FeatureCollection", "features": features });
    let note = text::csv_done(lang, features.len(), failed);
    Ok(json!({
        "layers": [{ "name": name, "geojson": collection }],
        "messages": [note],
    }))
}

/// The CSV text the run carries: the host puts the file's bytes in `files`, a path still reads.
fn file_text(request: &Value, params: &Value, lang: Lang) -> Result<String, String> {
    let name = params.get("file").and_then(Value::as_str).unwrap_or_default();
    let carried = request
        .get("files")
        .and_then(Value::as_object)
        .and_then(|files| files.get("file"))
        .and_then(|entry| entry.get("data"))
        .and_then(Value::as_str)
        .or_else(|| {
            params
                .get("file")
                .and_then(Value::as_object)
                .and_then(|entry| entry.get("data"))
                .and_then(Value::as_str)
        });
    if let Some(data) = carried {
        let bytes = base64_decode(data).ok_or_else(|| text::csv_bad_content(lang))?;
        return String::from_utf8(bytes).map_err(|_| text::csv_bad_content(lang));
    }
    // A path from a script or a test still opens the file itself.
    if !name.is_empty() && std::path::Path::new(name).is_file() {
        return std::fs::read_to_string(name).map_err(|e| text::csv_read_failed(lang, name, &e.to_string()));
    }
    Err(text::csv_needs_file(lang))
}

/// Standard base64 with padding, enough for the host's file encoding.
fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0u8;
    for byte in text.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b'\n' | b'\r' | b' ' => continue,
            _ => return None,
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// The addresses of a CSV: the column the caller names, else the first one that looks like an address.
fn read_csv(body: &str, column: Option<&str>) -> Vec<(String, Map<String, Value>)> {
    let mut lines = body.lines().filter(|line| !line.trim().is_empty());
    let Some(header) = lines.next() else {
        return Vec::new();
    };
    let separator = if header.matches(';').count() >= header.matches(',').count() {
        ';'
    } else {
        ','
    };
    let names: Vec<String> = header
        .split(separator)
        .map(|name| name.trim().trim_matches('"').to_lowercase())
        .collect();
    let wanted = column.map(|name| name.to_lowercase());
    let index = wanted
        .as_ref()
        .and_then(|name| names.iter().position(|cell| cell == name))
        .or_else(|| {
            names
                .iter()
                .position(|cell| cell.contains("adres") || cell == "address")
        })
        .unwrap_or(0);
    let mut rows = Vec::new();
    for line in lines {
        let cells: Vec<&str> = line.split(separator).map(|cell| cell.trim().trim_matches('"')).collect();
        let Some(address) = cells.get(index).map(|cell| cell.trim()).filter(|cell| !cell.is_empty()) else {
            continue;
        };
        let mut extra = Map::new();
        for (position, cell) in cells.iter().enumerate() {
            if position != index && !cell.is_empty() {
                extra.insert(names.get(position).cloned().unwrap_or_default(), json!(cell));
            }
        }
        rows.push((address.to_string(), extra));
    }
    rows
}

/// Finds one address and adds it as a point layer.
pub fn geocode(host: &mut Host, params: &Value, lang: Lang) -> Result<Value, String> {
    let address = params
        .get("adres")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| text::uug_needs_address(lang))?;
    let url = format!("{UUG}?request=GetAddress&address={}", encode(address));
    let body = http::fetch(&url, 25, lang)?;
    let answer: Value = serde_json::from_str(&body).map_err(|_| text::uug_unreadable(lang))?;

    let candidates: Vec<&Value> = answer
        .get("results")
        .and_then(Value::as_object)
        .map(|results| results.values().collect())
        .unwrap_or_default();
    if candidates.is_empty() {
        return Err(text::uug_no_address(lang, address));
    }
    let near = match (
        params.get("x").and_then(Value::as_f64),
        params.get("y").and_then(Value::as_f64),
    ) {
        (Some(x), Some(y)) => Some((x, y)),
        _ => None,
    };
    let found = choose(&candidates, near);
    let found_count = candidates.len();
    let (x, y) = (
        found.get("x").and_then(number).ok_or_else(|| text::uug_unreadable(lang))?,
        found.get("y").and_then(number).ok_or_else(|| text::uug_unreadable(lang))?,
    );
    let [lon, lat] = tm::to_wgs84(x, y);

    let mut properties = Map::new();
    for (name, key) in [
        ("city", "city"),
        ("street", "street"),
        ("number", "number"),
        ("postcode", "code"),
        ("teryt", "teryt"),
        ("accuracy", "accuracy"),
        ("jednostka", "jednostka"),
    ] {
        if let Some(value) = found.get(key).cloned() {
            properties.insert(name.into(), value);
        }
    }
    properties.insert("query".into(), json!(address));
    properties.insert("pl1992".into(), json!([x, y]));

    let point = json!({ "type": "Point", "coordinates": [lon, lat] });
    let collection = json!({
        "type": "FeatureCollection",
        "features": [{
            "type": "Feature",
            "geometry": point,
            "properties": Value::Object(properties),
        }],
    });
    let name = text::uug_layer(lang, address);
    host.add_layer(&name, collection)?;
    host.call("layer.zoomTo", json!({ "layer": name }))?;
    host.notify(&text::uug_found(lang, address, lon, lat));
    if found_count > 1 {
        host.notify(&text::uug_candidates(lang, found_count));
    }
    Ok(json!({
        "layer": name,
        "lon": lon,
        "lat": lat,
        "x": x,
        "y": y,
        "candidates": found_count,
    }))
}

/// Finds the address at a point and adds it as a point layer.
///
/// The point is the map centre unless `lon` and `lat` name one. The service answers in degrees,
/// so nothing is projected on the way in or out.
pub fn reverse(host: &mut Host, params: &Value, lang: Lang) -> Result<Value, String> {
    let (lon, lat) = match (
        params.get("lon").and_then(Value::as_f64),
        params.get("lat").and_then(Value::as_f64),
    ) {
        (Some(lon), Some(lat)) => (lon, lat),
        _ => host.centre()?,
    };
    let url =
        format!("{UUG}?request=GetAddressReverse&location=POINT({lon:.6}%20{lat:.6})&srid=4326");
    let body = http::fetch(&url, 20, lang)?;
    let answer: Value = serde_json::from_str(&body).map_err(|_| text::uug_unreadable(lang))?;
    // The service answers the nearest address within its own radius, or nothing.
    let found = answer
        .get("results")
        .and_then(Value::as_object)
        .and_then(|results| results.values().next())
        .ok_or_else(|| text::uug_no_address_here(lang))?;
    let (found_lon, found_lat) = (
        found.get("x").and_then(number).ok_or_else(|| text::uug_unreadable(lang))?,
        found.get("y").and_then(number).ok_or_else(|| text::uug_unreadable(lang))?,
    );
    let address = address_of(found);
    let distance = found.get("distance").and_then(number);

    let mut properties = Map::new();
    for (name, key) in [
        ("city", "city"),
        ("street", "street"),
        ("number", "number"),
        ("postcode", "code"),
        ("teryt", "teryt"),
        ("simc", "simc"),
        ("ulic", "ulic"),
    ] {
        if let Some(value) = found.get(key).cloned() {
            properties.insert(name.into(), value);
        }
    }
    properties.insert("address".into(), json!(address));
    if let Some(distance) = distance {
        properties.insert("distance_m".into(), json!(distance));
    }
    let (x, y) = tm::to_2180(found_lon, found_lat);
    properties.insert("pl1992".into(), json!([x, y]));

    let collection = json!({
        "type": "FeatureCollection",
        "features": [{
            "type": "Feature",
            "geometry": { "type": "Point", "coordinates": [found_lon, found_lat] },
            "properties": Value::Object(properties),
        }],
    });
    let name = text::uug_reverse_layer(lang, &address);
    host.add_layer(&name, collection)?;
    host.notify(&text::uug_here(lang, &address, distance));
    Ok(json!({
        "layer": name,
        "address": address,
        "lon": found_lon,
        "lat": found_lat,
        "distance": distance,
    }))
}

/// The address a reverse answer names: street and number, then the city.
fn address_of(found: &Value) -> String {
    let part = |key: &str| {
        found
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
    };
    let mut out = String::new();
    match (part("street"), part("number")) {
        (Some(street), Some(number)) => out.push_str(&format!("{street} {number}")),
        (Some(street), None) => out.push_str(street),
        (None, Some(number)) => out.push_str(number),
        (None, None) => {}
    }
    if let Some(city) = part("city") {
        if !out.is_empty() {
            out.push_str(", ");
        }
        out.push_str(city);
    }
    out
}

/// The candidate to show: nearest to a given PL-1992 point, else the most accurate.
fn choose<'a>(candidates: &[&'a Value], near: Option<(f64, f64)>) -> &'a Value {
    let coords = |value: &Value| {
        Some((
            value.get("x").and_then(number)?,
            value.get("y").and_then(number)?,
        ))
    };
    if let Some((x, y)) = near {
        let nearest = candidates
            .iter()
            .filter_map(|value| {
                let (px, py) = coords(value)?;
                Some((*value, (px - x).powi(2) + (py - y).powi(2)))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((value, _)) = nearest {
            return value;
        }
    }
    let accuracy = |value: &Value| value.get("accuracy").and_then(number).unwrap_or(0.0);
    candidates
        .iter()
        .copied()
        .max_by(|a, b| accuracy(a).total_cmp(&accuracy(b)))
        .unwrap_or(candidates[0])
}

/// A number sent as JSON text or a number, comma decimal allowed.
fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => metres_of(s),
        _ => None,
    }
}

/// A height written as text, with a comma decimal allowed.
fn metres_of(answer: &str) -> Option<f64> {
    answer.trim().replace(',', ".").parse().ok()
}

/// Reads the terrain model's height at a point, in metres above sea level.
pub fn height(host: &mut Host, params: &Value, lang: Lang) -> Result<Value, String> {
    let (x, y) = point(host, params)?;
    // The service answers 0 for a point outside the country, which reads as sea level.
    let [lon, lat] = tm::to_wgs84(x, y);
    let [west, south, east, north] = catalog::POLAND;
    if !(west..=east).contains(&lon) || !(south..=north).contains(&lat) {
        return Err(text::height_outside(lang));
    }
    let url = format!("{NMT}?request=GetHByXY&x={x:.2}&y={y:.2}");
    let body = http::fetch(&url, 20, lang)?;
    let answer = body.trim();
    let metres = metres_of(answer)
        .ok_or_else(|| text::height_failed(lang, &answer.chars().take(80).collect::<String>()))?;
    host.notify(&text::height_at(lang, metres, x, y));
    Ok(json!({ "height": metres, "x": x, "y": y, "datum": "PL-KRON86-NH" }))
}

/// Percent-encodes a query value: everything but the unreserved characters of RFC 3986.
fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 3);
    for byte in text.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(*byte as char)
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
    fn an_address_names_street_number_and_city() {
        let full = json!({"street": "ulica Żurawia", "number": "3/5", "city": "Warszawa"});
        assert_eq!(address_of(&full), "ulica Żurawia 3/5, Warszawa");
        // A point in the countryside answers with the city alone.
        assert_eq!(address_of(&json!({"city": "Marki"})), "Marki");
        // A street without a number keeps the street.
        assert_eq!(address_of(&json!({"street": "ulica Polna", "city": "Marki"})), "ulica Polna, Marki");
        assert_eq!(address_of(&json!({})), "");
    }

    #[test]
    fn encodes_a_polish_address() {
        assert_eq!(
            encode("Poznań, ul. Głogowska 10"),
            "Pozna%C5%84%2C%20ul.%20G%C5%82ogowska%2010"
        );
        assert_eq!(encode("abc-1_2.3~"), "abc-1_2.3~");
    }

    #[test]
    fn reads_a_height_with_either_decimal_comma() {
        assert_eq!(metres_of("214.3"), Some(214.3));
        assert_eq!(metres_of("214,3\n"), Some(214.3));
        assert_eq!(metres_of("no height"), None);
    }

    #[test]
    fn reads_numbers_written_as_text_or_numbers() {
        assert_eq!(number(&json!(359579.19)), Some(359579.19));
        assert_eq!(number(&json!("359579,19")), Some(359579.19));
        assert_eq!(number(&json!(null)), None);
    }

    fn candidate(x: f64, y: f64, accuracy: &str) -> Value {
        json!({"x": x, "y": y, "accuracy": accuracy})
    }

    #[test]
    fn chooses_the_most_accurate_candidate() {
        let rough = candidate(0.0, 0.0, "0.5");
        let exact = candidate(0.0, 0.0, "1");
        assert_eq!(choose(&[&rough, &exact], None), &exact);
    }

    #[test]
    fn chooses_the_candidate_nearest_a_given_point() {
        let far = candidate(500_000.0, 500_000.0, "1");
        let near = candidate(100.0, 100.0, "0.4");
        assert_eq!(choose(&[&far, &near], Some((0.0, 0.0))), &near);
    }
    /// CSV files as people make them: separators swapped, quotes, blank lines, no header.
    #[test]
    fn a_csv_of_any_shape_gives_addresses_or_nothing() {
        let mut rng = Lcg::new(13);
        let separators = [';', ','];
        let rows = ["Poznań, Głogowska 10", "Warszawa;Marszałkowska 1", "", "  ", "\"quoted, with comma\""];
        for _ in 0..200 {
            let separator = *rng.pick(&separators);
            let count = rng.below(6) as usize;
            let mut body = format!("adres{separator}nazwa\n");
            for _ in 0..count {
                body.push_str(*rng.pick(&rows));
                body.push_str(&separator.to_string());
                body.push_str("x\n");
            }
            let read = read_csv(&body, None);
            assert!(read.len() <= 6);
            for (address, _) in read {
                assert!(!address.trim().is_empty());
            }
        }
        assert!(read_csv("", None).is_empty());
    }

}
