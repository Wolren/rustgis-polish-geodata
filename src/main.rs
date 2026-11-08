//! Polish Geodata: GUGiK data in RustGIS.
//!
//! A process plugin: RustGIS starts this program once per command or tool call and writes the
//! request on standard input, with the interface language as `lang`.
//!
//! Commands only do things that finish at once: a map layer, a drawn area, a parcel lookup. A
//! command holds the interface while it runs, so nothing that downloads belongs in one. The WFS
//! download is a tool: it runs in the background with progress and Cancel.
//!
//! The panel's map list starts a download tool, so that press holds the interface until the run ends.

mod cache;
mod catalog;
mod clip;
mod egib;
mod filter;
mod eziudp;
mod gk92;
mod gml;
mod host;
mod http;
mod katalog;
mod nmt;
mod obszar;
mod panel;
mod prg;
mod services;
mod sheets;
mod text;
mod tile;
mod tm;
mod tool;
mod uug;
mod uldk;
mod wfs;
mod zrodla;

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

fn run(
    request: &Value,
    id: &str,
    params: &Value,
    host: &mut host::Host,
    lang: Lang,
) -> Result<Value, String> {
    // A rectangle dragged on the map becomes the area before a command that reads it runs.
    if !matches!(
        id,
        "obszar-prostokat" | "obszar-przejmij" | "obszar-widok" | "obszar-json" | "obszar-anuluj"
            | "obszar-wyczysc"
    ) {
        if let Err(e) = obszar::take_rectangle(host, lang) {
            host.notify(&e);
        }
    }
    if let Some(overlay) = catalog::overlay(id) {
        return tile::toggle(host, overlay, lang);
    }
    // The family files name their own commands, one per service or map.
    if id.starts_with("wody-") && catalog::find_wody(id).is_some() {
        return tile::toggle_wody(host, id, request, lang);
    }
    if id.starts_with("carto-") {
        return tile::toggle_carto(host, id, lang);
    }
    let mut answer = match id {
        "obszar-rysuj" => obszar::draw(host, lang),
        "obszar-prostokat" => obszar::draw_rectangle(host, lang),
        "obszar-przejmij" => obszar::take_rectangle(host, lang)?
            .ok_or_else(|| text::area_nothing_drawn(lang)),
        "obszar-widok" => obszar::rectangle_from_view(host, lang),
        "obszar-json" => obszar::from_json(host, request, lang),
        "obszar-jednostka" => prg::area(host, params, lang),
        "dzialki-powiat" => egib::from_area(host, params, lang),
        "obszar-anuluj" => obszar::abort(host, lang),
        "obszar-wyczysc" => obszar::clear(host, lang),
        other if other.starts_with("usluga-") => services::toggle(host, lang, request),
        "katalog" => katalog::run(params, lang),
        "eziudp" => eziudp::search(params, lang),
        "wody-warstwy" => tile::wody_layers(params, lang),
        "zrodla" => zrodla::search(host, params, lang),
        "geokoduj" => uug::geocode(host, params, lang),
        "adres-odwrotnie" => uug::reverse(host, params, lang),
        "wysokosc" => uug::height(host, params, lang),
        "dzialka-id" => uldk::parcel_by_id(host, params, lang),
        "panel-state" => panel::state(host, lang, request),
        "mapa" => panel::mapa(host, request, lang),
        "ustawienia" => panel::preferences(host, request, lang),
        "dem" => nmt::add(host, lang, request, nmt::Kind::TerrainKron),
        "dem-evr" => nmt::add(host, lang, request, nmt::Kind::TerrainEvrf),
        "dem-nmpt" => nmt::add(host, lang, request, nmt::Kind::SurfaceKron),
        "dem-style" => nmt::style(host, lang, request),
        "zoom-polska" => zoom_poland(host, lang),
        "wyczysc-cache" => clear_cache(host, lang),
        "przytnij" => cut(host, request, lang),
        "parcel" => uldk::parcel(host, params, lang),
        other => match zrodla::ribbon_target(other) {
            Some(entry) => zrodla::add_service(host, entry, lang),
            None => Err(text::unknown_command(lang, other)),
        },
    }?;
    // The pane reads the panel's state once, when it opens: a command that changes it answers too.
    if redraws_panel(id) {
        if let Ok(state) = panel::state(host, lang, request) {
            panel::attach(&mut answer, state);
        }
    }
    Ok(answer)
}

/// The commands whose work changes what the panel shows, so each answers the panel's state.
fn redraws_panel(id: &str) -> bool {
    matches!(
        id,
        "obszar-widok"
            | "obszar-json"
            | "obszar-jednostka"
            | "obszar-rysuj"
            | "obszar-prostokat"
            | "obszar-przejmij"
            | "obszar-anuluj"
            | "obszar-wyczysc"
            | "parcel"
            | "adres-odwrotnie"
            | "wyczysc-cache"
            | "dem-style"
            | "mapa"
            | "przytnij"
            | "ustawienia"
    )
}

/// Cuts a layer to a boundary layer: the one the request names, else the area layer.
fn cut(host: &mut host::Host, request: &Value, lang: Lang) -> Result<Value, String> {
    let list = clip::layers(host, lang)?;
    let (layer, boundary) = clip::plan(&list, request);
    let Some(boundary) = boundary else {
        return Err(match clip::asked(request) {
            Some(name) => text::boundary_gone(lang, &name),
            None => text::clip_no_boundary(lang),
        });
    };
    let layer = layer.ok_or_else(|| text::nothing_to_clip(lang))?;
    if layer == boundary {
        return Err(text::clip_same(lang, &layer));
    }
    clip::to_area(host, &layer, &boundary, lang)
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
