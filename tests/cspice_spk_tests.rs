//! SPK interpolation tests comparing muad-dib against CSPICE.
//!
//! Tests validate state_at() and state_of() functions using spkgeo_c.
//!
//! Requires: test.bsp (SPK file), naif0012.tls (leap seconds kernel)
//! Run with: cargo test --test cspice_spk_tests -- --test-threads=1

#![cfg(feature = "cspice")]

mod cspice_common;

use cspice_common::{
    assert_close, cspice_spkgeo, lsk_path, spk_path, CspiceKernels, CSPICE_LOCK,
};
use muad_dib::kernel::SpiceKernel;
use muad_dib::spice::{SpkInterpolateExt, SpkSegmentViewInterpolate};
use muad_dib::types::{EpochTDB, NaifId};
use muad_dib::{DAFFile, DAFSegment};
use std::fs::File;

/// Tolerance for position (1 meter = 0.001 km).
const POSITION_TOLERANCE: f64 = 1e-3;

/// Tolerance for velocity (1 mm/s = 1e-6 km/s).
const VELOCITY_TOLERANCE: f64 = 1e-6;

// ============================================================================
// SPK Type 9 (Lagrange) Interpolation Tests
// ============================================================================

#[test]
fn validate_spk_type9_midpoint() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    // Load kernels for CSPICE
    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());
    cspice_kernels.load(&spk_path());

    // Load kernel for muad-dib
    let kernel = SpiceKernel::load(&spk_path()).expect("Failed to load SPK");

    // Get the first SPK segment to find coverage
    let file = File::open(spk_path()).expect("Failed to open SPK");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF");
    let first_segment = daf.into_iter().next().unwrap().unwrap();

    let spk = match first_segment {
        DAFSegment::SPK(s) => s,
        _ => panic!("Expected SPK segment"),
    };

    // Query at midpoint of coverage
    let midpoint = (spk.initial_epoch + spk.final_epoch) / 2.0;
    let target = spk.target_code;
    let center = spk.center_code;

    // CSPICE query
    let (cspice_state, _lt) = cspice_spkgeo(target, midpoint, "J2000", center);

    // muad-dib query
    let muad_state = kernel
        .state_of(NaifId(target), EpochTDB(midpoint), NaifId(center))
        .expect("muad-dib state_of failed");

    // Compare position
    for i in 0..3 {
        assert_close(
            muad_state.position[i],
            cspice_state[i],
            POSITION_TOLERANCE,
            &format!("Position[{}] at midpoint", i),
        );
    }

    // Compare velocity
    for i in 0..3 {
        assert_close(
            muad_state.velocity[i],
            cspice_state[i + 3],
            VELOCITY_TOLERANCE,
            &format!("Velocity[{}] at midpoint", i),
        );
    }
}

#[test]
fn validate_spk_type9_near_start() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());
    cspice_kernels.load(&spk_path());

    let kernel = SpiceKernel::load(&spk_path()).expect("Failed to load SPK");

    let file = File::open(spk_path()).expect("Failed to open SPK");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF");
    let first_segment = daf.into_iter().next().unwrap().unwrap();

    let spk = match first_segment {
        DAFSegment::SPK(s) => s,
        _ => panic!("Expected SPK segment"),
    };

    // Query near start (100 seconds after initial epoch)
    let epoch = spk.initial_epoch + 100.0;
    let target = spk.target_code;
    let center = spk.center_code;

    let (cspice_state, _) = cspice_spkgeo(target, epoch, "J2000", center);
    let muad_state = kernel
        .state_of(NaifId(target), EpochTDB(epoch), NaifId(center))
        .expect("muad-dib state_of failed");

    for i in 0..3 {
        assert_close(
            muad_state.position[i],
            cspice_state[i],
            POSITION_TOLERANCE,
            &format!("Position[{}] near start", i),
        );
    }
    for i in 0..3 {
        assert_close(
            muad_state.velocity[i],
            cspice_state[i + 3],
            VELOCITY_TOLERANCE,
            &format!("Velocity[{}] near start", i),
        );
    }
}

#[test]
fn validate_spk_type9_near_end() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());
    cspice_kernels.load(&spk_path());

    let kernel = SpiceKernel::load(&spk_path()).expect("Failed to load SPK");

    let file = File::open(spk_path()).expect("Failed to open SPK");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF");
    let first_segment = daf.into_iter().next().unwrap().unwrap();

    let spk = match first_segment {
        DAFSegment::SPK(s) => s,
        _ => panic!("Expected SPK segment"),
    };

    // Query near end (100 seconds before final epoch)
    let epoch = spk.final_epoch - 100.0;
    let target = spk.target_code;
    let center = spk.center_code;

    let (cspice_state, _) = cspice_spkgeo(target, epoch, "J2000", center);
    let muad_state = kernel
        .state_of(NaifId(target), EpochTDB(epoch), NaifId(center))
        .expect("muad-dib state_of failed");

    for i in 0..3 {
        assert_close(
            muad_state.position[i],
            cspice_state[i],
            POSITION_TOLERANCE,
            &format!("Position[{}] near end", i),
        );
    }
    for i in 0..3 {
        assert_close(
            muad_state.velocity[i],
            cspice_state[i + 3],
            VELOCITY_TOLERANCE,
            &format!("Velocity[{}] near end", i),
        );
    }
}

#[test]
fn validate_spk_at_segment_boundaries() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());
    cspice_kernels.load(&spk_path());

    let kernel = SpiceKernel::load(&spk_path()).expect("Failed to load SPK");

    let file = File::open(spk_path()).expect("Failed to open SPK");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF");
    let first_segment = daf.into_iter().next().unwrap().unwrap();

    let spk = match first_segment {
        DAFSegment::SPK(s) => s,
        _ => panic!("Expected SPK segment"),
    };

    let target = spk.target_code;
    let center = spk.center_code;

    // Test exactly at initial epoch
    {
        let epoch = spk.initial_epoch;
        let (cspice_state, _) = cspice_spkgeo(target, epoch, "J2000", center);
        let muad_state = kernel
            .state_of(NaifId(target), EpochTDB(epoch), NaifId(center))
            .expect("muad-dib state_of failed at initial epoch");

        for i in 0..3 {
            assert_close(
                muad_state.position[i],
                cspice_state[i],
                POSITION_TOLERANCE,
                &format!("Position[{}] at initial epoch", i),
            );
        }
    }

    // Test exactly at final epoch
    {
        let epoch = spk.final_epoch;
        let (cspice_state, _) = cspice_spkgeo(target, epoch, "J2000", center);
        let muad_state = kernel
            .state_of(NaifId(target), EpochTDB(epoch), NaifId(center))
            .expect("muad-dib state_of failed at final epoch");

        for i in 0..3 {
            assert_close(
                muad_state.position[i],
                cspice_state[i],
                POSITION_TOLERANCE,
                &format!("Position[{}] at final epoch", i),
            );
        }
    }
}

#[test]
fn validate_spk_multiple_epochs() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());
    cspice_kernels.load(&spk_path());

    let kernel = SpiceKernel::load(&spk_path()).expect("Failed to load SPK");

    let file = File::open(spk_path()).expect("Failed to open SPK");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF");
    let first_segment = daf.into_iter().next().unwrap().unwrap();

    let spk = match first_segment {
        DAFSegment::SPK(s) => s,
        _ => panic!("Expected SPK segment"),
    };

    let target = spk.target_code;
    let center = spk.center_code;
    let duration = spk.final_epoch - spk.initial_epoch;

    // Test at 10 points across the coverage
    for i in 0..=10 {
        let fraction = i as f64 / 10.0;
        let epoch = spk.initial_epoch + fraction * duration;

        let (cspice_state, _) = cspice_spkgeo(target, epoch, "J2000", center);
        let muad_state = kernel
            .state_of(NaifId(target), EpochTDB(epoch), NaifId(center))
            .expect(&format!("muad-dib state_of failed at fraction {}", fraction));

        for j in 0..3 {
            assert_close(
                muad_state.position[j],
                cspice_state[j],
                POSITION_TOLERANCE,
                &format!("Position[{}] at fraction {}", j, fraction),
            );
        }
        for j in 0..3 {
            assert_close(
                muad_state.velocity[j],
                cspice_state[j + 3],
                VELOCITY_TOLERANCE,
                &format!("Velocity[{}] at fraction {}", j, fraction),
            );
        }
    }
}

// ============================================================================
// Multiple Segment Tests
// ============================================================================

#[test]
fn validate_spk_all_segments() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());
    cspice_kernels.load(&spk_path());

    let kernel = SpiceKernel::load(&spk_path()).expect("Failed to load SPK");

    let file = File::open(spk_path()).expect("Failed to open SPK");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF");

    for (seg_idx, seg_result) in daf.enumerate() {
        let segment = seg_result.expect("Segment parse failed");
        let spk = match segment {
            DAFSegment::SPK(s) => s,
            _ => continue,
        };

        let target = spk.target_code;
        let center = spk.center_code;
        let midpoint = (spk.initial_epoch + spk.final_epoch) / 2.0;

        // Query at segment midpoint
        let (cspice_state, _) = cspice_spkgeo(target, midpoint, "J2000", center);
        let muad_state = kernel
            .state_of(NaifId(target), EpochTDB(midpoint), NaifId(center))
            .expect(&format!(
                "muad-dib state_of failed for segment {} (target={})",
                seg_idx, target
            ));

        for i in 0..3 {
            assert_close(
                muad_state.position[i],
                cspice_state[i],
                POSITION_TOLERANCE,
                &format!("Segment {} position[{}]", seg_idx, i),
            );
        }
        for i in 0..3 {
            assert_close(
                muad_state.velocity[i],
                cspice_state[i + 3],
                VELOCITY_TOLERANCE,
                &format!("Segment {} velocity[{}]", seg_idx, i),
            );
        }
    }
}

// ============================================================================
// Segment View Direct Interpolation Tests
// ============================================================================

#[test]
fn validate_spk_segment_view_interpolation() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());
    cspice_kernels.load(&spk_path());

    let kernel = SpiceKernel::load(&spk_path()).expect("Failed to load SPK");

    // Get a segment view and test direct interpolation
    let segment = kernel
        .spk_segments()
        .next()
        .expect("No SPK segments found");

    let target = segment.target_code;
    let center = segment.center_code;
    let midpoint = (segment.initial_epoch + segment.final_epoch) / 2.0;

    // Get view and interpolate directly
    let view = kernel.spk_view(segment);
    let muad_state = view.state_at(EpochTDB(midpoint)).expect("state_at failed");

    // Compare with CSPICE
    let (cspice_state, _) = cspice_spkgeo(target, midpoint, "J2000", center);

    for i in 0..3 {
        assert_close(
            muad_state.position[i],
            cspice_state[i],
            POSITION_TOLERANCE,
            &format!("Direct view position[{}]", i),
        );
    }
    for i in 0..3 {
        assert_close(
            muad_state.velocity[i],
            cspice_state[i + 3],
            VELOCITY_TOLERANCE,
            &format!("Direct view velocity[{}]", i),
        );
    }
}

// ============================================================================
// State Vector Property Tests
// ============================================================================

#[test]
fn validate_spk_state_continuity() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());
    cspice_kernels.load(&spk_path());

    let kernel = SpiceKernel::load(&spk_path()).expect("Failed to load SPK");

    let file = File::open(spk_path()).expect("Failed to open SPK");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF");
    let first_segment = daf.into_iter().next().unwrap().unwrap();

    let spk = match first_segment {
        DAFSegment::SPK(s) => s,
        _ => panic!("Expected SPK segment"),
    };

    let target = spk.target_code;
    let center = spk.center_code;
    let midpoint = (spk.initial_epoch + spk.final_epoch) / 2.0;

    // Test continuity: states at nearby epochs should be close
    let dt = 1.0; // 1 second
    let state1 = kernel
        .state_of(NaifId(target), EpochTDB(midpoint), NaifId(center))
        .unwrap();
    let state2 = kernel
        .state_of(NaifId(target), EpochTDB(midpoint + dt), NaifId(center))
        .unwrap();

    // Position change should be approximately velocity * dt
    for i in 0..3 {
        let expected_pos_change = state1.velocity[i] * dt;
        let actual_pos_change = state2.position[i] - state1.position[i];

        // Should be within 1% for 1 second interval (accounting for acceleration)
        let tolerance = expected_pos_change.abs() * 0.01 + 1e-6;
        assert!(
            (actual_pos_change - expected_pos_change).abs() < tolerance,
            "Position change inconsistent with velocity at index {}: expected ~{}, got {}",
            i,
            expected_pos_change,
            actual_pos_change
        );
    }
}

#[test]
fn validate_spk_position_magnitude() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());
    cspice_kernels.load(&spk_path());

    let kernel = SpiceKernel::load(&spk_path()).expect("Failed to load SPK");

    let file = File::open(spk_path()).expect("Failed to open SPK");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF");
    let first_segment = daf.into_iter().next().unwrap().unwrap();

    let spk = match first_segment {
        DAFSegment::SPK(s) => s,
        _ => panic!("Expected SPK segment"),
    };

    let target = spk.target_code;
    let center = spk.center_code;
    let midpoint = (spk.initial_epoch + spk.final_epoch) / 2.0;

    let muad_state = kernel
        .state_of(NaifId(target), EpochTDB(midpoint), NaifId(center))
        .unwrap();
    let (cspice_state, _) = cspice_spkgeo(target, midpoint, "J2000", center);

    // Compare magnitudes
    let muad_dist = muad_state.distance();
    let cspice_dist = (cspice_state[0].powi(2) + cspice_state[1].powi(2) + cspice_state[2].powi(2)).sqrt();

    assert_close(
        muad_dist,
        cspice_dist,
        POSITION_TOLERANCE,
        "Position magnitude",
    );

    let muad_speed = muad_state.speed();
    let cspice_speed = (cspice_state[3].powi(2) + cspice_state[4].powi(2) + cspice_state[5].powi(2)).sqrt();

    assert_close(
        muad_speed,
        cspice_speed,
        VELOCITY_TOLERANCE,
        "Velocity magnitude",
    );
}
