//! Polish Geodata: GUGiK data in RustGIS.
//!
//! A process plugin: RustGIS starts this program once per command or tool call and writes the
//! request on standard input, with the interface language as `lang`. Map layers and the parcel
//! lookup are commands; the download is a tool, so it runs in the background (with progress and
//! Cancel) instead of freezing the window.

mod cache;
mod catalog;
mod clip;
mod filter;
mod gk92;
mod gml;
mod host;
mod http;
mod nmt;
mod obszar;
mod dane;
mod services;
mod panel;
mod text;
mod tile;
mod tm;
mod tool;
mod uldk;
mod wfs;

use serde_json::{json, Value};
use std::io::Write;
use text::Lang;

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
    let kind = request
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("command");
    let id = request
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let params = request.get("params").cloned().unwrap_or(Value::Null);
    let lang = Lang::from_code(request.get("lang").and_then(Value::as_str).unwrap_or("en"));

    let result = if kind == "tool" {
        tool::run(&request, lang)
    } else {
        let mut host = host::Host::new(lang);
        run(&request, &id, &params, &mut host, lang)
    };

    match result {
        Ok(value) => finish(&json!({ "ok": value })),
        Err(e) => finish(&json!({ "error": e })),
    }
}

fn run(request: &Value, id: &str, params: &Value, host: &mut host::Host, lang: Lang) -> Result<Value, String> {
    if let Some(overlay) = catalog::overlay(id) {
        return tile::toggle(host, overlay, lang);
    }
    match id {
        "obszar-rysuj" => obszar::draw(host, lang, obszar::Mode::Rectangle),
        "obszar-poligon" => obszar::draw(host, lang, obszar::Mode::Polygon),
        "obszar-linia" => obszar::draw(host, lang, obszar::Mode::Line),
        "obszar-widok" => obszar::from_view(host, lang),
        "obszar-anuluj" => obszar::abort(host, lang),
        "usluga" => services::toggle(host, lang, request),
        other if other.starts_with("usluga-") => services::toggle(host, lang, request),
        other if other.starts_with("dane-") => dane::add(host, lang, request, other),
        "panel-state" => panel::state(host, lang, request),
        "dem" => nmt::add(host, lang, request, nmt::Kind::TerrainKron),
        "dem-evr" => nmt::add(host, lang, request, nmt::Kind::TerrainEvrf),
        "dem-nmpt" => nmt::add(host, lang, request, nmt::Kind::SurfaceKron),
        "dem-style" => nmt::style(host, lang, request),
        "zoom-polska" => zoom_poland(host, lang),
        "wyczysc-cache" => clear_cache(host, lang),
        "przytnij" => cut(host, params, lang),
        "parcel" => uldk::parcel(host, params, lang),
        other => Err(text::unknown_command(lang, other)),
    }
}

/// Cuts a layer to the area boundary. With no layer named it takes the newest one, so an agent
/// can call it straight after a download without having to name the layer.
fn cut(host: &mut host::Host, params: &Value, lang: Lang) -> Result<Value, String> {
    let area = match params.get("obszar").and_then(Value::as_str).filter(|s| !s.is_empty()) {
        Some(named) => named.to_string(),
        None => clip::existing_area(host, lang)?,
    };
    let named = params.get("layer").and_then(Value::as_str).unwrap_or_default();
    let layer = if named.is_empty() {
        clip::newest_layer(host, &area, lang)?
    } else {
        named.to_string()
    };
    clip::to_area(host, &layer, &area, lang)
}

/// Brings the map to Poland, the only area GUGiK serves.
fn zoom_poland(host: &mut host::Host, lang: Lang) -> Result<Value, String> {
    host.call("map.zoomToExtent", json!({ "extent": catalog::POLAND }))?;
    host.notify(&text::zoomed(lang));
    Ok(json!({ "zoomed": true }))
}

fn clear_cache(host: &mut host::Host, lang: Lang) -> Result<Value, String> {
    let (answers, answer_bytes) = cache::clear()?;
    // A downloaded model costs far more than an answer, so one button clears both.
    let (models, model_bytes) = cache::dem_clear();
    let files = answers + models;
    let bytes = answer_bytes + model_bytes;
    host.notify(&text::cache_cleared(lang, files, bytes as f64 / 1e6));
    Ok(json!({ "files": files, "bytes": bytes }))
}

fn finish(value: &Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{value}");
    let _ = out.flush();
}
