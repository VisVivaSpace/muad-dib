//! Coordinate conversion tests comparing muad-dib against CSPICE.
//!
//! Tests validate rectangular/latitudinal/spherical/cylindrical conversions
//! using pure math (no kernel loading required).
//!
//! Run with: cargo test --test cspice_coord_tests -- --test-threads=1

#![cfg(feature = "cspice")]

mod cspice_common;

use cspice_common::{
    assert_close, assert_vector_close, cspice_cylrec, cspice_latrec, cspice_reccyl, cspice_reclat,
    cspice_recsph, cspice_sphrec, CSPICE_LOCK,
};
use muad_dib::spice::coord::{Cylindrical, Latitudinal, Rectangular, Spherical};

/// Tolerance for coordinate conversions.
/// Using 1e-10 to account for floating-point precision limits in trigonometric
/// operations, especially for coordinate values around 5000 km.
const COORD_TOLERANCE: f64 = 1e-10;

/// Compare angles with wraparound handling.
/// Angles that differ by 2π are considered equivalent.
fn assert_angle_close(a: f64, b: f64, tolerance: f64, msg: &str) {
    let diff = (a - b).abs();
    let two_pi = 2.0 * std::f64::consts::PI;
    // Check if angles are equivalent (diff is near 0 or near 2π)
    let adjusted_diff = diff.min((two_pi - diff).abs());
    assert!(
        adjusted_diff < tolerance,
        "{}: {} != {} (diff={}, adjusted_diff={}, tolerance={})",
        msg,
        a,
        b,
        diff,
        adjusted_diff,
        tolerance
    );
}

/// Test points covering various coordinate configurations.
const TEST_POINTS: [[f64; 3]; 8] = [
    [6378.0, 0.0, 0.0],       // +X axis
    [0.0, 6378.0, 0.0],       // +Y axis
    [0.0, 0.0, 6378.0],       // +Z (north pole)
    [0.0, 0.0, -6378.0],      // -Z (south pole)
    [1000.0, 2000.0, 3000.0], // General point
    [-5000.0, -5000.0, 0.0],  // Negative XY
    [100.0, -200.0, 300.0],   // Mixed signs
    [1.0, 1.0, 1.0],          // Small values
];

// ============================================================================
// Rectangular to Latitudinal Tests
// ============================================================================

#[test]
fn validate_rectangular_to_latitudinal() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    for point in TEST_POINTS.iter() {
        // CSPICE conversion
        let (cspice_r, cspice_lon, cspice_lat) = cspice_reclat(point);

        // muad-dib conversion
        let rect = Rectangular(*point);
        let lat = rect.to_latitudinal();

        // Compare
        assert_close(
            lat.radius,
            cspice_r,
            COORD_TOLERANCE,
            &format!("radius for {:?}", point),
        );
        // Longitude can differ by 2π and still represent the same angle
        assert_angle_close(
            lat.longitude,
            cspice_lon,
            COORD_TOLERANCE,
            &format!("longitude for {:?}", point),
        );
        assert_close(
            lat.latitude,
            cspice_lat,
            COORD_TOLERANCE,
            &format!("latitude for {:?}", point),
        );
    }
}

#[test]
fn validate_latitudinal_to_rectangular() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    // Test values: (radius, longitude, latitude)
    let test_values: [(f64, f64, f64); 6] = [
        (6378.0, 0.0, 0.0),                          // Equator, prime meridian
        (6378.0, std::f64::consts::FRAC_PI_2, 0.0),  // Equator, 90° E
        (6378.0, 0.0, std::f64::consts::FRAC_PI_2),  // North pole
        (6378.0, 0.0, -std::f64::consts::FRAC_PI_2), // South pole
        (1000.0, 0.5, 0.3),                          // General point
        (5000.0, -2.0, -0.5),                        // Negative angles
    ];

    for (radius, lon, lat_angle) in test_values.iter() {
        // CSPICE conversion
        let cspice_rect = cspice_latrec(*radius, *lon, *lat_angle);

        // muad-dib conversion
        let lat = Latitudinal::new(*radius, *lon, *lat_angle);
        let rect: Rectangular = lat.into();

        // Compare
        assert_vector_close(
            &rect.0,
            &cspice_rect,
            COORD_TOLERANCE,
            &format!("rectangular for ({}, {}, {})", radius, lon, lat_angle),
        );
    }
}

#[test]
fn validate_latitudinal_round_trip() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    for point in TEST_POINTS.iter() {
        // Skip origin (undefined angles)
        if point.iter().all(|&x| x == 0.0) {
            continue;
        }

        // Convert to latitudinal and back
        let rect = Rectangular(*point);
        let lat = rect.to_latitudinal();
        let back: Rectangular = lat.into();

        // Compare with original
        assert_vector_close(
            &back.0,
            point,
            COORD_TOLERANCE,
            &format!("round-trip for {:?}", point),
        );

        // Also compare CSPICE round-trip
        let (r, lon, lat_angle) = cspice_reclat(point);
        let cspice_back = cspice_latrec(r, lon, lat_angle);
        assert_vector_close(
            &cspice_back,
            point,
            COORD_TOLERANCE,
            &format!("CSPICE round-trip for {:?}", point),
        );
    }
}

// ============================================================================
// Rectangular to Spherical Tests
// ============================================================================

#[test]
fn validate_rectangular_to_spherical() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    for point in TEST_POINTS.iter() {
        // CSPICE conversion
        let (cspice_r, cspice_colat, cspice_lon) = cspice_recsph(point);

        // muad-dib conversion
        let rect = Rectangular(*point);
        let sph = rect.to_spherical();

        // Compare
        assert_close(
            sph.radius,
            cspice_r,
            COORD_TOLERANCE,
            &format!("radius for {:?}", point),
        );
        assert_close(
            sph.colatitude,
            cspice_colat,
            COORD_TOLERANCE,
            &format!("colatitude for {:?}", point),
        );
        // Longitude can differ by 2π and still represent the same angle
        assert_angle_close(
            sph.longitude,
            cspice_lon,
            COORD_TOLERANCE,
            &format!("longitude for {:?}", point),
        );
    }
}

#[test]
fn validate_spherical_to_rectangular() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    // Test values: (radius, colatitude, longitude)
    let test_values: [(f64, f64, f64); 6] = [
        (6378.0, std::f64::consts::FRAC_PI_2, 0.0), // Equator, prime meridian
        (6378.0, 0.0, 0.0),                         // North pole
        (6378.0, std::f64::consts::PI, 0.0),        // South pole
        (
            6378.0,
            std::f64::consts::FRAC_PI_2,
            std::f64::consts::FRAC_PI_2,
        ), // Equator, 90° E
        (1000.0, 0.8, 0.5),                         // General point
        (5000.0, 2.0, -1.5),                        // Other general point
    ];

    for (r, colat, lon) in test_values.iter() {
        // CSPICE conversion
        let cspice_rect = cspice_sphrec(*r, *colat, *lon);

        // muad-dib conversion
        let sph = Spherical::new(*r, *colat, *lon);
        let rect: Rectangular = sph.into();

        // Compare
        assert_vector_close(
            &rect.0,
            &cspice_rect,
            COORD_TOLERANCE,
            &format!("rectangular for ({}, {}, {})", r, colat, lon),
        );
    }
}

#[test]
fn validate_spherical_round_trip() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    for point in TEST_POINTS.iter() {
        // Skip origin (undefined angles)
        if point.iter().all(|&x| x == 0.0) {
            continue;
        }

        // Convert to spherical and back
        let rect = Rectangular(*point);
        let sph = rect.to_spherical();
        let back: Rectangular = sph.into();

        // Compare with original
        assert_vector_close(
            &back.0,
            point,
            COORD_TOLERANCE,
            &format!("round-trip for {:?}", point),
        );

        // Also compare CSPICE round-trip
        let (r, colat, lon) = cspice_recsph(point);
        let cspice_back = cspice_sphrec(r, colat, lon);
        assert_vector_close(
            &cspice_back,
            point,
            COORD_TOLERANCE,
            &format!("CSPICE round-trip for {:?}", point),
        );
    }
}

// ============================================================================
// Rectangular to Cylindrical Tests
// ============================================================================

#[test]
fn validate_rectangular_to_cylindrical() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    for point in TEST_POINTS.iter() {
        // CSPICE conversion
        let (cspice_r, cspice_lon, cspice_z) = cspice_reccyl(point);

        // muad-dib conversion
        let rect = Rectangular(*point);
        let cyl = rect.to_cylindrical();

        // Compare
        assert_close(
            cyl.r,
            cspice_r,
            COORD_TOLERANCE,
            &format!("r for {:?}", point),
        );
        // Longitude can differ by 2π and still represent the same angle
        assert_angle_close(
            cyl.longitude,
            cspice_lon,
            COORD_TOLERANCE,
            &format!("longitude for {:?}", point),
        );
        assert_close(
            cyl.z,
            cspice_z,
            COORD_TOLERANCE,
            &format!("z for {:?}", point),
        );
    }
}

#[test]
fn validate_cylindrical_to_rectangular() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    // Test values: (r, longitude, z)
    let test_values: [(f64, f64, f64); 6] = [
        (6378.0, 0.0, 0.0),                         // On X axis at z=0
        (6378.0, std::f64::consts::FRAC_PI_2, 0.0), // On Y axis at z=0
        (0.0, 0.0, 6378.0),                         // On Z axis (north)
        (1000.0, 0.5, 2000.0),                      // General point
        (3000.0, -1.0, -500.0),                     // Negative z and angle
        (100.0, std::f64::consts::PI, 100.0),       // 180 degrees
    ];

    for (r, lon, z) in test_values.iter() {
        // CSPICE conversion
        let cspice_rect = cspice_cylrec(*r, *lon, *z);

        // muad-dib conversion
        let cyl = Cylindrical::new(*r, *lon, *z);
        let rect: Rectangular = cyl.into();

        // Compare
        assert_vector_close(
            &rect.0,
            &cspice_rect,
            COORD_TOLERANCE,
            &format!("rectangular for ({}, {}, {})", r, lon, z),
        );
    }
}

#[test]
fn validate_cylindrical_round_trip() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    for point in TEST_POINTS.iter() {
        // Convert to cylindrical and back
        let rect = Rectangular(*point);
        let cyl = rect.to_cylindrical();
        let back: Rectangular = cyl.into();

        // Compare with original
        assert_vector_close(
            &back.0,
            point,
            COORD_TOLERANCE,
            &format!("round-trip for {:?}", point),
        );

        // Also compare CSPICE round-trip
        let (r, lon, z) = cspice_reccyl(point);
        let cspice_back = cspice_cylrec(r, lon, z);
        assert_vector_close(
            &cspice_back,
            point,
            COORD_TOLERANCE,
            &format!("CSPICE round-trip for {:?}", point),
        );
    }
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn validate_origin_handling() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let origin = [0.0, 0.0, 0.0];
    let rect = Rectangular(origin);

    // Latitudinal
    let lat = rect.to_latitudinal();
    assert_eq!(lat.radius, 0.0, "Origin should have zero radius");

    // Spherical
    let sph = rect.to_spherical();
    assert_eq!(sph.radius, 0.0, "Origin should have zero radius");

    // Cylindrical
    let cyl = rect.to_cylindrical();
    assert_eq!(cyl.r, 0.0, "Origin should have zero r");
    assert_eq!(cyl.z, 0.0, "Origin should have zero z");
}

#[test]
fn validate_poles() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    // North pole
    let north = [0.0, 0.0, 6378.0];
    let (cspice_r, cspice_colat, _) = cspice_recsph(&north);
    let sph = Rectangular(north).to_spherical();
    assert_close(sph.radius, cspice_r, COORD_TOLERANCE, "north pole radius");
    assert_close(
        sph.colatitude,
        cspice_colat,
        COORD_TOLERANCE,
        "north pole colatitude",
    );
    assert_close(
        sph.colatitude,
        0.0,
        COORD_TOLERANCE,
        "north pole should be at colatitude 0",
    );

    // South pole
    let south = [0.0, 0.0, -6378.0];
    let (cspice_r, cspice_colat, _) = cspice_recsph(&south);
    let sph = Rectangular(south).to_spherical();
    assert_close(sph.radius, cspice_r, COORD_TOLERANCE, "south pole radius");
    assert_close(
        sph.colatitude,
        cspice_colat,
        COORD_TOLERANCE,
        "south pole colatitude",
    );
    assert_close(
        sph.colatitude,
        std::f64::consts::PI,
        COORD_TOLERANCE,
        "south pole should be at colatitude π",
    );
}

#[test]
fn validate_axes() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    // +X axis
    let x_point = [1000.0, 0.0, 0.0];
    let (r, lon, lat) = cspice_reclat(&x_point);
    let muad = Rectangular(x_point).to_latitudinal();
    assert_close(muad.radius, r, COORD_TOLERANCE, "+X radius");
    assert_angle_close(muad.longitude, lon, COORD_TOLERANCE, "+X longitude");
    assert_close(muad.latitude, lat, COORD_TOLERANCE, "+X latitude");
    assert_angle_close(
        muad.longitude,
        0.0,
        COORD_TOLERANCE,
        "+X should have longitude 0",
    );
    assert_close(
        muad.latitude,
        0.0,
        COORD_TOLERANCE,
        "+X should have latitude 0",
    );

    // +Y axis
    let y_point = [0.0, 1000.0, 0.0];
    let (r, lon, lat) = cspice_reclat(&y_point);
    let muad = Rectangular(y_point).to_latitudinal();
    assert_close(muad.radius, r, COORD_TOLERANCE, "+Y radius");
    assert_angle_close(muad.longitude, lon, COORD_TOLERANCE, "+Y longitude");
    assert_close(muad.latitude, lat, COORD_TOLERANCE, "+Y latitude");
    assert_angle_close(
        muad.longitude,
        std::f64::consts::FRAC_PI_2,
        COORD_TOLERANCE,
        "+Y should have longitude π/2",
    );

    // -X axis
    let neg_x = [-1000.0, 0.0, 0.0];
    let (r, lon, _) = cspice_reclat(&neg_x);
    let muad = Rectangular(neg_x).to_latitudinal();
    assert_close(muad.radius, r, COORD_TOLERANCE, "-X radius");
    assert_angle_close(muad.longitude, lon, COORD_TOLERANCE, "-X longitude");
    assert_angle_close(
        muad.longitude.abs(),
        std::f64::consts::PI,
        COORD_TOLERANCE,
        "-X should have longitude ±π",
    );
}
