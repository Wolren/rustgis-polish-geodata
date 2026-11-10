//! Adds a WMTS overlay from a GetTile template the caller names.

use crate::host::Host;
use crate::text::{self, Lang};
use serde_json::{json, Value};

pub fn add(host: &mut Host, params: &Value, lang: Lang) -> Result<Value, String> {
    let (url, name) = checked(params, lang)?;
    host.call(
        "map.addTiles",
        json!({"url": url, "name": name, "kind": "raster", "overlay": true}),
    )?;
    host.notify(&text::wmts_added(lang, &name));
    Ok(json!({"name": name, "url": url}))
}

/// The template and the layer name; a template without the three placeholders would draw one tile.
fn checked(params: &Value, lang: Lang) -> Result<(String, String), String> {
    let url = params
        .get("url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .ok_or_else(|| text::wmts_needs_url(lang))?;
    if !(url.contains("{z}") && url.contains("{x}") && url.contains("{y}")) {
        return Err(text::wmts_needs_placeholders(lang));
    }
    let name = params
        .get("nazwa")
        .or_else(|| params.get("name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("WMTS");
    Ok((url.to_string(), name.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_template_needs_its_three_placeholders() {
        let ok = json!({"url": "https://x?TILEMATRIX=E:{z}&TILEROW={y}&TILECOL={x}", "nazwa": "Orto"});
        assert_eq!(checked(&ok, Lang::En).unwrap(), ("https://x?TILEMATRIX=E:{z}&TILEROW={y}&TILECOL={x}".to_string(), "Orto".to_string()));
        let bad = json!({"url": "https://x/tiles/{z}/{x}"});
        assert!(checked(&bad, Lang::En).unwrap_err().contains("placeholders"));
    }

    #[test]
    fn an_empty_url_is_refused() {
        assert!(checked(&json!({}), Lang::En).is_err());
    }
}
