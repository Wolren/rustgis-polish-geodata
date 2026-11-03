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

use crate::cache;
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
    let mut download = gather(layer, bbox, polygon.as_deref(), report, lang, Srs::LonLat)?;
    if download.features.is_empty() && download.failed == 0 {
        // The address and street service answers an empty collection outside the national grid.
        let grid = area.and_then(|collection| filter::polygon_filter_grid(layer, collection).ok());
        if let Ok(retry) = gather(layer, bbox, grid.as_deref(), report, lang, Srs::Grid) {
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
) -> Result<Download, String> {
    let areas = split_cells(bbox);
    report(&text::split_areas(lang, layer.name, areas.len()));

    let mut cells: Vec<Cell> = Vec::new();
    if areas.len() == 1 {
        cells.push(fetch_cell(layer, areas[0], polygon, report, lang, srs));
    } else {
        let total = areas.len();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            for (index, area) in areas.iter().enumerate() {
                let sender = sender.clone();
                let area = *area;
                scope.spawn(move || {
                    let done = fetch_cell(layer, area, polygon, &mut |_: &str| {}, lang, srs);
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
    let mut failed = 0usize;
    let mut answered = 0usize;
    let mut first_error: Option<String> = None;
    for cell in cells {
        truncated |= cell.truncated;
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
    Ok(Download { features, truncated, failed, fallback: None })
}

/// One cell's answer: the features that arrived, and the requests that did not.
#[derive(Default)]
struct Cell {
    features: Vec<Value>,
    truncated: bool,
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
        let starts: Vec<usize> = (0..PAGES_IN_FLIGHT).map(|i| start + i * PAGE).collect();
        let mut read = 0usize;
        for (_, page) in fetch_pages(layer, bbox, filter, &starts, lang, srs) {
            match page {
                Ok(page) => {
                    cell.answered += 1;
                    // A short page ends the sequence; the pages after it are past the end.
                    if page.len() < PAGE {
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
        report(&text::count(lang, layer.name, cell.features.len()));
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
) -> Vec<(usize, Result<Vec<Value>, String>)> {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        for &offset in starts {
            let sender = sender.clone();
            scope.spawn(move || {
                let url = page_url(layer, bbox, offset, filter, srs);
                let swap = matches!(srs, Srs::LonLat);
                let parsed = http::get_unstored(&url, 120, lang).and_then(|body| {
                    let mut rows = gml::parse_features(&body, swap, lang)?;
                    // Only an answer parsed as features is stored, so an exception never sticks.
                    if !rows.is_empty() {
                        cache::put(&url, &body);
                    }
                    if matches!(srs, Srs::Grid) {
                        to_lon_lat(&mut rows);
                    }
                    Ok(rows)
                });
                let _ = sender.send((offset, parsed));
            });
        }
        drop(sender);
        let mut pages: Vec<(usize, Result<Vec<Value>, String>)> = receiver.into_iter().collect();
        pages.sort_by_key(|(offset, _)| *offset);
        pages
    })
}

/// Turns features served in the national grid into longitude and latitude.
///
/// EPSG:2180 answers carry the northing first, so a pair is (northing, easting).
fn to_lon_lat(features: &mut [Value]) {
    for feature in features {
        if let Some(geometry) = feature.get_mut("geometry") {
            if let Some(coordinates) = geometry.get_mut("coordinates") {
                convert(coordinates);
            }
        }
    }
}

fn convert(value: &mut Value) {
    let Some(array) = value.as_array_mut() else {
        return;
    };
    if array.len() >= 2 && array[0].is_number() && array[1].is_number() {
        let northing = array[0].as_f64().unwrap_or(0.0);
        let easting = array[1].as_f64().unwrap_or(0.0);
        let (lon, lat) = gk92::inverse(northing, easting);
        array[0] = json!(lon);
        array[1] = json!(lat);
        return;
    }
    for item in array {
        convert(item);
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
    url.push_str(&format!("&MAXFEATURES={PAGE}&STARTINDEX={start}"));
    url
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog;

    #[test]
    fn builds_a_wfs_1_1_request() {
        let layer = catalog::wfs_by_label("dzialki").unwrap();
        let url = page_url(layer, [21.0, 52.2, 21.01, 52.21], 500, None, Srs::LonLat);
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
        );
        assert!(url.contains("&FILTER=%3CFilter%3E"));
        // GUGiK answers "BBOX and FILTER are mutually exclusive" if both are sent.
        assert!(!url.contains("BBOX="));
        assert!(url.contains("STARTINDEX=0"));
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
        let url = page_url(layer, [21.0, 52.2, 21.01, 52.21], 0, Some("%3CFilter%3E"), Srs::Grid);
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
