//! Kernel pool access tests comparing muad-dib against CSPICE.
//!
//! Tests validate get_f64(), get_i32(), pool_has(), and pool_count() functions.
//!
//! Requires: test.tpc (text PCK file)
//! Run with: cargo test --test cspice_pool_tests -- --test-threads=1

#![cfg(feature = "cspice")]

mod cspice_common;

use cspice_common::{
    assert_close, cspice_dtpool, cspice_gdpool, cspice_gipool, tpc_path, CspiceKernels,
    CSPICE_LOCK,
};
use muad_dib::kernel::SpiceKernel;
use muad_dib::spice::KernelPoolExt;

/// Tolerance for exact match (kernel pool values should match exactly).
const POOL_TOLERANCE: f64 = 1e-15;

// ============================================================================
// Basic Pool Access Tests
// ============================================================================

#[test]
fn validate_gdpool_body399_radii() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    // Load TPC for CSPICE
    let mut kernels = CspiceKernels::new();
    kernels.load(&tpc_path());

    // Load TPC for muad-dib
    let kernel = SpiceKernel::load(&tpc_path()).expect("Failed to load TPC");

    // Get Earth radii
    let cspice_radii = cspice_gdpool("BODY399_RADII").expect("CSPICE failed to get BODY399_RADII");
    let muad_radii = kernel
        .get_f64("BODY399_RADII")
        .expect("muad-dib failed to get BODY399_RADII");

    // Compare
    assert_eq!(
        cspice_radii.len(),
        muad_radii.len(),
        "Radii count mismatch"
    );
    for i in 0..cspice_radii.len() {
        assert_close(
            muad_radii[i],
            cspice_radii[i],
            POOL_TOLERANCE,
            &format!("BODY399_RADII[{}]", i),
        );
    }

    // Verify specific values (Earth equatorial and polar radii)
    assert!(
        (muad_radii[0] - 6378.14).abs() < 0.01,
        "Earth equatorial radius should be ~6378 km"
    );
    assert!(
        (muad_radii[2] - 6356.75).abs() < 0.01,
        "Earth polar radius should be ~6357 km"
    );
}

#[test]
fn validate_gdpool_body399_pole_ra() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut kernels = CspiceKernels::new();
    kernels.load(&tpc_path());

    let kernel = SpiceKernel::load(&tpc_path()).expect("Failed to load TPC");

    let cspice_values = cspice_gdpool("BODY399_POLE_RA").expect("CSPICE failed");
    let muad_values = kernel.get_f64("BODY399_POLE_RA").expect("muad-dib failed");

    assert_eq!(cspice_values.len(), muad_values.len(), "Count mismatch");
    for i in 0..cspice_values.len() {
        assert_close(
            muad_values[i],
            cspice_values[i],
            POOL_TOLERANCE,
            &format!("BODY399_POLE_RA[{}]", i),
        );
    }
}

#[test]
fn validate_gdpool_body399_pm() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut kernels = CspiceKernels::new();
    kernels.load(&tpc_path());

    let kernel = SpiceKernel::load(&tpc_path()).expect("Failed to load TPC");

    let cspice_values = cspice_gdpool("BODY399_PM").expect("CSPICE failed");
    let muad_values = kernel.get_f64("BODY399_PM").expect("muad-dib failed");

    assert_eq!(cspice_values.len(), muad_values.len(), "Count mismatch");
    for i in 0..cspice_values.len() {
        assert_close(
            muad_values[i],
            cspice_values[i],
            POOL_TOLERANCE,
            &format!("BODY399_PM[{}]", i),
        );
    }
}

#[test]
fn validate_gdpool_sun_pole() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut kernels = CspiceKernels::new();
    kernels.load(&tpc_path());

    let kernel = SpiceKernel::load(&tpc_path()).expect("Failed to load TPC");

    // Sun pole RA (body 10)
    let cspice_ra = cspice_gdpool("BODY10_POLE_RA").expect("CSPICE failed");
    let muad_ra = kernel.get_f64("BODY10_POLE_RA").expect("muad-dib failed");

    assert_eq!(cspice_ra.len(), muad_ra.len(), "Count mismatch");
    for i in 0..cspice_ra.len() {
        assert_close(
            muad_ra[i],
            cspice_ra[i],
            POOL_TOLERANCE,
            &format!("BODY10_POLE_RA[{}]", i),
        );
    }
}

// ============================================================================
// Case Insensitivity Tests
// ============================================================================

#[test]
fn validate_case_insensitivity() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut kernels = CspiceKernels::new();
    kernels.load(&tpc_path());

    let kernel = SpiceKernel::load(&tpc_path()).expect("Failed to load TPC");

    // CSPICE is case-insensitive
    let cspice_upper = cspice_gdpool("BODY399_RADII").expect("Upper case failed");
    let cspice_lower = cspice_gdpool("body399_radii").expect("Lower case failed");
    let cspice_mixed = cspice_gdpool("Body399_Radii").expect("Mixed case failed");

    // muad-dib should also be case-insensitive
    let muad_upper = kernel.get_f64("BODY399_RADII").expect("Upper case failed");
    let muad_lower = kernel.get_f64("body399_radii").expect("Lower case failed");
    let muad_mixed = kernel.get_f64("Body399_Radii").expect("Mixed case failed");

    // All should be equal
    assert_eq!(cspice_upper, cspice_lower, "CSPICE case sensitivity issue");
    assert_eq!(cspice_upper, cspice_mixed, "CSPICE case sensitivity issue");
    assert_eq!(muad_upper, muad_lower, "muad-dib case sensitivity issue");
    assert_eq!(muad_upper, muad_mixed, "muad-dib case sensitivity issue");

    // CSPICE and muad-dib should match
    for i in 0..muad_upper.len() {
        assert_close(
            muad_upper[i],
            cspice_upper[i],
            POOL_TOLERANCE,
            &format!("Case insensitive value[{}]", i),
        );
    }
}

// ============================================================================
// Pool Existence Tests (dtpool equivalent)
// ============================================================================

#[test]
fn validate_dtpool_against_pool_has() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut kernels = CspiceKernels::new();
    kernels.load(&tpc_path());

    let kernel = SpiceKernel::load(&tpc_path()).expect("Failed to load TPC");

    // Variables that should exist
    let should_exist = [
        "BODY399_RADII",
        "BODY399_PM",
        "BODY399_POLE_RA",
        "BODY399_POLE_DEC",
        "BODY10_POLE_RA",
        "BODY301_RADII",
    ];

    for name in should_exist.iter() {
        let cspice_exists = cspice_dtpool(name).is_some();
        let muad_exists = kernel.pool_has(name);

        assert_eq!(
            cspice_exists, muad_exists,
            "Existence mismatch for {}: CSPICE={}, muad-dib={}",
            name, cspice_exists, muad_exists
        );
        assert!(muad_exists, "{} should exist", name);
    }

    // Variables that should NOT exist
    let should_not_exist = [
        "NONEXISTENT_VAR",
        "BODY9999_RADII",
        "RANDOM_GARBAGE_12345",
    ];

    for name in should_not_exist.iter() {
        let cspice_exists = cspice_dtpool(name).is_some();
        let muad_exists = kernel.pool_has(name);

        assert_eq!(
            cspice_exists, muad_exists,
            "Non-existence mismatch for {}: CSPICE={}, muad-dib={}",
            name, cspice_exists, muad_exists
        );
        assert!(!muad_exists, "{} should not exist", name);
    }
}

#[test]
fn validate_pool_count() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut kernels = CspiceKernels::new();
    kernels.load(&tpc_path());

    let kernel = SpiceKernel::load(&tpc_path()).expect("Failed to load TPC");

    // BODY399_RADII should have 3 values
    let cspice_info = cspice_dtpool("BODY399_RADII").expect("CSPICE dtpool failed");
    let muad_count = kernel.pool_count("BODY399_RADII").expect("muad-dib pool_count failed");

    assert_eq!(
        cspice_info.0, muad_count,
        "Count mismatch for BODY399_RADII: CSPICE={}, muad-dib={}",
        cspice_info.0, muad_count
    );
    assert_eq!(muad_count, 3, "BODY399_RADII should have 3 values");

    // BODY399_PM should have 3 values
    let cspice_info = cspice_dtpool("BODY399_PM").expect("CSPICE dtpool failed");
    let muad_count = kernel.pool_count("BODY399_PM").expect("muad-dib pool_count failed");

    assert_eq!(cspice_info.0, muad_count, "Count mismatch for BODY399_PM");
}

// ============================================================================
// Scalar Value Tests
// ============================================================================

#[test]
fn validate_get_f64_scalar() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut kernels = CspiceKernels::new();
    kernels.load(&tpc_path());

    let kernel = SpiceKernel::load(&tpc_path()).expect("Failed to load TPC");

    // Get first value of BODY399_RADII
    let cspice_values = cspice_gdpool("BODY399_RADII").expect("CSPICE failed");
    let muad_scalar = kernel.get_f64_scalar("BODY399_RADII").expect("muad-dib failed");

    assert_close(
        muad_scalar,
        cspice_values[0],
        POOL_TOLERANCE,
        "Scalar value",
    );
}

// ============================================================================
// Multiple Body Tests
// ============================================================================

#[test]
fn validate_multiple_body_radii() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut kernels = CspiceKernels::new();
    kernels.load(&tpc_path());

    let kernel = SpiceKernel::load(&tpc_path()).expect("Failed to load TPC");

    // Test radii for multiple bodies
    let bodies = [
        ("BODY10_RADII", "Sun"),           // Sun
        ("BODY199_RADII", "Mercury"),       // Mercury
        ("BODY299_RADII", "Venus"),         // Venus
        ("BODY399_RADII", "Earth"),         // Earth
        ("BODY301_RADII", "Moon"),          // Moon
        ("BODY499_RADII", "Mars"),          // Mars
    ];

    for (var_name, body_name) in bodies.iter() {
        let cspice_result = cspice_gdpool(var_name);
        let muad_result = kernel.get_f64(var_name);

        // Both should exist or both should not exist
        assert_eq!(
            cspice_result.is_some(),
            muad_result.is_some(),
            "Existence mismatch for {} ({})",
            var_name,
            body_name
        );

        if let (Some(cspice_vals), Some(muad_vals)) = (cspice_result, muad_result) {
            assert_eq!(
                cspice_vals.len(),
                muad_vals.len(),
                "Length mismatch for {} ({})",
                var_name,
                body_name
            );
            for i in 0..cspice_vals.len() {
                assert_close(
                    muad_vals[i],
                    cspice_vals[i],
                    POOL_TOLERANCE,
                    &format!("{}[{}] ({})", var_name, i, body_name),
                );
            }
        }
    }
}

#[test]
fn validate_orientation_constants() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut kernels = CspiceKernels::new();
    kernels.load(&tpc_path());

    let kernel = SpiceKernel::load(&tpc_path()).expect("Failed to load TPC");

    // Test orientation constants for Earth
    let orientation_vars = [
        "BODY399_POLE_RA",
        "BODY399_POLE_DEC",
        "BODY399_PM",
    ];

    for var_name in orientation_vars.iter() {
        let cspice_vals = cspice_gdpool(var_name).expect(&format!("CSPICE failed for {}", var_name));
        let muad_vals = kernel
            .get_f64(var_name)
            .expect(&format!("muad-dib failed for {}", var_name));

        assert_eq!(cspice_vals.len(), muad_vals.len(), "Length mismatch for {}", var_name);
        for i in 0..cspice_vals.len() {
            assert_close(
                muad_vals[i],
                cspice_vals[i],
                POOL_TOLERANCE,
                &format!("{}[{}]", var_name, i),
            );
        }
    }
}

// ============================================================================
// Integer Value Tests
// ============================================================================

#[test]
fn validate_get_i32() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut kernels = CspiceKernels::new();
    kernels.load(&tpc_path());

    let kernel = SpiceKernel::load(&tpc_path()).expect("Failed to load TPC");

    // BODY399_RADII as integers (truncated)
    let cspice_floats = cspice_gdpool("BODY399_RADII").expect("CSPICE failed");
    let muad_ints = kernel.get_i32("BODY399_RADII").expect("muad-dib failed");

    assert_eq!(cspice_floats.len(), muad_ints.len(), "Length mismatch");

    for i in 0..cspice_floats.len() {
        let expected = cspice_floats[i] as i32;
        assert_eq!(
            muad_ints[i], expected,
            "Integer value mismatch at index {}",
            i
        );
    }
}

// ============================================================================
// Nutation/Precession Angle Tests
// ============================================================================

#[test]
fn validate_nutation_precession_angles() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut kernels = CspiceKernels::new();
    kernels.load(&tpc_path());

    let kernel = SpiceKernel::load(&tpc_path()).expect("Failed to load TPC");

    // Earth-Moon system nutation precession angles
    let var_name = "BODY3_NUT_PREC_ANGLES";

    if let Some(cspice_vals) = cspice_gdpool(var_name) {
        let muad_vals = kernel.get_f64(var_name).expect("muad-dib failed");

        assert_eq!(cspice_vals.len(), muad_vals.len(), "Length mismatch");
        for i in 0..cspice_vals.len() {
            assert_close(
                muad_vals[i],
                cspice_vals[i],
                POOL_TOLERANCE,
                &format!("{}[{}]", var_name, i),
            );
        }
    }
}
