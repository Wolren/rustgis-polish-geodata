//! The elevation model: one GUGiK WCS coverage of the area the user picked.
//!
//! The area never comes from the map view. It is the layer named in `area`, else the layer with a
//! selection, else the active vector layer. Its vertices go to PL-1992 whole metres, so every cell
//! is 1 m. PL-KRON86-NH travels as GeoTIFF and RustGIS downloads it on its own; the ASCII systems
//! come back through this program, which writes the `.prj` the grid needs to be placed.
//!
//! A command holds the window while it runs, so the ASCII grid's area is measured first: GUGiK
//! moved 99.5 MB in 50 s (measured 2026-10-27), and one click may hold the window for 12 s.

use crate::cache;
use crate::gk92;
use crate::host::Host;
use crate::http;
use crate::obszar;
use crate::text::{self, Lang};
use serde_json::{json, Value};
use std::path::Path;

const TERRAIN_ASCII: &str =
    "https://mapy.geoportal.gov.pl/wss/service/PZGIK/NMT/GRID1/WCS/DigitalTerrainModel";
const TERRAIN_TIFF: &str =
    "https://mapy.geoportal.gov.pl/wss/service/PZGIK/NMT/GRID1/WCS/DigitalTerrainModelFormatTIFF";
const SURFACE_ASCII: &str =
    "https://mapy.geoportal.gov.pl/wss/service/PZGIK/NMPT/GRID1/WCS/DigitalSurfaceModel";

/// Bytes one cell costs on the wire: float32 GeoTIFF, and the text an ASCII grid writes.
const TIFF_BYTES: f64 = 4.0;
const ASCII_BYTES: f64 = 19.6;
/// Download speeds measured against GUGiK: 256 MB of GeoTIFF in 43 s, 99.5 MB of ASCII in 50 s.
const TIFF_BYTES_PER_SECOND: f64 = 5.9e6;
const ASCII_BYTES_PER_SECOND: f64 = 2.0e6;
/// One click may hold the window this long. The download is not interruptible.
const BUDGET_SECONDS: f64 = 12.0;
/// GUGiK drops a TLS connection now and then: one request in three failed this way (2026-10-27).
const ATTEMPTS: u32 = 3;
const RETRY_PAUSE: std::time::Duration = std::time::Duration::from_secs(1);

/// Which coverage the click asks for. Each one is a ribbon entry of its own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// The terrain model in PL-KRON86-NH, the only one GUGiK publishes as GeoTIFF.
    TerrainKron,
    /// The terrain model in PL-EVRF2007-NH, an ASCII grid of 19.6 bytes a cell.
    TerrainEvrf,
    /// The surface model, buildings and trees kept, an ASCII grid.
    SurfaceKron,
}

impl Kind {
    pub fn surface(self) -> bool {
        matches!(self, Kind::SurfaceKron)
    }

    pub fn datum(self) -> &'static str {
        match self {
            Kind::TerrainEvrf => "PL-EVRF2007-NH",
            _ => "PL-KRON86-NH",
        }
    }

    pub fn coverage(self) -> &'static str {
        match self {
            Kind::TerrainKron => "DTM_PL-KRON86-NH_TIFF",
            Kind::TerrainEvrf => "DTM_PL-EVRF2007-NH",
            Kind::SurfaceKron => "DSM_PL-KRON86-NH",
        }
    }

    fn endpoint(self) -> &'static str {
        match self {
            Kind::TerrainKron => TERRAIN_TIFF,
            Kind::TerrainEvrf => TERRAIN_ASCII,
            Kind::SurfaceKron => SURFACE_ASCII,
        }
    }

    fn tiff(self) -> bool {
        matches!(self, Kind::TerrainKron)
    }

    fn bytes_per_cell(self) -> f64 {
        if self.tiff() {
            TIFF_BYTES
        } else {
            ASCII_BYTES
        }
    }

    fn bytes_per_second(self) -> f64 {
        if self.tiff() {
            TIFF_BYTES_PER_SECOND
        } else {
            ASCII_BYTES_PER_SECOND
        }
    }
}

/// Adds the elevation model of the area, or says what the area needs.
pub fn add(host: &mut Host, lang: Lang, request: &Value, kind: Kind) -> Result<Value, String> {
    let (source, features) = area(host, lang, request, text::nmt_needs_area(lang))?;
    let boxes = envelopes(&features).ok_or_else(|| text::area_no_geometry(lang, &source))?;
    if !gk92::inside_poland(
        (boxes.lon[0] + boxes.lon[2]) / 2.0,
        (boxes.lon[1] + boxes.lon[3]) / 2.0,
    ) {
        return Err(text::nmt_outside_poland(lang));
    }

    let (x0, y0, width, height) = snap(boxes.metres);
    let cells = f64::from(width) * f64::from(height);
    let bytes = cells * kind.bytes_per_cell();
    let seconds = bytes / kind.bytes_per_second();

    let wanted = text::nmt_layer_name(kind.surface(), kind.datum());
    let address = url(kind, x0, y0, width, height);
    let mut megabytes = bytes / 1e6;
    let mut pending = false;
    let name = if kind.tiff() {
        // The app downloads a GeoTIFF off the frame, so it may answer that the file is on its way.
        let answer = attempts(|| host.call("layer.addUrl", json!({ "url": address, "name": wanted })))?;
        pending = answer.get("pending").and_then(Value::as_bool) == Some(true);
        if pending {
            wanted.clone()
        } else {
            resolved_name(host, &wanted)
        }
    } else {
        let file = cache::dem_file(&format!(
            "{}_{x0}_{y0}_{width}x{height}.asc",
            kind.coverage()
        ));
        if file.exists() {
            // The grid is already on disk: the click costs no download, so no budget either.
            megabytes = file.metadata().map(|meta| meta.len()).unwrap_or(0) as f64 / 1e6;
        } else {
            // A big grid is measured first: the second press within two minutes downloads it.
            if seconds > BUDGET_SECONDS {
                let key = format!("{}:{x0},{y0},{width}x{height}", kind.coverage());
                if cache::dem_confirm(std::time::SystemTime::now()).as_deref() != Some(key.as_str()) {
                    cache::remember_dem_confirm(&key, std::time::SystemTime::now());
                    return Err(text::nmt_confirm_box(
                        lang,
                        cells,
                        megabytes,
                        seconds,
                        cache::CONFIRM_SECONDS,
                    ));
                }
            }
            cache::forget_dem_confirm();
            megabytes = http::download(&address, &file, 110, lang)? as f64 / 1e6;
        }
        // An ASCII grid carries no CRS, so the sidecar places it.
        let prj = file.with_extension("prj");
        std::fs::write(&prj, gk92::WKT)
            .map_err(|e| text::file_failed(lang, &path_text(&prj), &e.to_string()))?;
        let path = path_text(&file);
        attempts(|| host.call("layer.addData", json!({ "path": path, "name": wanted })))?;
        resolved_name(host, &wanted)
    };

    // A GeoTIFF still on its way is not on the map, so there is nothing to shade yet.
    let styled = !pending && host.call("raster.symbology", look(&name, colours(request))).is_ok();
    tidy(host, &source, boxes.lon, &features, lang)?;
    if pending {
        host.notify(&text::nmt_pending(lang, &wanted, &source));
    } else {
        host.notify(&text::nmt_added(
            lang,
            &name,
            kind.datum(),
            (width, height),
            megabytes,
            &source,
            colours(request),
        ));
        // A model is a rectangle, so a drawn shape becomes a box, and the layer shows that box.
        if text::AREA_LAYERS.contains(&source.as_str()) && !covers_its_box(&features, boxes.lon) {
            host.notify(&text::nmt_box_note(lang, &source, boxes.lon));
        }
    }
    Ok(json!({
        "layer": name,
        "area": source,
        "coverage": kind.coverage(),
        "datum": kind.datum(),
        "format": if kind.tiff() { "GeoTIFF" } else { "ASCII grid" },
        "resolution": 1,
        "width": width,
        "height": height,
        "box": [x0, y0, x0 + i64::from(width), y0 + i64::from(height)],
        "megabytes": megabytes.round(),
        "styled": styled,
        "pending": pending,
        "look": if colours(request) { "colours" } else { "shading" },
    }))
}

/// Leaves the map usable after a build: a drawn area becomes the box, and the tool returns to explore.
fn tidy(
    host: &mut Host,
    source: &str,
    box_lon: [f64; 4],
    features: &[Value],
    lang: Lang,
) -> Result<(), String> {
    if text::AREA_LAYERS.contains(&source) {
        // The layer's own geometry decides: a rectangle is left alone, a drawn shape is boxed.
        let mode = obszar::Mode::of(features).unwrap_or_else(obszar::Mode::stored);
        if mode == obszar::Mode::Drawn {
            obszar::snap(host, source, box_lon, lang)?;
        }
    }
    host.call("tool.set", json!({ "tool": "explore" })).ok();
    Ok(())
}

/// Colours or shades the elevation model already on the map, a separate click on purpose.
pub fn style(host: &mut Host, lang: Lang, request: &Value) -> Result<Value, String> {
    let named = request
        .get("params")
        .and_then(|p| p.get("layer"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let layer = if named.is_empty() {
        newest_model(host, lang)?
    } else {
        named.to_string()
    };
    host.call("raster.symbology", look(&layer, colours(request)))?;
    host.notify(&text::nmt_styled(lang, &layer, colours(request)));
    Ok(json!({ "layer": layer, "look": if colours(request) { "colours" } else { "shading" } }))
}

/// A shaded relief shows the ground's shape, which is what a model is for. Colours are the second
/// choice, and they band by equal intervals: quantiles bunch on flat ground.
fn look(layer: &str, colours: bool) -> Value {
    if colours {
        json!({
            "layer": layer, "type": "classify", "classes": 12,
            "ramp": "terrain", "method": "equalinterval"
        })
    } else {
        json!({
            "layer": layer, "type": "shadedRelief", "band": 1,
            "azimuth": 315, "altitude": 45, "grey": true
        })
    }
}

fn colours(request: &Value) -> bool {
    request
        .get("params")
        .and_then(|params| params.get("colours"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// The area the user pointed at, and whether only its selection counts.
pub fn area(
    host: &mut Host,
    lang: Lang,
    request: &Value,
    missing: String,
) -> Result<(String, Vec<Value>), String> {
    let named = request
        .get("params")
        .and_then(|params| params.get("area"))
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty());
    let layers = host.call("host.layers", json!({}))?;
    let list = layers.as_array().cloned().unwrap_or_default();
    let (name, only_selected) = match named {
        Some(want) => {
            let layer = list
                .iter()
                .find(|layer| name_of(layer).as_deref() == Some(want))
                .ok_or_else(|| text::area_not_passed(lang, want))?;
            if !vector(layer) {
                return Err(text::area_no_geometry(lang, want));
            }
            (want.to_string(), selected(layer))
        }
        None => choose(&list, &text::AREA_LAYERS).ok_or(missing)?,
    };
    let answer = host.call(
        "host.features",
        json!({ "layer": name, "selected": only_selected }),
    )?;
    let features: Vec<Value> = answer
        .get("features")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if features.is_empty() {
        return Err(text::area_no_geometry(lang, &name));
    }
    Ok((name, features))
}

fn name_of(layer: &Value) -> Option<String> {
    layer
        .get("name")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn vector(layer: &Value) -> bool {
    layer.get("raster").and_then(Value::as_bool) != Some(true)
        && layer.get("features").and_then(Value::as_i64).unwrap_or(0) > 0
}

fn selected(layer: &Value) -> bool {
    layer.get("selected").and_then(Value::as_i64).unwrap_or(0) > 0
}

fn active(layer: &Value) -> bool {
    layer.get("active").and_then(Value::as_bool) == Some(true)
}

/// The layer an elevation model builds from when no area is named: a selection on the active layer
/// first, because selecting features is a deliberate pick, then the drawn area layer, then the
/// active layer whole. A selection on some other layer is not consulted: a click must not take its
/// area from a layer the user is not looking at.
fn choose(list: &[Value], areas: &[&str]) -> Option<(String, bool)> {
    if let Some(layer) = list
        .iter()
        .find(|layer| vector(layer) && active(layer) && selected(layer))
    {
        return name_of(layer).map(|name| (name, true));
    }
    let drawn = list.iter().find(|layer| {
        vector(layer)
            && name_of(layer)
                .map(|name| areas.contains(&name.as_str()))
                .unwrap_or(false)
    });
    if let Some(layer) = drawn {
        return name_of(layer).map(|name| (name, false));
    }
    list.iter()
        .find(|layer| vector(layer) && active(layer))
        .and_then(|layer| name_of(layer).map(|name| (name, false)))
}

/// The area's extent in degrees and in PL-1992 metres, taken from every vertex.
pub struct Boxes {
    pub lon: [f64; 4],
    pub metres: [f64; 4],
}

impl Boxes {
    fn new() -> Boxes {
        Boxes {
            lon: [
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ],
            metres: [
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ],
        }
    }

    fn empty(&self) -> bool {
        self.metres[2] <= self.metres[0] || self.metres[3] <= self.metres[1]
    }
}

/// Whether the shape drawn is already the box: four corners, and every one of them on the box's.
/// A triangle, an L or a freehand ring is not, and the model will cover a rectangle around it.
fn covers_its_box(features: &[Value], box_lon: [f64; 4]) -> bool {
    let [west, south, east, north] = box_lon;
    let corners = [[west, south], [east, south], [east, north], [west, north]];
    let mut points: Vec<[f64; 2]> = Vec::new();
    for feature in features {
        let Some(coordinates) = feature
            .get("geometry")
            .and_then(|geometry| geometry.get("coordinates"))
        else {
            continue;
        };
        walk(coordinates, &mut |lon, lat| {
            let point = [(lon * 1e7).round() / 1e7, (lat * 1e7).round() / 1e7];
            if !points.contains(&point) {
                points.push(point);
            }
        });
    }
    points.len() == 4
        && corners.iter().all(|corner| {
            let corner = [
                (corner[0] * 1e7).round() / 1e7,
                (corner[1] * 1e7).round() / 1e7,
            ];
            points.contains(&corner)
        })
}

pub fn envelopes(features: &[Value]) -> Option<Boxes> {
    let mut boxes = Boxes::new();
    for feature in features {
        let Some(coordinates) = feature
            .get("geometry")
            .and_then(|geometry| geometry.get("coordinates"))
        else {
            continue;
        };
        walk(coordinates, &mut |lon, lat| {
            let (east, north) = gk92::forward(lon, lat);
            if !east.is_finite() || !north.is_finite() {
                return;
            }
            boxes.lon[0] = boxes.lon[0].min(lon);
            boxes.lon[1] = boxes.lon[1].min(lat);
            boxes.lon[2] = boxes.lon[2].max(lon);
            boxes.lon[3] = boxes.lon[3].max(lat);
            boxes.metres[0] = boxes.metres[0].min(east);
            boxes.metres[1] = boxes.metres[1].min(north);
            boxes.metres[2] = boxes.metres[2].max(east);
            boxes.metres[3] = boxes.metres[3].max(north);
        });
    }
    (!boxes.empty()).then_some(boxes)
}

/// Walks a GeoJSON coordinate tree, which nests as deep as the geometry type does.
fn walk(node: &Value, visit: &mut impl FnMut(f64, f64)) {
    let Value::Array(items) = node else { return };
    if items.len() >= 2 && items[0].is_number() && items[1].is_number() {
        if let (Some(x), Some(y)) = (items[0].as_f64(), items[1].as_f64()) {
            visit(x, y);
        }
        return;
    }
    for item in items {
        walk(item, visit);
    }
}

/// The whole-metre box a one-metre grid needs: floor the low edges, ceil the high ones.
pub fn snap(metres: [f64; 4]) -> (i64, i64, u32, u32) {
    let x0 = metres[0].floor();
    let y0 = metres[1].floor();
    let x1 = metres[2].ceil();
    let y1 = metres[3].ceil();
    (
        x0 as i64,
        y0 as i64,
        (x1 - x0).max(1.0) as u32,
        (y1 - y0).max(1.0) as u32,
    )
}

/// The coverage's address: a whole-metre PL-1992 box, one cell per metre.
pub fn url(kind: Kind, x0: i64, y0: i64, width: u32, height: u32) -> String {
    let format = if kind.tiff() {
        "image/tiff"
    } else {
        "image/x-aaigrid"
    };
    let x1 = x0 + i64::from(width);
    let y1 = y0 + i64::from(height);
    format!(
        "{}?SERVICE=WCS&VERSION=1.0.0&REQUEST=GetCoverage&FORMAT={format}&COVERAGE={}&BBOX={x0},{y0},{x1},{y1}&CRS=EPSG:2180&RESPONSE_CRS=EPSG:2180&WIDTH={width}&HEIGHT={height}",
        kind.endpoint(),
        kind.coverage()
    )
}

/// Runs a host call until it answers. The app downloads the address itself, and a dropped TLS
/// connection comes back as an error, so a second try usually lands.
fn attempts(mut call: impl FnMut() -> Result<Value, String>) -> Result<Value, String> {
    let mut last = String::new();
    for attempt in 0..ATTEMPTS {
        match call() {
            Ok(answer) => return Ok(answer),
            Err(error) => last = error,
        }
        if attempt + 1 < ATTEMPTS {
            std::thread::sleep(RETRY_PAUSE);
        }
    }
    Err(last)
}

/// The name the layer ended up with: the app appends a counter when the name is taken, and the
/// call answer does not carry it, so the newest layer under that name is the one just added.
fn resolved_name(host: &mut Host, wanted: &str) -> String {
    let Ok(layers) = host.call("host.layers", json!({})) else {
        return wanted.to_string();
    };
    let list = layers.as_array().cloned().unwrap_or_default();
    let mut best: Option<(i64, String)> = None;
    for layer in &list {
        let Some(name) = layer.get("name").and_then(Value::as_str) else {
            continue;
        };
        if !name.starts_with(wanted) {
            continue;
        }
        let id = layer.get("id").and_then(Value::as_i64).unwrap_or(0);
        if best.as_ref().map(|(seen, _)| id > *seen).unwrap_or(true) {
            best = Some((id, name.to_string()));
        }
    }
    best.map(|(_, name)| name)
        .unwrap_or_else(|| wanted.to_string())
}

/// The newest raster this plugin added, by its name.
fn newest_model(host: &mut Host, lang: Lang) -> Result<String, String> {
    let layers = host.call("host.layers", json!({}))?;
    let list = layers.as_array().cloned().unwrap_or_default();
    let mut best: Option<(i64, String)> = None;
    for layer in &list {
        if layer.get("raster").and_then(Value::as_bool) != Some(true) {
            continue;
        }
        let Some(name) = layer.get("name").and_then(Value::as_str) else {
            continue;
        };
        if !(name.starts_with("NMT ") || name.starts_with("NMPT ")) {
            continue;
        }
        let id = layer.get("id").and_then(Value::as_i64).unwrap_or(0);
        if best.as_ref().map(|(seen, _)| id > *seen).unwrap_or(true) {
            best = Some((id, name.to_string()));
        }
    }
    best.map(|(_, name)| name)
        .ok_or_else(|| text::nmt_nothing_to_style(lang))
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn polygon(ring: &[(f64, f64)]) -> Value {
        let coordinates: Vec<Value> = ring.iter().map(|(x, y)| json!([x, y])).collect();
        json!({
            "type": "Feature",
            "properties": {},
            "geometry": { "type": "Polygon", "coordinates": [coordinates] }
        })
    }

    #[test]
    fn reads_the_box_of_a_drawn_area() {
        // The corner PROJ puts at 348129, 497807 in PL-1992.
        let drawn = polygon(&[
            (16.770772893, 52.325191293),
            (16.800000000, 52.325191293),
            (16.800000000, 52.340000000),
            (16.770772893, 52.325191293),
        ]);
        let boxes = envelopes(&[drawn]).expect("a box");
        let (x0, y0, width, height) = snap(boxes.metres);
        assert!(
            (347_000..=348_129).contains(&x0),
            "the west edge covers the corner: {x0}"
        );
        assert!(
            (496_000..=497_807).contains(&y0),
            "the south edge covers the corner: {y0}"
        );
        assert!(width > 2_000 && width < 2_100, "{width}");
        assert!(height > 1_500 && height < 1_700, "{height}");
    }

    fn layer(name: &str, active: bool, selected: i64, features: i64) -> Value {
        json!({ "name": name, "active": active, "selected": selected, "features": features, "raster": false })
    }

    #[test]
    fn the_active_layers_selection_wins() {
        let list = [
            layer("a", false, 40, 100),
            layer("b", true, 2, 100),
            layer("c", false, 0, 100),
        ];
        assert_eq!(choose(&list, &[]), Some(("b".to_string(), true)));
    }

    #[test]
    fn a_selection_on_another_layer_is_ignored() {
        let list = [layer("a", true, 0, 100), layer("b", false, 9, 100)];
        // The active layer, not a selection the user cannot see.
        assert_eq!(choose(&list, &[]), Some(("a".to_string(), false)));
    }

    #[test]
    fn the_drawn_area_layer_beats_the_active_layer() {
        let list = [
            layer("Area to download", false, 0, 1),
            layer("parcels", true, 0, 6),
        ];
        assert_eq!(
            choose(&list, &["Area to download"]),
            Some(("Area to download".to_string(), false))
        );
    }

    #[test]
    fn the_active_layer_alone_is_the_area() {
        let list = [layer("a", false, 0, 4), layer("b", true, 0, 6)];
        assert_eq!(
            choose(&list, &["Area to download"]),
            Some(("b".to_string(), false))
        );
    }

    #[test]
    fn rasters_and_empty_layers_are_never_the_area() {
        let list = [
            json!({ "name": "model", "active": true, "selected": 3, "features": 0, "raster": true }),
            json!({ "name": "empty", "active": false, "selected": 0, "features": 0, "raster": false }),
        ];
        assert_eq!(
            choose(&list, &["Area to download"]),
            None,
            "no vector feature to build from"
        );
    }

    #[test]
    fn a_flat_feature_has_no_box() {
        let point = json!({ "type": "Feature", "properties": {}, "geometry": { "type": "Point", "coordinates": [16.9, 52.4] } });
        assert!(envelopes(&[point]).is_none(), "one vertex is not an area");
    }

    #[test]
    fn snapping_covers_the_edges_to_whole_metres() {
        let (x0, y0, width, height) = snap([348_129.26, 497_807.86, 350_445.85, 500_000.0]);
        assert_eq!((x0, y0), (348_129, 497_807));
        assert_eq!((width, height), (2317, 2193));
    }

    #[test]
    fn the_address_asks_for_one_metre_cells() {
        let address = url(Kind::TerrainKron, 348_129, 497_807, 2317, 2193);
        assert!(
            address.contains("COVERAGE=DTM_PL-KRON86-NH_TIFF"),
            "{address}"
        );
        assert!(address.contains("FORMAT=image/tiff"), "{address}");
        assert!(
            address.contains("BBOX=348129,497807,350446,500000"),
            "{address}"
        );
        assert!(
            address.contains("CRS=EPSG:2180&RESPONSE_CRS=EPSG:2180"),
            "{address}"
        );
        assert!(address.contains("WIDTH=2317&HEIGHT=2193"), "{address}");
        let ascii = url(Kind::TerrainEvrf, 0, 0, 1000, 1000);
        assert!(ascii.contains("FORMAT=image/x-aaigrid"), "{ascii}");
        assert!(ascii.contains("COVERAGE=DTM_PL-EVRF2007-NH"), "{ascii}");
    }

    #[test]
    fn only_the_kron86_terrain_model_is_geotiff() {
        assert!(Kind::TerrainKron.tiff());
        assert!(!Kind::TerrainEvrf.tiff(), "EVRF2007 comes as an ASCII grid");
        assert!(!Kind::SurfaceKron.tiff(), "the surface model has no TIFF");
        assert_eq!(Kind::SurfaceKron.coverage(), "DSM_PL-KRON86-NH");
        assert_eq!(Kind::TerrainEvrf.datum(), "PL-EVRF2007-NH");
    }

    #[test]
    fn a_shaded_relief_is_grey_and_banded_colours_are_not() {
        let shaded = look("NMT", false);
        assert_eq!(shaded["type"], "shadedRelief");
        assert_eq!(shaded["grey"], true, "a ramp turns a relief into bands");
        let coloured = look("NMT", true);
        assert_eq!(coloured["type"], "classify");
        assert_eq!(
            coloured["method"], "equalinterval",
            "quantiles bunch on flat ground"
        );
        assert_eq!(coloured["ramp"], "terrain");
    }

    #[test]
    fn the_budget_matches_the_measured_speed() {
        // 4 by 4 km as GeoTIFF is 64 MB, about 11 s at 5.9 MB/s.
        let cells = 16_000_000.0;
        let seconds = cells * TIFF_BYTES / TIFF_BYTES_PER_SECOND;
        assert!(seconds < BUDGET_SECONDS && seconds > 10.0, "{seconds}");
        // The same area as ASCII is 313 MB and over four minutes, so it is refused first.
        let ascii = cells * ASCII_BYTES / ASCII_BYTES_PER_SECOND;
        assert!(ascii > BUDGET_SECONDS * 10.0, "{ascii}");
    }
}
