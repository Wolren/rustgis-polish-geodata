//! GML → GeoJSON.
//!
//! GUGiK's WFS answers GML only (it offers no GeoJSON output), so the plugin does the
//! conversion itself. The parser is deliberately generic: it walks any `wfs:member`
//! feature, takes every element without child elements as a property, and converts the
//! GML geometry it finds (Point, LineString, Polygon and their multi forms).
//!
//! GUGiK answers `SRSNAME=EPSG:4326`, whose GML coordinate order is **lat lon**. The
//! `swap` flag turns that into GeoJSON's `[lon, lat]`.

use quick_xml::events::Event;
use quick_xml::Reader;
use serde_json::{json, Map, Value};

struct Node {
    name: String,
    id: Option<String>,
    text: String,
    children: Vec<Node>,
}

impl Node {
    fn new(name: &[u8], id: Option<String>) -> Self {
        Node { name: String::from_utf8_lossy(name).into_owned(), id, text: String::new(), children: Vec::new() }
    }

    fn has_children(&self) -> bool {
        !self.children.is_empty()
    }
}

fn parse_tree(xml: &str) -> Result<Vec<Node>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack: Vec<Node> = Vec::new();
    let mut roots: Vec<Node> = Vec::new();
    loop {
        match reader.read_event().map_err(|e| format!("GML could not be read: {e}"))? {
            Event::Start(e) => {
                let id = gml_id(&e);
                let name = e.local_name().as_ref().to_vec();
                stack.push(Node::new(&name, id));
            }
            Event::Empty(e) => {
                let id = gml_id(&e);
                let name = e.local_name().as_ref().to_vec();
                push_node(&mut stack, &mut roots, Node::new(&name, id));
            }
            Event::Text(t) => {
                if let Some(top) = stack.last_mut() {
                    let text = t
                        .unescape()
                        .map(|c| c.into_owned())
                        .unwrap_or_else(|_| String::from_utf8_lossy(&t).into_owned());
                    top.text.push_str(&text);
                }
            }
            Event::CData(t) => {
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&String::from_utf8_lossy(&t));
                }
            }
            Event::End(_) => {
                if let Some(node) = stack.pop() {
                    push_node(&mut stack, &mut roots, node);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(roots)
}

fn gml_id(e: &quick_xml::events::BytesStart<'_>) -> Option<String> {
    for attr in e.attributes().flatten() {
        if attr.key.local_name().as_ref() == b"id" {
            return attr.unescape_value().ok().map(|v| v.into_owned());
        }
    }
    None
}

fn push_node(stack: &mut Vec<Node>, roots: &mut Vec<Node>, node: Node) {
    match stack.last_mut() {
        Some(parent) => parent.children.push(node),
        None => roots.push(node),
    }
}

const GEOMETRIES: &[&str] = &[
    "Point",
    "LineString",
    "LinearRing",
    "Curve",
    "Polygon",
    "Surface",
    "MultiPoint",
    "MultiLineString",
    "MultiCurve",
    "MultiPolygon",
    "MultiSurface",
];

fn is_geometry(name: &str) -> bool {
    GEOMETRIES.contains(&name)
}

fn find_geometry<'a>(node: &'a Node) -> Option<&'a Node> {
    if is_geometry(&node.name) {
        return Some(node);
    }
    node.children.iter().find_map(find_geometry)
}

fn descendants<'a>(node: &'a Node, name: &str, out: &mut Vec<&'a Node>) {
    for child in &node.children {
        if child.name == name {
            out.push(child);
        }
        descendants(child, name, out);
    }
}

fn find_descendant<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    if node.name == name {
        return Some(node);
    }
    node.children.iter().find_map(|c| find_descendant(c, name))
}

/// Reads a coordinate list: GML 3's `posList`/`pos` (whitespace separated) or GML 2's
/// `coordinates` (`x,y x,y`), looking inside `node` first and then its descendants.
fn coordinates(node: &Node, swap: bool) -> Vec<[f64; 2]> {
    for name in ["posList", "coordinates", "pos"] {
        if let Some(found) = find_descendant(node, name) {
            let text = found.text.trim();
            if text.is_empty() {
                continue;
            }
            let numbers: Vec<f64> = text
                .split(|c: char| c.is_whitespace() || c == ',')
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.parse::<f64>().ok())
                .collect();
            let mut out = Vec::with_capacity(numbers.len() / 2);
            for pair in numbers.chunks_exact(2) {
                out.push(if swap { [pair[1], pair[0]] } else { [pair[0], pair[1]] });
            }
            if !out.is_empty() {
                return out;
            }
        }
    }
    Vec::new()
}

/// A polygon's rings: the exterior first, then the interiors (or a surface's patches).
fn rings(node: &Node, swap: bool) -> Vec<Vec<[f64; 2]>> {
    let mut out: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut patches: Vec<&Node> = Vec::new();
    descendants(node, "PolygonPatch", &mut patches);

    let push_ring = |ring_node: &Node, out: &mut Vec<Vec<[f64; 2]>>| {
        let mut ring = coordinates(ring_node, swap);
        if ring.len() >= 3 {
            if ring.first() != ring.last() {
                ring.push(ring[0]);
            }
            out.push(ring);
        }
    };

    for child in &node.children {
        match child.name.as_str() {
            "exterior" | "outerBoundaryIs" => push_ring(child, &mut out),
            "interior" | "innerBoundaryIs" => push_ring(child, &mut out),
            _ => {}
        }
    }
    for patch in patches {
        for child in &patch.children {
            if matches!(child.name.as_str(), "exterior" | "outerBoundaryIs" | "interior" | "innerBoundaryIs") {
                push_ring(child, &mut out);
            }
        }
    }
    out
}

fn line(node: &Node, swap: bool) -> Vec<[f64; 2]> {
    coordinates(node, swap)
}

fn geometry(node: &Node, swap: bool) -> Option<Value> {
    match node.name.as_str() {
        "Point" => {
            let p = coordinates(node, swap).into_iter().next()?;
            Some(json!({ "type": "Point", "coordinates": p }))
        }
        "LineString" | "LinearRing" | "Curve" => {
            let line = line(node, swap);
            (line.len() >= 2).then(|| json!({ "type": "LineString", "coordinates": line }))
        }
        "Polygon" | "Surface" => {
            let rings = rings(node, swap);
            (!rings.is_empty()).then(|| json!({ "type": "Polygon", "coordinates": rings }))
        }
        "MultiPoint" => {
            let mut points = Vec::new();
            let mut members: Vec<&Node> = Vec::new();
            descendants(node, "pointMember", &mut members);
            descendants(node, "Point", &mut members);
            for member in members {
                if let Some(Value::Object(mut o)) = geometry(member, swap) {
                    if let Some(Value::Array(c)) = o.remove("coordinates") {
                        points.push(Value::Array(c));
                    }
                }
            }
            (!points.is_empty()).then(|| json!({ "type": "MultiPoint", "coordinates": points }))
        }
        "MultiLineString" | "MultiCurve" => {
            let mut lines = Vec::new();
            for member in &node.children {
                if matches!(member.name.as_str(), "lineStringMember" | "curveMember") {
                    if let Some(inner) = find_geometry(member) {
                        if line(inner, swap).len() >= 2 {
                            lines.push(Value::Array(
                                line(inner, swap).into_iter().map(Value::from).collect(),
                            ));
                        }
                    }
                }
            }
            (!lines.is_empty()).then(|| json!({ "type": "MultiLineString", "coordinates": lines }))
        }
        "MultiPolygon" | "MultiSurface" => {
            let mut polygons: Vec<Value> = Vec::new();
            let mut members: Vec<&Node> = Vec::new();
            descendants(node, "polygonMember", &mut members);
            descendants(node, "surfaceMember", &mut members);
            for member in members {
                if let Some(inner) = find_geometry(member) {
                    if let Some(Value::Object(mut o)) = geometry(inner, swap) {
                        if let Some(Value::Array(c)) = o.remove("coordinates") {
                            polygons.push(Value::Array(c));
                        }
                    }
                }
            }
            (!polygons.is_empty()).then(|| json!({ "type": "MultiPolygon", "coordinates": polygons }))
        }
        _ => None,
    }
}

fn properties(feature: &Node) -> Map<String, Value> {
    let mut out = Map::new();
    for child in &feature.children {
        if child.name == "boundedBy" || is_geometry(&child.name) || find_geometry(child).is_some() {
            continue;
        }
        if child.has_children() {
            continue;
        }
        let text = child.text.trim();
        if text.is_empty() {
            continue;
        }
        out.insert(child.name.clone(), Value::String(text.to_string()));
    }
    out
}

fn feature(node: &Node, swap: bool) -> Option<Value> {
    let geom = find_geometry(node).and_then(|g| geometry(g, swap))?;
    let mut out = Map::new();
    out.insert("type".into(), json!("Feature"));
    if let Some(id) = &node.id {
        out.insert("id".into(), json!(id));
    }
    out.insert("geometry".into(), geom);
    out.insert("properties".into(), Value::Object(properties(node)));
    Some(Value::Object(out))
}

/// Every feature in a `wfs:FeatureCollection`, as GeoJSON features.
///
/// The document is walked as a stream: each `<wfs:member>`/`<gml:featureMember>` block is
/// sliced out of the input and parsed on its own. A country-wide PRG answer is 40 MB, and
/// building one tree for all of it would cost hundreds of megabytes; a feature at a time does
/// not.
pub fn parse_features(xml: &str, swap: bool) -> Result<Vec<Value>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut features = Vec::new();
    let mut in_member = false;
    let mut depth = 0usize;
    let mut feature_start: Option<usize> = None;

    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event().map_err(|e| format!("GML could not be read: {e}"))? {
            Event::Start(e) => {
                let local = e.local_name();
                if local.as_ref() == b"member" || local.as_ref() == b"featureMember" {
                    in_member = true;
                    depth = 0;
                } else if in_member {
                    if depth == 0 {
                        feature_start = Some(before);
                    }
                    depth += 1;
                }
            }
            Event::End(e) => {
                if !in_member {
                    continue;
                }
                let local = e.local_name();
                if local.as_ref() == b"member" || local.as_ref() == b"featureMember" {
                    in_member = false;
                    continue;
                }
                if depth == 0 {
                    continue;
                }
                depth -= 1;
                if depth == 0 {
                    let end = reader.buffer_position() as usize;
                    let start = feature_start.take().unwrap_or(end);
                    if start < end {
                        if let Some(node) = xml.get(start..end).and_then(|f| parse_fragment(f, swap)) {
                            features.push(node);
                        }
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(features)
}

/// Parses one feature's XML into a GeoJSON feature.
fn parse_fragment(xml: &str, swap: bool) -> Option<Value> {
    let roots = parse_tree(xml).ok()?;
    let node = roots.first()?;
    feature(node, swap)
}

/// A WKT polygon or multipolygon (`SRID=2180;POLYGON((…))`) as GeoJSON;
/// `transform` turns one `(x, y)` into `[lon, lat]`.
pub fn wkt_to_geojson(wkt: &str, transform: impl Fn(f64, f64) -> [f64; 2]) -> Option<Value> {
    let text = wkt.trim();
    let body = match text.rfind(';') {
        Some(i) => &text[i + 1..],
        None => text,
    };
    let body = body.trim();
    let upper = body.to_ascii_uppercase();
    let (multi, rest) = if upper.starts_with("MULTIPOLYGON") {
        (true, &body[12..])
    } else if upper.starts_with("POLYGON") {
        (false, &body[7..])
    } else {
        return None;
    };
    let rings = wkt_rings(rest, &transform);
    if rings.is_empty() {
        return None;
    }
    if multi {
        let polygons: Vec<Value> = rings.into_iter().map(|r| Value::Array(vec![Value::Array(r)])).collect();
        Some(json!({ "type": "MultiPolygon", "coordinates": polygons }))
    } else {
        let rings: Vec<Value> = rings.into_iter().map(Value::Array).collect();
        Some(json!({ "type": "Polygon", "coordinates": rings }))
    }
}

/// Splits a WKT polygon body (`((x y, …), (…))`) into rings of transformed points.
fn wkt_rings(body: &str, transform: &impl Fn(f64, f64) -> [f64; 2]) -> Vec<Vec<Value>> {
    let mut rings = Vec::new();
    let mut depth = 0usize;
    let mut current: Vec<[f64; 2]> = Vec::new();
    let mut number = String::new();
    let mut numbers: Vec<f64> = Vec::new();
    let flush = |numbers: &mut Vec<f64>, current: &mut Vec<[f64; 2]>| {
        if numbers.len() >= 2 {
            current.push(transform(numbers[0], numbers[1]));
        }
        numbers.clear();
    };
    for ch in body.chars() {
        match ch {
            '(' => {
                depth += 1;
                if depth >= 2 {
                    current.clear();
                    numbers.clear();
                }
            }
            ')' => {
                if depth >= 2 {
                    flush(&mut numbers, &mut current);
                    if current.len() >= 3 {
                        if current.first() != current.last() {
                            current.push(current[0]);
                        }
                        rings.push(current.iter().map(|p| json!(p)).collect());
                    }
                    current.clear();
                }
                depth = depth.saturating_sub(1);
            }
            ' ' | ',' => {
                if !number.is_empty() {
                    if let Ok(v) = number.parse::<f64>() {
                        numbers.push(v);
                    }
                    number.clear();
                }
                if ch == ',' && numbers.len() >= 2 {
                    flush(&mut numbers, &mut current);
                }
            }
            c => number.push(c),
        }
    }
    rings
}

/// A stable key for a feature: its `gml:id`, or — for GUGiK's PRG layers, which carry no
/// properties and no id — the bounding box of its geometry. Two cells of a split download
/// return the same feature identically, so the bbox matches exactly.
pub fn feature_key(feature: &Value) -> Option<String> {
    if let Some(id) = feature.get("id").and_then(Value::as_str) {
        return Some(format!("id:{id}"));
    }
    let geometry = feature.get("geometry")?;
    let mut bbox = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    let mut found = false;
    collect(geometry, &mut bbox, &mut found);
    found.then(|| format!("b:{:.7},{:.7},{:.7},{:.7}", bbox[0], bbox[1], bbox[2], bbox[3]))
}

fn collect(value: &Value, bbox: &mut [f64; 4], found: &mut bool) {
    if let Some(array) = value.as_array() {
        if array.len() >= 2 && array[0].is_number() && array[1].is_number() {
            let (x, y) = (array[0].as_f64().unwrap_or(0.0), array[1].as_f64().unwrap_or(0.0));
            bbox[0] = bbox[0].min(x);
            bbox[1] = bbox[1].min(y);
            bbox[2] = bbox[2].max(x);
            bbox[3] = bbox[3].max(y);
            *found = true;
        } else {
            for item in array {
                collect(item, bbox, found);
            }
        }
    } else if let Some(object) = value.as_object() {
        if let Some(inner) = object.get("coordinates") {
            collect(inner, bbox, found);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DZIALKA: &str = r#"<?xml version='1.0' encoding='UTF-8' ?>
<wfs:FeatureCollection xmlns:ms="http://mapserver.gis.umn.edu/mapserver" xmlns:gml="http://www.opengis.net/gml" xmlns:wfs="http://www.opengis.net/wfs">
  <gml:featureMember>
    <ms:dzialki gml:id="dzialki.101104_5.0017.1004/2">
      <gml:boundedBy><gml:Envelope srsName="urn:ogc:def:crs:EPSG::4326"><gml:lowerCorner>52.001211 18.766699</gml:lowerCorner><gml:upperCorner>52.007290 18.771934</gml:upperCorner></gml:Envelope></gml:boundedBy>
      <ms:geom>
        <gml:Polygon srsName="urn:ogc:def:crs:EPSG::4326">
          <gml:exterior><gml:LinearRing><gml:posList srsDimension="2">52.007235 18.767087 52.007290 18.766699 52.003943 18.769432 52.007235 18.767087</gml:posList></gml:LinearRing></gml:exterior>
        </gml:Polygon>
      </ms:geom>
      <ms:ID_DZIALKI>101104_5.0017.1004/2</ms:ID_DZIALKI>
      <ms:NUMER_OBREBU>17</ms:NUMER_OBREBU>
    </ms:dzialki>
  </gml:featureMember>
</wfs:FeatureCollection>"#;

    #[test]
    fn reads_a_parcel_and_swaps_to_lon_lat() {
        let features = parse_features(DZIALKA, true).unwrap();
        assert_eq!(features.len(), 1);
        let f = &features[0];
        assert_eq!(f["properties"]["ID_DZIALKI"], "101104_5.0017.1004/2");
        assert_eq!(f["properties"]["NUMER_OBREBU"], "17");
        assert_eq!(f["geometry"]["type"], "Polygon");
        let first = &f["geometry"]["coordinates"][0][0];
        assert!((first[0].as_f64().unwrap() - 18.767087).abs() < 1e-9);
        assert!((first[1].as_f64().unwrap() - 52.007235).abs() < 1e-9);
        let ring = f["geometry"]["coordinates"][0].as_array().unwrap();
        assert_eq!(ring.first(), ring.last());
    }

    #[test]
    fn reads_a_point() {
        let xml = r#"<wfs:FeatureCollection xmlns:wfs="http://www.opengis.net/wfs" xmlns:gml="http://www.opengis.net/gml" xmlns:ms="http://mapserver.gis.umn.edu/mapserver">
<gml:featureMember><ms:punkty><ms:geom><gml:Point><gml:pos>52.1 21.0</gml:pos></gml:Point></ms:geom><ms:NAZWA>Test</ms:NAZWA></ms:punkty></gml:featureMember>
</wfs:FeatureCollection>"#;
        let f = &parse_features(xml, true).unwrap()[0];
        assert_eq!(f["geometry"]["coordinates"][0], 21.0);
        assert_eq!(f["geometry"]["coordinates"][1], 52.1);
        assert_eq!(f["properties"]["NAZWA"], "Test");
    }

    #[test]
    fn reads_wkt_with_srid() {
        let wkt = "SRID=2180;POLYGON((500000 400000,500100 400000,500100 400100,500000 400000))";
        let gj = wkt_to_geojson(wkt, |x, y| [x / 1000.0, y / 1000.0]).unwrap();
        assert_eq!(gj["type"], "Polygon");
        assert_eq!(gj["coordinates"][0].as_array().unwrap().len(), 4);
    }
}
