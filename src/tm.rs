//! EPSG:2180 (PUWG 1992 / "Gauss-Krüger zone 5") ↔ WGS 84.
//!
//! ULDK, GUGiK's parcel service, takes coordinates in EPSG:2180 only: `srid=4326` answers
//! "no results". So the plugin projects the map centre into 2180 for the query and projects
//! the returned WKT back for the map. The series are the standard Transverse Mercator ones
//! (Snyder), accurate to a few centimetres over Poland.

use std::f64::consts::PI;

const A: f64 = 6_378_137.0;
const F: f64 = 1.0 / 298.257_222_101;
const K0: f64 = 0.9993;
const LON0: f64 = 19.0;
const EASTING0: f64 = 500_000.0;
const NORTHING0: f64 = -5_300_000.0;

fn e2() -> f64 {
    F * (2.0 - F)
}

fn ep2() -> f64 {
    e2() / (1.0 - e2())
}

/// Longitude, latitude (degrees) → `(easting, northing)` in EPSG:2180.
pub fn to_2180(lon: f64, lat: f64) -> (f64, f64) {
    let e2 = e2();
    let ep2 = ep2();
    let phi = lat.to_radians();
    let dlam = (lon - LON0).to_radians();
    let (sin, cos, tan) = (phi.sin(), phi.cos(), phi.tan());
    let n = A / (1.0 - e2 * sin * sin).sqrt();
    let t = tan * tan;
    let c = ep2 * cos * cos;
    let a = dlam * cos;
    let m = A
        * ((1.0 - e2 / 4.0 - 3.0 * e2.powi(2) / 64.0 - 5.0 * e2.powi(3) / 256.0) * phi
            - (3.0 * e2 / 8.0 + 3.0 * e2.powi(2) / 32.0 + 45.0 * e2.powi(3) / 1024.0) * (2.0 * phi).sin()
            + (15.0 * e2.powi(2) / 256.0 + 45.0 * e2.powi(3) / 1024.0) * (4.0 * phi).sin()
            - (35.0 * e2.powi(3) / 3072.0) * (6.0 * phi).sin());
    let easting = EASTING0
        + K0 * n
            * (a + (1.0 - t + c) * a.powi(3) / 6.0
                + (5.0 - 18.0 * t + t * t + 72.0 * c - 58.0 * ep2) * a.powi(5) / 120.0);
    let northing = NORTHING0
        + K0
            * (m + n * tan
                * (a.powi(2) / 2.0
                    + (5.0 - t + 9.0 * c + 4.0 * c * c) * a.powi(4) / 24.0
                    + (61.0 - 58.0 * t + t * t + 600.0 * c - 330.0 * ep2) * a.powi(6) / 720.0));
    (easting, northing)
}

/// `(easting, northing)` in EPSG:2180 → `[longitude, latitude]` in degrees.
pub fn to_wgs84(easting: f64, northing: f64) -> [f64; 2] {
    let e2 = e2();
    let ep2 = ep2();
    let x = easting - EASTING0;
    let y = northing - NORTHING0;
    let m = y / K0;
    let mu = m / (A * (1.0 - e2 / 4.0 - 3.0 * e2.powi(2) / 64.0 - 5.0 * e2.powi(3) / 256.0));
    let e1 = (1.0 - (1.0 - e2).sqrt()) / (1.0 + (1.0 - e2).sqrt());
    let phi1 = mu
        + (3.0 * e1 / 2.0 - 27.0 * e1.powi(3) / 32.0) * (2.0 * mu).sin()
        + (21.0 * e1.powi(2) / 16.0 - 55.0 * e1.powi(4) / 32.0) * (4.0 * mu).sin()
        + (151.0 * e1.powi(3) / 96.0) * (6.0 * mu).sin()
        + (1097.0 * e1.powi(4) / 512.0) * (8.0 * mu).sin();
    let (sin1, cos1, tan1) = (phi1.sin(), phi1.cos(), phi1.tan());
    let c1 = ep2 * cos1 * cos1;
    let t1 = tan1 * tan1;
    let n1 = A / (1.0 - e2 * sin1 * sin1).sqrt();
    let r1 = A * (1.0 - e2) / (1.0 - e2 * sin1 * sin1).powf(1.5);
    let d = x / (n1 * K0);
    let lat = phi1
        - (n1 * tan1 / r1)
            * (d.powi(2) / 2.0
                - (5.0 + 3.0 * t1 + 10.0 * c1 - 4.0 * c1 * c1 - 9.0 * ep2) * d.powi(4) / 24.0
                + (61.0 + 90.0 * t1 + 298.0 * c1 + 45.0 * t1 * t1 - 252.0 * ep2 - 3.0 * c1 * c1)
                    * d.powi(6)
                    / 720.0);
    let lon = LON0.to_radians()
        + (d - (1.0 + 2.0 * t1 + c1) * d.powi(3) / 6.0
            + (5.0 - 2.0 * c1 + 28.0 * t1 - 3.0 * c1 * c1 + 8.0 * ep2 + 24.0 * t1 * t1) * d.powi(5)
                / 120.0)
            / cos1;
    [lon * 180.0 / PI, lat * 180.0 / PI]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_maps_to_the_false_origin() {
        let (e, n) = to_2180(LON0, 0.0);
        assert!((e - 500_000.0).abs() < 0.001, "easting {e}");
        assert!((n - (-5_300_000.0)).abs() < 0.001, "northing {n}");
    }

    #[test]
    fn warsaw_is_near_637_486() {
        // A parcel in Warsaw: ULDK reports its vertices around 637 300 / 486 000.
        let (e, n) = to_2180(21.0117, 52.2297);
        assert!((e - 637_800.0).abs() < 3_000.0, "easting {e}");
        assert!((n - 486_000.0).abs() < 3_000.0, "northing {n}");
    }

    #[test]
    fn round_trip_is_stable() {
        // The series are good to a few centimetres over Poland; 1e-6° is about 10 cm.
        for (lon, lat) in [(14.2, 54.4), (19.0, 52.0), (24.1, 49.0), (22.5, 51.2)] {
            let (e, n) = to_2180(lon, lat);
            let [lon2, lat2] = to_wgs84(e, n);
            assert!((lon - lon2).abs() < 1e-6, "{lon} → {lon2}");
            assert!((lat - lat2).abs() < 1e-6, "{lat} → {lat2}");
        }
    }
}
