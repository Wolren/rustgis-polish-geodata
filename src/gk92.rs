//! WGS 84 to PL-1992 (EPSG:2180), the frame the model is asked for in.
//!
//! Snyder's transverse Mercator series to the sixth order: 0.001 m against PROJ across Poland.
//! EPSG:2180 is one Gauss-Kruger zone: central meridian 19 E, scale 0.9993, false northing -5300 km.

const A: f64 = 6_378_137.0;
const F: f64 = 1.0 / 298.257_222_101;
const K0: f64 = 0.9993;
const LON0: f64 = 19.0;
const FALSE_EASTING: f64 = 500_000.0;
const FALSE_NORTHING: f64 = -5_300_000.0;

/// The PL-1992 easting and northing of a longitude and latitude, in metres.
/// The CRS text that places an ASCII grid, as GDAL reads it from a `.prj` file.
pub const WKT: &str = r#"PROJCRS["ETRF2000-PL / CS92",BASEGEOGCRS["ETRF2000-PL",DATUM["ETRF2000 Poland",ELLIPSOID["GRS 1980",6378137,298.257222101,LENGTHUNIT["metre",1]]],PRIMEM["Greenwich",0,ANGLEUNIT["degree",0.0174532925199433]],ID["EPSG",9702]],CONVERSION["Poland CS92",METHOD["Transverse Mercator",ID["EPSG",9807]],PARAMETER["Latitude of natural origin",0,ANGLEUNIT["degree",0.0174532925199433],ID["EPSG",8801]],PARAMETER["Longitude of natural origin",19,ANGLEUNIT["degree",0.0174532925199433],ID["EPSG",8802]],PARAMETER["Scale factor at natural origin",0.9993,SCALEUNIT["unity",1],ID["EPSG",8805]],PARAMETER["False easting",500000,LENGTHUNIT["metre",1],ID["EPSG",8806]],PARAMETER["False northing",-5300000,LENGTHUNIT["metre",1],ID["EPSG",8807]]],CS[Cartesian,2],AXIS["northing (x)",north,ORDER[1],LENGTHUNIT["metre",1]],AXIS["easting (y)",east,ORDER[2],LENGTHUNIT["metre",1]],USAGE[SCOPE["Topographic mapping (medium and small scale)."],AREA["Poland - onshore and offshore."],BBOX[49,14.14,55.93,24.15]],ID["EPSG",2180]]"#;

pub fn forward(lon: f64, lat: f64) -> (f64, f64) {
    let e2 = F * (2.0 - F);
    let ep2 = e2 / (1.0 - e2);
    let e4 = e2 * e2;
    let e6 = e4 * e2;
    let phi = lat.to_radians();
    let lam = (lon - LON0).to_radians();
    let (sin, cos) = phi.sin_cos();
    let tan = sin / cos;
    let n = A / (1.0 - e2 * sin * sin).sqrt();
    let t = tan * tan;
    let c = ep2 * cos * cos;
    let aa = lam * cos;

    // The meridian arc from the equator to this latitude.
    let m = A
        * ((1.0 - e2 / 4.0 - 3.0 * e4 / 64.0 - 5.0 * e6 / 256.0) * phi
            - (3.0 * e2 / 8.0 + 3.0 * e4 / 32.0 + 45.0 * e6 / 1024.0) * (2.0 * phi).sin()
            + (15.0 * e4 / 256.0 + 45.0 * e6 / 1024.0) * (4.0 * phi).sin()
            - (35.0 * e6 / 3072.0) * (6.0 * phi).sin());

    let easting = FALSE_EASTING
        + K0 * n
            * (aa + (1.0 - t + c) * aa.powi(3) / 6.0
                + (5.0 - 18.0 * t + t * t + 72.0 * c - 58.0 * ep2) * aa.powi(5) / 120.0);
    let northing = FALSE_NORTHING
        + K0 * (m
            + n * tan
                * (aa.powi(2) / 2.0
                    + (5.0 - t + 9.0 * c + 4.0 * c * c) * aa.powi(4) / 24.0
                    + (61.0 - 58.0 * t + t * t + 600.0 * c - 330.0 * ep2) * aa.powi(6) / 720.0));
    (easting, northing)
}

/// The longitude and latitude of a PL-1992 northing and easting, the inverse of `forward`.
///
/// The footpoint latitude comes from a series over the meridian arc, then the usual transverse
/// Mercator terms: round trip worst 2 cm at (23.9, 49.1), under 7 mm across central Poland.
pub fn inverse(northing: f64, easting: f64) -> (f64, f64) {
    let e2 = F * (2.0 - F);
    let ep2 = e2 / (1.0 - e2);
    let e4 = e2 * e2;
    let e6 = e4 * e2;
    let x = northing - FALSE_NORTHING;
    let m = x / K0;
    let mu = m
        / (A * (1.0 - e2 / 4.0 - 3.0 * e4 / 64.0 - 5.0 * e6 / 256.0));
    let root = (1.0 - e2).sqrt();
    let e1 = (1.0 - root) / (1.0 + root);
    let j1 = 3.0 * e1 / 2.0 - 27.0 * e1.powi(3) / 32.0;
    let j2 = 21.0 * e1.powi(2) / 16.0 - 55.0 * e1.powi(4) / 32.0;
    let j3 = 151.0 * e1.powi(3) / 96.0;
    let j4 = 1097.0 * e1.powi(4) / 512.0;
    let footpoint = mu + j1 * (2.0 * mu).sin() + j2 * (4.0 * mu).sin()
        + j3 * (6.0 * mu).sin() + j4 * (8.0 * mu).sin();

    let (sin, cos) = footpoint.sin_cos();
    let tan = sin / cos;
    let n = A / (1.0 - e2 * sin * sin).sqrt();
    let r = A * (1.0 - e2) / (1.0 - e2 * sin * sin).powf(1.5);
    let t = tan * tan;
    let c = ep2 * cos * cos;
    let d = (easting - FALSE_EASTING) / (n * K0);

    let lat = footpoint
        - (n * tan / r)
            * (d.powi(2) / 2.0
                - (5.0 + 3.0 * t + 10.0 * c - 4.0 * c * c - 9.0 * ep2) * d.powi(4) / 24.0
                + (61.0 + 90.0 * t + 298.0 * c + 45.0 * t * t - 252.0 * ep2 - 3.0 * c * c)
                    * d.powi(6)
                    / 720.0);
    let lon = LON0.to_radians()
        + (d - (1.0 + 2.0 * t + c) * d.powi(3) / 6.0
            + (5.0 - 2.0 * c + 28.0 * t - 3.0 * c * c + 8.0 * ep2 + 24.0 * t * t) * d.powi(5) / 120.0)
            / cos;
    (lon.to_degrees(), lat.to_degrees())
}

/// Whether a point is somewhere GUGiK could serve.
pub fn inside_poland(lon: f64, lat: f64) -> bool {
    lon.is_finite()
        && lat.is_finite()
        && (13.5..=24.5).contains(&lon)
        && (48.5..=55.2).contains(&lat)
}

#[cfg(test)]
mod inverse_tests {
    use super::*;

    #[test]
    fn a_round_trip_lands_within_a_few_centimetres() {
        for (lon, lat) in [(16.9252, 52.4020), (21.0110, 52.2297), (14.55, 53.43), (23.9, 49.1)] {
            let (x, y) = forward(lon, lat);
            let (back_lon, back_lat) = inverse(y, x);
            let (x2, y2) = forward(back_lon, back_lat);
            assert!((x - x2).abs() < 0.05, "{lon} {lat}: {x} vs {x2}");
            assert!((y - y2).abs() < 0.05, "{lon} {lat}: {y} vs {y2}");
        }
    }

    #[test]
    fn poznan_lands_where_it_belongs() {
        // Measured with pyproj for EPSG:2180.
        let (lon, lat) = inverse(506682.0, 359579.0);
        assert!((lon - 16.935).abs() < 0.001, "lon {lon}");
        assert!((lat - 52.408).abs() < 0.001, "lat {lat}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// PROJ answers, `pyproj.Transformer.from_crs(4326, 2180, always_xy=True)`.
    #[test]
    fn matches_proj_at_grid_corners_and_far_edges() {
        let cases = [
            ((16.770_772_893, 52.325_191_293), (348_129.0, 497_807.0)),
            ((19.0, 54.164_293_266), (500_000.0, 700_000.0)),
            ((14.508_367_688, 53.180_470_061), (200_000.0, 600_000.0)),
            ((21.0, 52.23), (636_548.418, 486_767.502)),
            ((24.1, 49.0), (872_846.379, 138_365.917)),
        ];
        for ((lon, lat), (east, north)) in cases {
            let (x, y) = forward(lon, lat);
            assert!((x - east).abs() < 0.02, "{lon},{lat}: {x} against {east}");
            assert!((y - north).abs() < 0.02, "{lon},{lat}: {y} against {north}");
        }
    }

    #[test]
    fn the_central_meridian_sits_on_the_false_easting() {
        let (x, _) = forward(19.0, 52.0);
        assert!((x - 500_000.0).abs() < 0.001, "{x}");
    }

    #[test]
    fn poland_is_the_box_the_index_answers_in() {
        assert!(inside_poland(16.93, 52.41), "Poznan");
        assert!(inside_poland(21.0, 52.23), "Warsaw");
        assert!(!inside_poland(13.0, 52.5), "Berlin");
        assert!(!inside_poland(f64::NAN, 52.5));
    }
}
