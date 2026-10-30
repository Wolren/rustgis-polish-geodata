//! EGiB straight from the county: parcels and buildings for the area on screen.
//!
//! GUGiK's national KIEG service only rasterises parcels, so nothing in it can be selected or
//! filtered by a boundary. Every county runs its own WFS EGiB and registers it in EZiUDP, and
//! that WFS answers GeoJSON for a bounding box. This module walks the chain:
//!
//! 1. ULDK `GetCountyByXY` names the county under the middle of the view (`1465|Warszawa`).
//! 2. EZiUDP gives the county's own WFS address for the EGiB dataset.
//! 3. Its capabilities name the feature types — `wfs:dzialki`, `wfs:budynki`.
//! 4. One `GetFeature` per type, bounded by the view and capped, becomes a layer.
//!
//! Nothing is downloaded county-wide: `service.add` would drag up to 50 000 objects of the whole
//! county, while a bounded request answers only what is on screen. A county that answers no
//! GeoJSON cannot be drawn by RustGIS at all, and the error says so instead of adding an empty
//! layer.

use crate::host::Host;
use crate::http;
use crate::text::{self, Lang};
use quick_xml::events::Event;
use quick_xml::Reader;
use serde_json::{json, Value};

const ULDK: &str = "https://uldk.gugik.gov.pl/";
const EZIUDP: &str = "https://integracja.gugik.gov.pl/eziudp/api.php";
/// Above this scale the view covers more than a district's worth of parcels.
const MAX_SCALE: f64 = 25_000.0;
/// Features per type in one request; a 1 km view at 1:1000 holds about 400 parcels.
const MAX_FEATURES: usize = 5_000;
/// The output formats asked of a WFS, in order: GeoServer takes the first, MapServer the second.
const FORMATS: &[&str] = &["application/json", "json"];

/// The county's own WFS, with what its capabilities say it publishes.
struct Service {
    url: String,
    version: String,
    types: Vec<String>,
    area: String,
}

/// The parcel and building layers of the visible area.
pub fn from_view(host: &mut Host, lang: Lang) -> Result<Value, String> {
    let view = host.call("host.view", json!({}))?;
    let scale = view
        .get("scale")
        .and_then(Value::as_f64)
        .unwrap_or(f64::INFINITY);
    if scale > MAX_SCALE {
        return Err(text::egib_scale(lang, scale, MAX_SCALE));
    }

    let extent = host.extent()?;
    let (lon, lat) = ((extent[0] + extent[2]) / 2.0, (extent[1] + extent[3]) / 2.0);
    let (teryt, county) = county_at(lon, lat, lang)?;
    let service = service_for(&teryt, &county, lang)?;

    let parcels = pick(&service.types, "dzialk");
    let buildings = pick(&service.types, "budynk");
    if parcels.is_none() && buildings.is_none() {
        return Err(text::egib_no_layers(lang, &service.area));
    }

    let mut added: Vec<Value> = Vec::new();
    let mut counts = [0usize; 2];
    let mut truncated = false;
    for (index, (kind, type_name)) in [("parcels", parcels), ("buildings", buildings)]
        .into_iter()
        .enumerate()
    {
        let Some(type_name) = type_name else {
            continue;
        };
        let (features, cut) = features_for(&service, &type_name, extent, lang)?;
        truncated |= cut;
        let collection = json!({ "type": "FeatureCollection", "features": features });
        let name = text::egib_layer(lang, kind == "parcels", &service.area);
        host.add_layer(&name, collection)?;
        counts[index] = features.len();
        added.push(json!({ "layer": name, "type": type_name, "features": counts[index] }));
    }

    // A layer added to an empty map pulls the view to itself; the user asked about this view.
    host.call("map.zoomToExtent", json!({ "extent": extent }))?;
    host.notify(&text::egib_added(
        lang, counts[0], counts[1], &service.area, truncated,
    ));

    Ok(json!({
        "county": county,
        "teryt": teryt,
        "wfs": service.url,
        "layers": added,
        "truncated": truncated,
    }))
}

/// The county under a point: ULDK answers a status line, then `teryt|name`.
pub(crate) fn county_at(lon: f64, lat: f64, lang: Lang) -> Result<(String, String), String> {
    let url = format!("{ULDK}?request=GetCountyByXY&xy={lon:.6},{lat:.6},4326&result=teryt,county");
    let body = http::get(&url, 20, lang)?;
    parse_county(&body).ok_or_else(|| text::egib_no_county(lang, body.trim()))
}

/// Reads ULDK's county answer; `None` when it is an error line or not a county.
fn parse_county(body: &str) -> Option<(String, String)> {
    let mut lines = body.lines().map(str::trim).filter(|line| !line.is_empty());
    if lines.next()? != "0" {
        return None;
    }
    let row = lines.next()?;
    let (teryt, name) = row.split_once('|').unwrap_or((row, ""));
    let teryt = teryt.trim();
    if teryt.len() != 4 || !teryt.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((teryt.to_string(), name.trim().to_string()))
}

/// The county's own EGiB WFS, from the national register, with its version and feature types.
fn service_for(teryt: &str, county: &str, lang: Lang) -> Result<Service, String> {
    let url = format!("{EZIUDP}?format=json&teryt={teryt}&usluga=wfs");
    let body = http::get(&url, 60, lang)?;
    let register: Value = serde_json::from_str(&body).map_err(|_| text::eziudp_unreadable(lang))?;
    let record = egib_record(&register).ok_or_else(|| text::egib_no_wfs(lang, county, teryt))?;
    let address = record
        .get("uslugi")
        .and_then(|services| services.get("pobierania"))
        .and_then(Value::as_array)
        .and_then(|list| list.first())
        .and_then(Value::as_str)
        .map(strip_protocol)
        .ok_or_else(|| text::egib_no_wfs(lang, county, teryt))?;
    let area = record
        .get("obszar")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .unwrap_or(county)
        .to_string();
    let (version, types) = capabilities(&address, lang)?;
    if types.is_empty() {
        return Err(text::egib_no_layers(lang, &area));
    }
    Ok(Service { url: address, version, types, area })
}

/// The register writes some addresses as `WFS=http://…`, naming the protocol in the value.
fn strip_protocol(address: &str) -> String {
    let address = address.trim();
    for prefix in ["WFS=", "WMS=", "CSW="] {
        if let Some(rest) = address.strip_prefix(prefix) {
            return rest.trim().to_string();
        }
    }
    address.to_string()
}

/// The EGiB record among the register's rows for one county.
fn egib_record(register: &Value) -> Option<&Value> {
    register
        .get("data")
        .and_then(Value::as_array)?
        .iter()
        .find(|row| {
            row.get("zbior")
                .and_then(Value::as_str)
                .is_some_and(|dataset| dataset.to_lowercase().contains("ewidencja grunt"))
        })
}

/// Fetches the capabilities and reads the version and the type names out of them.
fn capabilities(address: &str, lang: Lang) -> Result<(String, Vec<String>), String> {
    let mut url = address.to_string();
    url.push(if url.contains('?') { '&' } else { '?' });
    url.push_str("SERVICE=WFS&REQUEST=GetCapabilities");
    let body = http::get(&url, 60, lang)?;
    parse_capabilities(&body).ok_or_else(|| text::egib_unreadable(lang, address))
}

/// The WFS version and its feature types; `None` when the document is not capabilities.
pub fn parse_capabilities(body: &str) -> Option<(String, Vec<String>)> {
    let mut reader = Reader::from_str(body);
    reader.config_mut().trim_text(true);
    let mut version = String::from("2.0.0");
    let mut types = Vec::new();
    let mut in_type = false;
    let mut reading_name = false;
    let mut name = String::new();
    let mut found = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                let local = element.local_name().as_ref().to_vec();
                if local.eq_ignore_ascii_case(b"wfs_capabilities") {
                    found = true;
                    for attribute in element.attributes().flatten() {
                        if attribute.key.local_name().as_ref().eq_ignore_ascii_case(b"version") {
                            if let Ok(value) = attribute.unescape_value() {
                                let value = value.trim().to_string();
                                if !value.is_empty() {
                                    version = value;
                                }
                            }
                        }
                    }
                } else if local.eq_ignore_ascii_case(b"featuretype") {
                    in_type = true;
                } else if in_type && local.eq_ignore_ascii_case(b"name") {
                    reading_name = true;
                    name.clear();
                }
            }
            Ok(Event::Text(text)) if reading_name => {
                name.push_str(&text.unescape().map(|c| c.into_owned()).unwrap_or_default());
            }
            Ok(Event::End(element)) => {
                let local = element.local_name().as_ref().to_vec();
                if local.eq_ignore_ascii_case(b"name") && reading_name {
                    reading_name = false;
                    let name = name.trim().to_string();
                    if !name.is_empty() {
                        types.push(name);
                    }
                } else if local.eq_ignore_ascii_case(b"featuretype") {
                    in_type = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
    }
    if !found {
        return None;
    }
    Some((version, types))
}

/// The first type whose name carries the word: `dzialk` finds `wfs:dzialki`.
fn pick(types: &[String], word: &str) -> Option<String> {
    types
        .iter()
        .find(|name| name.to_lowercase().contains(word))
        .cloned()
}

/// The features of one type inside the view.
///
/// Three answers are tried, because a county picks one of them: GeoJSON (what RustGIS draws
/// straight away), GML in the national grid, and GML in degrees. GeoJSON is only offered by about
/// 163 of the 380 counties; the rest publish GML, and the parser written for the national WFS
/// catalogue reads it.
fn features_for(
    service: &Service,
    type_name: &str,
    bbox: [f64; 4],
    lang: Lang,
) -> Result<(Vec<Value>, bool), String> {
    let version = if service.version.is_empty() {
        "2.0.0".to_string()
    } else {
        service.version.clone()
    };

    let mut last = String::new();
    for format in FORMATS {
        let url = getfeature_url(&service.url, &version, type_name, bbox, MAX_FEATURES, Some(format));
        // Live cadastral data: never answered from the on-disk cache.
        let body = match http::fetch(&url, 120, lang) {
            Ok(body) => body,
            Err(error) => {
                last = error;
                continue;
            }
        };
        if let Some(collection) = as_collection(&body) {
            let features = collection
                .get("features")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let truncated = features.len() >= MAX_FEATURES;
            return Ok((features, truncated));
        }
        last = body.chars().take(200).collect();
    }

    // GML in degrees: WFS 1.1 and 2.0 read EPSG:4326 as latitude, longitude; 1.0 reads it as
    // longitude, latitude.
    let url = getfeature_url(&service.url, &version, type_name, bbox, MAX_FEATURES, None);
    if let Ok(body) = http::fetch(&url, 120, lang) {
        let swap = !version.starts_with("1.0");
        if let Ok(features) = crate::gml::parse_features(&body, swap, lang) {
            let count = features.len();
            if count > 0 {
                return Ok((features, count >= MAX_FEATURES));
            }
        }
        last = body.chars().take(200).collect();
    }

    // GML in the national grid: a county that answers nothing in degrees usually answers here.
    let url = grid_url(&service.url, &version, type_name, bbox, MAX_FEATURES);
    if let Ok(body) = http::fetch(&url, 120, lang) {
        if let Ok(mut features) = crate::gml::parse_features(&body, false, lang) {
            let count = features.len();
            if count > 0 {
                crate::wfs::grid_to_lon_lat(&mut features);
                return Ok((features, count >= MAX_FEATURES));
            }
        }
        last = body.chars().take(200).collect();
    }

    Err(text::egib_not_geojson(lang, &service.area, &last))
}

/// The feature collection in an answer, when the answer is one.
fn as_collection(body: &str) -> Option<Value> {
    let value: Value = serde_json::from_str(body).ok()?;
    if value.get("features").and_then(Value::as_array).is_some() {
        Some(value)
    } else {
        None
    }
}

/// The GetFeature request: the view's bounding box, one page, GeoJSON.
///
/// WFS 2.0 names types with `TYPENAMES` and limits with `COUNT`; 1.x use `TYPENAME` and
/// `MAXFEATURES`. GML and WFS 1.1 read an EPSG:4326 box as latitude, longitude — the URN says so
/// explicitly — while 1.0 reads longitude, latitude.
pub fn getfeature_url(
    address: &str,
    version: &str,
    type_name: &str,
    bbox: [f64; 4],
    limit: usize,
    format: Option<&str>,
) -> String {
    let separator = if address.contains('?') { '&' } else { '?' };
    let two = version.starts_with('2');
    let type_key = if two { "TYPENAMES" } else { "TYPENAME" };
    let limit_key = if two { "COUNT" } else { "MAXFEATURES" };
    let box_value = if version.starts_with("1.0") {
        format!("{:.6},{:.6},{:.6},{:.6}", bbox[0], bbox[1], bbox[2], bbox[3])
    } else {
        format!(
            "{:.6},{:.6},{:.6},{:.6},urn:ogc:def:crs:EPSG::4326",
            bbox[1], bbox[0], bbox[3], bbox[2]
        )
    };
    // A county server may refuse an OUTPUTFORMAT it does not list; without one, WFS answers GML,
    // which is the format every service must support.
    let format = match format {
        Some(format) => format!("&OUTPUTFORMAT={format}"),
        None => String::new(),
    };
    format!(
        "{address}{separator}SERVICE=WFS&VERSION={version}&REQUEST=GetFeature&{type_key}={type_name}\
&SRSNAME=EPSG:4326{format}&{limit_key}={limit}&BBOX={box_value}"
    )
}

/// The same request in PL-1992 metres, for a county that filters only in the national grid.
///
/// WFS 2.0 reads EPSG:2180 in its own axis order, northing before easting, and only a URN makes
/// that explicit; 1.x take easting, northing with the plain code. Both were measured against a
/// county MapServer: the other combination answers an empty collection.
pub fn grid_url(
    address: &str,
    version: &str,
    type_name: &str,
    bbox: [f64; 4],
    limit: usize,
) -> String {
    let separator = if address.contains('?') { '&' } else { '?' };
    let two = version.starts_with('2');
    let type_key = if two { "TYPENAMES" } else { "TYPENAME" };
    let limit_key = if two { "COUNT" } else { "MAXFEATURES" };
    let (x1, y1) = crate::gk92::forward(bbox[0], bbox[1]);
    let (x2, y2) = crate::gk92::forward(bbox[2], bbox[3]);
    let (west, south) = (x1.min(x2), y1.min(y2));
    let (east, north) = (x1.max(x2), y1.max(y2));
    let (srs, box_value) = if two {
        (
            "urn:ogc:def:crs:EPSG::2180",
            format!("{south:.2},{west:.2},{north:.2},{east:.2}"),
        )
    } else {
        ("EPSG:2180", format!("{west:.2},{south:.2},{east:.2},{north:.2}"))
    };
    format!(
        "{address}{separator}SERVICE=WFS&VERSION={version}&REQUEST=GetFeature&{type_key}={type_name}\
&SRSNAME={srs}&{limit_key}={limit}&BBOX={box_value}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_county_under_a_point() {
        assert_eq!(
            parse_county("\n0\n1465|Warszawa\n"),
            Some(("1465".to_string(), "Warszawa".to_string()))
        );
        assert_eq!(parse_county("-1 brak wyników"), None);
        assert_eq!(parse_county("0\n146|Warszawa"), None);
    }

    #[test]
    fn asks_in_the_national_grid_when_a_county_has_no_degrees() {
        let url = grid_url("https://x/wfs", "2.0.0", "wfs:dzialki", [21.0, 52.2, 21.01, 52.21], 500);
        assert!(url.contains("SRSNAME=urn:ogc:def:crs:EPSG::2180"));
        assert!(url.contains("COUNT=500"));
        // WFS 2.0 reads the grid northing first: Warsaw is around easting 637 000, northing 483 000.
        let bbox = url.split("BBOX=").nth(1).unwrap();
        let parts: Vec<f64> = bbox.split(',').map(|p| p.parse().unwrap()).collect();
        assert!((482_000.0..486_000.0).contains(&parts[0]), "{bbox}");
        assert!((636_000.0..638_000.0).contains(&parts[1]), "{bbox}");
        assert!(parts[2] > parts[0] && parts[3] > parts[1], "{bbox}");

        // WFS 1.x reads the plain code easting first.
        let one = grid_url("https://x/wfs", "1.1.0", "ms:dzialki", [21.0, 52.2, 21.01, 52.21], 500);
        assert!(one.contains("SRSNAME=EPSG:2180"));
        assert!(one.contains("MAXFEATURES=500"));
        let bbox = one.split("BBOX=").nth(1).unwrap();
        let parts: Vec<f64> = bbox.split(',').map(|p| p.parse().unwrap()).collect();
        assert!((636_000.0..638_000.0).contains(&parts[0]), "{bbox}");
        assert!((482_000.0..486_000.0).contains(&parts[1]), "{bbox}");
    }

    #[test]
    fn reads_the_types_out_of_capabilities() {
        let xml = r#"<?xml version="1.0"?>
<wfs:WFS_Capabilities version="2.0.0" xmlns:wfs="http://www.opengis.net/wfs/2.0">
  <wfs:FeatureTypeList>
    <wfs:FeatureType><wfs:Name>wfs:dzialki</wfs:Name></wfs:FeatureType>
    <wfs:FeatureType><wfs:Name>wfs:budynki</wfs:Name></wfs:FeatureType>
  </wfs:FeatureTypeList>
</wfs:WFS_Capabilities>"#;
        let (version, types) = parse_capabilities(xml).expect("capabilities");
        assert_eq!(version, "2.0.0");
        assert_eq!(types, vec!["wfs:dzialki", "wfs:budynki"]);
        assert_eq!(pick(&types, "dzialk").as_deref(), Some("wfs:dzialki"));
        assert_eq!(pick(&types, "budynk").as_deref(), Some("wfs:budynki"));
        assert!(pick(&types, "adres").is_none());
    }

    #[test]
    fn refuses_an_answer_that_is_not_capabilities() {
        assert!(parse_capabilities("<html><body>404</body></html>").is_none());
        assert!(parse_capabilities("{\"error\": \"no\"}").is_none());
    }

    #[test]
    fn builds_a_wfs_2_request_in_latitude_first() {
        let url = getfeature_url(
            "https://wms2.um.warszawa.pl/geoserver/wfs/wfs",
            "2.0.0",
            "wfs:dzialki",
            [21.00, 52.20, 21.01, 52.21],
            5000,
            Some("application/json"),
        );
        assert!(url.contains("TYPENAMES=wfs:dzialki"));
        assert!(url.contains("COUNT=5000"));
        assert!(url.contains("SRSNAME=EPSG:4326"));
        assert!(url.contains("OUTPUTFORMAT=application/json"));
        // WFS 2.0 reads EPSG:4326 as latitude, longitude.
        assert!(url.contains("BBOX=52.200000,21.000000,52.210000,21.010000,urn:ogc:def:crs:EPSG::4326"));
    }

    #[test]
    fn builds_a_wfs_1_0_request_in_longitude_first() {
        let url = getfeature_url("https://x/wfs?", "1.0.0", "dzialki", [21.0, 52.2, 21.01, 52.21], 10, Some("json"));
        assert!(url.contains("TYPENAME=dzialki"));
        assert!(url.contains("MAXFEATURES=10"));
        assert!(url.contains("BBOX=21.000000,52.200000,21.010000,52.210000"));
        // The address already had a query string: the parameters must not start a new one.
        assert!(url.contains("wfs?&SERVICE=WFS"));
    }

    #[test]
    fn takes_the_egib_row_out_of_the_register() {
        let register = json!({"data": [
            {"zbior": "Ewidencja Miejscowosci, Ulic i Adresow (EMUiA)", "uslugi": {"pobierania": ["https://a"]}},
            {"zbior": "Ewidencja Gruntow i Budynkow (EGiB)", "obszar": "miasto stoleczne Warszawa",
             "uslugi": {"pobierania": ["WFS=https://wms2.um.warszawa.pl/geoserver/wfs/wfs"]}}
        ]});
        let record = egib_record(&register).expect("EGiB row");
        assert_eq!(record["obszar"], "miasto stoleczne Warszawa");
        assert_eq!(
            strip_protocol("WFS=https://wms2.um.warszawa.pl/geoserver/wfs/wfs"),
            "https://wms2.um.warszawa.pl/geoserver/wfs/wfs"
        );
    }
}
