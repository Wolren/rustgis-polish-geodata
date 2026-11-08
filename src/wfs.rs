//! Downloading features from GUGiK's WFS.
//!
//! GUGiK answers full-detail GML (a whole country's counties is ~42 MB and half a minute of
//! server time), so the plugin:
//!
//! - splits a large area into a 2×2 grid and fetches the cells **in parallel** (each cell's
//!   answer is a quarter of the bytes, and four requests finish in roughly the time of one),
//! - pages each cell with `MAXFEATURES`/`STARTINDEX` on WFS 1.1 (2.0 would filter in the
//!   layer's native EPSG:2180 and answer nothing),
//! - removes the features that fall into two cells, by their `gml:id`.

use crate::catalog::WfsLayer;
use crate::filter;
use crate::gk92;
use crate::gml;
use crate::http;
use crate::text::{self, Lang};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::time::Instant;

const PAGE: usize = 500;
/// Pages requested at once. Four parallel pages beat sequential by 2.57x, measured.
const PAGES_IN_FLIGHT: usize = 4;
/// A feature heavier than this switches the run to small pages.
const HEAVY_BYTES: u64 = 30_000;
/// The page size once features are heavy.
const LIGHT_PAGE: usize = 50;
const BUDGET_SECS: u64 = 240;
/// One download stops at 6000 features; the tool gate reads this same number.
pub const MAX_TOTAL: usize = 6000;
/// Above this width or height, an area is split into four cells fetched at once.
const SPLIT_SPAN: f64 = 1.5;

/// The coordinate system a request filters in. Most services filter in lon and lat; the address
/// and street service answers nothing there and needs the national grid.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Srs {
    LonLat,
    Grid,
}

impl Srs {
    fn name(self) -> &'static str {
        match self {
            Srs::LonLat => "EPSG:4326",
            Srs::Grid => "EPSG:2180",
        }
    }
}

/// Whether the area is wider than the layer's limit in either direction.
pub fn too_large(layer: &WfsLayer, bbox: [f64; 4]) -> bool {
    bbox[2] - bbox[0] > layer.max_span || bbox[3] - bbox[1] > layer.max_span
}

/// What one download brought back, and what the user still has to be told.
pub struct Download {
    pub features: Vec<Value>,
    pub truncated: bool,
    /// Bytes the answers took, as the service sent them.
    pub bytes: u64,
    /// Whether the run stopped at its byte limit rather than at the end of the data.
    pub limit_hit: bool,
    /// Requests that never answered, so the list is short of their features.
    pub failed: usize,
    /// `Some` when the shape could not filter, naming why the box went in instead.
    pub fallback: Option<filter::ShapeProblem>,
}

/// Downloads every feature for `bbox`, reporting progress through `report`.
///
/// `area` is the area layer's GeoJSON. A single polygon goes in as the service's filter instead
/// of its bounding box, which cut 468 features to 189 on a measured strip. A request that fails
/// costs only its own features, and the answer counts them.
pub fn fetch(
    layer: &WfsLayer,
    bbox: [f64; 4],
    area: Option<&Value>,
    report: &mut dyn FnMut(&str),
    lang: Lang,
    limit: u64,
) -> Result<Download, String> {
    if ![bbox[0], bbox[1], bbox[2], bbox[3]]
        .iter()
        .all(|v| v.is_finite())
    {
        return Err(text::span_invalid(lang));
    }
    let mut fallback = None;
    let polygon = area.and_then(|collection| match filter::polygon_filter(layer, collection) {
        Ok(filter) => Some(filter),
        Err(problem) => {
            fallback = Some(problem);
            None
        }
    });
    let mut download = gather(layer, bbox, polygon.as_deref(), report, lang, Srs::LonLat, limit)?;
    if download.features.is_empty() && download.failed == 0 {
        // The address and street service answers an empty collection outside the national grid.
        let grid = area.and_then(|collection| filter::polygon_filter_grid(layer, collection).ok());
        if let Ok(retry) = gather(layer, bbox, grid.as_deref(), report, lang, Srs::Grid, limit) {
            if !retry.features.is_empty() {
                download = retry;
            }
        }
    }
    download.fallback = fallback;
    Ok(download)
}

/// One attempt over the area in a given coordinate system; a failed request is counted, not fatal.
fn gather(
    layer: &WfsLayer,
    bbox: [f64; 4],
    polygon: Option<&str>,
    report: &mut dyn FnMut(&str),
    lang: Lang,
    srs: Srs,
    limit: u64,
) -> Result<Download, String> {
    let areas = split_cells(bbox);
    report(&text::split_areas(lang, layer.name, areas.len()));

    // The budget is shared out, so a run of many cells cannot pass the limit.
    let budget = (limit / areas.len().max(1) as u64).max(1);
    let mut cells: Vec<Cell> = Vec::new();
    if areas.len() == 1 {
        cells.push(fetch_cell(layer, areas[0], polygon, report, lang, srs, budget));
    } else {
        let total = areas.len();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            for (index, area) in areas.iter().enumerate() {
                let sender = sender.clone();
                let area = *area;
                scope.spawn(move || {
                    let done = fetch_cell(layer, area, polygon, &mut |_: &str| {}, lang, srs, budget);
                    let _ = sender.send((index, done));
                });
            }
            drop(sender);
            let mut done = 0;
            for (_, cell) in receiver {
                done += 1;
                match cell.first_error.as_deref() {
                    Some(error) => {
                        report(&text::area_progress_error(lang, layer.name, done, total, error))
                    }
                    None => report(&text::area_progress(
                        lang,
                        layer.name,
                        done,
                        total,
                        cell.features.len(),
                    )),
                }
                cells.push(cell);
            }
        });
    }

    let mut features: Vec<Value> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut truncated = false;
    let mut bytes = 0u64;
    let mut limit_hit = false;
    let mut failed = 0usize;
    let mut answered = 0usize;
    let mut first_error: Option<String> = None;
    for cell in cells {
        truncated |= cell.truncated;
        bytes += cell.bytes;
        limit_hit |= cell.limit_hit;
        failed += cell.failed;
        answered += cell.answered;
        if first_error.is_none() {
            first_error = cell.first_error;
        }
        for feature in cell.features {
            match gml::feature_key(&feature) {
                Some(key) => {
                    if seen.insert(key) {
                        features.push(feature);
                    }
                }
                None => features.push(feature),
            }
        }
    }
    if features.is_empty() && answered == 0 {
        if let Some(error) = first_error {
            return Err(error);
        }
    }
    if features.len() > MAX_TOTAL {
        features.truncate(MAX_TOTAL);
        truncated = true;
    }
    Ok(Download { features, truncated, bytes, limit_hit, failed, fallback: None })
}

/// One cell's answer: the features that arrived, and the requests that did not.
#[derive(Default)]
struct Cell {
    features: Vec<Value>,
    truncated: bool,
    bytes: u64,
    limit_hit: bool,
    /// Requests that answered, an empty page included.
    answered: usize,
    failed: usize,
    first_error: Option<String>,
}

/// One area, in parallel page batches; a page that fails costs its features, not the cell.
///
/// GUGiK answers in ~1.6 s per request whatever the page size, and caps a response near 995
/// features (measured), so throughput comes from overlapping requests, not bigger pages.
fn fetch_cell(
    layer: &WfsLayer,
    bbox: [f64; 4],
    filter: Option<&str>,
    report: &mut dyn FnMut(&str),
    lang: Lang,
    srs: Srs,
    limit: u64,
) -> Cell {
    let started = Instant::now();
    let mut cell = Cell::default();
    let mut start = 0usize;
    let mut exhausted = false;
    while !exhausted {
        if started.elapsed().as_secs() > BUDGET_SECS {
            cell.truncated = true;
            break;
        }
        if cell.features.len() >= MAX_TOTAL {
            cell.truncated = true;
            break;
        }
        // The first page goes alone: a page of heavy geometries is measured before more follow.
        let in_flight = if cell.answered == 0 { 1 } else { PAGES_IN_FLIGHT };
        // GUGiK answers nothing past a light first page, so the first page keeps the full size.
        let count = match cell.features.len() {
            0 => PAGE,
            seen if cell.bytes / seen as u64 > HEAVY_BYTES => LIGHT_PAGE,
            _ => PAGE,
        };
        let starts: Vec<usize> = if layer.pages {
            (0..in_flight).map(|i| start + i * PAGE).collect()
        } else {
            // A service that refuses STARTINDEX answers one page, so a second page is a failure.
            vec![0]
        };
        let mut read = 0usize;
        for (_, page_bytes, page) in fetch_pages(layer, bbox, filter, &starts, lang, srs, count) {
            cell.bytes += page_bytes;
            match page {
                Ok(page) => {
                    cell.answered += 1;
                    // A short page ends the sequence; the pages after it are past the end.
                    if page.len() < count {
                        exhausted = true;
                    }
                    read += page.len();
                    cell.features.extend(page);
                }
                Err(error) => {
                    cell.failed += 1;
                    cell.first_error.get_or_insert(error);
                }
            }
            if exhausted {
                break;
            }
        }
        report(&text::count_size(lang, layer.name, cell.features.len(), cell.bytes));
        if cell.bytes > limit {
            // Over the byte limit the run stops here, and the tool says so before it adds anything.
            cell.truncated = true;
            cell.limit_hit = true;
            break;
        }
        if !layer.pages {
            // One page is all the service gives: a full page means more features were left out.
            cell.truncated |= read >= count;
            break;
        }
        if read == 0 {
            break;
        }
        start += read;
    }
    cell
}

/// Fetches the given page offsets at once; a page that fails comes back as its error.
fn fetch_pages(
    layer: &WfsLayer,
    bbox: [f64; 4],
    filter: Option<&str>,
    starts: &[usize],
    lang: Lang,
    srs: Srs,
    count: usize,
) -> Vec<(usize, u64, Result<Vec<Value>, String>)> {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        for &offset in starts {
            let sender = sender.clone();
            scope.spawn(move || {
                let url = page_url(layer, bbox, offset, filter, srs, count);
                let swap = matches!(srs, Srs::LonLat);
                // The body is parsed as it arrives, so a heavy page never becomes one string.
                let answered = http::stream_body(&url, 120, lang, |reader| {
                    gml::parse_features_stream(reader, swap, lang)
                })
                .map(|(size, mut rows)| {
                    if matches!(srs, Srs::Grid) {
                        to_lon_lat(&mut rows, layer.xy);
                    }
                    (size, Ok(rows))
                });
                let sent = match answered {
                    Ok((size, rows)) => (offset, size, rows),
                    Err(error) => (offset, 0, Err(error)),
                };
                let _ = sender.send(sent);
            });
        }
        drop(sender);
        let mut pages: Vec<(usize, u64, Result<Vec<Value>, String>)> = receiver.into_iter().collect();
        pages.sort_by_key(|(offset, _, _)| *offset);
        pages
    })
}

/// Turns features served in the national grid into longitude and latitude.
///
/// GUGiK answers a pair as (northing, easting); GDOŚ answers (easting, northing), so `xy` decides.
fn to_lon_lat(features: &mut [Value], xy: bool) {
    for feature in features {
        if let Some(geometry) = feature.get_mut("geometry") {
            if let Some(coordinates) = geometry.get_mut("coordinates") {
                convert(coordinates, xy);
            }
        }
    }
}

fn convert(value: &mut Value, xy: bool) {
    let Some(array) = value.as_array_mut() else {
        return;
    };
    if array.len() >= 2 && array[0].is_number() && array[1].is_number() {
        let first = array[0].as_f64().unwrap_or(0.0);
        let second = array[1].as_f64().unwrap_or(0.0);
        let (northing, easting) = if xy { (second, first) } else { (first, second) };
        let (lon, lat) = gk92::inverse(northing, easting);
        array[0] = json!(lon);
        array[1] = json!(lat);
        return;
    }
    for item in array {
        convert(item, xy);
    }
}

/// The area itself, or four quadrants of it when it is wide.
fn split_cells(bbox: [f64; 4]) -> Vec<[f64; 4]> {
    let [w, s, e, n] = bbox;
    if e - w <= SPLIT_SPAN && n - s <= SPLIT_SPAN {
        return vec![bbox];
    }
    let (mx, my) = ((w + e) / 2.0, (s + n) / 2.0);
    vec![
        [w, s, mx, my],
        [mx, s, e, my],
        [w, my, mx, n],
        [mx, my, e, n],
    ]
}

/// The GetFeature URL. GUGiK refuses `BBOX` and `FILTER` in one request, so a polygon filter
/// replaces the bounding box rather than narrowing it.
fn page_url(
    layer: &WfsLayer,
    bbox: [f64; 4],
    start: usize,
    filter: Option<&str>,
    srs: Srs,
    count: usize,
) -> String {
    let mut url = format!(
        "{}?SERVICE=WFS&VERSION=1.1.0&REQUEST=GetFeature&TYPENAME={}&SRSNAME={}",
        layer.url,
        layer.type_name,
        srs.name()
    );
    match filter {
        Some(filter) => {
            url.push_str("&FILTER=");
            url.push_str(filter);
        }
        None => {
            let box_lon = match srs {
                Srs::LonLat => bbox,
                // The grid is in metres, north and east, so the corners convert one by one.
                Srs::Grid => {
                    let (x1, y1) = gk92::forward(bbox[0], bbox[1]);
                    let (x2, y2) = gk92::forward(bbox[2], bbox[3]);
                    [x1.min(x2), y1.min(y2), x1.max(x2), y1.max(y2)]
                }
            };
            url.push_str(&format!(
                "&BBOX={:.2},{:.2},{:.2},{:.2}",
                box_lon[0], box_lon[1], box_lon[2], box_lon[3]
            ));
        }
    }
    url.push_str(&format!("&MAXFEATURES={count}"));
    // The first page needs no offset, and a service without paging refuses one.
    if start > 0 {
        url.push_str(&format!("&STARTINDEX={start}"));
    }
    url
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog;

    #[test]
    fn the_first_page_asks_for_no_offset() {
        let layer = catalog::wfs_by_label("dzialki").unwrap();
        let url = page_url(layer, [21.0, 52.2, 21.01, 52.21], 0, None, Srs::LonLat, PAGE);
        // GDOŚ reads a STARTINDEX as a request for a natural order it cannot do.
        assert!(!url.contains("STARTINDEX="), "{url}");
        assert!(url.contains("MAXFEATURES="), "{url}");
    }

    #[test]
    fn the_gdos_types_say_they_do_not_page() {
        let culture = catalog::wfs_by_label("Ramsar (obszary wodno-błotne)").expect("a GDOŚ type");
        assert!(!culture.pages, "GDOŚ refuses STARTINDEX");
        assert_eq!(culture.geom_field, "geom");
        let gugik = catalog::wfs_by_label("dzialki").expect("a GUGiK type");
        assert!(gugik.pages, "GUGiK pages every answer");
    }


    #[test]
    fn builds_a_wfs_1_1_request() {
        let layer = catalog::wfs_by_label("dzialki").unwrap();
        let url = page_url(layer, [21.0, 52.2, 21.01, 52.21], 500, None, Srs::LonLat, PAGE);
        assert!(url.contains("VERSION=1.1.0"));
        assert!(url.contains("TYPENAME=dzialki"));
        assert!(url.contains("BBOX=21.00,52.20,21.01,52.21"));
        assert!(url.contains("STARTINDEX=500"));
        assert!(!url.contains("FILTER="));
    }

    #[test]
    fn drops_the_bbox_when_a_filter_is_given() {
        let layer = catalog::wfs_by_label("gminy").unwrap();
        let url = page_url(
            layer,
            [21.0, 52.2, 21.01, 52.21],
            0,
            Some("%3CFilter%3E"),
            Srs::LonLat,
            PAGE,
        );
        assert!(url.contains("&FILTER=%3CFilter%3E"));
        // GUGiK answers "BBOX and FILTER are mutually exclusive" if both are sent.
        assert!(!url.contains("BBOX="));
        // The first page asks for no offset, which some services refuse outright.
        assert!(!url.contains("STARTINDEX="), "{url}");
    }

    #[test]
    fn refuses_a_huge_area() {
        let dzialki = catalog::wfs_by_label("dzialki").unwrap();
        let gminy = catalog::wfs_by_label("gminy").unwrap();
        assert!(too_large(dzialki, [21.0, 52.0, 21.5, 52.4]));
        assert!(!too_large(dzialki, [21.0, 52.2, 21.01, 52.21]));
        assert!(!too_large(gminy, [21.0, 52.0, 22.5, 53.0]));
    }

    #[test]
    fn keeps_the_polygon_in_the_grid_retry() {
        let layer = catalog::wfs_by_label("gminy").unwrap();
        let url = page_url(layer, [21.0, 52.2, 21.01, 52.21], 0, Some("%3CFilter%3E"), Srs::Grid, PAGE);
        assert!(url.contains("SRSNAME=EPSG:2180"), "{url}");
        assert!(url.contains("&FILTER=%3CFilter%3E"), "{url}");
        assert!(!url.contains("BBOX="), "a retry never widens past the drawn shape: {url}");
    }

    #[test]
    fn splits_a_wide_area_into_four_cells() {
        let cells = split_cells([14.0, 49.0, 24.0, 55.0]);
        assert_eq!(cells.len(), 4);
        assert_eq!(cells[0], [14.0, 49.0, 19.0, 52.0]);
        assert_eq!(cells[3], [19.0, 52.0, 24.0, 55.0]);
        assert_eq!(split_cells([21.0, 52.2, 21.01, 52.21]).len(), 1);
    }
}
