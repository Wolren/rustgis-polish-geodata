//! Two GUGiK point services: the address geocoder (UUG) and the terrain model's height.
//!
//! Both are asked in PL-1992 metres, so a point given as longitude and latitude is projected
//! first and the answer is projected back. Neither is cached: an address is looked up because the
//! user wants it now, and a height is a property of the terrain, not of a document.

use crate::host::Host;
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

    let found = answer
        .get("results")
        .and_then(Value::as_object)
        .and_then(|results| results.values().next())
        .ok_or_else(|| text::uug_no_address(lang, address))?;
    let number = |key: &str| {
        found
            .get(key)
            .and_then(Value::as_str)
            .and_then(|v| v.parse::<f64>().ok())
    };
    let (x, y) = (
        number("x").ok_or_else(|| text::uug_unreadable(lang))?,
        number("y").ok_or_else(|| text::uug_unreadable(lang))?,
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
    Ok(json!({ "layer": name, "lon": lon, "lat": lat, "x": x, "y": y }))
}

/// Reads the terrain model's height at a point, in metres above sea level.
pub fn height(host: &mut Host, params: &Value, lang: Lang) -> Result<Value, String> {
    let (x, y) = point(host, params)?;
    let url = format!("{NMT}?request=GetHByXY&x={x:.2}&y={y:.2}");
    let body = http::fetch(&url, 20, lang)?;
    let answer = body.trim();
    let metres: f64 = answer
        .parse()
        .map_err(|_| text::height_failed(lang, &answer.chars().take(80).collect::<String>()))?;
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

    #[test]
    fn encodes_a_polish_address() {
        assert_eq!(
            encode("Poznań, ul. Głogowska 10"),
            "Pozna%C5%84%2C%20ul.%20G%C5%82ogowska%2010"
        );
        assert_eq!(encode("abc-1_2.3~"), "abc-1_2.3~");
    }

    #[test]
    fn reads_a_height() {
        let metres: f64 = "214.3".trim().parse().unwrap();
        assert_eq!(metres, 214.3);
    }
}
