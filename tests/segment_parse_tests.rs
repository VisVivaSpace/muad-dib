//! Tests for type-specific segment data parsing with real kernel files.
//!
//! These tests verify that the parsed SpkData and CkData structures contain
//! valid, well-formed data when parsing real SPICE kernel files.

#![cfg(feature = "test-data")]

use muad_dib::kernel::{CkData, SpiceKernel};
use muad_dib::types::NaifId;

// =============================================================================
// SPK Type 2 (Chebyshev) from de440s.bsp
// =============================================================================

/// Parse de440s.bsp Type 2 data and verify Chebyshev record structure.
#[test]
fn test_parse_type2_from_de440s() {
    let kernel = SpiceKernel::load("test_data/de440s.bsp").expect("Failed to load de440s.bsp");

    // Earth (399) should have Type 2 data in de440s
    let earth = NaifId(399);
    let views: Vec<_> = kernel.spk_views_for(earth).collect();
    assert!(!views.is_empty(), "de440s should have segments for Earth");

    let view = &views[0];
    assert_eq!(view.spk_type(), 2, "de440s Earth segment should be Type 2");

    let data = view.data();
    let type2 = data
        .as_type2()
        .expect("Should parse as Type 2 Chebyshev data");

    assert!(
        !type2.records.is_empty(),
        "Type 2 data should have Chebyshev records"
    );
    assert!(type2.degree > 0, "Polynomial degree should be > 0");
    assert!(
        type2.interval_length > 0.0,
        "Interval length should be positive"
    );
}

/// Verify Chebyshev record internal consistency for de440s.bsp Type 2 data.
#[test]
fn test_type2_chebyshev_record_structure() {
    let kernel = SpiceKernel::load("test_data/de440s.bsp").expect("Failed to load de440s.bsp");

    let earth = NaifId(399);
    let views: Vec<_> = kernel.spk_views_for(earth).collect();
    let view = &views[0];
    let data = view.data();
    let type2 = data.as_type2().expect("Should be Type 2");

    let expected_coeff_count = (type2.degree + 1) as usize;

    for (i, record) in type2.records.iter().enumerate() {
        // Radius (half-interval) should be positive
        assert!(
            record.radius > 0.0,
            "Record {}: radius should be positive, got {}",
            i,
            record.radius
        );

        // Coefficient arrays should all have the same length (degree + 1)
        assert_eq!(
            record.x_coeffs.len(),
            expected_coeff_count,
            "Record {}: x_coeffs length should be {} (degree+1)",
            i,
            expected_coeff_count
        );
        assert_eq!(
            record.y_coeffs.len(),
            expected_coeff_count,
            "Record {}: y_coeffs length should match degree+1",
            i
        );
        assert_eq!(
            record.z_coeffs.len(),
            expected_coeff_count,
            "Record {}: z_coeffs length should match degree+1",
            i
        );
    }

    // Midpoints should be in increasing order
    for i in 1..type2.records.len() {
        assert!(
            type2.records[i].midpoint > type2.records[i - 1].midpoint,
            "Record midpoints should be increasing: records[{}]={} <= records[{}]={}",
            i,
            type2.records[i].midpoint,
            i - 1,
            type2.records[i - 1].midpoint,
        );
    }
}

// =============================================================================
// SPK Type 9 (Lagrange) from test.bsp
// =============================================================================

/// Parse test.bsp Type 9 data and verify Lagrange state records.
#[test]
fn test_parse_type9_from_test_bsp() {
    let kernel = SpiceKernel::load("test_data/test.bsp").expect("Failed to load test.bsp");

    // Find a Type 9 segment
    let mut found_type9 = false;
    for body in kernel.spk_bodies() {
        for view in kernel.spk_views_for(body) {
            if view.spk_type() == 9 {
                found_type9 = true;
                let data = view.data();
                let type9 = data
                    .as_type9()
                    .expect("Should parse as Type 9 Lagrange data");

                assert!(
                    !type9.states.is_empty(),
                    "Type 9 data should have state records"
                );
                assert!(type9.window_size > 0, "Window size should be > 0");

                // Epochs should be sorted
                for i in 1..type9.states.len() {
                    assert!(
                        type9.states[i].epoch >= type9.states[i - 1].epoch,
                        "Type 9 epochs should be sorted: states[{}].epoch={} < states[{}].epoch={}",
                        i,
                        type9.states[i].epoch,
                        i - 1,
                        type9.states[i - 1].epoch,
                    );
                }
                break;
            }
        }
        if found_type9 {
            break;
        }
    }

    assert!(
        found_type9,
        "test.bsp should contain at least one Type 9 segment"
    );
}

// =============================================================================
// SPK Type 13 (Hermite) from gmat-hermite.bsp
// =============================================================================

/// Parse gmat-hermite.bsp Type 13 data and verify Hermite state records.
#[test]
fn test_parse_type13_from_gmat_hermite() {
    let kernel =
        SpiceKernel::load("test_data/gmat-hermite.bsp").expect("Failed to load gmat-hermite.bsp");

    let sc = NaifId(-10000001);
    let views: Vec<_> = kernel.spk_views_for(sc).collect();
    assert!(
        !views.is_empty(),
        "gmat-hermite.bsp should have segments for spacecraft"
    );

    let view = &views[0];
    assert_eq!(
        view.spk_type(),
        13,
        "gmat-hermite should be Type 13 (Hermite)"
    );

    let data = view.data();
    let type13 = data
        .as_type13()
        .expect("Should parse as Type 13 Hermite data");

    assert!(
        !type13.states.is_empty(),
        "Type 13 data should have state records"
    );
    assert!(type13.window_size > 0, "Window size should be > 0");

    // Epochs should be sorted
    for i in 1..type13.states.len() {
        assert!(
            type13.states[i].epoch >= type13.states[i - 1].epoch,
            "Type 13 epochs should be sorted"
        );
    }

    // State records should have 6-component data (position + velocity)
    let state = &type13.states[0];
    // Verify position components are not all zero (spacecraft should have nonzero position)
    let pos_mag = (state.x * state.x + state.y * state.y + state.z * state.z).sqrt();
    assert!(
        pos_mag > 0.0,
        "Spacecraft position magnitude should be nonzero"
    );
}

// =============================================================================
// CK Parsing Tests
// =============================================================================

/// Parse test.bc CK data and verify pointing records.
#[test]
fn test_parse_ck_type_from_test_bc() {
    let kernel = SpiceKernel::load("test_data/test.bc").expect("Failed to load test.bc");

    let instruments = kernel.ck_instruments();
    assert!(!instruments.is_empty(), "test.bc should have instruments");

    for inst in &instruments {
        let views: Vec<_> = kernel.ck_views_for(*inst).collect();
        assert!(
            !views.is_empty(),
            "Should have CK views for instrument {}",
            inst
        );

        for view in &views {
            let data = view.data();

            match data {
                CkData::Type1(ck1) => {
                    assert!(
                        !ck1.records.is_empty(),
                        "CK Type 1 should have pointing records"
                    );
                    for (i, rec) in ck1.records.iter().enumerate() {
                        let q = rec.quaternion();
                        let norm = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
                        assert!(
                            (norm - 1.0).abs() < 1e-6,
                            "CK1 record {}: quaternion should be unit, norm={}",
                            i,
                            norm
                        );
                    }
                }
                CkData::Type3(ck3) => {
                    assert!(
                        !ck3.records.is_empty(),
                        "CK Type 3 should have pointing records"
                    );
                    for (i, rec) in ck3.records.iter().enumerate() {
                        let q = rec.quaternion();
                        let norm = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
                        assert!(
                            (norm - 1.0).abs() < 1e-6,
                            "CK3 record {}: quaternion should be unit, norm={}",
                            i,
                            norm
                        );
                    }
                    assert!(
                        !ck3.interval_starts.is_empty(),
                        "CK Type 3 should have interval starts"
                    );
                }
                CkData::Raw { ck_type, .. } => {
                    // Raw is acceptable for unsupported CK types
                    eprintln!(
                        "Instrument {} has unsupported CK type {}, parsed as Raw",
                        inst, ck_type
                    );
                }
            }
        }
    }
}
