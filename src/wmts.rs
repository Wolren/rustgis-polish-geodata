//! Adds a WMTS overlay from a GetTile template or a capabilities URL.

use crate::host::Host;
use crate::text::{self, Lang};
use quick_xml::events::Event;
use quick_xml::Reader;
use serde_json::{json, Value};

/// The template a ribbon click adds: the national ortho over Web Mercator.
pub const NATIONAL_ORTHO: &str = "https://mapy.geoportal.gov.pl/wss/service/PZGIK/ORTO/WMTS/StandardResolution\
?SERVICE=WMTS&REQUEST=GetTile&VERSION=1.0.0&LAYER=ORTOFOTOMAPA&STYLE=default&TILEMATRIXSET=EPSG:3857\
&TILEMATRIX=EPSG:3857:{z}&TILEROW={y}&TILECOL={x}&FORMAT=image/jpeg";

/// A click adds the national ortho; a URL adds that service instead, template or capabilities.
pub fn add(host: &mut Host, params: &Value, lang: Lang) -> Result<Value, String> {
    let (url, name) = checked(params, lang)?;
    let template = if has_placeholders(&url) {
        url.clone()
    } else if url.contains('{') || url.contains('}') {
        return Err(text::wmts_needs_placeholders(lang));
    } else {
        let body = crate::http::fetch(&capabilities_url(&url), 60, lang)?;
        let base = url.split('?').next().unwrap_or(&url);
        template_from_caps(&body, base).ok_or_else(|| text::wmts_no_layer(lang))?
    };
    host.call(
        "map.addTiles",
        json!({"url": template, "name": name, "kind": "raster", "overlay": true}),
    )?;
    host.notify(&text::wmts_added(lang, &name));
    Ok(json!({"name": name, "url": template}))
}

fn checked(params: &Value, _lang: Lang) -> Result<(String, String), String> {
    let url = params
        .get("url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .unwrap_or(NATIONAL_ORTHO);
    let name = params
        .get("nazwa")
        .or_else(|| params.get("name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("Ortofoto WMTS");
    Ok((url.to_string(), name.to_string()))
}

/// A template is a URL with the host's three placeholders.
pub fn has_placeholders(url: &str) -> bool {
    url.contains("{z}") && url.contains("{x}") && url.contains("{y}")
}

fn capabilities_url(url: &str) -> String {
    if url.to_ascii_lowercase().contains("request=") {
        return url.to_string();
    }
    let separator = if url.contains('?') { '&' } else { '?' };
    format!("{url}{separator}SERVICE=WMTS&REQUEST=GetCapabilities")
}

/// The GetTile template of the first layer that names a tile matrix set, over Web Mercator.
pub fn template_from_caps(xml: &str, base: &str) -> Option<String> {
    let root = tree(xml)?;
    let mut layers = Vec::new();
    descendants(&root, "Layer", &mut layers);
    for layer in layers {
        let Some(identifier) = child(layer, "Identifier").map(text_of).filter(|id| !id.is_empty()) else {
            continue;
        };
        let mut links: Vec<&str> = Vec::new();
        for link in layer.children.iter().filter(|c| c.name == "TileMatrixSetLink") {
            if let Some(set) = child(link, "TileMatrixSet").map(text_of).filter(|set| !set.is_empty()) {
                links.push(set);
            }
        }
        // The host draws Web Mercator tiles: a set without 3857 would land the wrong matrix.
        let Some(set) = links
            .iter()
            .find(|set| set.contains("3857") || set.contains("900913"))
            .copied()
            .or_else(|| links.first().copied())
        else {
            continue;
        };
        let Some(format) = child(layer, "Format")
            .map(text_of)
            .filter(|format| format.starts_with("image/"))
        else {
            continue;
        };
        let style = child(layer, "Style")
            .and_then(|style| child(style, "Identifier"))
            .map(text_of)
            .unwrap_or("default");
        let prefix = matrix_prefix(&root, set);
        return Some(format!(
            "{base}?SERVICE=WMTS&REQUEST=GetTile&VERSION=1.0.0&LAYER={identifier}&STYLE={style}\
             &TILEMATRIXSET={set}&TILEMATRIX={prefix}{{z}}&TILEROW={{y}}&TILECOL={{x}}&FORMAT={format}"
        ));
    }
    None
}

/// The matrix id prefix, so the host's zoom level lands on the right matrix.
fn matrix_prefix(root: &Node, set: &str) -> String {
    let mut sets = Vec::new();
    descendants(root, "TileMatrixSet", &mut sets);
    for node in sets {
        let identifier = child(node, "Identifier").map(text_of);
        if identifier != Some(set) {
            continue;
        }
        let Some(matrix) = child(node, "TileMatrix").and_then(|m| child(m, "Identifier")).map(text_of) else {
            return String::new();
        };
        return match matrix.rsplit_once(':') {
            Some((prefix, digits)) if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) => {
                format!("{prefix}:")
            }
            _ => String::new(),
        };
    }
    String::new()
}

#[derive(Default)]
struct Node {
    name: String,
    text: String,
    children: Vec<Node>,
}

fn tree(xml: &str) -> Option<Node> {
    let mut reader = Reader::from_str(xml);
    let mut stack: Vec<Node> = Vec::new();
    let mut top: Option<Node> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                stack.push(Node {
                    name: String::from_utf8_lossy(element.local_name().as_ref()).to_string(),
                    ..Default::default()
                });
            }
            Ok(Event::Text(content)) => {
                if let Some(node) = stack.last_mut() {
                    node.text.push_str(&content.unescape().unwrap_or_default());
                }
            }
            Ok(Event::End(_)) => {
                if let Some(node) = stack.pop() {
                    match stack.last_mut() {
                        Some(parent) => parent.children.push(node),
                        None => top = Some(node),
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    top
}

fn child<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    node.children.iter().find(|c| c.name == name)
}

fn descendants<'a>(node: &'a Node, name: &str, out: &mut Vec<&'a Node>) {
    for c in &node.children {
        if c.name == name {
            out.push(c);
        }
        descendants(c, name, out);
    }
}

fn text_of(node: &Node) -> &str {
    node.text.trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAPS: &str = r#"<?xml version="1.0"?>
<Capabilities xmlns:ows="http://www.opengis.net/ows/1.1">
  <Contents>
    <Layer>
      <ows:Title>Ortofoto</ows:Title>
      <ows:Identifier>ORTOFOTOMAPA</ows:Identifier>
      <Style isDefault="true"><ows:Identifier>default</ows:Identifier></Style>
      <Format>image/png</Format>
      <TileMatrixSetLink><TileMatrixSet>EPSG:2180</TileMatrixSet></TileMatrixSetLink>
      <TileMatrixSetLink><TileMatrixSet>EPSG:3857</TileMatrixSet></TileMatrixSetLink>
    </Layer>
    <TileMatrixSet>
      <ows:Identifier>EPSG:2180</ows:Identifier>
      <TileMatrix><ows:Identifier>EPSG:2180:0</ows:Identifier></TileMatrix>
    </TileMatrixSet>
    <TileMatrixSet>
      <ows:Identifier>EPSG:3857</ows:Identifier>
      <TileMatrix><ows:Identifier>EPSG:3857:0</ows:Identifier></TileMatrix>
      <TileMatrix><ows:Identifier>EPSG:3857:1</ows:Identifier></TileMatrix>
    </TileMatrixSet>
  </Contents>
</Capabilities>"#;

    #[test]
    fn a_capabilities_template_names_the_layer_and_the_matrix() {
        let template = template_from_caps(CAPS, "https://x/wmts").expect("a template");
        assert!(template.starts_with("https://x/wmts?SERVICE=WMTS&REQUEST=GetTile"), "{template}");
        assert!(template.contains("LAYER=ORTOFOTOMAPA"), "{template}");
        assert!(template.contains("TILEMATRIXSET=EPSG:3857"), "{template}");
        assert!(template.contains("TILEMATRIX=EPSG:3857:{z}"), "{template}");
        assert!(template.contains("TILEROW={y}&TILECOL={x}"), "{template}");
        assert!(template.contains("FORMAT=image/png"), "{template}");
    }

    #[test]
    fn a_capabilities_without_a_layer_gives_nothing() {
        assert!(template_from_caps("<Capabilities/>", "https://x").is_none());
    }

    #[test]
    fn placeholders_must_all_be_there() {
        assert!(has_placeholders("https://x/{z}/{x}/{y}"));
        assert!(!has_placeholders("https://x/{z}/{x}"));
    }

    #[test]
    fn a_template_url_is_taken_as_is() {
        let params = json!({"url": "https://x/{z}/{x}/{y}", "nazwa": "Orto"});
        assert_eq!(checked(&params, Lang::En).unwrap().1, "Orto");
    }
}
