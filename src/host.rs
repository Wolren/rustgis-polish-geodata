//! Talking to RustGIS: the process-plugin protocol.
//!
//! RustGIS starts this program once per command and writes one request line on standard
//! input. While handling it, we may ask RustGIS to do things (`{"call": …}`) and read its
//! answer, then finish with `{"ok": …}` or `{"error": …}`.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};

/// A progress line for the Tools pane, usable without a [`Host`] (a tool has no host calls).
pub fn progress_line(text: &str) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{}", json!({ "progress": text }));
    let _ = out.flush();
}

pub struct Host {
    input: BufReader<std::io::Stdin>,
}

impl Host {
    pub fn new() -> Self {
        Host { input: BufReader::new(std::io::stdin()) }
    }

    fn send(&self, value: &Value) -> Result<(), String> {
        let mut out = std::io::stdout().lock();
        writeln!(out, "{value}").map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())
    }

    /// Asks RustGIS to run a command (or a `host.*` request) and returns its answer.
    pub fn call(&mut self, op: &str, params: Value) -> Result<Value, String> {
        self.send(&json!({ "call": { "op": op, "params": params } }))?;
        let mut line = String::new();
        let read = self.input.read_line(&mut line).map_err(|e| e.to_string())?;
        if read == 0 {
            return Err("RustGIS closed the connection".into());
        }
        let reply: Value =
            serde_json::from_str(line.trim()).map_err(|e| format!("unreadable reply from RustGIS: {e}"))?;
        if let Some(error) = reply.get("error") {
            return Err(error.as_str().unwrap_or("RustGIS reported an error").to_string());
        }
        reply.get("ok").cloned().ok_or_else(|| "RustGIS sent a reply without an answer".to_string())
    }

    /// Shows a message in the status bar. Never fails the command.
    pub fn notify(&mut self, text: &str) {
        let _ = self.call("host.notify", json!({ "text": text }));
    }


    /// Adds a GeoJSON layer to the project (`edit` permission).
    pub fn add_layer(&mut self, name: &str, geojson: Value) -> Result<Value, String> {
        self.call("host.addLayer", json!({ "name": name, "geojson": geojson }))
    }

    /// The map view's extent as `[west, south, east, north]`, in degrees.
    pub fn extent(&mut self) -> Result<[f64; 4], String> {
        let answer = self.call("map.extent", json!({}))?;
        let e = answer
            .get("extent")
            .and_then(Value::as_array)
            .ok_or_else(|| "map.extent returned no extent".to_string())?;
        if e.len() != 4 {
            return Err("map.extent returned an unexpected extent".into());
        }
        let mut out = [0.0; 4];
        for (i, v) in e.iter().enumerate() {
            out[i] = v.as_f64().ok_or_else(|| "map.extent returned a non-numeric extent".to_string())?;
        }
        Ok(out)
    }

    /// The map centre as `(longitude, latitude)`.
    pub fn centre(&mut self) -> Result<(f64, f64), String> {
        let view = self.call("host.view", json!({}))?;
        let c = view
            .get("center")
            .and_then(Value::as_array)
            .ok_or_else(|| "host.view returned no centre".to_string())?;
        let lon = c.first().and_then(Value::as_f64).ok_or_else(|| "no longitude".to_string())?;
        let lat = c.get(1).and_then(Value::as_f64).ok_or_else(|| "no latitude".to_string())?;
        Ok((lon, lat))
    }
}
