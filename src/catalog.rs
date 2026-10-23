//! The catalogue: GUGiK overlays (web map services) and GUGiK features (WFS).
//!
//! Overlay addresses are KVP templates. RustGIS fills in `{z}`/`{x}`/`{y}` for XYZ and
//! WMTS layers, and `{bbox-epsg-3857}`, `{width}`, `{height}` for WMS ones.

/// Where the data comes from, shown as the layer's attribution.
pub const ATTRIBUTION: &str = "GUGiK · geoportal.gov.pl";

/// Poland's extent, `[west, south, east, north]` — used to bring the map to the data.
pub const POLAND: [f64; 4] = [14.07, 48.99, 24.15, 54.84];

pub struct Overlay {
    pub id: &'static str,
    pub name: &'static str,
    pub url: &'static str,
    pub max_zoom: u8,
    /// The shallowest zoom the service draws at; below it its tiles are blank.
    pub min_zoom: u8,
    /// The tile size the service's cartography is drawn for (BDOT10k needs 512).
    pub tile_size: u16,
}

/// BDOT10k's composition service needs its layer list in one request; these are the layers
/// the service draws (the unnamed groups in its capabilities can't be requested).
const BDOT_LAYERS: &str = "RZab,TPrz,UTrw,TLes,RKr,RTr,Mo,WPow,Szu,LBrz,WPowEt,GrPol,Rez,GrPK,GrPN,\
PRur,ZbTA,BudCm,TerCm,BudSp,Bzn,BHydA,BHydL,IUTA,ObOrA,ObPL,Prom,UTrL,NazDr,NrDr,Umo,UTrP,ObKom,\
INUTP,ZbTP,NazUl,ObOrP,WyBT";

pub const OVERLAYS: &[Overlay] = &[
    Overlay {
        id: "orto",
        name: "Ortofoto (GUGiK)",
        url: "https://mapy.geoportal.gov.pl/wss/service/PZGIK/ORTO/WMTS/StandardResolution?\
SERVICE=WMTS&REQUEST=GetTile&VERSION=1.0.0&LAYER=ORTOFOTOMAPA&STYLE=default&\
TILEMATRIXSET=EPSG:3857&TILEMATRIX=EPSG:3857:{z}&TILEROW={y}&TILECOL={x}&FORMAT=image/jpeg",
        max_zoom: 19,
        min_zoom: 0,
        tile_size: 256,
    },
    Overlay {
        id: "orto-hi",
        name: "Ortofoto Hi-Res (GUGiK)",
        url: "https://mapy.geoportal.gov.pl/wss/service/PZGIK/ORTO/WMS/HighResolution?\
SERVICE=WMS&VERSION=1.3.0&REQUEST=GetMap&LAYERS=Raster&STYLES=&CRS=EPSG:3857&\
BBOX={bbox-epsg-3857}&WIDTH={width}&HEIGHT={height}&FORMAT=image/jpeg&TRANSPARENT=false",
        max_zoom: 19,
        min_zoom: 0,
        tile_size: 256,
    },
    Overlay {
        id: "bdot",
        name: "BDOT10k (GUGiK)",
        url: BDOT_URL,
        max_zoom: 18,
        min_zoom: 13,
        tile_size: 512,
    },
    Overlay {
        id: "dzialki-wms",
        name: "Działki (KIEG)",
        url: KIEG_URL,
        max_zoom: 21,
        min_zoom: 15,
        tile_size: 256,
    },
    Overlay {
        id: "adresy",
        name: "Adresy (GUGiK)",
        url: "https://mapy.geoportal.gov.pl/wss/ext/KrajowaIntegracjaNumeracjiAdresowej?\
SERVICE=WMS&VERSION=1.3.0&REQUEST=GetMap&LAYERS=A07_Punkty_adresowe,A08_Ulice&STYLES=&\
CRS=EPSG:3857&BBOX={bbox-epsg-3857}&WIDTH={width}&HEIGHT={height}&FORMAT=image/png&TRANSPARENT=true",
        max_zoom: 21,
        min_zoom: 15,
        tile_size: 256,
    },
    Overlay {
        id: "prg-wms",
        name: "Granice PRG",
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
        name: "Nazwy miejscowości (PRNG)",
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
        name: "NMT · cieniowanie",
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
pub struct WfsLayer {
    pub id: &'static str,
    pub name: &'static str,
    /// The WFS endpoint, without its query string.
    pub url: &'static str,
    /// The feature type, as the service names it (WFS 1.1 `TYPENAME`).
    pub type_name: &'static str,
    /// The widest area one download may cover, in degrees.
    pub max_span: f64,
    /// The geometry property a WFS spatial filter names (the service accepts it bare).
    pub geom_field: &'static str,
}

pub const WFS_LAYERS: &[WfsLayer] = &[
    WfsLayer {
        id: "dzialki",
        name: "Działki (EGiB)",
        url: "https://mapy.geoportal.gov.pl/wss/service/PZGIK/EGIB/WFS/UslugaZbiorcza",
        type_name: "dzialki",
        max_span: 0.15,
        geom_field: "geom",
    },
    WfsLayer {
        id: "budynki",
        name: "Budynki (EGiB)",
        url: "https://mapy.geoportal.gov.pl/wss/service/PZGIK/EGIB/WFS/UslugaZbiorcza",
        type_name: "budynki",
        max_span: 0.15,
        geom_field: "geom",
    },
    WfsLayer {
        id: "gminy",
        name: "Gminy (PRG)",
        url: "https://mapy.geoportal.gov.pl/wss/service/PZGIK/PRG/WFS/AdministrativeBoundaries",
        type_name: "A03_Granice_gmin",
        max_span: 20.0,
        geom_field: "msGeometry",
    },
    WfsLayer {
        id: "powiaty",
        name: "Powiaty (PRG)",
        url: "https://mapy.geoportal.gov.pl/wss/service/PZGIK/PRG/WFS/AdministrativeBoundaries",
        type_name: "A02_Granice_powiatow",
        max_span: 20.0,
        geom_field: "msGeometry",
    },
    WfsLayer {
        id: "wojewodztwa",
        name: "Województwa (PRG)",
        url: "https://mapy.geoportal.gov.pl/wss/service/PZGIK/PRG/WFS/AdministrativeBoundaries",
        type_name: "A01_Granice_wojewodztw",
        max_span: 20.0,
        geom_field: "msGeometry",
    },
];

pub fn overlay(id: &str) -> Option<&'static Overlay> {
    OVERLAYS.iter().find(|o| o.id == id)
}

pub fn wfs(id: &str) -> Option<&'static WfsLayer> {
    WFS_LAYERS.iter().find(|w| w.id == id)
}

/// The layer a tool's choice label names.
pub fn wfs_by_label(label: &str) -> Option<&'static WfsLayer> {
    let id = match label {
        "Działki (EGiB)" => "dzialki",
        "Budynki (EGiB)" => "budynki",
        "Gminy (PRG)" => "gminy",
        "Powiaty (PRG)" => "powiaty",
        "Województwa (PRG)" => "wojewodztwa",
        _ => return None,
    };
    wfs(id)
}

/// The address RustGIS should fetch tiles from: BDOT10k's layer list filled in.
pub fn url_for(overlay: &Overlay) -> String {
    if overlay.id == "bdot" {
        BDOT_URL.replace("PLACEHOLDER", BDOT_LAYERS)
    } else {
        overlay.url.to_string()
    }
}
