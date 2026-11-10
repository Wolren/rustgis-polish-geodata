//! GML → GeoJSON.
//!
//! GUGiK's WFS answers GML only (it offers no GeoJSON output), so the plugin does the
//! conversion itself. The parser is deliberately generic: it walks any `wfs:member`
//! feature, takes every element without child elements as a property, and converts the
//! GML geometry it finds (Point, LineString, Polygon and their multi forms).
//!
//! GUGiK answers `SRSNAME=EPSG:4326`, whose GML coordinate order is **lat lon**. The
//! `swap` flag turns that into GeoJSON's `[lon, lat]`.

use crate::text::{self, Lang};
use quick_xml::events::Event;
use quick_xml::Reader;
use serde_json::{json, Map, Value};

struct Node {
    name: String,
    id: Option<String>,
    /// `srsDimension`, which sets the stride a coordinate list is read in.
    dimension: Option<usize>,
    text: String,
    children: Vec<Node>,
}

impl Node {
    fn new(name: &[u8], id: Option<String>, dimension: Option<usize>) -> Self {
        Node { name: String::from_utf8_lossy(name).into_owned(), id, dimension, text: String::new(), children: Vec::new() }
    }

    fn has_children(&self) -> bool {
        !self.children.is_empty()
    }
}

/// Why a GML document could not be read as a tree.
enum TreeError {
    Xml(String),
    Unterminated,
}

/// Builds the element tree, and refuses a document that stops with an element still open.
fn parse_tree(xml: &str) -> Result<Vec<Node>, TreeError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack: Vec<Node> = Vec::new();
    let mut roots: Vec<Node> = Vec::new();
    loop {
        match reader
            .read_event()
            .map_err(|e| TreeError::Xml(e.to_string()))?
        {
            Event::Start(e) => {
                let id = gml_id(&e);
                let dimension = srs_dimension(&e);
                let name = e.local_name().as_ref().to_vec();
                stack.push(Node::new(&name, id, dimension));
            }
            Event::Empty(e) => {
                let id = gml_id(&e);
                let dimension = srs_dimension(&e);
                let name = e.local_name().as_ref().to_vec();
                push_node(&mut stack, &mut roots, Node::new(&name, id, dimension));
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
    // quick-xml reports no unclosed tag at Eof, so the stack is the only witness.
    if !stack.is_empty() {
        return Err(TreeError::Unterminated);
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

/// The `srsDimension` an element states for its coordinate list, when it states one.
fn srs_dimension(e: &quick_xml::events::BytesStart<'_>) -> Option<usize> {
    for attr in e.attributes().flatten() {
        if attr.key.local_name().as_ref() == b"srsDimension" {
            return attr.unescape_value().ok()?.trim().parse().ok();
        }
    }
    None
}

fn push_node(stack: &mut [Node], roots: &mut Vec<Node>, node: Node) {
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

fn find_geometry(node: &Node) -> Option<&Node> {
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

/// Puts a degree pair in longitude and latitude, whichever order the service wrote it in.
///
/// Returns true when every pair ended in degrees; a pair still in metres comes from a service
/// that ignored the requested CRS, and the caller should say so.
pub fn normalize_lon_lat(features: &mut [Value]) -> bool {
    let mut degrees = true;
    for feature in features {
        if let Some(geometry) = feature.get_mut("geometry") {
            let mut here = true;
            normalize_geometry(geometry, 0, &mut here);
            degrees &= here;
        }
    }
    degrees
}

fn normalize_geometry(value: &mut Value, depth: usize, degrees: &mut bool) {
    if depth > 8 {
        return;
    }
    if let Some(items) = value.as_array_mut() {
        if items.len() >= 2 && items[0].is_number() && items[1].is_number() {
            let first = items[0].as_f64().unwrap_or(0.0);
            let second = items[1].as_f64().unwrap_or(0.0);
            let [lon, lat] = place(first, second);
            *degrees &= lon.abs() <= 180.0 && lat.abs() <= 90.0;
            items[0] = json!(lon);
            items[1] = json!(lat);
            return;
        }
        for item in items {
            normalize_geometry(item, depth + 1, degrees);
        }
    } else if let Some(object) = value.as_object_mut() {
        if let Some(coordinates) = object.get_mut("coordinates") {
            normalize_geometry(coordinates, depth + 1, degrees);
        }
    }
}

fn place(first: f64, second: f64) -> [f64; 2] {
    if inside(first, second) {
        return [first, second];
    }
    if inside(second, first) {
        return [second, first];
    }
    [first, second]
}

fn inside(lon: f64, lat: f64) -> bool {
    (13.4..=25.0).contains(&lon) && (48.4..=55.3).contains(&lat)
}

/// Reads a coordinate list: GML 3's `posList`/`pos` (whitespace separated) or GML 2's
/// `coordinates` (`x,y x,y`), inside `node` or below it.
///
/// Only `posList`/`pos` take `swap`: GML 2 `coordinates` is always x,y. The stride is the
/// element's `srsDimension`, so a 3D list keeps the first two numbers of every point.
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
            let step = found.dimension.unwrap_or(2).max(2);
            let flip = swap && name != "coordinates";
            let mut out = Vec::with_capacity(numbers.len() / step);
            let mut i = 0;
            while i + step <= numbers.len() {
                out.push(if flip {
                    [numbers[i + 1], numbers[i]]
                } else {
                    [numbers[i], numbers[i + 1]]
                });
                i += step;
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

/// Every feature in a `wfs:FeatureCollection`, as GeoJSON features; an OGC exception is an error.
///
/// The whole document becomes one tree, so every namespace a feature uses is in scope. A
/// country-wide PRG answer is 40 MB, and the tree is built once for all of it.
pub fn parse_features(xml: &str, swap: bool, lang: Lang) -> Result<Vec<Value>, String> {
    // The whole document is read as one tree, so every namespace a feature uses is in scope.
    let roots = parse_tree(xml).map_err(|e| match e {
        TreeError::Xml(detail) => text::gml_unreadable(lang, &detail),
        TreeError::Unterminated => text::gml_unclosed(lang),
    })?;
    // An exception report holds no feature, so its own text is the answer to report.
    if let Some(exception) = exception_of(&roots) {
        return Err(text::service_exception(lang, &exception));
    }
    let mut features = Vec::new();
    for root in &roots {
        collect_members(root, swap, &mut features);
    }
    Ok(features)
}

/// Reads features from a GML stream, one member at a time, so no whole document sits in a tree.
///
/// A 5 MB page of communes costs about 1.5 s and a tree of a million nodes when read whole. The
/// stream keeps one member in memory, and the answer grows feature by feature.
pub fn parse_features_stream<R: std::io::BufRead>(
    reader: R,
    swap: bool,
    lang: Lang,
) -> Result<Vec<Value>, String> {
    let mut reader = Reader::from_reader(reader);
    reader.config_mut().trim_text(false);
    let mut buffer = Vec::new();
    let mut stack: Vec<Node> = Vec::new();
    let mut outside: Vec<Node> = Vec::new();
    let mut features: Vec<Value> = Vec::new();
    loop {
        buffer.clear();
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(e)) => {
                let name = e.local_name().as_ref().to_vec();
                stack.push(Node::new(&name, gml_id(&e), srs_dimension(&e)));
            }
            Ok(Event::Empty(e)) => {
                let name = e.local_name().as_ref().to_vec();
                let node = Node::new(&name, gml_id(&e), srs_dimension(&e));
                push_node(&mut stack, &mut outside, node);
            }
            Ok(Event::Text(t)) => {
                if let Some(top) = stack.last_mut() {
                    let text = t
                        .unescape()
                        .map(|c| c.into_owned())
                        .unwrap_or_else(|_| String::from_utf8_lossy(&t).into_owned());
                    top.text.push_str(&text);
                }
            }
            Ok(Event::CData(t)) => {
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&String::from_utf8_lossy(&t));
                }
            }
            Ok(Event::End(_)) => {
                if let Some(node) = stack.pop() {
                    // A finished member becomes features and leaves memory.
                    if is_member(&node) {
                        collect_members(&node, swap, &mut features);
                    } else {
                        push_node(&mut stack, &mut outside, node);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(text::gml_unreadable(lang, &e.to_string())),
            _ => {}
        }
    }
    if !stack.is_empty() {
        return Err(text::gml_unclosed(lang));
    }
    if let Some(exception) = exception_of(&outside) {
        return Err(text::service_exception(lang, &exception));
    }
    // A server that wraps no member still gets its features read, at the cost of the tree.
    if features.is_empty() {
        for root in &outside {
            collect_members(root, swap, &mut features);
        }
    }
    Ok(features)
}

/// Whether a finished element is a feature wrapper: WFS 2.0 writes `member`, 1.1.0 `featureMember`.
fn is_member(node: &Node) -> bool {
    matches!(node.name.as_str(), "member" | "featureMember")
}

/// The text of the OGC exception the document carries, clipped to one message line.
fn exception_of(roots: &[Node]) -> Option<String> {
    let node = roots.iter().find_map(find_exception)?;
    let mut text = String::new();
    exception_text(node, &mut text);
    let text = if text.is_empty() { node.name.as_str() } else { text.as_str() };
    Some(clip(text, EXCEPTION_CHARS))
}

fn find_exception(node: &Node) -> Option<&Node> {
    if matches!(node.name.as_str(), "ServiceException" | "ServiceExceptionReport" | "ExceptionReport") {
        return Some(node);
    }
    node.children.iter().find_map(find_exception)
}

/// Joins every text run below an exception element, since its message may sit in a child.
fn exception_text(node: &Node, out: &mut String) {
    for word in node.text.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    for child in &node.children {
        exception_text(child, out);
    }
}

/// The message keeps this much of an exception: GUGiK's first sentence fits well inside.
const EXCEPTION_CHARS: usize = 300;

/// At most `max` characters of `text`, with an ellipsis where the rest was cut.
fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Walks the tree for member wrappers and parses the feature inside each.
fn collect_members(node: &Node, swap: bool, out: &mut Vec<Value>) {
    let name = node.name.to_ascii_lowercase();
    // WFS 1.1 writes one `featureMember` per feature; some services pack them in `featureMembers`.
    if name.ends_with("member") || name.ends_with("members") {
        for child in &node.children {
            if let Some(feature) = feature(child, swap) {
                out.push(feature);
            }
        }
        return;
    }
    for child in &node.children {
        collect_members(child, swap, out);
    }
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

/// A stable key for a feature: its `gml:id` or, when GUGiK's PRG layers carry no properties
/// and no id, the bounding box of its geometry. Two cells of a split download
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
    use serde_json::json;

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
        let features = parse_features(DZIALKA, true, Lang::En).unwrap();
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
        let f = &parse_features(xml, true, Lang::En).unwrap()[0];
        assert_eq!(f["geometry"]["coordinates"][0], 21.0);
        assert_eq!(f["geometry"]["coordinates"][1], 52.1);
        assert_eq!(f["properties"]["NAZWA"], "Test");
    }

    #[test]
    fn reports_an_ogc_exception_instead_of_an_empty_answer() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<ServiceExceptionReport version="1.2.0" xmlns="http://www.opengis.net/ogc">
  <ServiceException code="InvalidParameterValue">msWFSGetFeature(): SRSNAME is not supported</ServiceException>
</ServiceExceptionReport>"#;
        let error = parse_features(xml, true, Lang::En).unwrap_err();
        assert!(error.contains("SRSNAME is not supported"), "{error}");
        assert!(!error.contains('\n'), "one line: {error}");
    }

    #[test]
    fn reports_a_document_cut_mid_element() {
        let cut = DZIALKA.split("  </gml:featureMember>").next().unwrap();
        let error = parse_features(cut, true, Lang::En).unwrap_err();
        assert!(error.contains("ends before"), "{error}");
    }

    #[test]
    fn keeps_x_y_order_in_gml2_coordinates() {
        let xml = r#"<wfs:FeatureCollection xmlns:wfs="http://www.opengis.net/wfs" xmlns:gml="http://www.opengis.net/gml" xmlns:ms="http://mapserver.gis.umn.edu/mapserver">
<gml:featureMember><ms:punkt><ms:geom><gml:Point srsName="EPSG:4326"><gml:coordinates>21.0,52.1 21.1,52.2</gml:coordinates></gml:Point></ms:geom><ms:NAZWA>Test</ms:NAZWA></ms:punkt></gml:featureMember>
</wfs:FeatureCollection>"#;
        let f = &parse_features(xml, true, Lang::En).unwrap()[0];
        assert_eq!(f["geometry"]["coordinates"][0], 21.0);
        assert_eq!(f["geometry"]["coordinates"][1], 52.1);
    }

    #[test]
    fn steps_a_three_dimensional_pos_list_by_its_dimension() {
        let xml = r#"<wfs:FeatureCollection xmlns:wfs="http://www.opengis.net/wfs" xmlns:gml="http://www.opengis.net/gml" xmlns:ms="http://mapserver.gis.umn.edu/mapserver">
<gml:featureMember><ms:linia><ms:geom><gml:LineString srsName="urn:ogc:def:crs:EPSG::4326"><gml:posList srsDimension="3">52.1 21.0 100 52.2 21.1 110</gml:posList></gml:LineString></ms:geom></ms:linia></gml:featureMember>
</wfs:FeatureCollection>"#;
        let f = &parse_features(xml, true, Lang::En).unwrap()[0];
        let line = f["geometry"]["coordinates"].as_array().unwrap();
        assert_eq!(line.len(), 2);
        assert_eq!(line[0][0], 21.0);
        assert_eq!(line[1][1], 52.2);
    }

    #[test]
    fn reads_wkt_with_srid() {
        let wkt = "SRID=2180;POLYGON((500000 400000,500100 400000,500100 400100,500000 400000))";
        let gj = wkt_to_geojson(wkt, |x, y| [x / 1000.0, y / 1000.0]).unwrap();
        assert_eq!(gj["type"], "Polygon");
        assert_eq!(gj["coordinates"][0].as_array().unwrap().len(), 4);
    }

    #[test]
    fn a_degree_pair_reads_in_either_order() {
        let mut features = vec![
            json!({"geometry": {"type": "Point", "coordinates": [52.4, 16.9]}}),
            json!({"geometry": {"type": "Point", "coordinates": [16.9, 52.4]}}),
        ];
        normalize_lon_lat(&mut features);
        assert_eq!(features[0]["geometry"]["coordinates"][0], json!(16.9));
        assert_eq!(features[0]["geometry"]["coordinates"][1], json!(52.4));
        assert_eq!(features[1]["geometry"]["coordinates"][0], json!(16.9));
    }

    #[test]
    fn a_pair_in_metres_is_left_and_reported() {
        // 2180 metres: the axis order cannot be told apart by range, so the caller must say.
        let mut features = vec![json!({"geometry": {"type": "Point", "coordinates": [507142.0, 216500.0]}})];
        assert!(!normalize_lon_lat(&mut features));
        assert_eq!(features[0]["geometry"]["coordinates"][0], json!(507142.0));
    }
}

#[cfg(test)]
mod serviced_points {
    use super::*;
    use crate::text::Lang;

    /// The address service answers in EPSG:2180 with the northing first, an empty collection
    /// when the box arrives in lon and lat, and namespaces only on the root element.
    #[test]
    fn parses_an_answer_served_in_the_national_grid() {
        let xml = include_str!("../testdata/kina-point.xml");
        let mut features = parse_features(xml, false, Lang::En).expect("the answer parses");
        assert_eq!(features.len(), 1, "one address point");
        let geometry = features[0].get_mut("geometry").expect("geometry");
        let coordinates = geometry.get_mut("coordinates").expect("coordinates");
        let point = coordinates.as_array().expect("a point");
        let northing = point[0].as_f64().unwrap();
        let easting = point[1].as_f64().unwrap();
        let (lon, lat) = crate::gk92::inverse(northing, easting);
        assert!((lon - 16.93).abs() < 0.02, "lon {lon}");
        assert!((lat - 52.40).abs() < 0.02, "lat {lat}");
    }

    #[test]
    fn a_stream_reads_both_member_wrappers() {
        let body = r#"<wfs:FeatureCollection xmlns:wfs="http://www.opengis.net/wfs" xmlns:gml="http://www.opengis.net/gml" xmlns:ms="http://mapserver.gis.umn.edu/mapserver">
            <wfs:member><ms:dane><gml:Point><gml:pos>16.9 52.4</gml:pos></gml:Point><ms:nazwa>Poznań</ms:nazwa></ms:dane></wfs:member>
            <gml:featureMember><ms:dane><gml:Point><gml:pos>17.0 52.5</gml:pos></gml:Point><ms:nazwa>Kórnik</ms:nazwa></ms:dane></gml:featureMember>
        </wfs:FeatureCollection>"#;
        let rows = parse_features_stream(std::io::Cursor::new(body.as_bytes()), false, Lang::En).expect("stream");
        assert_eq!(rows.len(), 2, "{rows:?}");
        assert_eq!(rows[0]["properties"]["nazwa"], "Poznań");
    }

    #[test]
    fn a_stream_reports_an_exception_and_an_unclosed_document() {
        let exception = r#"<ows:ExceptionReport xmlns:ows="http://www.opengis.net/ows"><ows:Exception><ows:ExceptionText>Boom</ows:ExceptionText></ows:Exception></ows:ExceptionReport>"#;
        let refused = parse_features_stream(std::io::Cursor::new(exception.as_bytes()), false, Lang::En);
        assert!(refused.is_err());

        let cut = r#"<wfs:FeatureCollection><wfs:member><ms:dane><gml:Point>"#;
        assert!(parse_features_stream(std::io::Cursor::new(cut.as_bytes()), false, Lang::En).is_err());
    }
}

#[cfg(test)]
mod bench {
    use super::parse_features;
    use crate::text::Lang;

    /// Times a page the caller names, so a parser change is measured on real data.
    #[test]
    #[ignore]
    fn parse_bench() {
        let path = std::env::var("PAGE").expect("PAGE");
        let body = std::fs::read_to_string(&path).expect("page");
        let started = std::time::Instant::now();
        let features = parse_features(&body, true, Lang::En).expect("parsed");
        println!(
            "bench: {} features, {} bytes, {:.3} s",
            features.len(),
            body.len(),
            started.elapsed().as_secs_f64()
        );
    }
}
