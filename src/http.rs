//! Small HTTP helper: a GET with a timeout and a name RustGIS's users can trace.
//!
//! GUGiK's servers drop a part of the connections this machine opens (measured: schannel closes
//! without a close_notify about one time in three), so a transport failure is tried again.

use crate::cache;
use crate::text::{self, Lang};
use std::io::{Read, Write};
use std::time::Duration;

const UA: &str = concat!("polish-geodata/", env!("CARGO_PKG_VERSION"), " (RustGIS plugin)");
/// The largest document one request may bring back. A page of 500 communes is a few MB.
const MAX_BYTES: u64 = 64 * 1024 * 1024;
/// A coverage is never smaller than its header, so a short answer is an error page.
const MIN_COVERAGE_BYTES: u64 = 1024;
/// One failed connection is retried this many times, with a pause that grows.
const ATTEMPTS: u32 = 3;
/// An error body is read up to this many bytes to find the exception it carries.
const ERROR_BYTES: u64 = 8192;
/// An exception line longer than this is cut: its first sentence names the fault.
const ERROR_CHARS: usize = 200;

/// Why one GET failed. A transport failure is worth another attempt; an HTTP status is not.
///
/// A status carries the first useful line of its body, where WFS and WCS put the exception.
enum Failure {
    Status(u16, String),
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

/// A GET answered from the cache when fresh, else from the server; the caller stores the body.
pub fn get_unstored(url: &str, timeout_secs: u64, lang: Lang) -> Result<String, String> {
    match cache::get(url) {
        Some(cached) => Ok(cached),
        None => fetch(url, timeout_secs, lang),
    }
}

/// A GET that always reaches the server, bypassing the cache.
pub fn fetch(url: &str, timeout_secs: u64, lang: Lang) -> Result<String, String> {
    let mut attempt = 0;
    loop {
        match request(url, timeout_secs) {
            Ok(body) => return Ok(body),
            Err(Failure::Status(code, detail)) => {
                return Err(text::http_status(lang, code, short(url), &detail))
            }
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

/// One agent for the whole process, so a page of addresses reuses its connection.
///
/// A fresh agent per request costs a TLS handshake each time: a download of four pages pays four.
fn agent() -> &'static ureq::Agent {
    static AGENT: std::sync::OnceLock<ureq::Agent> = std::sync::OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::AgentBuilder::new()
            .max_idle_connections(16)
            .max_idle_connections_per_host(8)
            .build()
    })
}

fn request(url: &str, timeout_secs: u64) -> Result<String, Failure> {
    let response = agent()
        .get(url)
        .timeout(Duration::from_secs(timeout_secs))
        .set("User-Agent", UA)
        // GML is highly compressible: GUGiK sends 5 MB of a page as 400 KB when asked.
        .set("Accept-Encoding", "gzip")
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(code, response) => Failure::Status(code, error_detail(response)),
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

/// Streams a body into `sink`, from the cache when fresh, and keeps it for the next run.
///
/// The sink parses while the bytes arrive, so a 40 MB answer never becomes one string and one
/// tree. The bytes are copied to the cache as they pass, so a second run reads a file.
pub fn stream_body<T>(
    url: &str,
    timeout_secs: u64,
    lang: Lang,
    sink: impl FnOnce(&mut dyn std::io::BufRead) -> Result<T, String>,
) -> Result<(u64, T), String> {
    if let Some(cached) = cache::get(url) {
        let mut reader = std::io::BufReader::new(cached.as_bytes());
        let value = sink(&mut reader)?;
        return Ok((cached.len() as u64, value));
    }
    let response = agent()
        .get(url)
        .timeout(Duration::from_secs(timeout_secs))
        .set("User-Agent", UA)
        .set("Accept-Encoding", "gzip")
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(code, response) => {
                text::http_status(lang, code, short(url), &error_detail(response))
            }
            ureq::Error::Transport(t) => text::http_transport(short(url), &t.to_string()),
        })?;
    let dir = cache::dir();
    let _ = std::fs::create_dir_all(&dir);
    let part = dir.join(format!("{}.part", cache::key(url)));
    let file = match std::fs::File::create(&part) {
        Ok(file) => Some(file),
        Err(_) => None,
    };
    let tee = Tee { inner: response.into_reader(), copy: file, written: 0 };
    let mut reader = std::io::BufReader::with_capacity(64 * 1024, tee);
    let value = match sink(&mut reader) {
        Ok(value) => value,
        Err(error) => {
            let _ = std::fs::remove_file(&part);
            return Err(error);
        }
    };
    let counted = reader.into_inner();
    let bytes = counted.written;
    // A body that parsed keeps its bytes, and the next run reads them from the file.
    if counted.copy.is_some() {
        let _ = std::fs::rename(&part, dir.join(cache::key(url)));
    }
    Ok((bytes, value))
}

/// A reader that copies every byte it hands on, so a stream can be cached as it is parsed.
struct Tee<R: Read> {
    inner: R,
    copy: Option<std::fs::File>,
    written: u64,
}

impl<R: Read> Read for Tee<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let read = self.inner.read(buffer)?;
        if read > 0 {
            self.written += read as u64;
            if let Some(file) = self.copy.as_mut() {
                let _ = file.write_all(&buffer[..read]);
            }
        }
        Ok(read)
    }
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
    let response = agent()
        .get(url)
        .timeout(Duration::from_secs(timeout_secs))
        .set("User-Agent", UA)
        .set("Accept-Encoding", "gzip")
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(code, response) => {
                text::http_status(lang, code, short(url), &error_detail(response))
            }
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

/// The first line of an error body that carries text, with the markup taken off it.
fn error_detail(response: ureq::Response) -> String {
    let mut bytes = Vec::new();
    let read = response.into_reader().take(ERROR_BYTES).read_to_end(&mut bytes);
    if read.is_err() {
        return String::new();
    }
    first_text_line(&String::from_utf8_lossy(&bytes))
}

/// The first line of a body that holds words of its own, clipped for a message.
fn first_text_line(body: &str) -> String {
    for line in body.lines() {
        let text = without_tags(line);
        if !text.is_empty() {
            return clip(&text, ERROR_CHARS);
        }
    }
    String::new()
}

/// Drops every `<…>` span, so an XML or HTML error leaves the sentence it wraps.
fn without_tags(line: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for ch in line.chars() {
        match ch {
            '<' => inside = true,
            '>' => {
                inside = false;
                out.push(' ');
            }
            c if !inside => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn short(url: &str) -> &str {
    url.split('?').next().unwrap_or(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_the_sentence_out_of_an_xml_error_body() {
        let body = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<ServiceExceptionReport version=\"1.2.0\">\n  <ServiceException code=\"InvalidParameterValue\">msWFSGetFeature(): BBOX and FILTER are mutually exclusive</ServiceException>\n</ServiceExceptionReport>";
        assert_eq!(
            first_text_line(body),
            "msWFSGetFeature(): BBOX and FILTER are mutually exclusive"
        );
        assert_eq!(first_text_line("<html><body>Bad request</body></html>"), "Bad request");
        assert_eq!(first_text_line("  \n\n"), "");
    }

    #[test]
    fn clips_a_long_exception_line() {
        let body = "x".repeat(ERROR_CHARS + 50);
        assert_eq!(first_text_line(&body).chars().count(), ERROR_CHARS);
        assert!(first_text_line(&body).ends_with('…'));
    }
}
