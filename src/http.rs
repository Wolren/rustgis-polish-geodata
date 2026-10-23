//! Small HTTP helper: a GET with a timeout and a name RustGIS's users can trace.

use std::io::Read;
use std::time::Duration;

const UA: &str = "dane-pl/0.4 (RustGIS plugin)";
/// The largest document one request may bring back. A page of 500 communes is a few MB.
const MAX_BYTES: u64 = 64 * 1024 * 1024;

pub fn get(url: &str, timeout_secs: u64) -> Result<String, String> {
    let response = ureq::get(url)
        .timeout(Duration::from_secs(timeout_secs))
        .set("User-Agent", UA)
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(code, _) => format!("HTTP {code} z {}", short(url)),
            ureq::Error::Transport(t) => format!("{}: {t}", short(url)),
        })?;

    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("nie udało się odczytać odpowiedzi: {e}"))?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(format!("odpowiedź z {} jest zbyt duża", short(url)));
    }
    String::from_utf8(bytes).map_err(|_| format!("odpowiedź z {} nie jest tekstem UTF-8", short(url)))
}

fn short(url: &str) -> &str {
    url.split('?').next().unwrap_or(url)
}
