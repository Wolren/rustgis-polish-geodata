//! EZiUDP: the national register of spatial data sets and services.
//!
//! Every public body reports what it holds to the Główny Geodeta Kraju, and the register answers
//! `api.php` as JSON: the dataset, the body that reports it, the unit that holds it, and the
//! addresses of its CSW, WMS/WMTS, WFS/WCS and download services. This is the one place that knows
//! about *all* Polish data, so it is what an agent asks when the built-in catalogue is not enough.
//!
//! The register answers with everything it has when it is given no filter: 12 000 records and nine
//! megabytes. A query therefore has to narrow it, and this command says so instead of downloading
//! the register. A narrowed query comes back whole: the register ignores `limit` and `index`, and
//! keeps no row out.

use crate::http;
use crate::text::{self, Lang};
use serde_json::{json, Value};

const API: &str = "https://integracja.gugik.gov.pl/eziudp/api.php";

/// Searches the register: by dataset (`szukaj`), by the body that reports it (`instytucja`), by
/// TERYT, or by the kind of service (`usluga`: `wms`, `wfs`, `csw`…).
pub fn search(params: &Value, lang: Lang) -> Result<Value, String> {
    let text_of = |key: &str| {
        params
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let dataset = text_of("szukaj");
    let body = text_of("instytucja");
    let teryt = text_of("teryt");
    let usluga = text_of("usluga");
    // A service kind alone matches almost the whole register, so it never counts as a filter.
    if dataset.is_none() && body.is_none() && teryt.is_none() {
        return Err(text::eziudp_needs_query(lang));
    }

    let mut url = format!("{API}?format=json");
    for (key, value) in [
        ("zbior", dataset.as_deref()),
        ("nazwa", body.as_deref()),
        ("teryt", teryt.as_deref()),
        ("usluga", usluga.as_deref()),
    ] {
        if let Some(value) = value {
            url.push_str(&format!("&{key}={}", encode(value)));
        }
    }

    let answer = http::fetch(&url, 60, lang)?;
    let register: Value =
        serde_json::from_str(&answer).map_err(|_| text::eziudp_unreadable(lang))?;
    let rows = register
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let total = register
        .get("metadata")
        .and_then(|m| m.get("count"))
        .and_then(Value::as_u64)
        .unwrap_or(rows.len() as u64);

    let entries = project(&rows);

    // An empty narrowed answer is not a fault: say so instead of leaving a bare zero.
    let note = entries.is_empty().then(|| {
        let query = [text_of("szukaj"), text_of("instytucja"), text_of("teryt")]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(", ");
        text::eziudp_nothing(lang, &query)
    });
    Ok(json!({
        "total": total,
        "returned": entries.len(),
        "entries": entries,
        "note": note,
        "how": "run_command  plugin:dane-pl/usluga  {\"service\": \"<name>\"} adds a WMS by its catalogue name",
    }))
}

/// One entry per register row: the dataset, the body that reports it and its service addresses.
fn project(rows: &[Value]) -> Vec<Value> {
    rows.iter()
        .map(|row| {
            let services = row.get("uslugi").cloned().unwrap_or(Value::Null);
            let (wfs, wfs_protocol) = wfs_endpoint(&services);
            json!({
                "id": row.get("idiip").cloned().unwrap_or(Value::Null),
                "dataset": row.get("zbior").cloned().unwrap_or(Value::Null),
                "institution": row.get("organ").cloned().unwrap_or(Value::Null),
                "unit": row.get("jednostka").cloned().unwrap_or(Value::Null),
                "area": row.get("obszar").cloned().unwrap_or(Value::Null),
                "teryt": row.get("teryt").cloned().unwrap_or(Value::Null),
                "wms": service(&services, "przegladania"),
                "wfs": wfs,
                "wfs_protocol": wfs_protocol,
                "csw": service(&services, "wyszukiwania"),
            })
        })
        .collect()
}

/// The first address of one service kind, which is what a click or a script can use.
///
/// The register writes some of them as `WMS=http://…`, naming the protocol in the value; a caller
/// wants the address itself.
fn service(services: &Value, kind: &str) -> Value {
    services
        .get(kind)
        .and_then(Value::as_array)
        .and_then(|list| list.first())
        .and_then(Value::as_str)
        .map(|raw| json!(split(raw).0))
        .unwrap_or(Value::Null)
}

/// An address split from its protocol prefix, with the protocol the prefix names.
fn split(raw: &str) -> (&str, Option<&'static str>) {
    if let Some((head, rest)) = raw.split_once('=') {
        for protocol in ["WFS", "WMS", "CSW", "WCS"] {
            if head.eq_ignore_ascii_case(protocol) {
                return (rest.trim(), Some(protocol));
            }
        }
    }
    (raw.trim(), None)
}

/// Whether an address is a WFS endpoint, by prefix or by shape.
fn is_wfs((address, protocol): &(&str, Option<&'static str>)) -> bool {
    let lower = address.to_ascii_lowercase();
    if !lower.starts_with("http") {
        return false;
    }
    match protocol {
        Some(name) => *name == "WFS",
        None => lower.contains("wfs"),
    }
}

/// The first WFS download address, with the protocol it was published under.
fn wfs_endpoint(services: &Value) -> (Value, Value) {
    let found = services
        .get("pobierania")
        .and_then(Value::as_array)
        .and_then(|list| list.iter().filter_map(Value::as_str).map(split).find(is_wfs));
    match found {
        Some((address, _)) => (json!(address), json!("WFS")),
        None => (Value::Null, Value::Null),
    }
}

/// Percent-encodes a query value; Polish names carry diacritics.
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
    fn refuses_to_download_the_whole_register() {
        for params in [json!({}), json!({"usluga": "wms"})] {
            let error = search(&params, Lang::En).unwrap_err();
            assert!(error.contains("narrow"), "{error}");
        }
    }

    #[test]
    fn takes_the_first_address_of_each_service() {
        let services = json!({"przegladania": ["https://a", "https://b"], "pobierania": []});
        assert_eq!(service(&services, "przegladania"), json!("https://a"));
        assert_eq!(service(&services, "pobierania"), Value::Null);
    }

    #[test]
    fn takes_a_wfs_address_not_a_wcs_one() {
        let services = json!({"pobierania": [
            "https://x/PZGIK/ORTO/WCS/StandardResolution",
            "WFS=https://x/wfs",
        ]});
        let (address, protocol) = wfs_endpoint(&services);
        assert_eq!(address, json!("https://x/wfs"));
        assert_eq!(protocol, json!("WFS"));
    }

    /// A filtered query comes back whole: the register ignores limit and index.
    #[test]
    fn keeps_every_row_the_register_sent() {
        let rows: Vec<Value> = (0..120)
            .map(|n| json!({"idiip": n, "zbior": format!("zbiór {n}"), "organ": "Starosta"}))
            .collect();
        let entries = project(&rows);
        assert_eq!(entries.len(), 120);
        assert_eq!(entries[119]["dataset"], json!("zbiór 119"));
    }

    #[test]
    fn finds_a_wfs_endpoint_by_the_address_shape() {
        let services =
            json!({"pobierania": ["https://x/wss/service/PZGIK/ORTO/WFS/Skorowidze"]});
        assert_eq!(
            wfs_endpoint(&services).0,
            json!("https://x/wss/service/PZGIK/ORTO/WFS/Skorowidze")
        );
    }

    #[test]
    fn leaves_the_wfs_field_empty_without_a_wfs_endpoint() {
        let services = json!({"pobierania": [
            "https://x/PZGIK/ORTO/WCS/StandardResolution",
            "WCS=https://x/wcs",
            "ftp://x/archive.zip",
        ]});
        assert_eq!(wfs_endpoint(&services), (Value::Null, Value::Null));
    }
}
