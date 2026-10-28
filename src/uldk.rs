//! ULDK: Poland's parcel locator.
//!
//! `GetParcelByXY` takes `x,y` in EPSG:2180, so the map centre is projected first and the
//! returned WKT is projected back. The answer is one status line followed by the requested
//! fields, pipe-separated, the last one the geometry.

use crate::gml;
use crate::host::Host;
use crate::http;
use crate::text::{self, Lang};
use crate::tm;
use serde_json::{json, Map, Value};

const ULDK: &str = "https://uldk.gugik.gov.pl/";

/// The fields asked of ULDK, for a parcel found by a point or by its identifier.
const FIELDS: &str = "parcel,voivodeship,county,commune,teryt,geom_wkt";

/// The parcel under a point: `x`/`y` in PL-1992, or the map centre.
pub fn parcel(host: &mut Host, params: &Value, lang: Lang) -> Result<Value, String> {
    let (lon, lat) = match (params.get("x").and_then(Value::as_f64), params.get("y").and_then(Value::as_f64)) {
        (Some(x), Some(y)) => (x, y),
        _ => host.centre()?,
    };
    let (easting, northing) = tm::to_2180(lon, lat);
    let url = format!(
        "{ULDK}?request=GetParcelByXY&xy={easting:.2},{northing:.2}&srid=2180&result={FIELDS}"
    );
    let body = http::get(&url, 20, lang)?;
    answer(host, &body, lang, Some((lon, lat)))
}

/// The parcel an identifier names: `id` as ULDK writes it (`306401_1.0001.14/4`), or `teryt`,
/// `obreb`, `numer` and `arkusz` to build it.
pub fn parcel_by_id(host: &mut Host, params: &Value, lang: Lang) -> Result<Value, String> {
    let id = identifier(params, lang)?;
    let url = format!("{ULDK}?request=GetParcelById&id={id}&srid=2180&result={FIELDS}");
    let body = http::get(&url, 20, lang)?;
    answer(host, &body, lang, None)
}

/// ULDK's identifier for the parcel: the one given, or `TERYT_R.OBREB.NUMER/ARKUSZ`.
fn identifier(params: &Value, lang: Lang) -> Result<String, String> {
    let field = |key: &str| {
        params
            .get(key)
            .map(|value| match value {
                Value::String(s) => s.trim().to_string(),
                Value::Number(n) => n.to_string(),
                _ => String::new(),
            })
            .filter(|s| !s.is_empty())
    };
    if let Some(id) = field("id") {
        return Ok(id);
    }
    let teryt = field("teryt").ok_or_else(|| text::uldk_needs_id(lang))?;
    let numer = field("numer").ok_or_else(|| text::uldk_needs_id(lang))?;
    let obreb = field("obreb").unwrap_or_else(|| "0001".to_string());
    let arkusz = field("arkusz").unwrap_or_else(|| "1".to_string());
    let rodzaj = field("rodzaj").unwrap_or_else(|| "1".to_string());
    Ok(format!("{teryt}_{rodzaj}.{obreb}.{numer}/{arkusz}"))
}

/// Turns ULDK's answer into a layer, with the point that was asked about beside it.
fn answer(host: &mut Host, body: &str, lang: Lang, asked: Option<(f64, f64)>) -> Result<Value, String> {
    let (fields, wkt) = parse(body, lang)?;

    let mut properties = Map::new();
    for (name, value) in ["parcel", "voivodeship", "county", "commune", "teryt"].iter().zip(fields.iter()) {
        properties.insert((*name).into(), json!(value));
    }
    if let Some((lon, lat)) = asked {
        properties.insert("queryPoint".into(), json!([lon, lat]));
    }

    let geometry = gml::wkt_to_geojson(&wkt, tm::to_wgs84).ok_or_else(|| text::uldk_bad_geometry(lang))?;
    let feature = json!({
        "type": "Feature",
        "geometry": geometry,
        "properties": Value::Object(properties),
    });
    let collection = json!({ "type": "FeatureCollection", "features": [feature] });

    let parcel = fields.first().cloned().unwrap_or_default();
    let name = text::uldk_layer(lang, &parcel);
    host.add_layer(&name, collection)?;
    host.call("layer.zoomTo", json!({ "layer": name }))?;
    host.notify(&text::uldk_found(lang, &parcel));
    Ok(json!({ "layer": name, "parcel": parcel }))
}

/// Splits ULDK's answer into its fields and its WKT geometry.
fn parse(body: &str, lang: Lang) -> Result<(Vec<String>, String), String> {
    let answer = body.trim();
    let mut chars = answer.char_indices();
    let (_, first) = chars.next().ok_or_else(|| text::uldk_empty(lang))?;
    if first != '0' {
        let (_, message) = answer.split_once(' ').unwrap_or((answer, answer));
        return Err(text::uldk_message(message.trim()));
    }
    let rest = answer[1..].trim_start();
    if rest.is_empty() {
        return Err(text::uldk_no_parcel(lang));
    }
    // The geometry comes last; everything before it is pipe-separated fields.
    let geometry_at = rest
        .find("SRID=")
        .or_else(|| rest.find("POLYGON"))
        .or_else(|| rest.find("MULTIPOLYGON"))
        .ok_or_else(|| text::uldk_no_geometry(lang))?;
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
        let (fields, wkt) = parse(body, Lang::En).unwrap();
        assert_eq!(fields[0], "139/1");
        assert_eq!(fields[4], "146501_1");
        assert!(wkt.starts_with("SRID=2180;POLYGON"));
    }

    #[test]
    fn reports_an_error_line() {
        let err = parse("-1 brak wyników", Lang::En).unwrap_err();
        assert!(err.contains("brak wyników"));
    }

    #[test]
    fn names_the_layer_in_the_requested_language() {
        assert_eq!(text::uldk_layer(Lang::En, "139/1"), "Parcel 139/1");
        assert_eq!(text::uldk_layer(Lang::Pl, "139/1"), "Działka 139/1");
    }
}
