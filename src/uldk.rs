//! ULDK: Poland's parcel locator.
//!
//! `GetParcelByXY` takes `x,y` in EPSG:2180, so the map centre is projected first and the
//! returned WKT is projected back. The answer is one status line followed by the requested
//! fields, pipe-separated, the last one the geometry.

use crate::gml;
use crate::host::{self, Host};
use crate::http;
use crate::text::{self, Lang};
use crate::tm;
use serde_json::{json, Map, Value};

const ULDK: &str = "https://uldk.gugik.gov.pl/";

/// The fields asked of ULDK, for a parcel found by a point or by its identifier.
const FIELDS: &str = "parcel,voivodeship,county,commune,teryt,geom_wkt";

/// The parcel under a point: `x`/`y` in PL-1992, or the map centre.
pub fn parcel(host: &mut Host, params: &Value, lang: Lang) -> Result<Value, String> {
    let given = (
        params.get("x").and_then(Value::as_f64),
        params.get("y").and_then(Value::as_f64),
    );
    let (easting, northing, asked) = match given {
        (Some(x), Some(y)) => (x, y, tm::to_wgs84(x, y)),
        _ => {
            let (lon, lat) = host.centre()?;
            let (easting, northing) = tm::to_2180(lon, lat);
            (easting, northing, [lon, lat])
        }
    };
    let url = format!(
        "{ULDK}?request=GetParcelByXY&xy={easting:.2},{northing:.2}&srid=2180&result={FIELDS}"
    );
    let body = http::get(&url, 20, lang)?;
    answer(host, &body, lang, Some(asked))
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

/// Checks a CSV of parcel identifiers: one layer with what ULDK found, and a count of what it did not.
pub fn parcels_csv(request: &Value, lang: Lang) -> Result<Value, String> {
    let params = request.get("params").cloned().unwrap_or(Value::Null);
    let body = crate::uug::file_text(request, &params, lang)?;
    let column = params
        .get("kolumna")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty());
    let limit = params
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(200)
        .clamp(1, 2000) as usize;
    let rows = crate::uug::read_csv(&body, column);
    if rows.is_empty() {
        return Err(text::csv_empty(lang, "CSV"));
    }
    let total = rows.len().min(limit);
    let mut features = Vec::new();
    let mut missing = Vec::new();
    for (index, (id, extra)) in rows.iter().take(total).enumerate() {
        let url = format!("{ULDK}?request=GetParcelById&id={id}&srid=2180&result={FIELDS}");
        match http::fetch(&url, 20, lang) {
            Ok(answer) if answer.starts_with('0') => match answer_body(&answer, lang) {
                Ok((_fields, wkt)) => {
                    let Some(geometry) = gml::wkt_to_geojson(&wkt, tm::to_wgs84) else {
                        missing.push(json!(id));
                        continue;
                    };
                    let mut properties = Map::new();
                    properties.insert("id".into(), json!(id));
                    for (key, value) in extra {
                        properties.insert(key.clone(), value.clone());
                    }
                    features.push(json!({
                        "type": "Feature",
                        "geometry": geometry,
                        "properties": Value::Object(properties),
                    }));
                }
                Err(_) => missing.push(json!(id)),
            },
            _ => missing.push(json!(id)),
        }
        if index % 20 == 19 {
            host::progress_line(&text::uldk_progress(lang, index + 1, total));
        }
    }
    if features.is_empty() {
        return Err(text::uldk_none_found(lang, total));
    }
    let name = text::uldk_csv_layer(lang);
    let collection = json!({ "type": "FeatureCollection", "features": features });
    let note = text::uldk_csv_done(lang, features.len(), missing.len());
    Ok(json!({
        "layers": [{ "name": name, "geojson": collection }],
        "messages": [note],
    }))
}

/// ULDK's answer as its fields and its WKT, without building a layer.
fn answer_body(body: &str, lang: Lang) -> Result<(Vec<String>, String), String> {
    parse(body, lang)
}

/// Turns ULDK's answer into a layer, with the point that was asked about beside it.
fn answer(host: &mut Host, body: &str, lang: Lang, asked: Option<[f64; 2]>) -> Result<Value, String> {
    let (fields, wkt) = parse(body, lang)?;

    let mut properties = Map::new();
    for (name, value) in ["parcel", "voivodeship", "county", "commune", "teryt"].iter().zip(fields.iter()) {
        properties.insert((*name).into(), json!(value));
    }
    if let Some([lon, lat]) = asked {
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
    let first = answer.chars().next().ok_or_else(|| text::uldk_empty(lang))?;
    if !first.is_ascii_digit() {
        // A body with no status digit is a proxy or HTML page, shown as it came.
        let line = answer.split_whitespace().collect::<Vec<_>>().join(" ");
        let snippet: String = line.chars().take(100).collect();
        return Err(text::uldk_message(&snippet));
    }
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
    fn reports_an_html_page_verbatim() {
        let body = "<html>\n  <body>502 Bad Gateway</body>\n</html>";
        let err = parse(body, Lang::En).unwrap_err();
        assert!(err.starts_with("ULDK: <html>"));
        assert!(err.contains("502 Bad Gateway"));
    }

    #[test]
    fn names_the_layer_in_the_requested_language() {
        assert_eq!(text::uldk_layer(Lang::En, "139/1"), "Parcel 139/1");
        assert_eq!(text::uldk_layer(Lang::Pl, "139/1"), "Działka 139/1");
    }
}
