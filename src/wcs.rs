//! The WCS catalogue: GUGiK coverages added over the area the user picked.
//!
//! `data/wcs.json` holds the coverages `tools/wcs.py` proved with a real GetCoverage: the
//! endpoint, the coverage id, the CRS it is asked in and its format.
//!
//! The cell follows the area. It is the finest one whose transfer fits the 12 s a click may hold
//! the window, at the speeds measured against GUGiK in `nmt.rs`; a request may ask for a coarser
//! cell (`rozdzielczosc`), never a finer one than the budget allows.
//!
//! A GeoTIFF passes through the app's own download, so the window keeps running. An ASCII grid
//! comes through this program, which writes the `.prj` the grid needs to be placed.

use crate::cache;
use crate::gk92;
use crate::host::Host;
use crate::http;
use crate::nmt;
use crate::text::{self, Lang};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

/// Bytes one cell costs on the wire: float32 GeoTIFF, RGB GeoTIFF, and the text an ASCII grid writes.
const TIFF_BYTES: f64 = 4.0;
const IMAGE_BYTES: f64 = 3.0;
const ASCII_BYTES: f64 = 19.6;
/// Download speeds measured against GUGiK: 256 MB of GeoTIFF in 43 s, 99.5 MB of ASCII in 50 s.
const TIFF_BYTES_PER_SECOND: f64 = 5.9e6;
const ASCII_BYTES_PER_SECOND: f64 = 2.0e6;
/// One click may hold the window this long. No WCS transfer is interruptible.
const BUDGET_SECONDS: f64 = 12.0;
/// The practical side: 4096 px measured 67 MB and 11 s, the budget's own edge.
const MAX_SIDE: f64 = 4096.0;

/// One coverage, as `data/wcs.json` stores it.
#[derive(Clone, Deserialize)]
pub struct Entry {
    pub id: String,
    /// The plugin command that adds this coverage, one per catalogue entry.
    pub cmd: String,
    /// The published product name, the same in both interface languages.
    pub label: String,
    /// Who publishes the service, as the layer's attribution.
    pub source: String,
    /// The service address, without its query string.
    pub url: String,
    /// The coverage id GetCoverage names.
    pub coverage: String,
    /// The longitude/latitude box the coverage covers.
    #[serde(rename = "box")]
    pub bbox: [f64; 4],
    /// The CRS asked in and answered in: `EPSG:2180`, `EPSG:4326` or `EPSG:3857`.
    pub crs: String,
    /// The output format the service publishes.
    pub format: String,
    /// `imagery` draws as it is, `terrain` and `surface` are heights and get a look.
    pub kind: String,
    /// The ribbon menu the entry sits in, read by `tools/manifest.py`.
    #[allow(dead_code)]
    pub theme: String,
    /// The height datum a model names, empty for imagery.
    #[serde(default)]
    pub datum: String,
    /// What the build proved, for a reader of the file.
    #[allow(dead_code)]
    #[serde(default)]
    pub probe: String,
}

impl Entry {
    fn model(&self) -> bool {
        self.kind == "terrain" || self.kind == "surface"
    }

    fn surface(&self) -> bool {
        self.kind == "surface"
    }

    fn ascii(&self) -> bool {
        self.format.to_ascii_lowercase().contains("aai")
    }
}

const RAW: &str = include_str!("../data/wcs.json");

/// The catalogue, parsed once per process.
pub fn all() -> &'static [Entry] {
    static PARSED: std::sync::OnceLock<Vec<Entry>> = std::sync::OnceLock::new();
    PARSED.get_or_init(|| serde_json::from_str(RAW).unwrap_or_default())
}

/// The coverage a request names, by id, command, label or coverage id.
pub fn find(wanted: &str) -> Option<&'static Entry> {
    let wanted = wanted.trim();
    let key = slug(wanted);
    all().iter().find(|entry| {
        entry.id == wanted
            || entry.cmd == wanted
            || entry.label.eq_ignore_ascii_case(wanted)
            || entry.coverage.eq_ignore_ascii_case(wanted)
            || slug(&entry.id) == key
            || slug(&entry.label) == key
    })
}

/// The coverage a request names: the `service` parameter, or the command that carried it.
pub fn chosen(request: &Value) -> Option<&'static Entry> {
    let named = request
        .get("params")
        .and_then(|params| params.get("service"))
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty());
    if let Some(wanted) = named {
        return find(wanted);
    }
    request
        .get("id")
        .and_then(Value::as_str)
        .and_then(|id| all().iter().find(|entry| entry.cmd == id))
}

/// A name in the shape a command id uses: lower case letters and digits only.
fn slug(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Adds one catalogued coverage over the area, or says what the request still needs.
pub fn add(host: &mut Host, lang: Lang, request: &Value) -> Result<Value, String> {
    let entry = chosen(request).ok_or_else(|| {
        let wanted = request
            .get("params")
            .and_then(|params| params.get("service"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        text::wcs_unknown(lang, wanted, all().len())
    })?;
    let crs = Crs::parse(&entry.crs).ok_or_else(|| text::wcs_crs_unknown(lang, &entry.crs))?;
    if entry.ascii() && crs != Crs::Pl1992 {
        return Err(text::wcs_ascii_needs_2180(lang, &entry.coverage, &entry.crs));
    }
    let budget = Budget::of(entry).ok_or_else(|| text::wcs_format_unknown(lang, &entry.format))?;

    let (source, features) = nmt::area(host, lang, request, text::wcs_needs_area(lang))?;
    let boxes = nmt::envelopes(&features).ok_or_else(|| text::area_no_geometry(lang, &source))?;
    // A coverage covers part of the world: a layer of the whole world would ask for all of it.
    let centre = ((boxes.lon[0] + boxes.lon[2]) / 2.0, (boxes.lon[1] + boxes.lon[3]) / 2.0);
    if !covers(entry.bbox, centre.0, centre.1) {
        return Err(text::wcs_outside(lang, &entry.coverage, entry.bbox));
    }
    let span = (boxes.metres[2] - boxes.metres[0], boxes.metres[3] - boxes.metres[1]);
    let fit = match grid(budget, span, cell_of(request)) {
        Ok(fit) => fit,
        Err((wanted, smallest)) => {
            return Err(text::wcs_too_big(
                lang,
                &entry.coverage,
                wanted.cells,
                wanted.cell,
                wanted.megabytes,
                wanted.seconds,
                smallest,
            ))
        }
    };

    let bbox = crs.bbox(&boxes);
    let address = url_at(entry, bbox, fit.width, fit.height);
    let wanted = layer_name(entry, fit.cell);
    let mut megabytes = fit.megabytes;
    let mut pending = false;
    let mut look = None;
    let name = if entry.ascii() {
        let file = cache::dem_file(&cache_name(entry, bbox, &fit));
        if file.exists() {
            // The grid is already on disk: the click costs no download.
            megabytes = file.metadata().map(|meta| meta.len()).unwrap_or(0) as f64 / 1e6;
        } else {
            http::download(&address, &file, 110, lang)?;
        }
        let head = head_of(&file)?;
        if !ascii_grid(&head) {
            let _ = std::fs::remove_file(&file);
            return Err(text::wcs_bad_answer(lang, &entry.coverage, &head_text(&head)));
        }
        // An ASCII grid carries no CRS, so the sidecar places it.
        let prj = file.with_extension("prj");
        std::fs::write(&prj, gk92::WKT)
            .map_err(|e| text::file_failed(lang, &path_text(&prj), &e.to_string()))?;
        let path = path_text(&file);
        nmt::attempts(|| host.call("layer.addData", json!({ "path": path, "name": wanted })))?;
        nmt::resolved_name(host, &wanted)
    } else {
        // The app downloads a GeoTIFF off the frame, so it may answer that the file is on its way.
        let answer =
            nmt::attempts(|| host.call("layer.addUrl", json!({ "url": address, "name": wanted, "coverage": true })))?;
        pending = answer.get("pending").and_then(Value::as_bool) == Some(true);
        if pending {
            wanted.clone()
        } else {
            nmt::resolved_name(host, &wanted)
        }
    };

    // A pending GeoTIFF is not on the map yet, so there is nothing to shade.
    let styled = if pending || !entry.model() {
        false
    } else {
        let wanted_look = nmt::look_of(request);
        look = Some(wanted_look.name());
        host.call("raster.symbology", nmt::look_json(&name, wanted_look))
            .is_ok()
    };
    nmt::tidy(host, &source, boxes.lon, &features, lang)?;
    if pending {
        host.notify(&text::wcs_pending(lang, &wanted, &source));
    } else {
        host.notify(&text::wcs_added(
            lang,
            &name,
            &entry.coverage,
            fit.cell,
            (fit.width, fit.height),
            megabytes,
            &source,
            look,
        ));
        if text::AREA_LAYERS.contains(&source.as_str()) && !nmt::covers_its_box(&features, boxes.lon) {
            host.notify(&text::wcs_box_note(lang, &source, boxes.lon));
        }
    }
    Ok(json!({
        "layer": name,
        "area": source,
        "service": entry.label,
        "coverage": entry.coverage,
        "source": entry.source,
        "bounds": entry.bbox,
        "crs": crs.code(),
        "format": entry.format,
        "cell": round2(fit.cell),
        "width": fit.width,
        "height": fit.height,
        "box": bbox,
        "megabytes": megabytes.round(),
        "styled": styled,
        "pending": pending,
        "look": look,
    }))
}

/// The CRS a coverage is asked in, and the box that names an area in it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Crs {
    /// PL-1992, the metre grid every GUGiK model answers in.
    Pl1992,
    /// WGS 84 degrees.
    LonLat,
    /// Web Mercator metres.
    Mercator,
}

impl Crs {
    pub fn parse(text: &str) -> Option<Crs> {
        let digits: String = text.chars().filter(char::is_ascii_digit).collect();
        match digits.as_str() {
            "2180" => Some(Crs::Pl1992),
            "4326" => Some(Crs::LonLat),
            "3857" => Some(Crs::Mercator),
            _ => None,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Crs::Pl1992 => "EPSG:2180",
            Crs::LonLat => "EPSG:4326",
            Crs::Mercator => "EPSG:3857",
        }
    }

    /// The area as this CRS's box, low edges first.
    pub fn bbox(self, boxes: &nmt::Boxes) -> [f64; 4] {
        match self {
            Crs::Pl1992 => {
                let (x0, y0, width, height) = nmt::snap(boxes.metres);
                [
                    x0 as f64,
                    y0 as f64,
                    (x0 + i64::from(width)) as f64,
                    (y0 + i64::from(height)) as f64,
                ]
            }
            Crs::LonLat => boxes.lon,
            Crs::Mercator => {
                let (west, south) = mercator(boxes.lon[0], boxes.lon[1]);
                let (east, north) = mercator(boxes.lon[2], boxes.lon[3]);
                [west, south, east, north]
            }
        }
    }

    /// The box in the text a GetCoverage request takes: whole metres for PL-1992.
    fn box_text(self, bbox: [f64; 4]) -> String {
        match self {
            Crs::Pl1992 => format!("{:.0},{:.0},{:.0},{:.0}", bbox[0], bbox[1], bbox[2], bbox[3]),
            Crs::Mercator => format!("{:.3},{:.3},{:.3},{:.3}", bbox[0], bbox[1], bbox[2], bbox[3]),
            Crs::LonLat => format!("{:.7},{:.7},{:.7},{:.7}", bbox[0], bbox[1], bbox[2], bbox[3]),
        }
    }
}

/// Whether a coverage's longitude/latitude box holds a point.
pub fn covers(bbox: [f64; 4], lon: f64, lat: f64) -> bool {
    (bbox[0]..=bbox[2]).contains(&lon) && (bbox[1]..=bbox[3]).contains(&lat)
}

/// The Web Mercator easting and northing of a longitude and latitude, in metres.
pub fn mercator(lon: f64, lat: f64) -> (f64, f64) {
    const RADIUS: f64 = 6_378_137.0;
    let x = lon.to_radians() * RADIUS;
    let y = ((lat.to_radians() / 2.0 + std::f64::consts::FRAC_PI_4).tan()).ln() * RADIUS;
    (x, y)
}

/// What one cell of this coverage costs, and how fast GUGiK moved it.
#[derive(Clone, Copy)]
pub struct Budget {
    bytes_per_cell: f64,
    bytes_per_second: f64,
}

impl Budget {
    pub fn of(entry: &Entry) -> Option<Budget> {
        if entry.ascii() {
            return Some(Budget {
                bytes_per_cell: ASCII_BYTES,
                bytes_per_second: ASCII_BYTES_PER_SECOND,
            });
        }
        if entry.model() {
            return Some(Budget {
                bytes_per_cell: TIFF_BYTES,
                bytes_per_second: TIFF_BYTES_PER_SECOND,
            });
        }
        if entry.format.to_ascii_lowercase().contains("tif") {
            return Some(Budget {
                bytes_per_cell: IMAGE_BYTES,
                bytes_per_second: TIFF_BYTES_PER_SECOND,
            });
        }
        None
    }
}

/// A grid whose transfer fits the click's budget.
#[derive(Clone, Copy, Debug)]
pub struct Fit {
    pub width: u32,
    pub height: u32,
    /// The cell that results, in metres: the area's width over its columns.
    pub cell: f64,
    pub cells: f64,
    pub megabytes: f64,
    pub seconds: f64,
}

/// The finest cell the budget and the pixel cap allow, or the grid the request asked for.
///
/// `Err` carries the grid the request asked for and the smallest cell that would fit, so the
/// message can name both.
pub fn grid(budget: Budget, span: (f64, f64), wanted: Option<f64>) -> Result<Fit, (Fit, f64)> {
    let span = (span.0.abs().max(1.0), span.1.abs().max(1.0));
    let cells = BUDGET_SECONDS * budget.bytes_per_second / budget.bytes_per_cell;
    let longest = span.0.max(span.1);
    // The area's own shape fixes the aspect: one cell size serves both sides.
    let smallest = (span.0 * span.1 / cells)
        .sqrt()
        .max(longest / MAX_SIDE)
        .clamp(0.001, longest);
    let cell = match wanted {
        Some(want) if want.is_finite() && want > 0.0 => want,
        _ => smallest,
    };
    let fit = fit_of(budget, span, cell);
    if cell < smallest - 1e-9 {
        return Err((fit, smallest));
    }
    Ok(fit)
}

fn fit_of(budget: Budget, span: (f64, f64), cell: f64) -> Fit {
    let width = (span.0 / cell).ceil().max(1.0).min(f64::from(u32::MAX)) as u32;
    let height = (span.1 / cell).ceil().max(1.0).min(f64::from(u32::MAX)) as u32;
    let cells = f64::from(width) * f64::from(height);
    let bytes = cells * budget.bytes_per_cell;
    Fit {
        width,
        height,
        cell: (span.0 / f64::from(width) + span.1 / f64::from(height)) / 2.0,
        cells,
        megabytes: bytes / 1e6,
        seconds: bytes / budget.bytes_per_second,
    }
}

/// The GetCoverage address of a coverage over a box, scaled to the grid.
pub fn url_at(entry: &Entry, bbox: [f64; 4], width: u32, height: u32) -> String {
    let crs = Crs::parse(&entry.crs).unwrap_or(Crs::Pl1992);
    format!(
        "{}?SERVICE=WCS&VERSION=1.0.0&REQUEST=GetCoverage&FORMAT={}&COVERAGE={}&BBOX={}&CRS={}&RESPONSE_CRS={}&WIDTH={width}&HEIGHT={height}",
        entry.url,
        entry.format,
        entry.coverage,
        crs.box_text(bbox),
        crs.code(),
        crs.code(),
    )
}

/// The cell size the request asks for, in metres; the budget caps it.
fn cell_of(request: &Value) -> Option<f64> {
    request
        .get("params")
        .and_then(|params| params.get("rozdzielczosc"))
        .and_then(Value::as_f64)
        .filter(|cell| cell.is_finite() && *cell > 0.0)
}

/// The layer's name: a model names its product, datum and cell; imagery keeps its label.
pub fn layer_name(entry: &Entry, cell: f64) -> String {
    if !entry.model() {
        return entry.label.clone();
    }
    let product = if entry.surface() { "NMPT" } else { "NMT" };
    format!("{product} {} m {}", num_text(cell), entry.datum)
}

/// A cell size as a layer name writes it: `1`, `1.8`, `0.25`.
fn num_text(value: f64) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// The cache file an ASCII grid of this box and grid lands in.
fn cache_name(entry: &Entry, bbox: [f64; 4], fit: &Fit) -> String {
    format!(
        "{}_{:.0}_{:.0}_{}x{}.asc",
        entry.coverage, bbox[0], bbox[1], fit.width, fit.height
    )
}

/// The first bytes of a file, enough to tell an ASCII grid from a service's exception.
fn head_of(path: &Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut head = vec![0u8; 256];
    let read = file.read(&mut head).map_err(|e| e.to_string())?;
    head.truncate(read);
    Ok(head)
}

/// Whether the answer starts like an ASCII grid: `ncols` is the first header word.
pub fn ascii_grid(head: &[u8]) -> bool {
    let text = String::from_utf8_lossy(head);
    text.trim_start().to_ascii_lowercase().starts_with("ncols")
}

fn head_text(head: &[u8]) -> String {
    let text = String::from_utf8_lossy(head).replace(['\n', '\r'], " ");
    text.chars().take(300).collect()
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str) -> &'static Entry {
        find(id).unwrap_or_else(|| panic!("{id} not in data/wcs.json"))
    }

    /// The file in the binary parses whole: one incomplete entry used to empty all of it.
    #[test]
    fn the_catalogue_is_read_whole() {
        let raw: Vec<serde_json::Value> =
            serde_json::from_str(RAW).expect("data/wcs.json is not a JSON array");
        assert_eq!(all().len(), raw.len(), "an entry was skipped: a field is missing");
        assert!(all().len() >= 8, "only {} coverages", all().len());
        for coverage in all() {
            assert!(!coverage.id.is_empty() && !coverage.url.is_empty(), "{}", coverage.id);
            assert!(coverage.cmd.starts_with("wcs-"), "{} has no wcs- prefix", coverage.id);
            assert!(coverage.url.starts_with("https://"), "{}", coverage.id);
            assert!(Crs::parse(&coverage.crs).is_some(), "{}: {}", coverage.id, coverage.crs);
            assert!(Budget::of(coverage).is_some(), "{}: {}", coverage.id, coverage.format);
            assert!(!coverage.label.is_empty() && !coverage.source.is_empty(), "{}", coverage.id);
            assert!(
                coverage.bbox[0] < coverage.bbox[2] && coverage.bbox[1] < coverage.bbox[3],
                "{} has no bounds",
                coverage.id
            );
            if coverage.model() {
                assert!(coverage.datum.starts_with("PL-"), "{} has no datum", coverage.id);
            }
        }
    }

    #[test]
    fn every_command_and_coverage_id_resolves_to_one_entry() {
        for coverage in all() {
            assert_eq!(find(&coverage.cmd).map(|found| found.id.as_str()), Some(coverage.id.as_str()));
            assert_eq!(
                find(&coverage.coverage).map(|found| found.id.as_str()),
                Some(coverage.id.as_str()),
                "{}",
                coverage.coverage
            );
            assert_eq!(
                find(&coverage.label).map(|found| found.id.as_str()),
                Some(coverage.id.as_str())
            );
        }
        assert!(find("wcs-nothing").is_none());
    }

    #[test]
    fn a_command_alone_names_its_coverage() {
        let request = json!({ "id": "wcs-orto-hi-res" });
        assert_eq!(chosen(&request).map(|entry| entry.id.as_str()), Some("orto-hi-res"));
        let named = json!({ "id": "wcs-orto-hi-res", "params": { "service": "Ortofoto standard (WCS)" } });
        assert_eq!(chosen(&named).map(|entry| entry.id.as_str()), Some("orto-standard"));
    }

    #[test]
    fn the_address_asks_for_the_box_the_format_and_the_grid() {
        let address = url_at(entry("nmt-kron86"), [348_129.0, 497_807.0, 350_446.0, 500_000.0], 2317, 2193);
        assert!(address.contains("REQUEST=GetCoverage"), "{address}");
        assert!(address.contains("COVERAGE=DTM_PL-KRON86-NH"), "{address}");
        assert!(address.contains("FORMAT=image/x-aaigrid"), "{address}");
        assert!(address.contains("BBOX=348129,497807,350446,500000"), "{address}");
        assert!(address.contains("CRS=EPSG:2180&RESPONSE_CRS=EPSG:2180"), "{address}");
        assert!(address.contains("WIDTH=2317&HEIGHT=2193"), "{address}");
        let image = url_at(entry("orto-hi-res"), [1.0, 2.0, 3.0, 4.0], 64, 32);
        assert!(image.contains("COVERAGE=Orthoimagery_High_Resolution"), "{image}");
        assert!(image.contains("FORMAT=GEOTIFF"), "{image}");
    }

    /// A big area gets a coarse cell; a request for a finer one is refused with the cell that fits.
    #[test]
    fn the_cell_follows_the_area_and_the_budget() {
        let budget = Budget::of(entry("nmt-kron86-tiff")).unwrap();
        let near = grid(budget, (4_000.0, 4_000.0), None).expect("a 4 km grid");
        assert!(near.cell >= 0.9 && near.cell <= 1.1, "{}", near.cell);
        assert!(near.width <= 4096 && near.height <= 4096, "{near:?}");
        assert!(near.seconds <= BUDGET_SECONDS + 0.01, "{near:?}");
        let far = grid(budget, (12_000.0, 4_000.0), None).expect("a wider grid");
        assert!(far.cell > near.cell, "a bigger area needs a coarser cell");
        assert!(far.seconds <= BUDGET_SECONDS + 0.01, "{far:?}");
        let (wanted, smallest) = grid(budget, (4_000.0, 4_000.0), Some(0.1)).unwrap_err();
        assert!(wanted.seconds > BUDGET_SECONDS, "{wanted:?}");
        assert!(smallest > 0.9 && smallest < 1.1, "{smallest}");
        let coarser = grid(budget, (4_000.0, 4_000.0), Some(5.0)).expect("a 5 m grid");
        assert_eq!((coarser.width, coarser.height), (800, 800));
    }

    /// The ASCII grid's text is the expensive side: the same area gets a coarser cell than a TIFF.
    #[test]
    fn the_ascii_grid_costs_more_than_a_geotiff() {
        let ascii = grid(Budget::of(entry("nmt-kron86")).unwrap(), (4_000.0, 4_000.0), None).unwrap();
        let tiff = grid(Budget::of(entry("nmt-kron86-tiff")).unwrap(), (4_000.0, 4_000.0), None).unwrap();
        assert!(ascii.cell > tiff.cell, "{} against {}", ascii.cell, tiff.cell);
        assert!(ascii.seconds <= BUDGET_SECONDS + 0.01, "{ascii:?}");
        assert!(ascii.width < 4096, "{ascii:?}");
    }

    #[test]
    fn a_model_layer_names_its_product_datum_and_cell() {
        assert_eq!(layer_name(entry("nmt-kron86"), 1.0), "NMT 1 m PL-KRON86-NH");
        assert_eq!(layer_name(entry("nmt-kron86-tiff"), 1.8), "NMT 1.8 m PL-KRON86-NH");
        assert_eq!(layer_name(entry("nmpt-evrf2007"), 0.25), "NMPT 0.25 m PL-EVRF2007-NH");
        assert_eq!(layer_name(entry("orto-standard"), 0.5), "Ortofoto standard (WCS)");
    }

    #[test]
    fn a_box_is_named_in_the_coverage_s_own_crs() {
        let boxes = nmt::Boxes { lon: [16.77, 52.32, 16.80, 52.34], metres: [348_129.4, 497_807.9, 350_446.1, 500_000.2] };
        assert_eq!(Crs::Pl1992.bbox(&boxes), [348_129.0, 497_807.0, 350_447.0, 500_001.0]);
        assert_eq!(Crs::LonLat.bbox(&boxes), boxes.lon);
        let (x, y) = mercator(16.77, 52.32);
        let merc = Crs::Mercator.bbox(&boxes);
        assert!((merc[0] - x).abs() < 1.0 && (merc[1] - y).abs() < 1.0, "{merc:?}");
        assert!(merc[2] > merc[0] && merc[3] > merc[1]);
        assert_eq!(Crs::parse("EPSG:2180"), Some(Crs::Pl1992));
        assert_eq!(Crs::parse("epsg:3857"), Some(Crs::Mercator));
        assert_eq!(Crs::parse("EPSG:9999"), None);
    }

    #[test]
    fn a_centre_outside_the_coverage_box_is_refused() {
        let poland = entry("orto-standard").bbox;
        assert!(covers(poland, 16.9, 52.4));
        assert!(!covers(poland, 0.0, 0.0), "the world's centre is outside Poland");
        assert!(!covers(poland, 16.9, 60.0));
        assert!(covers(poland, poland[0], poland[1]), "a corner is inside");
    }

    #[test]
    fn an_ascii_grid_is_told_apart_from_a_service_exception() {
        assert!(ascii_grid(b"\nncols        100\nnrows 100\n"));
        assert!(!ascii_grid(b"<?xml version='1.0'?><ServiceExceptionReport>"));
        assert!(!ascii_grid(b"II*\x00"));
    }

    #[test]
    fn the_cache_name_holds_the_coverage_the_origin_and_the_grid() {
        let fit = Fit { width: 100, height: 50, cell: 1.0, cells: 5_000.0, megabytes: 0.1, seconds: 0.05 };
        assert_eq!(
            cache_name(entry("nmt-kron86"), [348_129.0, 497_807.0, 350_446.0, 500_000.0], &fit),
            "DTM_PL-KRON86-NH_348129_497807_100x50.asc"
        );
    }
}
