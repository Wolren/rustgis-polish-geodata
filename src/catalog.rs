//! The catalogue: GUGiK overlays (web map services), GUGiK features (WFS), and the Wody Polskie
//! and Cartomatic families, whose entries draw all their layers in one GetMap picture.
//!
//! Overlay addresses are KVP templates. RustGIS fills in `{z}`/`{x}`/`{y}` for XYZ and
//! WMTS layers, and `{bbox-epsg-3857}`, `{width}`, `{height}` for WMS ones.
//!
//! A WFS entry carries GUGiK's own title, which is the same in both languages: "Działki" is the
//! name of the dataset, not a word to translate. The plugin's own sentences (buttons, errors,
//! progress) are translated; data names are not.

use crate::text::Lang;

/// Where the data comes from, shown as the layer's attribution.
pub const ATTRIBUTION: &str = "GUGiK · geoportal.gov.pl";

/// Poland's extent, `[west, south, east, north]`, used to bring the map to the data.
pub const POLAND: [f64; 4] = [14.07, 48.99, 24.15, 54.84];

pub struct Overlay {
    pub id: &'static str,
    pub name_en: &'static str,
    pub name_pl: &'static str,
    pub url: &'static str,
    pub max_zoom: u8,
    /// The shallowest zoom the service draws at; below it its tiles are blank.
    pub min_zoom: u8,
    /// The tile size the service's cartography is drawn for (BDOT10k needs 512).
    pub tile_size: u16,
}

impl Overlay {
    pub fn name(&self, lang: Lang) -> &'static str {
        match lang {
            Lang::En => self.name_en,
            Lang::Pl => self.name_pl,
        }
    }

    /// Whether a layer already on the map is this overlay, named in either language.
    pub fn matches(&self, name: &str) -> bool {
        name == self.name_en || name == self.name_pl
    }
}

/// BDOT10k's composition service needs its layer list in one request; these are the layers
/// the service draws (the unnamed groups in its capabilities can't be requested).
const BDOT_LAYERS: &str =
    "RZab,TPrz,UTrw,TLes,RKr,RTr,Mo,WPow,Szu,LBrz,WPowEt,GrPol,Rez,GrPK,GrPN,\
PRur,ZbTA,BudCm,TerCm,BudSp,Bzn,BHydA,BHydL,IUTA,ObOrA,ObPL,Prom,UTrL,NazDr,NrDr,Umo,UTrP,ObKom,\
INUTP,ZbTP,NazUl,ObOrP,WyBT";

pub const OVERLAYS: &[Overlay] = &[
    Overlay {
        id: "orto",
        name_en: "Orthophoto (GUGiK)",
        name_pl: "Ortofoto (GUGiK)",
        url: "https://mapy.geoportal.gov.pl/wss/service/PZGIK/ORTO/WMTS/StandardResolution?\
SERVICE=WMTS&REQUEST=GetTile&VERSION=1.0.0&LAYER=ORTOFOTOMAPA&STYLE=default&\
TILEMATRIXSET=EPSG:3857&TILEMATRIX=EPSG:3857:{z}&TILEROW={y}&TILECOL={x}&FORMAT=image/jpeg",
        max_zoom: 19,
        min_zoom: 0,
        tile_size: 256,
    },
    Overlay {
        id: "orto-hi",
        name_en: "Orthophoto Hi-Res (GUGiK)",
        name_pl: "Ortofoto Hi-Res (GUGiK)",
        url: "https://mapy.geoportal.gov.pl/wss/service/PZGIK/ORTO/WMS/HighResolution?\
SERVICE=WMS&VERSION=1.3.0&REQUEST=GetMap&LAYERS=Raster&STYLES=&CRS=EPSG:3857&\
BBOX={bbox-epsg-3857}&WIDTH={width}&HEIGHT={height}&FORMAT=image/jpeg&TRANSPARENT=false",
        max_zoom: 19,
        min_zoom: 0,
        tile_size: 256,
    },
    Overlay {
        id: "bdot",
        name_en: "BDOT10k (GUGiK)",
        name_pl: "BDOT10k (GUGiK)",
        url: BDOT_URL,
        max_zoom: 18,
        min_zoom: 13,
        tile_size: 512,
    },
    Overlay {
        id: "dzialki-wms",
        name_en: "Parcels (KIEG)",
        name_pl: "Działki (KIEG)",
        url: KIEG_URL,
        max_zoom: 21,
        min_zoom: 15,
        tile_size: 256,
    },
    Overlay {
        id: "adresy",
        name_en: "Addresses (GUGiK)",
        name_pl: "Adresy (GUGiK)",
        url:
            "https://mapy.geoportal.gov.pl/wss/ext/KrajowaIntegracjaNumeracjiAdresowej?\
SERVICE=WMS&VERSION=1.3.0&REQUEST=GetMap&LAYERS=A07_Punkty_adresowe,A08_Ulice&STYLES=&\
CRS=EPSG:3857&BBOX={bbox-epsg-3857}&WIDTH={width}&HEIGHT={height}&FORMAT=image/png&TRANSPARENT=true",
        max_zoom: 21,
        min_zoom: 15,
        tile_size: 256,
    },
    Overlay {
        id: "prg-wms",
        name_en: "PRG boundaries",
        name_pl: "Granice PRG",
        url: "https://mapy.geoportal.gov.pl/wss/service/PZGIK/PRG/WMS/AdministrativeBoundaries?\
SERVICE=WMS&VERSION=1.3.0&REQUEST=GetMap&LAYERS=A00_Granice_panstwa,A01_Granice_wojewodztw,\
A02_Granice_powiatow,A03_Granice_gmin,A04_Granice_miast&STYLES=&CRS=EPSG:3857&\
BBOX={bbox-epsg-3857}&WIDTH={width}&HEIGHT={height}&FORMAT=image/png&TRANSPARENT=true",
        max_zoom: 18,
        min_zoom: 0,
        tile_size: 256,
    },
    Overlay {
        id: "prng",
        name_en: "Place names (PRNG)",
        name_pl: "Nazwy geograficzne (PRNG)",
        url: "https://mapy.geoportal.gov.pl/wss/service/PZGiK/PRNG/WMS/GeographicalNames?\
SERVICE=WMS&VERSION=1.3.0&REQUEST=GetMap&LAYERS=M1_UrzedoweNazwyMiejscowosci,\
M2_PozostaleNazwyMiejscowosci&STYLES=&CRS=EPSG:3857&BBOX={bbox-epsg-3857}&\
WIDTH={width}&HEIGHT={height}&FORMAT=image/png&TRANSPARENT=true",
        max_zoom: 18,
        min_zoom: 0,
        tile_size: 256,
    },
    Overlay {
        id: "nmt",
        name_en: "DEM hillshade",
        name_pl: "NMT · cieniowanie",
        url: "https://mapy.geoportal.gov.pl/wss/service/PZGIK/NMT/GRID1/WMS/ShadedRelief?\
SERVICE=WMS&VERSION=1.3.0&REQUEST=GetMap&LAYERS=Raster&STYLES=&CRS=EPSG:3857&\
BBOX={bbox-epsg-3857}&WIDTH={width}&HEIGHT={height}&FORMAT=image/png&TRANSPARENT=false",
        max_zoom: 18,
        min_zoom: 0,
        tile_size: 256,
    },
];

const KIEG_URL: &str = "https://integracja.gugik.gov.pl/cgi-bin/KrajowaIntegracjaEwidencjiGruntow?\
SERVICE=WMS&VERSION=1.3.0&REQUEST=GetMap&LAYERS=dzialki,budynki,numery_dzialek,obreby&STYLES=&\
CRS=EPSG:3857&BBOX={bbox-epsg-3857}&WIDTH={width}&HEIGHT={height}&FORMAT=image/png&TRANSPARENT=true";

const BDOT_URL: &str = "https://mapy.geoportal.gov.pl/wss/service/pub/guest/kompozycja_BDOT10k_WMS/\
MapServer/WMSServer?SERVICE=WMS&VERSION=1.3.0&REQUEST=GetMap&LAYERS=PLACEHOLDER&STYLES=&\
CRS=EPSG:3857&BBOX={bbox-epsg-3857}&WIDTH={width}&HEIGHT={height}&FORMAT=image/png&TRANSPARENT=true";

/// A feature type downloaded from a GUGiK WFS and added as a vector layer.
#[derive(Clone)]
pub struct WfsLayer {
    pub id: &'static str,
    /// GUGiK's short name for the type, the same in both interface languages: a dataset's name is a
    /// proper name, not a word to translate.
    pub name: &'static str,
    /// The full catalogue title, which the sheet-index tools pass as the dataset value.
    pub title: &'static str,
    /// The WFS endpoint, without its query string.
    pub url: &'static str,
    /// The feature type, as the service names it (WFS 1.1 `TYPENAME`).
    pub type_name: &'static str,
    /// The widest area one download may cover, in degrees.
    pub max_span: f64,
    /// The geometry property a WFS spatial filter names (the service accepts it bare).
    pub geom_field: &'static str,
    /// The download tool whose picker offers this dataset (empty for a sheet index).
    pub tool: &'static str,
    /// A tile or sheet index: it locates downloadable archives, which this plugin cannot fetch.
    /// Kept in the catalogue (a script may still name it), not offered in the download tools.
    pub index: bool,
}

pub fn overlay(id: &str) -> Option<&'static Overlay> {
    OVERLAYS.iter().find(|o| o.id == id)
}

/// One entry of `data/wfs.json`, as the file stores it.
#[derive(serde::Deserialize)]
struct Raw {
    id: String,
    label: String,
    title: String,
    url: String,
    #[serde(rename = "type")]
    type_name: String,
    geom: String,
    max_span: f64,
    #[serde(default)]
    index: bool,
    #[serde(default)]
    tool: String,
}

const RAW: &str = include_str!("../data/wfs.json");

fn leaked(text: &str) -> &'static str {
    Box::leak(text.to_string().into_boxed_str())
}

/// Every national WFS type that answered a real request, proved once when the file was built.
///
/// Entries are read one at a time: an entry the file's builder left incomplete is skipped and the
/// rest of the catalogue still works. Reading the file as one array would lose every dataset to a
/// single bad entry.
pub fn wfs_layers() -> &'static [WfsLayer] {
    static ALL: std::sync::OnceLock<Vec<WfsLayer>> = std::sync::OnceLock::new();
    ALL.get_or_init(|| {
        let Ok(entries) = serde_json::from_str::<Vec<serde_json::Value>>(RAW) else {
            return Vec::new();
        };
        entries
            .into_iter()
            .filter_map(|entry| serde_json::from_value::<Raw>(entry).ok())
            .map(|entry| WfsLayer {
                id: leaked(&entry.id),
                name: leaked(&entry.label),
                title: leaked(&entry.title),
                url: leaked(&entry.url),
                type_name: leaked(&entry.type_name),
                max_span: entry.max_span,
                geom_field: leaked(&entry.geom),
                index: entry.index,
                tool: leaked(&entry.tool),
            })
            .collect()
    })
}

/// The datasets the download tools offer: the layers themselves, not the tile indexes.
pub fn offered() -> impl Iterator<Item = &'static WfsLayer> {
    wfs_layers().iter().filter(|layer| !layer.index)
}

/// The names this plugin gave three PRG layers before the catalogue carried GUGiK's own titles.
/// Scripts, saved tool forms and prompts written then keep working.
const ALIASES: &[(&str, &str)] = &[
    ("gminy", "A03_Granice_gmin"),
    ("powiaty", "A02_Granice_powiatow"),
    ("wojewodztwa", "A01_Granice_wojewodztw"),
];

/// The dataset a request or a tool names: its id, its title, or the bare type name.
pub fn wfs_by_label(label: &str) -> Option<&'static WfsLayer> {
    let wanted = label.trim();
    // A command id keeps the group prefix and drops the underscores of a type name.
    let bare = wanted
        .trim_start_matches("dane-")
        .trim_start_matches("wfs-");
    let key = slug(bare);
    let alias = ALIASES
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, kind)| slug(kind));
    let known = wfs_layers().iter().find(|w| {
        w.id == wanted
            || w.name.eq_ignore_ascii_case(wanted)
            || slug(w.id) == key
            || slug(w.id.trim_start_matches("dane-")) == key
            || slug(&w.type_name) == key
            || alias
                .as_deref()
                .is_some_and(|kind| slug(&w.type_name) == kind)
    });
    if known.is_some() {
        return known;
    }
    // The sheet-index tools pass the full title: 44 NMT indexes share one short label.
    let title = wanted.to_lowercase();
    if title.is_empty() {
        return None;
    }
    wfs_layers().iter().find(|w| w.title.to_lowercase() == title)
}

/// A name in the shape a command id uses: lower case letters and digits only.
fn slug(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// The address RustGIS should fetch tiles from: BDOT10k's layer list filled in.
pub fn url_for(overlay: &Overlay) -> String {
    if overlay.id == "bdot" {
        BDOT_URL.replace("PLACEHOLDER", BDOT_LAYERS)
    } else {
        overlay.url.to_string()
    }
}

/// The deepest zoom a family file names when it names none.
fn default_zoom() -> u8 {
    18
}

/// `data/wody.json`, as the file stores it: one publisher, one zoom, the services.
#[derive(serde::Deserialize)]
struct WodyFile {
    #[serde(default)]
    attribution: String,
    #[serde(default = "default_zoom")]
    zoom: u8,
    #[serde(default)]
    services: Vec<WodyEntry>,
}

impl Default for WodyFile {
    fn default() -> Self {
        WodyFile { attribution: String::new(), zoom: default_zoom(), services: Vec::new() }
    }
}

#[derive(serde::Deserialize)]
struct WodyEntry {
    id: String,
    title: String,
    url: String,
    #[serde(default)]
    layers: Vec<String>,
}

/// A Wody Polskie service: every layer it draws, in one request.
pub struct WodyService {
    pub id: &'static str,
    pub title: &'static str,
    /// The service address, without its query string.
    pub url: &'static str,
    /// The service's layer ids, in the order the file lists them.
    pub layers: &'static [&'static str],
}

impl WodyService {
    /// The `LAYERS` value: the layer ids in one comma-joined list.
    pub fn layers_csv(&self) -> String {
        self.layers.join(",")
    }
}

/// The publisher line and the deepest zoom a catalogue file gives its whole family.
pub struct Family {
    pub attribution: &'static str,
    pub zoom: u8,
}

const WODY_RAW: &str = include_str!("../data/wody.json");

fn wody_file() -> &'static WodyFile {
    static FILE: std::sync::OnceLock<WodyFile> = std::sync::OnceLock::new();
    FILE.get_or_init(|| serde_json::from_str(WODY_RAW).unwrap_or_default())
}

/// The Wody Polskie services, parsed once per process.
pub fn wody() -> &'static [WodyService] {
    static ALL: std::sync::OnceLock<Vec<WodyService>> = std::sync::OnceLock::new();
    ALL.get_or_init(|| {
        wody_file()
            .services
            .iter()
            .map(|entry| WodyService {
                id: leaked(&entry.id),
                title: leaked(&entry.title),
                url: leaked(&entry.url),
                layers: leaked_list(&entry.layers),
            })
            .collect()
    })
}

/// Who publishes the Wody Polskie services, and how deep the file lets them zoom.
pub fn wody_family() -> &'static Family {
    static META: std::sync::OnceLock<Family> = std::sync::OnceLock::new();
    META.get_or_init(|| Family {
        attribution: leaked(&wody_file().attribution),
        zoom: wody_file().zoom,
    })
}

fn leaked_list(items: &[String]) -> &'static [&'static str] {
    let list: Vec<&'static str> = items.iter().map(|item| leaked(item)).collect();
    Box::leak(list.into_boxed_slice())
}

/// `data/cartomatic.json`, as the file stores it: one publisher, one zoom, the maps by series.
#[derive(serde::Deserialize)]
struct CartoFile {
    #[serde(default)]
    attribution: String,
    #[serde(default = "default_zoom")]
    zoom: u8,
    #[serde(default)]
    series: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    maps: Vec<CartoEntry>,
}

impl Default for CartoFile {
    fn default() -> Self {
        CartoFile {
            attribution: String::new(),
            zoom: default_zoom(),
            series: std::collections::BTreeMap::new(),
            maps: Vec::new(),
        }
    }
}

#[derive(serde::Deserialize)]
struct CartoEntry {
    id: String,
    title: String,
    layer: String,
    #[serde(default)]
    series: String,
}

/// A Cartomatic map: one server layer, drawn as one picture of the view.
pub struct CartoMap {
    pub id: &'static str,
    pub title: &'static str,
    /// The server's layer name: the `LAYERS` value of the request.
    pub layer: &'static str,
    /// The series the map belongs to, as the file names it (`plans`, `topo`); the ribbon groups by it.
    #[allow(dead_code)]
    pub series: &'static str,
}

/// The server every layer of `data/cartomatic.json` lives on: the file names layers only.
pub const CARTO_URL: &str = "https://pastmaps.cenagis.edu.pl/geoserver/ihpan/wms/";

const CARTO_RAW: &str = include_str!("../data/cartomatic.json");

fn carto_file() -> &'static CartoFile {
    static FILE: std::sync::OnceLock<CartoFile> = std::sync::OnceLock::new();
    FILE.get_or_init(|| serde_json::from_str(CARTO_RAW).unwrap_or_default())
}

/// The Cartomatic maps, parsed once per process.
pub fn cartomatic() -> &'static [CartoMap] {
    static ALL: std::sync::OnceLock<Vec<CartoMap>> = std::sync::OnceLock::new();
    ALL.get_or_init(|| {
        carto_file()
            .maps
            .iter()
            .map(|entry| CartoMap {
                id: leaked(&entry.id),
                title: leaked(&entry.title),
                layer: leaked(&entry.layer),
                series: leaked(&entry.series),
            })
            .collect()
    })
}

/// Who publishes the Cartomatic maps, and how deep the file lets them zoom.
pub fn carto_family() -> &'static Family {
    static META: std::sync::OnceLock<Family> = std::sync::OnceLock::new();
    META.get_or_init(|| Family {
        attribution: leaked(&carto_file().attribution),
        zoom: carto_file().zoom,
    })
}

/// The series the file names, in name order, with the description it gives each one.
#[allow(dead_code)]
pub fn carto_series() -> &'static [(&'static str, &'static str)] {
    static SERIES: std::sync::OnceLock<Vec<(&'static str, &'static str)>> =
        std::sync::OnceLock::new();
    SERIES.get_or_init(|| {
        carto_file()
            .series
            .iter()
            .map(|(name, text)| (leaked(name), leaked(text)))
            .collect()
    })
}

/// A name as the families compare it: lower case letters and digits, separators dropped.
fn folded(text: &str) -> String {
    text.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase()
}

/// The Wody Polskie service a command names: its id, its title, or a slug of either.
pub fn find_wody(wanted: &str) -> Option<&'static WodyService> {
    let wanted = wanted.trim();
    let key = folded(wanted);
    let bare = folded(wanted.trim_start_matches("wody-"));
    wody().iter().find(|service| {
        service.id == wanted
            || folded(service.id) == key
            || folded(service.id.trim_start_matches("wody-")) == bare
            || folded(service.title) == key
            || folded(service.title) == bare
    })
}

/// The Cartomatic map a command names: its id, its title, or a slug of either.
pub fn find_carto(wanted: &str) -> Option<&'static CartoMap> {
    let wanted = wanted.trim();
    let key = folded(wanted);
    let bare = folded(wanted.trim_start_matches("carto-"));
    cartomatic().iter().find(|map| {
        map.id == wanted
            || folded(map.id) == key
            || folded(map.id.trim_start_matches("carto-")) == bare
            || folded(map.title) == key
            || folded(map.title) == bare
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_catalogue_entry_by_command_id() {
        for (command, kind) in [
            ("dane-punkty-adresowe", "adresyUlice:A07_Punkty_adresowe"),
            ("dane-budynki", "budynki"),
            ("dane-a03-granice-gmin", "A03_Granice_gmin"),
            (
                "dane-dane-punkty-adresowe",
                "adresyUlice:A07_Punkty_adresowe",
            ),
        ] {
            let found = wfs_by_label(command).unwrap_or_else(|| panic!("{command} not found"));
            assert_eq!(found.type_name, kind, "{command}");
        }
    }

    #[test]
    fn finds_a_dataset_by_its_title_id_or_type() {
        assert_eq!(wfs_by_label("Działki").map(|w| w.id), Some("dane-dzialki"));
        assert_eq!(wfs_by_label("działki").map(|w| w.id), Some("dane-dzialki"));
        assert_eq!(wfs_by_label("dzialki").map(|w| w.id), Some("dane-dzialki"));
        assert!(wfs_by_label("nothing").is_none());
    }

    /// The sheet-index tools pass the full title: 44 NMT indexes share one short label.
    #[test]
    fn a_dataset_is_found_by_its_full_title_in_any_case() {
        for layer in wfs_layers().iter().filter(|layer| layer.index) {
            let title = layer.title.to_lowercase();
            // The data side makes titles unique; a shared title cannot pick one entry.
            if wfs_layers().iter().filter(|other| other.title.to_lowercase() == title).count() > 1 {
                continue;
            }
            assert_eq!(
                wfs_by_label(&title).map(|found| found.id),
                Some(layer.id),
                "{} is not found by its title in lower case",
                layer.id
            );
            assert_eq!(
                wfs_by_label(layer.title).map(|found| found.id),
                Some(layer.id),
                "{} is not found by its title",
                layer.id
            );
        }
    }

    /// The catalogue is only as good as its file: one incomplete entry used to empty all of it.
    #[test]
    fn every_entry_of_the_file_is_read() {
        let entries: Vec<serde_json::Value> =
            serde_json::from_str(RAW).expect("data/wfs.json is not a JSON array");
        assert!(entries.len() > 100, "only {} entries", entries.len());
        assert_eq!(
            wfs_layers().len(),
            entries.len(),
            "an entry was skipped: a field is missing"
        );
    }

    #[test]
    fn every_dataset_has_a_name() {
        for overlay in OVERLAYS {
            assert!(
                !overlay.name_en.is_empty() && !overlay.name_pl.is_empty(),
                "{}",
                overlay.id
            );
        }
        for layer in wfs_layers() {
            assert!(!layer.name.is_empty(), "{}", layer.id);
            assert!(!layer.geom_field.is_empty(), "{}", layer.id);
            assert!(layer.max_span > 0.0, "{}", layer.id);
        }
    }

    /// A picker lists titles, so two datasets with one title cannot be told apart.
    #[test]
    fn the_offered_datasets_have_unique_titles() {
        let mut names: Vec<&str> = offered().map(|layer| layer.name).collect();
        let before = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(before, names.len(), "two offered datasets share a title");
    }

    #[test]
    fn the_tile_indexes_are_not_offered() {
        assert!(wfs_layers().iter().any(|layer| layer.index));
        assert!(offered().all(|layer| !layer.index));
        assert!(
            offered().count() > 80,
            "only {} datasets",
            offered().count()
        );
    }

    /// The manifest is generated from this catalogue, so the two must agree: every dataset the
    /// tools offer is one the catalogue has, and no dataset is offered twice or lost. The sheet-index
    /// tools offer the index entries by title, and `pobierz-arkusze` takes no dataset at all.
    #[test]
    fn the_download_tools_offer_every_dataset_exactly_once() {
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../plugin.json")).expect("plugin.json is not JSON");
        let mut layer_choices: Vec<String> = Vec::new();
        let mut index_ids: Vec<String> = Vec::new();
        for tool in manifest["tools"].as_array().expect("tools") {
            let params = tool["params"].as_array().expect("params");
            let Some(dataset) = params.iter().find(|p| p["name"] == "dane") else {
                continue;
            };
            for choice in dataset["choices"].as_array().expect("choices") {
                let choice = choice.as_str().expect("a choice is text");
                let entry = wfs_by_label(choice).unwrap_or_else(|| {
                    panic!(
                        "{} offers '{choice}', which the catalogue does not have",
                        tool["id"]
                    )
                });
                if entry.index {
                    index_ids.push(entry.id.to_string());
                } else {
                    layer_choices.push(entry.name.to_string());
                }
            }
        }
        let mut offered_names: Vec<String> =
            offered().map(|layer| layer.name.to_string()).collect();
        layer_choices.sort();
        offered_names.sort();
        assert_eq!(
            layer_choices, offered_names,
            "the tools and the catalogue disagree"
        );

        if index_ids.is_empty() {
            return;
        }
        let mut all_index_ids: Vec<String> = wfs_layers()
            .iter()
            .filter(|layer| layer.index)
            .map(|layer| layer.id.to_string())
            .collect();
        let offered_index = index_ids.len();
        index_ids.sort();
        index_ids.dedup();
        assert_eq!(offered_index, index_ids.len(), "an index is offered twice");
        all_index_ids.sort();
        assert_eq!(
            index_ids, all_index_ids,
            "the index tools and the catalogue disagree"
        );
    }

    /// The files on disk may still be the empty placeholders, so the documented shape is read here.
    #[test]
    fn a_family_file_of_the_documented_shape_is_read() {
        let wody: WodyFile = serde_json::from_str(
            r#"{"attribution":"PGW Wody Polskie (ISOK)","zoom":18,"services":[
                {"id":"wody-pgwwp","title":"Przykładowa usługa","url":"https://example.test/WMSServer","layers":["0","1","2"]}]}"#,
        )
        .expect("the documented wody shape");
        assert_eq!(wody.attribution, "PGW Wody Polskie (ISOK)");
        assert_eq!(wody.zoom, 18);
        assert_eq!(wody.services[0].layers, ["0", "1", "2"]);

        let carto: CartoFile = serde_json::from_str(
            r#"{"attribution":"Cartomatic · IH PAN","zoom":18,"series":{"plans":"plany","topo":"topo"},
                "maps":[{"id":"carto-wig100k","title":"Przykładowa mapa","layer":"wig100k_3857","series":"topo"}]}"#,
        )
        .expect("the documented cartomatic shape");
        assert_eq!(carto.attribution, "Cartomatic · IH PAN");
        assert_eq!(carto.series["topo"], "topo");
        assert_eq!(carto.maps[0].layer, "wig100k_3857");
        assert_eq!(carto.maps[0].series, "topo");
    }

    /// A command id, a title and a slug all name the same service, so no entry may be shadowed.
    #[test]
    fn every_wody_service_has_a_unique_id_and_a_title() {
        for service in wody() {
            assert!(service.id.starts_with("wody-"), "{} has no wody- prefix", service.id);
            assert!(!service.title.trim().is_empty(), "{} has no title", service.id);
            assert!(!service.url.trim().is_empty(), "{} has no address", service.id);
            assert!(!service.layers.is_empty(), "{} names no layer", service.id);
        }
        let mut ids: Vec<&str> = wody().iter().map(|service| service.id).collect();
        let mut titles: Vec<String> = wody().iter().map(|service| folded(service.title)).collect();
        let before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(before, ids.len(), "two services share an id");
        titles.sort_unstable();
        titles.dedup();
        assert_eq!(before, titles.len(), "two services share a title");
    }

    #[test]
    fn every_carto_map_has_a_unique_id_and_a_title() {
        for map in cartomatic() {
            assert!(map.id.starts_with("carto-"), "{} has no carto- prefix", map.id);
            assert!(!map.title.trim().is_empty(), "{} has no title", map.id);
            assert!(!map.layer.trim().is_empty(), "{} names no layer", map.id);
        }
        let mut ids: Vec<&str> = cartomatic().iter().map(|map| map.id).collect();
        let mut titles: Vec<String> = cartomatic().iter().map(|map| folded(map.title)).collect();
        let before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(before, ids.len(), "two maps share an id");
        titles.sort_unstable();
        titles.dedup();
        assert_eq!(before, titles.len(), "two maps share a title");
    }

    /// A field the file's builder leaves incomplete loses the whole family, so the file is read whole.
    #[test]
    fn the_wody_file_is_read_whole() {
        let file: WodyFile =
            serde_json::from_str(WODY_RAW).expect("data/wody.json is not the family shape");
        assert_eq!(
            wody().len(),
            file.services.len(),
            "an entry was skipped: a field is missing"
        );
        assert_eq!(wody_family().zoom, file.zoom);
        assert_eq!(wody_family().attribution, file.attribution);
    }

    #[test]
    fn the_cartomatic_file_is_read_whole() {
        let file: CartoFile =
            serde_json::from_str(CARTO_RAW).expect("data/cartomatic.json is not the family shape");
        assert_eq!(
            cartomatic().len(),
            file.maps.len(),
            "an entry was skipped: a field is missing"
        );
        assert_eq!(carto_family().zoom, file.zoom);
        assert_eq!(carto_family().attribution, file.attribution);
        assert_eq!(carto_series().len(), file.series.len());
    }

    /// Every id, every title and a slug of either resolves to its own entry, in any case.
    #[test]
    fn finds_a_wody_service_by_id_title_and_slug() {
        for service in wody() {
            let by_id =
                find_wody(service.id).unwrap_or_else(|| panic!("{} not found by id", service.id));
            assert_eq!(by_id.id, service.id);
            let by_title = find_wody(service.title)
                .unwrap_or_else(|| panic!("{} not found by title", service.id));
            assert_eq!(by_title.id, service.id, "{} resolves elsewhere by title", service.id);
            for slug in [
                folded(service.title),
                folded(service.id),
                folded(service.id.trim_start_matches("wody-")),
            ] {
                assert_eq!(
                    find_wody(&slug).map(|found| found.id),
                    Some(service.id),
                    "{} is not found by '{slug}'",
                    service.id
                );
            }
            assert_eq!(
                find_wody(&service.id.to_uppercase()).map(|found| found.id),
                Some(service.id),
                "{} is not found in another case",
                service.id
            );
        }
        assert!(find_wody("wody-nothing").is_none());
    }

    #[test]
    fn finds_a_carto_map_by_id_title_and_slug() {
        for map in cartomatic() {
            let by_id = find_carto(map.id).unwrap_or_else(|| panic!("{} not found by id", map.id));
            assert_eq!(by_id.id, map.id);
            let by_title =
                find_carto(map.title).unwrap_or_else(|| panic!("{} not found by title", map.id));
            assert_eq!(by_title.id, map.id, "{} resolves elsewhere by title", map.id);
            for slug in [
                folded(map.title),
                folded(map.id),
                folded(map.id.trim_start_matches("carto-")),
            ] {
                assert_eq!(
                    find_carto(&slug).map(|found| found.id),
                    Some(map.id),
                    "{} is not found by '{slug}'",
                    map.id
                );
            }
            assert_eq!(
                find_carto(&map.id.to_uppercase()).map(|found| found.id),
                Some(map.id),
                "{} is not found in another case",
                map.id
            );
        }
        assert!(find_carto("carto-nothing").is_none());
    }

    /// A series the file does not describe leaves the map without a heading in the ribbon.
    #[test]
    fn every_carto_map_names_a_series_the_file_describes() {
        let known: Vec<&str> = carto_series().iter().map(|(name, _)| *name).collect();
        for map in cartomatic() {
            assert!(!map.series.is_empty(), "{} names no series", map.id);
            assert!(
                known.is_empty() || known.contains(&map.series),
                "{} names series '{}', which the file does not describe",
                map.id,
                map.series
            );
        }
    }
}
