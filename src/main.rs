//! Dane PL — Polish geodetic data in RustGIS.
//!
//! A process plugin: RustGIS starts this program once per command or tool call and writes the
//! request on standard input. Map layers and the parcel lookup are commands; the download is a
//! tool, so it runs in the background (with progress and Cancel) instead of freezing the window.

mod catalog;
mod gml;
mod host;
mod http;
mod obszar;
mod tile;
mod tm;
mod tool;
mod uldk;
mod wfs;

use serde_json::{json, Value};
use std::io::Write;

fn main() {
    let mut line = String::new();
    if std::io::stdin().read_line(&mut line).unwrap_or(0) == 0 {
        eprintln!("dane-pl: no request on standard input");
        std::process::exit(1);
    }
    let request: Value = match serde_json::from_str(&line) {
        Ok(v) => v,
        Err(e) => return finish(&json!({ "error": format!("unreadable request: {e}") })),
    };
    let kind = request.get("kind").and_then(Value::as_str).unwrap_or("command");
    let id = request.get("id").and_then(Value::as_str).unwrap_or_default().to_string();
    let params = request.get("params").cloned().unwrap_or(Value::Null);

    let result = if kind == "tool" {
        tool::run(&request)
    } else {
        let mut host = host::Host::new();
        run(&id, &params, &mut host)
    };

    match result {
        Ok(value) => finish(&json!({ "ok": value })),
        Err(e) => finish(&json!({ "error": e })),
    }
}

fn run(id: &str, params: &Value, host: &mut host::Host) -> Result<Value, String> {
    if let Some(overlay) = catalog::overlay(id) {
        return tile::toggle(host, overlay);
    }
    match id {
        "obszar-widok" => obszar::from_view(host),
        "parcel" => uldk::parcel(host, params),
        other => Err(format!("Dane PL nie ma polecenia '{other}'")),
    }
}

fn finish(value: &Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{value}");
    let _ = out.flush();
}
