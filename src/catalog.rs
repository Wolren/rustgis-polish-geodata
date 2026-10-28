//! The catalogue: GUGiK overlays (web map services) and GUGiK features (WFS).
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
        name_pl: "Nazwy miejscowości (PRNG)",
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
    /// GUGiK's title for the type, the same in both interface languages: a dataset's name is a
    /// proper name, not a word to translate.
    pub name: &'static str,
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
    wfs_layers().iter().find(|w| {
        w.id == wanted
            || w.name.eq_ignore_ascii_case(wanted)
            || slug(w.id) == key
            || slug(w.id.trim_start_matches("dane-")) == key
            || slug(&w.type_name) == key
            || alias
                .as_deref()
                .is_some_and(|kind| slug(&w.type_name) == kind)
    })
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
    /// tools offer is one the catalogue has, and no dataset is offered twice or lost.
    #[test]
    fn the_download_tools_offer_every_dataset_exactly_once() {
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../plugin.json")).expect("plugin.json is not JSON");
        let mut choices: Vec<String> = Vec::new();
        for tool in manifest["tools"].as_array().expect("tools") {
            let params = tool["params"].as_array().expect("params");
            let dataset = params
                .iter()
                .find(|p| p["name"] == "dane")
                .expect("a dataset parameter");
            for choice in dataset["choices"].as_array().expect("choices") {
                let choice = choice.as_str().expect("a choice is text");
                assert!(
                    wfs_by_label(choice).is_some(),
                    "{} offers '{choice}', which the catalogue does not have",
                    tool["id"]
                );
                choices.push(choice.to_string());
            }
        }
        let mut offered_names: Vec<String> =
            offered().map(|layer| layer.name.to_string()).collect();
        choices.sort();
        offered_names.sort();
        assert_eq!(
            choices, offered_names,
            "the tools and the catalogue disagree"
        );
    }
}
