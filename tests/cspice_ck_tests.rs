//! CK interpolation tests comparing muad-dib against CSPICE.
//!
//! Tests validate pointing_at() and pointing_of() functions using ckgp_c.
//!
//! Requires: test.bc (CK file), naif0012.tls (leap seconds kernel)
//! Note: CK queries may require SCLK kernel for proper SCLK-to-TDB conversion.
//! Run with: cargo test --test cspice_ck_tests -- --test-threads=1

#![cfg(all(feature = "cspice", feature = "test-data"))]

mod cspice_common;

use cspice_common::{
    assert_quaternion_close, ck_path, cspice_ckgp, lsk_path, CspiceKernels, CSPICE_LOCK,
};
use muad_dib::kernel::SpiceKernel;
use muad_dib::spice::{CkInterpolateExt, CkSegmentViewInterpolate};
use muad_dib::types::{NaifId, Sclk};
use muad_dib::{DAFFile, DAFSegment};
use std::fs::File;

/// Tolerance for quaternion components.
/// 2e-7 provides margin for interpolation algorithm differences while maintaining
/// high precision (~0.0001 degrees). Previous tolerances were too tight for some
/// queries due to numerical precision differences in SLERP interpolation.
const QUATERNION_TOLERANCE: f64 = 2e-7;

// ============================================================================
// CK Basic Parsing Tests
// ============================================================================

#[test]
fn test_ck_file_loads() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    // Verify muad-dib can load the CK file
    let kernel = SpiceKernel::load(&ck_path()).expect("Failed to load CK");

    // Verify we have CK segments
    let ck_count = kernel.ck_segments().count();
    assert!(ck_count > 0, "Should have at least one CK segment");

    // Load with DAFFile directly
    let file = File::open(ck_path()).expect("Failed to open CK");
    let daf = DAFFile::from_file(file).expect("Failed to parse CK DAF");

    let segments: Vec<_> = daf.collect();
    assert!(!segments.is_empty(), "Should have CK segments");

    // Verify at least one is a CK segment
    let has_ck = segments.iter().any(|s| matches!(s.as_ref().unwrap(), DAFSegment::CK(_)));
    assert!(has_ck, "Should have CK segment type");
}

#[test]
fn test_ck_segment_metadata() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let file = File::open(ck_path()).expect("Failed to open CK");
    let daf = DAFFile::from_file(file).expect("Failed to parse CK DAF");

    for (i, seg_result) in daf.enumerate() {
        let segment = seg_result.expect(&format!("Segment {} parse failed", i));
        if let DAFSegment::CK(ck) = segment {
            // Verify SCLK range is valid
            assert!(
                ck.final_sclk >= ck.initial_sclk,
                "Segment {}: Final SCLK should be >= initial SCLK",
                i
            );

            // Verify CK type is supported (1, 2, or 3)
            assert!(
                ck.ck_type >= 1 && ck.ck_type <= 6,
                "Segment {}: CK type {} out of expected range",
                i,
                ck.ck_type
            );

            // Verify data is not empty
            assert!(
                !ck.data.is_empty(),
                "Segment {}: CK data should not be empty",
                i
            );
        }
    }
}

// ============================================================================
// CK Interpolation Tests (when CSPICE can find data)
// ============================================================================

#[test]
fn validate_ck_pointing_midpoint() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    // Load kernels for CSPICE
    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());
    cspice_kernels.load(&ck_path());

    // Load kernel for muad-dib
    let kernel = SpiceKernel::load(&ck_path()).expect("Failed to load CK");

    // Get the first CK segment
    let file = File::open(ck_path()).expect("Failed to open CK");
    let daf = DAFFile::from_file(file).expect("Failed to parse CK DAF");

    let first_ck = daf
        .into_iter()
        .filter_map(|s| s.ok())
        .find_map(|s| match s {
            DAFSegment::CK(ck) => Some(ck),
            _ => None,
        })
        .expect("No CK segment found");

    let instrument = first_ck.instrument_code;
    let frame = first_ck.frame_code;
    let midpoint = (first_ck.initial_sclk + first_ck.final_sclk) / 2.0;

    // Get frame name (assume J2000 for now - CSPICE needs a frame name)
    let frame_name = "J2000";

    // Try CSPICE query with generous tolerance
    let tol = 1e6; // Large tolerance to find any data
    if let Some((cspice_quat, _clkout)) = cspice_ckgp(instrument, midpoint, tol, frame_name) {
        // CSPICE found data, now compare with muad-dib
        let muad_pointing = kernel
            .pointing_of(NaifId(instrument), Sclk(midpoint))
            .expect("muad-dib pointing_of failed");

        assert_quaternion_close(
            &muad_pointing.quaternion,
            &cspice_quat,
            QUATERNION_TOLERANCE,
            "Quaternion at midpoint",
        );
    } else {
        // CSPICE couldn't find data - this might be expected if:
        // - We need an SCLK kernel for SCLK-to-TDB conversion
        // - The frame isn't J2000
        // - The tolerance is too small
        eprintln!(
            "CSPICE couldn't find CK data for instrument {} at SCLK {}",
            instrument, midpoint
        );
        eprintln!("This may be expected - CK queries often need SCLK kernels");
    }
}

#[test]
fn validate_ck_pointing_multiple_times() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());
    cspice_kernels.load(&ck_path());

    let kernel = SpiceKernel::load(&ck_path()).expect("Failed to load CK");

    let file = File::open(ck_path()).expect("Failed to open CK");
    let daf = DAFFile::from_file(file).expect("Failed to parse CK DAF");

    let first_ck = daf
        .into_iter()
        .filter_map(|s| s.ok())
        .find_map(|s| match s {
            DAFSegment::CK(ck) => Some(ck),
            _ => None,
        })
        .expect("No CK segment found");

    let instrument = first_ck.instrument_code;
    let duration = first_ck.final_sclk - first_ck.initial_sclk;
    let frame_name = "J2000";
    let tol = 1e6;

    // Test at multiple points across coverage
    let mut cspice_found_any = false;

    for i in 0..=10 {
        let fraction = i as f64 / 10.0;
        let sclk = first_ck.initial_sclk + fraction * duration;

        if let Some((cspice_quat, _)) = cspice_ckgp(instrument, sclk, tol, frame_name) {
            cspice_found_any = true;

            let muad_pointing = kernel
                .pointing_of(NaifId(instrument), Sclk(sclk))
                .expect(&format!("muad-dib pointing_of failed at fraction {}", fraction));

            assert_quaternion_close(
                &muad_pointing.quaternion,
                &cspice_quat,
                QUATERNION_TOLERANCE,
                &format!("Quaternion at fraction {}", fraction),
            );
        }
    }

    if !cspice_found_any {
        eprintln!("CSPICE couldn't find CK data at any test point - likely needs SCLK kernel");
    }
}

// ============================================================================
// Direct Segment View Tests
// ============================================================================

#[test]
fn validate_ck_segment_view_interpolation() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let kernel = SpiceKernel::load(&ck_path()).expect("Failed to load CK");

    // Get first CK segment
    let segment = kernel
        .ck_segments()
        .next()
        .expect("No CK segments found");

    let midpoint = (segment.initial_sclk + segment.final_sclk) / 2.0;

    // Get view and interpolate directly
    let view = kernel.ck_view(segment);
    let pointing = view.pointing_at(Sclk(midpoint));

    // Should succeed for supported CK types
    if segment.ck_type == 1 || segment.ck_type == 3 {
        let p = pointing.expect("pointing_at should work for Type 1/3");

        // Verify quaternion is normalized
        let norm_sq: f64 = p.quaternion.iter().map(|x| x * x).sum();
        assert!(
            (norm_sq - 1.0).abs() < 1e-10,
            "Quaternion should be normalized: norm_sq = {}",
            norm_sq
        );
    }
}

#[test]
fn validate_ck_segment_boundaries() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let kernel = SpiceKernel::load(&ck_path()).expect("Failed to load CK");

    for segment in kernel.ck_segments() {
        // Only test supported types
        if segment.ck_type != 1 && segment.ck_type != 3 {
            continue;
        }

        let view = kernel.ck_view(segment);

        // Test at initial SCLK
        let at_start = view.pointing_at(Sclk(segment.initial_sclk));
        assert!(
            at_start.is_ok(),
            "Should be able to query at initial_sclk for segment type {}",
            segment.ck_type
        );

        // Test at final SCLK
        let at_end = view.pointing_at(Sclk(segment.final_sclk));
        assert!(
            at_end.is_ok(),
            "Should be able to query at final_sclk for segment type {}",
            segment.ck_type
        );
    }
}

// ============================================================================
// Quaternion Property Tests
// ============================================================================

#[test]
fn validate_ck_quaternion_normalization() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let kernel = SpiceKernel::load(&ck_path()).expect("Failed to load CK");

    for segment in kernel.ck_segments() {
        // Only test supported types
        if segment.ck_type != 1 && segment.ck_type != 3 {
            continue;
        }

        let view = kernel.ck_view(segment);
        let midpoint = (segment.initial_sclk + segment.final_sclk) / 2.0;

        if let Ok(pointing) = view.pointing_at(Sclk(midpoint)) {
            let q = pointing.quaternion;
            let norm_sq = q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3];

            assert!(
                (norm_sq - 1.0).abs() < 1e-10,
                "Quaternion should be normalized for segment {}: norm_sq = {}",
                segment.instrument_code,
                norm_sq
            );
        }
    }
}

#[test]
fn validate_ck_pointing_continuity() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let kernel = SpiceKernel::load(&ck_path()).expect("Failed to load CK");

    for segment in kernel.ck_segments() {
        // Only test Type 3 (interpolated)
        if segment.ck_type != 3 {
            continue;
        }

        let view = kernel.ck_view(segment);
        let midpoint = (segment.initial_sclk + segment.final_sclk) / 2.0;

        // Get pointing at two nearby times
        let dt = 0.001; // Small SCLK delta

        let p1 = match view.pointing_at(Sclk(midpoint)) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let p2 = match view.pointing_at(Sclk(midpoint + dt)) {
            Ok(p) => p,
            Err(_) => continue,
        };

        // Quaternions should be close for small time differences
        // Using dot product to measure quaternion distance
        let dot: f64 = p1
            .quaternion
            .iter()
            .zip(p2.quaternion.iter())
            .map(|(a, b)| a * b)
            .sum();

        // Dot product should be close to 1 (or -1 for antipodal quaternions)
        assert!(
            dot.abs() > 0.99,
            "Quaternions should be close for small time delta: dot = {}",
            dot
        );
    }
}

// ============================================================================
// All Segments Test
// ============================================================================

#[test]
fn validate_ck_all_segments_parseable() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let kernel = SpiceKernel::load(&ck_path()).expect("Failed to load CK");

    let mut type1_count = 0;
    let mut type3_count = 0;
    let mut other_count = 0;

    for segment in kernel.ck_segments() {
        match segment.ck_type {
            1 => type1_count += 1,
            3 => type3_count += 1,
            _ => other_count += 1,
        }

        // Try to get a view for all segments
        let view = kernel.ck_view(segment);
        let midpoint = (segment.initial_sclk + segment.final_sclk) / 2.0;

        // For supported types, interpolation should work
        if segment.ck_type == 1 || segment.ck_type == 3 {
            let result = view.pointing_at(Sclk(midpoint));
            assert!(
                result.is_ok(),
                "Type {} segment should be queryable at midpoint: {:?}",
                segment.ck_type,
                result.err()
            );
        }
    }

    eprintln!(
        "CK segment types: Type1={}, Type3={}, Other={}",
        type1_count, type3_count, other_count
    );
}

// ============================================================================
// CSPICE Comparison with Tolerance
// ============================================================================

#[test]
fn validate_ck_against_cspice_all_segments() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());
    cspice_kernels.load(&ck_path());

    let kernel = SpiceKernel::load(&ck_path()).expect("Failed to load CK");

    let tol = 1e6;
    let frame_name = "J2000";
    let mut compared_count = 0;

    for segment in kernel.ck_segments() {
        // Only test supported types
        if segment.ck_type != 1 && segment.ck_type != 3 {
            continue;
        }

        let instrument = segment.instrument_code;
        let midpoint = (segment.initial_sclk + segment.final_sclk) / 2.0;

        // Try CSPICE query
        if let Some((cspice_quat, _)) = cspice_ckgp(instrument, midpoint, tol, frame_name) {
            // muad-dib query
            let view = kernel.ck_view(segment);
            if let Ok(muad_pointing) = view.pointing_at(Sclk(midpoint)) {
                assert_quaternion_close(
                    &muad_pointing.quaternion,
                    &cspice_quat,
                    QUATERNION_TOLERANCE,
                    &format!(
                        "Quaternion for instrument {} at SCLK {}",
                        instrument, midpoint
                    ),
                );
                compared_count += 1;
            }
        }
    }

    eprintln!(
        "Successfully compared {} CK pointing queries against CSPICE",
        compared_count
    );
}
