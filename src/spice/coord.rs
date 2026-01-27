//! Coordinate system conversions.
//!
//! Provides type-safe conversions between coordinate systems:
//! - **Rectangular** (Cartesian): x, y, z
//! - **Latitudinal**: radius, longitude, latitude
//! - **Spherical**: radius, colatitude, longitude
//! - **Cylindrical**: r (radial), longitude, z
//!
//! All angles are in radians. Conversions follow NAIF CSPICE conventions.
//!
//! # Example
//!
//! ```
//! use muad_dib::spice::coord::{Rectangular, Latitudinal};
//!
//! let rect = Rectangular([6378.0, 0.0, 0.0]);
//! let lat = rect.to_latitudinal();
//! assert!((lat.radius - 6378.0).abs() < 1e-10);
//! assert!(lat.longitude.abs() < 1e-10);  // On +X axis
//! assert!(lat.latitude.abs() < 1e-10);   // On equator
//! ```

use std::f64::consts::PI;

/// Rectangular (Cartesian) coordinates.
///
/// Components are [x, y, z] in the same units as the source data (typically km).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rectangular(pub [f64; 3]);

impl Rectangular {
    /// Create from x, y, z components.
    #[inline]
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Rectangular([x, y, z])
    }

    /// Get the x component.
    #[inline]
    pub fn x(&self) -> f64 {
        self.0[0]
    }

    /// Get the y component.
    #[inline]
    pub fn y(&self) -> f64 {
        self.0[1]
    }

    /// Get the z component.
    #[inline]
    pub fn z(&self) -> f64 {
        self.0[2]
    }

    /// Compute the magnitude (Euclidean norm).
    #[inline]
    pub fn magnitude(&self) -> f64 {
        (self.0[0].powi(2) + self.0[1].powi(2) + self.0[2].powi(2)).sqrt()
    }

    /// Convert to latitudinal coordinates.
    ///
    /// Returns radius, longitude (-π to π), latitude (-π/2 to π/2).
    pub fn to_latitudinal(&self) -> Latitudinal {
        let [x, y, z] = self.0;
        let radius = self.magnitude();

        if radius == 0.0 {
            return Latitudinal {
                radius: 0.0,
                longitude: 0.0,
                latitude: 0.0,
            };
        }

        let longitude = y.atan2(x);
        let latitude = (z / radius).asin();

        Latitudinal {
            radius,
            longitude,
            latitude,
        }
    }

    /// Convert to spherical coordinates.
    ///
    /// Returns radius, colatitude (0 to π), longitude (-π to π).
    /// Colatitude is measured from the +Z axis.
    pub fn to_spherical(&self) -> Spherical {
        let [x, y, z] = self.0;
        let radius = self.magnitude();

        if radius == 0.0 {
            return Spherical {
                radius: 0.0,
                colatitude: 0.0,
                longitude: 0.0,
            };
        }

        let longitude = y.atan2(x);
        let colatitude = (z / radius).acos();

        Spherical {
            radius,
            colatitude,
            longitude,
        }
    }

    /// Convert to cylindrical coordinates.
    ///
    /// Returns r (radial distance in xy-plane), longitude (-π to π), z.
    pub fn to_cylindrical(&self) -> Cylindrical {
        let [x, y, z] = self.0;
        let r = (x.powi(2) + y.powi(2)).sqrt();
        let longitude = y.atan2(x);

        Cylindrical { r, longitude, z }
    }
}

/// Latitudinal coordinates (planetographic-style).
///
/// - `radius`: Distance from origin
/// - `longitude`: Angle in the xy-plane from +X axis, range (-π, π]
/// - `latitude`: Angle from the xy-plane toward +Z, range [-π/2, π/2]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Latitudinal {
    pub radius: f64,
    pub longitude: f64,
    pub latitude: f64,
}

impl Latitudinal {
    /// Create new latitudinal coordinates.
    #[inline]
    pub fn new(radius: f64, longitude: f64, latitude: f64) -> Self {
        Latitudinal {
            radius,
            longitude,
            latitude,
        }
    }

    /// Convert longitude from radians to degrees.
    #[inline]
    pub fn longitude_deg(&self) -> f64 {
        self.longitude * 180.0 / PI
    }

    /// Convert latitude from radians to degrees.
    #[inline]
    pub fn latitude_deg(&self) -> f64 {
        self.latitude * 180.0 / PI
    }
}

impl From<Latitudinal> for Rectangular {
    fn from(lat: Latitudinal) -> Self {
        let cos_lat = lat.latitude.cos();
        let x = lat.radius * cos_lat * lat.longitude.cos();
        let y = lat.radius * cos_lat * lat.longitude.sin();
        let z = lat.radius * lat.latitude.sin();
        Rectangular([x, y, z])
    }
}

/// Spherical coordinates.
///
/// - `radius`: Distance from origin
/// - `colatitude`: Angle from +Z axis, range [0, π]
/// - `longitude`: Angle in the xy-plane from +X axis, range (-π, π]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spherical {
    pub radius: f64,
    pub colatitude: f64,
    pub longitude: f64,
}

impl Spherical {
    /// Create new spherical coordinates.
    #[inline]
    pub fn new(radius: f64, colatitude: f64, longitude: f64) -> Self {
        Spherical {
            radius,
            colatitude,
            longitude,
        }
    }

    /// Convert colatitude from radians to degrees.
    #[inline]
    pub fn colatitude_deg(&self) -> f64 {
        self.colatitude * 180.0 / PI
    }

    /// Convert longitude from radians to degrees.
    #[inline]
    pub fn longitude_deg(&self) -> f64 {
        self.longitude * 180.0 / PI
    }
}

impl From<Spherical> for Rectangular {
    fn from(sph: Spherical) -> Self {
        let sin_co = sph.colatitude.sin();
        let x = sph.radius * sin_co * sph.longitude.cos();
        let y = sph.radius * sin_co * sph.longitude.sin();
        let z = sph.radius * sph.colatitude.cos();
        Rectangular([x, y, z])
    }
}

/// Cylindrical coordinates.
///
/// - `r`: Radial distance in the xy-plane
/// - `longitude`: Angle in the xy-plane from +X axis, range (-π, π]
/// - `z`: Height along the z-axis
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cylindrical {
    pub r: f64,
    pub longitude: f64,
    pub z: f64,
}

impl Cylindrical {
    /// Create new cylindrical coordinates.
    #[inline]
    pub fn new(r: f64, longitude: f64, z: f64) -> Self {
        Cylindrical { r, longitude, z }
    }

    /// Convert longitude from radians to degrees.
    #[inline]
    pub fn longitude_deg(&self) -> f64 {
        self.longitude * 180.0 / PI
    }
}

impl From<Cylindrical> for Rectangular {
    fn from(cyl: Cylindrical) -> Self {
        let x = cyl.r * cyl.longitude.cos();
        let y = cyl.r * cyl.longitude.sin();
        Rectangular([x, y, cyl.z])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    const EPSILON: f64 = 1e-12;

    fn assert_near(a: f64, b: f64, msg: &str) {
        assert!(
            (a - b).abs() < EPSILON,
            "{}: {} != {} (diff={})",
            msg,
            a,
            b,
            (a - b).abs()
        );
    }

    #[test]
    fn test_rectangular_to_latitudinal() {
        // Point on +X axis
        let rect = Rectangular([6378.0, 0.0, 0.0]);
        let lat = rect.to_latitudinal();
        assert_near(lat.radius, 6378.0, "radius");
        assert_near(lat.longitude, 0.0, "longitude");
        assert_near(lat.latitude, 0.0, "latitude");

        // Point on +Y axis
        let rect = Rectangular([0.0, 6378.0, 0.0]);
        let lat = rect.to_latitudinal();
        assert_near(lat.radius, 6378.0, "radius");
        assert_near(lat.longitude, FRAC_PI_2, "longitude");
        assert_near(lat.latitude, 0.0, "latitude");

        // Point on +Z axis (north pole)
        let rect = Rectangular([0.0, 0.0, 6378.0]);
        let lat = rect.to_latitudinal();
        assert_near(lat.radius, 6378.0, "radius");
        assert_near(lat.latitude, FRAC_PI_2, "latitude");
    }

    #[test]
    fn test_latitudinal_to_rectangular() {
        // Round-trip test
        let original = Rectangular([1000.0, 2000.0, 3000.0]);
        let lat = original.to_latitudinal();
        let back: Rectangular = lat.into();

        assert_near(back.x(), original.x(), "x");
        assert_near(back.y(), original.y(), "y");
        assert_near(back.z(), original.z(), "z");
    }

    #[test]
    fn test_rectangular_to_spherical() {
        // Point on +Z axis
        let rect = Rectangular([0.0, 0.0, 6378.0]);
        let sph = rect.to_spherical();
        assert_near(sph.radius, 6378.0, "radius");
        assert_near(sph.colatitude, 0.0, "colatitude at north pole");

        // Point on -Z axis
        let rect = Rectangular([0.0, 0.0, -6378.0]);
        let sph = rect.to_spherical();
        assert_near(sph.colatitude, PI, "colatitude at south pole");

        // Point on equator +X
        let rect = Rectangular([6378.0, 0.0, 0.0]);
        let sph = rect.to_spherical();
        assert_near(sph.colatitude, FRAC_PI_2, "colatitude on equator");
    }

    #[test]
    fn test_spherical_to_rectangular() {
        // Round-trip test
        let original = Rectangular([1000.0, 2000.0, 3000.0]);
        let sph = original.to_spherical();
        let back: Rectangular = sph.into();

        assert_near(back.x(), original.x(), "x");
        assert_near(back.y(), original.y(), "y");
        assert_near(back.z(), original.z(), "z");
    }

    #[test]
    fn test_rectangular_to_cylindrical() {
        // Point on +X axis
        let rect = Rectangular([6378.0, 0.0, 1000.0]);
        let cyl = rect.to_cylindrical();
        assert_near(cyl.r, 6378.0, "r");
        assert_near(cyl.longitude, 0.0, "longitude");
        assert_near(cyl.z, 1000.0, "z");

        // Point on +Y axis
        let rect = Rectangular([0.0, 6378.0, 2000.0]);
        let cyl = rect.to_cylindrical();
        assert_near(cyl.r, 6378.0, "r");
        assert_near(cyl.longitude, FRAC_PI_2, "longitude");
        assert_near(cyl.z, 2000.0, "z");
    }

    #[test]
    fn test_cylindrical_to_rectangular() {
        // Round-trip test
        let original = Rectangular([1000.0, 2000.0, 3000.0]);
        let cyl = original.to_cylindrical();
        let back: Rectangular = cyl.into();

        assert_near(back.x(), original.x(), "x");
        assert_near(back.y(), original.y(), "y");
        assert_near(back.z(), original.z(), "z");
    }

    #[test]
    fn test_origin() {
        let origin = Rectangular([0.0, 0.0, 0.0]);

        let lat = origin.to_latitudinal();
        assert_eq!(lat.radius, 0.0);

        let sph = origin.to_spherical();
        assert_eq!(sph.radius, 0.0);

        let cyl = origin.to_cylindrical();
        assert_eq!(cyl.r, 0.0);
        assert_eq!(cyl.z, 0.0);
    }

    #[test]
    fn test_magnitude() {
        let rect = Rectangular([3.0, 4.0, 0.0]);
        assert_near(rect.magnitude(), 5.0, "3-4-5 triangle");

        let rect = Rectangular([1.0, 2.0, 2.0]);
        assert_near(rect.magnitude(), 3.0, "1-2-2 vector");
    }

    #[test]
    fn test_degree_conversions() {
        let lat = Latitudinal::new(1.0, PI / 4.0, PI / 6.0);
        assert_near(lat.longitude_deg(), 45.0, "longitude degrees");
        assert_near(lat.latitude_deg(), 30.0, "latitude degrees");

        let sph = Spherical::new(1.0, PI / 3.0, PI / 2.0);
        assert_near(sph.colatitude_deg(), 60.0, "colatitude degrees");
        assert_near(sph.longitude_deg(), 90.0, "longitude degrees");

        let cyl = Cylindrical::new(1.0, -PI / 4.0, 0.0);
        assert_near(cyl.longitude_deg(), -45.0, "longitude degrees");
    }
}
