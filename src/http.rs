//! Small HTTP helper: a GET with a timeout and a name RustGIS's users can trace.
//!
//! GUGiK's servers drop a part of the connections this machine opens (measured: schannel closes
//! without a close_notify about one time in three), so a transport failure is tried again.

use crate::cache;
use crate::text::{self, Lang};
use std::io::Read;
use std::time::Duration;

const UA: &str = concat!("dane-pl/", env!("CARGO_PKG_VERSION"), " (RustGIS plugin)");
/// The largest document one request may bring back. A page of 500 communes is a few MB.
const MAX_BYTES: u64 = 64 * 1024 * 1024;
/// A coverage is never smaller than its header, so a short answer is an error page.
const MIN_COVERAGE_BYTES: u64 = 1024;
/// One failed connection is retried this many times, with a pause that grows.
const ATTEMPTS: u32 = 3;

/// Why one GET failed. A transport failure is worth another attempt; an HTTP status is not.
enum Failure {
    Status(u16),
    Transport(String),
    TooBig,
    NotUtf8,
}

/// A GET, answered from the on-disk cache when a fresh copy exists.
pub fn get(url: &str, timeout_secs: u64, lang: Lang) -> Result<String, String> {
    if let Some(cached) = cache::get(url) {
        return Ok(cached);
    }
    let body = fetch(url, timeout_secs, lang)?;
    cache::put(url, &body);
    Ok(body)
}

/// A GET that always reaches the server, bypassing the cache.
pub fn fetch(url: &str, timeout_secs: u64, lang: Lang) -> Result<String, String> {
    let mut attempt = 0;
    loop {
        match request(url, timeout_secs) {
            Ok(body) => return Ok(body),
            Err(Failure::Status(code)) => return Err(text::http_status(lang, code, short(url))),
            Err(Failure::TooBig) => return Err(text::http_too_big(lang, short(url))),
            Err(Failure::NotUtf8) => return Err(text::http_not_utf8(lang, short(url))),
            Err(Failure::Transport(detail)) => {
                if attempt + 1 >= ATTEMPTS {
                    return Err(text::http_transport(short(url), &detail));
                }
                attempt += 1;
                std::thread::sleep(Duration::from_millis(400 * u64::from(attempt)));
            }
        }
    }
}

fn request(url: &str, timeout_secs: u64) -> Result<String, Failure> {
    let response = ureq::get(url)
        .timeout(Duration::from_secs(timeout_secs))
        .set("User-Agent", UA)
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(code, _) => Failure::Status(code),
            ureq::Error::Transport(t) => Failure::Transport(t.to_string()),
        })?;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Failure::Transport(e.to_string()))?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(Failure::TooBig);
    }
    String::from_utf8(bytes).map_err(|_| Failure::NotUtf8)
}

/// Streams a coverage to a file, so 20 or 99 MB never sits in memory.
///
/// The bytes land in `.part` and take the real name only when the answer is complete: the next
/// click reuses any file that already exists, so a half-written one would be a wrong model.
pub fn download(url: &str, path: &std::path::Path, timeout_secs: u64, lang: Lang) -> Result<u64, String> {
    let mut last = String::new();
    for attempt in 0..ATTEMPTS {
        match stream(url, path, timeout_secs, lang) {
            Ok(bytes) => return Ok(bytes),
            Err(error) => {
                last = error;
                if attempt + 1 < ATTEMPTS {
                    std::thread::sleep(Duration::from_secs(u64::from(attempt) + 1));
                }
            }
        }
    }
    Err(last)
}

fn stream(url: &str, path: &std::path::Path, timeout_secs: u64, lang: Lang) -> Result<u64, String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| text::file_failed(lang, &parent.display().to_string(), &e.to_string()))?;
    }
    let response = ureq::get(url)
        .timeout(Duration::from_secs(timeout_secs))
        .set("User-Agent", UA)
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(code, _) => text::http_status(lang, code, short(url)),
            ureq::Error::Transport(t) => text::http_transport(short(url), &t.to_string()),
        })?;

    let part = path.with_extension("part");
    let mut file = std::fs::File::create(&part)
        .map_err(|e| text::file_failed(lang, &part.display().to_string(), &e.to_string()))?;
    let copied = std::io::copy(&mut response.into_reader(), &mut file)
        .map_err(|e| text::http_read_failed(lang, &e.to_string()))?;
    if copied < MIN_COVERAGE_BYTES {
        let _ = std::fs::remove_file(&part);
        return Err(text::download_empty(lang, short(url)));
    }
    std::fs::rename(&part, path)
        .map_err(|e| text::file_failed(lang, &path.display().to_string(), &e.to_string()))?;
    Ok(copied)
}

fn short(url: &str) -> &str {
    url.split('?').next().unwrap_or(url)
}
