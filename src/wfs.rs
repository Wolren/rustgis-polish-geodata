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
use crate::gml;
use crate::http;
use serde_json::Value;
use std::collections::HashSet;
use std::time::Instant;

const PAGE: usize = 500;
/// Pages requested at once. Four parallel pages beat sequential by 2.57x, measured.
const PAGES_IN_FLIGHT: usize = 4;
const BUDGET_SECS: u64 = 240;
const MAX_TOTAL: usize = 6000;
/// Above this width or height, an area is split into four cells fetched at once.
const SPLIT_SPAN: f64 = 1.5;

/// Whether the area is wider than the layer's limit in either direction.
pub fn too_large(layer: &WfsLayer, bbox: [f64; 4]) -> bool {
    bbox[2] - bbox[0] > layer.max_span || bbox[3] - bbox[1] > layer.max_span
}

pub fn span_error(layer: &WfsLayer, bbox: [f64; 4]) -> String {
    format!(
        "obszar jest zbyt duży dla „{}”: {:.2}° × {:.2}°, a maksimum to {:.2}° — przybliż mapę albo wybierz mniejszy obszar.",
        layer.name,
        bbox[2] - bbox[0],
        bbox[3] - bbox[1],
        layer.max_span
    )
}

/// Downloads every feature for `bbox`, reporting progress through `report`.
///
/// `area` is the area layer's GeoJSON. When it is a single polygon the server filters by that
/// polygon instead of its bounding box, which cut 468 features to 189 on a measured strip.
pub fn fetch(
    layer: &WfsLayer,
    bbox: [f64; 4],
    area: Option<&Value>,
    report: &mut dyn FnMut(&str),
) -> Result<(Vec<Value>, bool), String> {
    if ![bbox[0], bbox[1], bbox[2], bbox[3]]
        .iter()
        .all(|v| v.is_finite())
    {
        return Err("obszar nie ma poprawnego zasięgu".into());
    }
    if too_large(layer, bbox) {
        return Err(span_error(layer, bbox));
    }
    let polygon = area.and_then(|a| filter::polygon_filter(layer, a));

    let areas = split_cells(bbox);
    report(&format!(
        "{}: {} {}",
        layer.name,
        areas.len(),
        if areas.len() == 1 {
            "obszar"
        } else {
            "obszary równolegle"
        }
    ));

    let mut parts: Vec<(Vec<Value>, bool)> = Vec::new();
    if areas.len() == 1 {
        parts.push(fetch_cell(layer, areas[0], polygon.as_deref(), report)?);
    } else {
        let total = areas.len();
        let mut results: Vec<Result<(Vec<Value>, bool), String>> = Vec::new();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            for (index, area) in areas.iter().enumerate() {
                let sender = sender.clone();
                let area = *area;
                let filter = polygon.clone();
                scope.spawn(move || {
                    let done = fetch_cell(layer, area, filter.as_deref(), &mut |_: &str| {});
                    let _ = sender.send((index, done));
                });
            }
            drop(sender);
            let mut done = 0;
            for (_, result) in receiver {
                done += 1;
                match &result {
                    Ok((features, _)) => report(&format!(
                        "{}: obszar {done}/{total}, {} obiektów",
                        layer.name,
                        features.len()
                    )),
                    Err(e) => report(&format!("{}: obszar {done}/{total} — {e}", layer.name)),
                }
                results.push(result);
            }
        });
        for result in results {
            parts.push(result?);
        }
    }

    let mut seen: HashSet<String> = HashSet::new();
    let mut features: Vec<Value> = Vec::new();
    let mut truncated = false;
    for (part, cut) in parts {
        truncated |= cut;
        for feature in part {
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
    if features.len() > MAX_TOTAL {
        features.truncate(MAX_TOTAL);
        truncated = true;
    }
    Ok((features, truncated))
}

/// One area, in parallel page batches.
///
/// GUGiK answers in ~1.6 s per request whatever the page size, and caps a response near 995
/// features (measured), so throughput comes from overlapping requests, not bigger pages.
fn fetch_cell(
    layer: &WfsLayer,
    bbox: [f64; 4],
    filter: Option<&str>,
    report: &mut dyn FnMut(&str),
) -> Result<(Vec<Value>, bool), String> {
    let started = Instant::now();
    let mut features: Vec<Value> = Vec::new();
    let mut truncated = false;
    let mut start = 0usize;
    let mut exhausted = false;
    while !exhausted {
        if started.elapsed().as_secs() > BUDGET_SECS {
            truncated = true;
            break;
        }
        if features.len() >= MAX_TOTAL {
            truncated = true;
            break;
        }
        let starts: Vec<usize> = (0..PAGES_IN_FLIGHT).map(|i| start + i * PAGE).collect();
        let mut batch = fetch_pages(layer, bbox, filter, &starts)?;
        // A short page ends the sequence; later pages in the batch are past the end.
        let mut read = 0usize;
        for page in batch.drain(..) {
            if page.len() < PAGE {
                exhausted = true;
            }
            read += page.len();
            features.extend(page);
            if exhausted {
                break;
            }
        }
        report(&format!("{}: {} obiektów", layer.name, features.len()));
        if read == 0 {
            break;
        }
        start += read;
    }
    Ok((features, truncated))
}

/// Fetches the given page offsets at once; a page that fails is reported, not retried.
fn fetch_pages(
    layer: &WfsLayer,
    bbox: [f64; 4],
    filter: Option<&str>,
    starts: &[usize],
) -> Result<Vec<Vec<Value>>, String> {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        for &offset in starts {
            let sender = sender.clone();
            scope.spawn(move || {
                let url = page_url(layer, bbox, offset, filter);
                let parsed = http::get(&url, 120).and_then(|body| gml::parse_features(&body, true));
                let _ = sender.send((offset, parsed));
            });
        }
        drop(sender);
        let mut pages: Vec<(usize, Vec<Value>)> = Vec::new();
        for (offset, parsed) in receiver {
            pages.push((offset, parsed?));
        }
        pages.sort_by_key(|(offset, _)| *offset);
        Ok(pages.into_iter().map(|(_, page)| page).collect())
    })
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
fn page_url(layer: &WfsLayer, bbox: [f64; 4], start: usize, filter: Option<&str>) -> String {
    let mut url = format!(
        "{}?SERVICE=WFS&VERSION=1.1.0&REQUEST=GetFeature&TYPENAME={}&SRSNAME=EPSG:4326",
        layer.url, layer.type_name
    );
    match filter {
        Some(filter) => {
            url.push_str("&FILTER=");
            url.push_str(filter);
        }
        None => url.push_str(&format!(
            "&BBOX={:.6},{:.6},{:.6},{:.6}",
            bbox[0], bbox[1], bbox[2], bbox[3]
        )),
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
        let layer = catalog::wfs("dzialki").unwrap();
        let url = page_url(layer, [21.0, 52.2, 21.01, 52.21], 500, None);
        assert!(url.contains("VERSION=1.1.0"));
        assert!(url.contains("TYPENAME=dzialki"));
        assert!(url.contains("BBOX=21.000000,52.200000,21.010000,52.210000"));
        assert!(url.contains("STARTINDEX=500"));
        assert!(!url.contains("FILTER="));
    }

    #[test]
    fn drops_the_bbox_when_a_filter_is_given() {
        let layer = catalog::wfs("gminy").unwrap();
        let url = page_url(layer, [21.0, 52.2, 21.01, 52.21], 0, Some("%3CFilter%3E"));
        assert!(url.contains("&FILTER=%3CFilter%3E"));
        // GUGiK answers "BBOX and FILTER are mutually exclusive" if both are sent.
        assert!(!url.contains("BBOX="));
        assert!(url.contains("STARTINDEX=0"));
    }

    #[test]
    fn refuses_a_huge_area() {
        let dzialki = catalog::wfs("dzialki").unwrap();
        let gminy = catalog::wfs("gminy").unwrap();
        assert!(too_large(dzialki, [21.0, 52.0, 21.5, 52.4]));
        assert!(!too_large(dzialki, [21.0, 52.2, 21.01, 52.21]));
        assert!(!too_large(gminy, [21.0, 52.0, 22.5, 53.0]));
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
