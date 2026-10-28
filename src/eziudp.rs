//! EZiUDP: the national register of spatial data sets and services.
//!
//! Every public body reports what it holds to the Główny Geodeta Kraju, and the register answers
//! `api.php` as JSON: the dataset, the body that reports it, the unit that holds it, and the
//! addresses of its CSW, WMS/WMTS, WFS/WCS and download services. This is the one place that knows
//! about *all* Polish data, so it is what an agent asks when the built-in catalogue is not enough.
//!
//! The register answers with everything it has when it is given no filter: 12 000 records and nine
//! megabytes. A query therefore has to narrow it, and this command says so instead of downloading
//! the register.

use crate::http;
use crate::text::{self, Lang};
use serde_json::{json, Value};

const API: &str = "https://integracja.gugik.gov.pl/eziudp/api.php";
/// Rows kept in the answer. The count of all matches comes with them.
const MAX_ROWS: usize = 50;

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
    if dataset.is_none() && body.is_none() && teryt.is_none() {
        return Err(text::eziudp_needs_query(lang));
    }

    let mut url = format!("{API}?format=json");
    for (key, value) in [
        ("zbior", dataset.as_deref()),
        ("nazwa", body.as_deref()),
        ("teryt", teryt.as_deref()),
        ("usluga", text_of("usluga").as_deref()),
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

    let entries: Vec<Value> = rows
        .iter()
        .take(MAX_ROWS)
        .map(|row| {
            let services = row.get("uslugi").cloned().unwrap_or(Value::Null);
            json!({
                "id": row.get("idiip").cloned().unwrap_or(Value::Null),
                "dataset": row.get("zbior").cloned().unwrap_or(Value::Null),
                "institution": row.get("organ").cloned().unwrap_or(Value::Null),
                "unit": row.get("jednostka").cloned().unwrap_or(Value::Null),
                "area": row.get("obszar").cloned().unwrap_or(Value::Null),
                "teryt": row.get("teryt").cloned().unwrap_or(Value::Null),
                "wms": service(&services, "przegladania"),
                "wfs": service(&services, "pobierania"),
                "csw": service(&services, "wyszukiwania"),
            })
        })
        .collect();

    Ok(json!({
        "total": total,
        "returned": entries.len(),
        "entries": entries,
        "how": "run_command  plugin:dane-pl/usluga  {\"service\": \"<name>\"} adds a WMS by its catalogue name",
    }))
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
        .map(|address| {
            let address = address
                .strip_prefix("WMS=")
                .or_else(|| address.strip_prefix("WFS="))
                .or_else(|| address.strip_prefix("CSW="))
                .unwrap_or(address);
            json!(address.trim())
        })
        .unwrap_or(Value::Null)
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
        let error = search(&json!({}), Lang::En).unwrap_err();
        assert!(error.contains("narrow"), "{error}");
    }

    #[test]
    fn takes_the_first_address_of_each_service() {
        let services = json!({"przegladania": ["https://a", "https://b"], "pobierania": []});
        assert_eq!(service(&services, "przegladania"), json!("https://a"));
        assert_eq!(service(&services, "pobierania"), Value::Null);
    }
}
