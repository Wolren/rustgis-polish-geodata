//! The sheet downloader: the files a GUGiK sheet index lists, into one folder.
//!
//! A sheet index is a WFS layer whose features carry a sheet code and a direct file address. The
//! tool reads the layer as GeoJSON, so a selection narrows the list, and writes every file into
//! the folder the user names. A file already there with the reported size is kept, so a run
//! resumes, and one report row per feature records what happened.

use crate::host;
use crate::http;
use crate::text::{self, Lang};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// The property names that hold a file address, in the order this plugin trusts them.
const URL_KEYS: [&str; 4] = ["url_do_pobrania", "url", "nazwa_pliku", "link"];
/// One run takes no more files than this; a selection narrows a bigger list.
const MAX_FILES: usize = 500;
/// GUGiK moved 5.9 MB/s (measured 2026-10-27), so 300 s covers a 1.7 GB sheet.
const TIMEOUT_SECONDS: u64 = 300;
/// The report beside the downloads, in the folder the user named.
const REPORT: &str = "pobrane.csv";
/// `wlk_pliku_mb` is rounded to 0.1 MB, so a file is compared with this slack.
const SIZE_SLACK: f64 = 0.05;
/// The report's status for a file already on disk, and the prefix of a failure's.
const SKIPPED: &str = "pominieto";
const FAILED: &str = "blad: ";
/// Files fetched at once: a big run is many independent files on one host.
const WORKERS: usize = 4;
/// Above this many megabytes a run needs the form's confirmation, not a surprise.
const LARGE_MB: f64 = 500.0;

/// One feature's file: what the run fetches, and what its report row names.
struct Sheet {
    godlo: String,
    url: String,
    name: String,
    expected: Option<u64>,
}

/// A feature of the index: its file, or the sheet code of one that offers no address.
enum Entry {
    Sheet(Sheet),
    NoAddress(String),
}

/// One line of `pobrane.csv`, in the file's own field order.
struct Row {
    godlo: String,
    url: String,
    bytes: u64,
    status: String,
}

/// Downloads every file the sheet index lists into the folder, and writes the report.
pub fn run(request: &Value, lang: Lang) -> Result<Value, String> {
    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
    let folder = params
        .get("folder")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if folder.is_empty() {
        return Err(text::folder_missing(lang));
    }
    let folder = PathBuf::from(folder);
    std::fs::create_dir_all(&folder)
        .map_err(|e| text::folder_failed(lang, &folder.display().to_string(), &e.to_string()))?;

    let layer = params
        .get("skorowidz")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    let collection = request
        .get("layers")
        .and_then(|layers| layers.get(layer))
        .ok_or_else(|| text::sheet_layer_missing(lang, layer))?;
    let entries = plan(collection);
    let total = ready(&entries, layer, lang)?;
    let (files, megabytes) = estimate(&entries);
    if megabytes > LARGE_MB
        && !params
            .get("confirm")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        return Err(text::sheets_unconfirmed(lang, files, megabytes, LARGE_MB));
    }
    host::progress_line(&text::sheets_estimate(lang, files, megabytes));

    // Rows that need no network are settled first, so the parallel run sees only files to fetch.
    let mut rows: Vec<Option<Row>> = (0..entries.len()).map(|_| None).collect();
    let mut missing = 0usize;
    let mut todo: Vec<(usize, &Sheet)> = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        match entry {
            Entry::NoAddress(godlo) => {
                missing += 1;
                rows[index] = Some(Row {
                    godlo: godlo.clone(),
                    url: String::new(),
                    bytes: 0,
                    status: SKIPPED.to_string(),
                });
            }
            Entry::Sheet(sheet) => {
                let path = folder.join(&sheet.name);
                match existing_size(&path, sheet.expected) {
                    Some(size) => {
                        rows[index] = Some(Row {
                            godlo: sheet.godlo.clone(),
                            url: sheet.url.clone(),
                            bytes: size,
                            status: SKIPPED.to_string(),
                        });
                    }
                    None => todo.push((index, sheet)),
                }
            }
        }
    }

    let count = todo.len();
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let fetched: Mutex<Vec<(usize, Row)>> = Mutex::new(Vec::with_capacity(count));
    std::thread::scope(|scope| {
        for _ in 0..WORKERS.min(count) {
            scope.spawn(|| loop {
                let at = next.fetch_add(1, Ordering::SeqCst);
                let Some((index, sheet)) = todo.get(at) else {
                    break;
                };
                let path = folder.join(&sheet.name);
                let row = match http::download(&sheet.url, &path, TIMEOUT_SECONDS, lang) {
                    Ok(size) => Row {
                        godlo: sheet.godlo.clone(),
                        url: sheet.url.clone(),
                        bytes: size,
                        status: "ok".to_string(),
                    },
                    Err(error) => Row {
                        godlo: sheet.godlo.clone(),
                        url: sheet.url.clone(),
                        bytes: 0,
                        status: format!("{FAILED}{}", clip(&error, 120)),
                    },
                };
                let finished = done.fetch_add(1, Ordering::SeqCst) + 1;
                host::progress_line(&text::sheet_progress(lang, finished, count, &sheet.name));
                fetched
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .push((*index, row));
            });
        }
    });
    if let Ok(got) = fetched.into_inner() {
        for (index, row) in got {
            rows[index] = Some(row);
        }
    }

    let rows: Vec<Row> = rows.into_iter().flatten().collect();
    let downloaded = rows.iter().filter(|row| row.status == "ok").count();
    let bytes: u64 = rows.iter().filter(|row| row.status == "ok").map(|row| row.bytes).sum();
    let skipped = rows.iter().filter(|row| row.status == SKIPPED).count();
    let failed = rows.iter().filter(|row| row.status.starts_with(FAILED)).count();
    let _ = total;

    let report = folder.join(REPORT);
    std::fs::write(&report, csv(&rows))
        .map_err(|e| text::report_failed(lang, &report.display().to_string(), &e.to_string()))?;

    let mut messages = Vec::new();
    if missing > 0 {
        messages.push(text::sheets_shortfall(lang, missing));
    }
    messages.push(text::sheets_summary(
        lang,
        downloaded,
        bytes as f64 / 1e6,
        skipped,
        failed,
        REPORT,
    ));
    Ok(json!({
        "files": downloaded,
        "bytes": bytes,
        "failed": failed,
        "skipped": skipped,
        "folder": folder.display().to_string(),
        "messages": messages,
    }))
}

/// The list's file count and its megabytes: the index rounds sizes, so this is a lower bound.
fn estimate(entries: &[Entry]) -> (usize, f64) {
    let mut files = 0usize;
    let mut bytes = 0u64;
    for entry in entries {
        if let Entry::Sheet(sheet) = entry {
            files += 1;
            bytes += sheet.expected.unwrap_or(0);
        }
    }
    (files, bytes as f64 / 1e6)
}

/// The number of files the list holds, or why the run cannot start.
fn ready(entries: &[Entry], layer: &str, lang: Lang) -> Result<usize, String> {
    let total = entries
        .iter()
        .filter(|entry| matches!(entry, Entry::Sheet(_)))
        .count();
    if total == 0 {
        return Err(text::nothing_to_download(lang, layer, entries.len()));
    }
    if total > MAX_FILES {
        return Err(text::too_many_files(lang, total, MAX_FILES));
    }
    Ok(total)
}

/// What each feature of the index asks for, in the layer's own order.
fn plan(collection: &Value) -> Vec<Entry> {
    let mut entries = Vec::new();
    for feature in collection
        .get("features")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let godlo = property(feature, "godlo");
        let sheet = url_of(feature).and_then(|url| file_name(&url).map(|name| (url, name)));
        match sheet {
            Some((url, name)) => entries.push(Entry::Sheet(Sheet {
                godlo,
                url,
                name,
                expected: expected_bytes(feature),
            })),
            None => entries.push(Entry::NoAddress(godlo)),
        }
    }
    entries
}

/// A feature's property as trimmed text, empty when it is missing.
fn property(feature: &Value, key: &str) -> String {
    feature
        .get("properties")
        .and_then(|properties| properties.get(key))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// The first file address a feature offers, among the properties this plugin reads.
fn url_of(feature: &Value) -> Option<String> {
    let properties = feature.get("properties")?;
    URL_KEYS.iter().find_map(|key| {
        properties
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| value.starts_with("http"))
            .map(str::to_string)
    })
}

/// The size the index reports in bytes; `wlk_pliku_mb` may be a number or text.
fn expected_bytes(feature: &Value) -> Option<u64> {
    let value = feature.get("properties")?.get("wlk_pliku_mb")?;
    let megabytes = match value {
        Value::Number(number) => number.as_f64()?,
        Value::String(text) => text.trim().replace(',', ".").parse::<f64>().ok()?,
        _ => return None,
    };
    (megabytes > 0.0).then(|| (megabytes * 1e6).round() as u64)
}

/// The size of a file already complete in the folder: one the index's own size agrees with.
fn existing_size(path: &Path, expected: Option<u64>) -> Option<u64> {
    let size = std::fs::metadata(path).ok()?.len();
    match expected {
        Some(want) if !same_size(size, want) => None,
        _ => Some(size),
    }
}

/// `wlk_pliku_mb` is rounded, so a size within five percent (or 64 KB) is the same file.
fn same_size(size: u64, expected: u64) -> bool {
    let slack = (expected as f64 * SIZE_SLACK).max(65536.0);
    (size as f64 - expected as f64).abs() <= slack
}

/// The last path segment of an address, percent-decoded, and never a path of its own.
fn file_name(url: &str) -> Option<String> {
    let path = url.split(['?', '#']).next()?;
    let last = path.rsplit('/').next()?;
    let name = percent_decode(last);
    let name = name.rsplit(['/', '\\']).next()?;
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }
    Some(name.to_string())
}

/// Reads `%XX` escapes as bytes, leaving a malformed escape as it stands.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' && at + 2 < bytes.len() {
            if let (Some(high), Some(low)) = (hex(bytes[at + 1]), hex(bytes[at + 2])) {
                out.push(high << 4 | low);
                at += 3;
                continue;
            }
        }
        out.push(bytes[at]);
        at += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// The report text: the header, then one CRLF row per feature of the index.
fn csv(rows: &[Row]) -> String {
    let mut text = String::from("godlo;url;bytes;status\r\n");
    for row in rows {
        text.push_str(&format!(
            "{};{};{};{}\r\n",
            field(&row.godlo),
            field(&row.url),
            row.bytes,
            field(&row.status)
        ));
    }
    text
}

/// An error short enough for one report cell.
fn clip(text: &str, limit: usize) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() <= limit {
        return line;
    }
    let cut: String = line.chars().take(limit.saturating_sub(1)).collect();
    format!("{cut}\u{2026}")
}

/// A report field, quoted when it could break the semicolon layout.
fn field(text: &str) -> String {
    if text.contains([';', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feature(properties: Value) -> Value {
        json!({ "type": "Feature", "properties": properties, "geometry": null })
    }

    #[test]
    fn a_big_list_is_refused_before_the_first_file() {
        let mut features = Vec::new();
        for n in 0..12 {
            features.push(feature(json!({
                "godlo": format!("S{n}"),
                "url_do_pobrania": format!("https://x.pl/S{n}.zip"),
                "wlk_pliku_mb": 500.0,
            })));
        }
        let collection = json!({ "type": "FeatureCollection", "features": features });
        let entries = plan(&collection);
        let (files, megabytes) = estimate(&entries);
        assert_eq!(files, 12);
        assert!((megabytes - 6000.0).abs() < 0.1, "{megabytes}");
        let request = json!({
            "id": "pobierz-arkusze",
            "params": { "folder": std::env::temp_dir().join("polish-geodata-refused").display().to_string(), "skorowidz": "sheets" },
            "layers": { "sheets": collection },
            "lang": "en",
        });
        let error = run(&request, Lang::En).unwrap_err();
        assert!(error.contains("6.0 GB"), "{error}");
        assert!(error.contains("Download anyway"), "{error}");
    }

    #[test]
    fn picks_the_first_address_a_feature_offers() {
        let primary = feature(json!({ "godlo": "A", "url_do_pobrania": "https://x.pl/A.zip" }));
        let plain = feature(json!({ "godlo": "B", "url": "https://x.pl/B.zip" }));
        let named = feature(json!({ "godlo": "C", "nazwa_pliku": "C.zip" }));
        let elsewhere = feature(json!({ "godlo": "D", "url": "ftp://x.pl/D.zip", "link": "https://x.pl/D.zip" }));
        assert_eq!(url_of(&primary).as_deref(), Some("https://x.pl/A.zip"));
        assert_eq!(url_of(&plain).as_deref(), Some("https://x.pl/B.zip"));
        assert_eq!(url_of(&named), None, "a bare file name is not an address");
        assert_eq!(url_of(&elsewhere).as_deref(), Some("https://x.pl/D.zip"));
        assert_eq!(url_of(&feature(json!({ "godlo": "E" }))), None);
        assert_eq!(url_of(&json!({ "type": "Feature" })), None);
    }

    #[test]
    fn takes_the_file_name_out_of_an_address() {
        assert_eq!(
            file_name("https://x.pl/dane/godlo_1.zip?token=abc&v=2").as_deref(),
            Some("godlo_1.zip")
        );
        assert_eq!(
            file_name("https://x.pl/dane/PL-1992%20arkusz%201.zip#top").as_deref(),
            Some("PL-1992 arkusz 1.zip")
        );
        assert_eq!(file_name("https://x.pl/").as_deref(), None);
        assert_eq!(file_name("https://x.pl/a/%2E%2E").as_deref(), None);
        // A decoded separator may not turn the name into a path.
        assert_eq!(file_name("https://x.pl/a/..%2Fescape.zip").as_deref(), Some("escape.zip"));
        assert_eq!(percent_decode("100%25"), "100%");
        assert_eq!(percent_decode("bad%2"), "bad%2");
    }

    #[test]
    fn writes_the_report_header_and_one_row() {
        let rows = vec![Row {
            godlo: "N-33-123-C-d-4".to_string(),
            url: "https://x.pl/N-33-123-C-d-4.zip".to_string(),
            bytes: 12,
            status: "ok".to_string(),
        }];
        assert_eq!(
            csv(&rows),
            "godlo;url;bytes;status\r\nN-33-123-C-d-4;https://x.pl/N-33-123-C-d-4.zip;12;ok\r\n"
        );
        assert_eq!(csv(&[]), "godlo;url;bytes;status\r\n");
    }

    #[test]
    fn refuses_a_list_with_no_file() {
        let collection = json!({
            "type": "FeatureCollection",
            "features": [
                feature(json!({ "godlo": "A" })),
                feature(json!({ "godlo": "B", "nazwa_pliku": "B.zip" }))
            ]
        });
        let entries = plan(&collection);
        assert_eq!(entries.len(), 2, "both features still take a report row");
        let english = ready(&entries, "SkorowidzOrtofomapy2024", Lang::En).unwrap_err();
        assert!(english.contains("no file"), "{english}");
        assert!(english.contains("SkorowidzOrtofomapy2024"), "{english}");
        let polish = ready(&entries, "SkorowidzOrtofomapy2024", Lang::Pl).unwrap_err();
        assert!(polish.contains("nie wskazuje"), "{polish}");
    }

    #[test]
    fn refuses_more_files_than_one_run_takes() {
        let features: Vec<Value> = (0..=MAX_FILES)
            .map(|i| {
                feature(json!({
                    "godlo": format!("G{i}"),
                    "url_do_pobrania": format!("https://x.pl/{i}.zip")
                }))
            })
            .collect();
        let entries = plan(&json!({ "type": "FeatureCollection", "features": features }));
        let error = ready(&entries, "Skorowidz", Lang::En).unwrap_err();
        assert!(error.contains(&(MAX_FILES + 1).to_string()), "{error}");
        assert!(error.contains(&MAX_FILES.to_string()), "{error}");
    }

    #[test]
    fn keeps_the_order_and_the_reported_size_of_the_index() {
        let collection = json!({
            "type": "FeatureCollection",
            "features": [
                feature(json!({
                    "godlo": "A",
                    "url_do_pobrania": "https://x.pl/A.zip?x=1",
                    "wlk_pliku_mb": 12.5
                })),
                feature(json!({ "godlo": "B" })),
                feature(json!({
                    "godlo": "C",
                    "nazwa_pliku": "https://x.pl/C.zip",
                    "wlk_pliku_mb": "3,5"
                }))
            ]
        });
        let entries = plan(&collection);
        assert_eq!(ready(&entries, "Skorowidz", Lang::En), Ok(2));
        match &entries[0] {
            Entry::Sheet(sheet) => {
                assert_eq!(sheet.godlo, "A");
                assert_eq!(sheet.name, "A.zip");
                assert_eq!(sheet.expected, Some(12_500_000));
            }
            Entry::NoAddress(_) => panic!("the first feature carries an address"),
        }
        assert!(matches!(&entries[1], Entry::NoAddress(godlo) if godlo == "B"));
        match &entries[2] {
            Entry::Sheet(sheet) => assert_eq!(sheet.expected, Some(3_500_000)),
            Entry::NoAddress(_) => panic!("the third feature carries a name address"),
        }
    }

    #[test]
    fn a_reported_size_lands_within_the_slack_of_the_index() {
        assert!(same_size(12_500_000, 12_500_000));
        assert!(same_size(12_500_001, 12_500_000));
        assert!(!same_size(9_000_000, 12_500_000));
    }
}
