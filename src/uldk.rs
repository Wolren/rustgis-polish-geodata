//! ULDK: Poland's parcel locator.
//!
//! `GetParcelByXY` takes `x,y` in EPSG:2180, so the map centre is projected first and the
//! returned WKT is projected back. The answer is one status line followed by the requested
//! fields, pipe-separated, the last one the geometry.

use crate::gml;
use crate::host::Host;
use crate::http;
use crate::tm;
use serde_json::{json, Map, Value};

const ULDK: &str = "https://uldk.gugik.gov.pl/";

pub fn parcel(host: &mut Host, params: &Value) -> Result<Value, String> {
    let (lon, lat) = match (params.get("x").and_then(Value::as_f64), params.get("y").and_then(Value::as_f64)) {
        (Some(x), Some(y)) => (x, y),
        _ => host.centre()?,
    };
    let (easting, northing) = tm::to_2180(lon, lat);
    let url = format!(
        "{ULDK}?request=GetParcelByXY&xy={easting:.2},{northing:.2}&srid=2180&\
result=parcel,voivodeship,county,commune,teryt,geom_wkt"
    );
    let body = http::get(&url, 20)?;
    let (fields, wkt) = parse(&body)?;

    let mut properties = Map::new();
    for (name, value) in ["parcel", "voivodeship", "county", "commune", "teryt"].iter().zip(fields.iter()) {
        properties.insert((*name).into(), json!(value));
    }
    properties.insert("queryPoint".into(), json!([lon, lat]));

    let geometry = gml::wkt_to_geojson(&wkt, tm::to_wgs84)
        .ok_or_else(|| "ULDK returned a geometry that could not be read".to_string())?;
    let feature = json!({
        "type": "Feature",
        "geometry": geometry,
        "properties": Value::Object(properties),
    });
    let collection = json!({ "type": "FeatureCollection", "features": [feature] });

    let parcel = fields.first().cloned().unwrap_or_default();
    let name = if parcel.is_empty() { "Działka (ULDK)".to_string() } else { format!("Działka {parcel}") };
    host.add_layer(&name, collection)?;
    host.call("layer.zoomTo", json!({ "layer": name }))?;
    host.notify(&format!("ULDK: działka {parcel}"));
    Ok(json!({ "layer": name, "parcel": parcel }))
}

/// Splits ULDK's answer into its fields and its WKT geometry.
fn parse(body: &str) -> Result<(Vec<String>, String), String> {
    let text = body.trim();
    let mut chars = text.char_indices();
    let (_, first) = chars.next().ok_or_else(|| "ULDK returned nothing".to_string())?;
    if first != '0' {
        let message = text.splitn(2, ' ').nth(1).unwrap_or(text).trim();
        return Err(format!("ULDK: {message}"));
    }
    let rest = text[1..].trim_start();
    if rest.is_empty() {
        return Err("ULDK returned no parcel at this point".into());
    }
    // The geometry comes last; everything before it is pipe-separated fields.
    let geometry_at = rest
        .find("SRID=")
        .or_else(|| rest.find("POLYGON"))
        .or_else(|| rest.find("MULTIPOLYGON"))
        .ok_or_else(|| "ULDK returned no geometry".to_string())?;
    let fields_part = rest[..geometry_at].trim_end_matches('|').trim();
    let wkt = rest[geometry_at..].trim().to_string();
    let fields: Vec<String> = fields_part.split('|').map(|f| f.trim().to_string()).collect();
    Ok((fields, wkt))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_parcel_answer() {
        let body = "0\n139/1|mazowieckie|Warszawa|Warszawa|146501_1|SRID=2180;POLYGON((637343 485978,637320 485979,637303 485981,637343 485978))";
        let (fields, wkt) = parse(body).unwrap();
        assert_eq!(fields[0], "139/1");
        assert_eq!(fields[4], "146501_1");
        assert!(wkt.starts_with("SRID=2180;POLYGON"));
    }

    #[test]
    fn reports_an_error_line() {
        let err = parse("-1 brak wyników").unwrap_err();
        assert!(err.contains("brak wyników"));
    }
}
