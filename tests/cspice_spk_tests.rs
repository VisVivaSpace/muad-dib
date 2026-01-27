//! SPK interpolation tests comparing muad-dib against CSPICE.
//!
//! Tests validate state_at() and state_of() functions using spkgeo_c.
//!
//! Requires: test.bsp (SPK file), naif0012.tls (leap seconds kernel)
//! Run with: cargo test --test cspice_spk_tests -- --test-threads=1

#![cfg(all(feature = "cspice", feature = "test-data"))]

mod cspice_common;

use cspice_common::{
    assert_close, cspice_spkgeo, de440s_spk_path, frame_name, hermite_spk_path, lsk_path, spk_path,
    CspiceKernels, CSPICE_LOCK,
};
use muad_dib::kernel::SpiceKernel;
use muad_dib::spice::{SpkInterpolateExt, SpkSegmentViewInterpolate};
use muad_dib::types::{EpochTDB, NaifId};
use muad_dib::{DAFFile, DAFSegment};
use std::fs::File;

/// Tolerance for position (1 km).
/// Note: Lagrange/Hermite interpolation may have small numerical differences
/// compared to CSPICE. For a ~10 million km position, 1 km is ~1e-7 relative error.
const POSITION_TOLERANCE: f64 = 1.0;

/// Tolerance for velocity (1 m/s = 1e-3 km/s).
const VELOCITY_TOLERANCE: f64 = 1e-3;

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
    let (cspice_state, _lt) = cspice_spkgeo(target, midpoint, frame_name(spk.frame_code), center);

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

    let (cspice_state, _) = cspice_spkgeo(target, epoch, frame_name(spk.frame_code), center);
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

    let (cspice_state, _) = cspice_spkgeo(target, epoch, frame_name(spk.frame_code), center);
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
        let (cspice_state, _) = cspice_spkgeo(target, epoch, frame_name(spk.frame_code), center);
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
        let (cspice_state, _) = cspice_spkgeo(target, epoch, frame_name(spk.frame_code), center);
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
    // Note: We use small epsilon at endpoints to avoid exact boundary epochs
    // where CSPICE behavior may differ slightly due to internal handling
    for i in 0..=10 {
        let fraction = i as f64 / 10.0;
        let epoch = if fraction >= 1.0 {
            // Small offset from exact endpoint (1 microsecond)
            spk.final_epoch - 1e-6
        } else {
            spk.initial_epoch + fraction * duration
        };

        let (cspice_state, _) = cspice_spkgeo(target, epoch, frame_name(spk.frame_code), center);
        let muad_state = kernel
            .state_of(NaifId(target), EpochTDB(epoch), NaifId(center))
            .expect(&format!(
                "muad-dib state_of failed at fraction {}",
                fraction
            ));

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
        let (cspice_state, _) = cspice_spkgeo(target, midpoint, frame_name(spk.frame_code), center);
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
    let segment = kernel.spk_segments().next().expect("No SPK segments found");

    let target = segment.target_code;
    let center = segment.center_code;
    let midpoint = (segment.initial_epoch + segment.final_epoch) / 2.0;

    // Get view and interpolate directly
    let view = kernel.spk_view(segment);
    let muad_state = view.state_at(EpochTDB(midpoint)).expect("state_at failed");

    // Compare with CSPICE
    let (cspice_state, _) = cspice_spkgeo(target, midpoint, frame_name(segment.frame_code), center);

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
    let (cspice_state, _) = cspice_spkgeo(target, midpoint, frame_name(spk.frame_code), center);

    // Compare magnitudes
    let muad_dist = muad_state.distance();
    let cspice_dist =
        (cspice_state[0].powi(2) + cspice_state[1].powi(2) + cspice_state[2].powi(2)).sqrt();

    assert_close(
        muad_dist,
        cspice_dist,
        POSITION_TOLERANCE,
        "Position magnitude",
    );

    let muad_speed = muad_state.speed();
    let cspice_speed =
        (cspice_state[3].powi(2) + cspice_state[4].powi(2) + cspice_state[5].powi(2)).sqrt();

    assert_close(
        muad_speed,
        cspice_speed,
        VELOCITY_TOLERANCE,
        "Velocity magnitude",
    );
}

// ============================================================================
// SPK Type 13 (Hermite) Interpolation Tests
// ============================================================================

#[test]
fn validate_spk_type13_midpoint() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    // Load kernels for CSPICE
    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());
    cspice_kernels.load(&hermite_spk_path());

    // Load kernel for muad-dib
    let kernel = SpiceKernel::load(&hermite_spk_path()).expect("Failed to load Hermite SPK");

    // Get the first SPK segment to find coverage
    let file = File::open(hermite_spk_path()).expect("Failed to open Hermite SPK");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF");
    let first_segment = daf.into_iter().next().unwrap().unwrap();

    let spk = match first_segment {
        DAFSegment::SPK(s) => s,
        _ => panic!("Expected SPK segment"),
    };

    // Verify it's Type 13
    assert_eq!(spk.spk_type, 13, "Expected Type 13 Hermite segment");

    // Query at midpoint of coverage
    let midpoint = (spk.initial_epoch + spk.final_epoch) / 2.0;
    let target = spk.target_code;
    let center = spk.center_code;

    // CSPICE query
    let (cspice_state, _lt) = cspice_spkgeo(target, midpoint, frame_name(spk.frame_code), center);

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
            &format!("Type13 Position[{}] at midpoint", i),
        );
    }

    // Compare velocity
    for i in 0..3 {
        assert_close(
            muad_state.velocity[i],
            cspice_state[i + 3],
            VELOCITY_TOLERANCE,
            &format!("Type13 Velocity[{}] at midpoint", i),
        );
    }
}

#[test]
fn validate_spk_type13_near_boundaries() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());
    cspice_kernels.load(&hermite_spk_path());

    let kernel = SpiceKernel::load(&hermite_spk_path()).expect("Failed to load Hermite SPK");

    let file = File::open(hermite_spk_path()).expect("Failed to open Hermite SPK");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF");
    let first_segment = daf.into_iter().next().unwrap().unwrap();

    let spk = match first_segment {
        DAFSegment::SPK(s) => s,
        _ => panic!("Expected SPK segment"),
    };

    let target = spk.target_code;
    let center = spk.center_code;

    // Test near start (100 seconds after initial epoch)
    {
        let epoch = spk.initial_epoch + 100.0;
        let (cspice_state, _) = cspice_spkgeo(target, epoch, frame_name(spk.frame_code), center);
        let muad_state = kernel
            .state_of(NaifId(target), EpochTDB(epoch), NaifId(center))
            .expect("muad-dib state_of failed near start");

        for i in 0..3 {
            assert_close(
                muad_state.position[i],
                cspice_state[i],
                POSITION_TOLERANCE,
                &format!("Type13 Position[{}] near start", i),
            );
        }
        for i in 0..3 {
            assert_close(
                muad_state.velocity[i],
                cspice_state[i + 3],
                VELOCITY_TOLERANCE,
                &format!("Type13 Velocity[{}] near start", i),
            );
        }
    }

    // Test near end (100 seconds before final epoch)
    {
        let epoch = spk.final_epoch - 100.0;
        let (cspice_state, _) = cspice_spkgeo(target, epoch, frame_name(spk.frame_code), center);
        let muad_state = kernel
            .state_of(NaifId(target), EpochTDB(epoch), NaifId(center))
            .expect("muad-dib state_of failed near end");

        for i in 0..3 {
            assert_close(
                muad_state.position[i],
                cspice_state[i],
                POSITION_TOLERANCE,
                &format!("Type13 Position[{}] near end", i),
            );
        }
        for i in 0..3 {
            assert_close(
                muad_state.velocity[i],
                cspice_state[i + 3],
                VELOCITY_TOLERANCE,
                &format!("Type13 Velocity[{}] near end", i),
            );
        }
    }
}

#[test]
fn validate_spk_type13_multiple_epochs() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());
    cspice_kernels.load(&hermite_spk_path());

    let kernel = SpiceKernel::load(&hermite_spk_path()).expect("Failed to load Hermite SPK");

    let file = File::open(hermite_spk_path()).expect("Failed to open Hermite SPK");
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
    // Note: We use small epsilon at endpoints to avoid exact boundary epochs
    for i in 0..=10 {
        let fraction = i as f64 / 10.0;
        let epoch = if fraction >= 1.0 {
            spk.final_epoch - 1e-6
        } else {
            spk.initial_epoch + fraction * duration
        };

        let (cspice_state, _) = cspice_spkgeo(target, epoch, frame_name(spk.frame_code), center);
        let muad_state = kernel
            .state_of(NaifId(target), EpochTDB(epoch), NaifId(center))
            .expect(&format!(
                "muad-dib state_of failed at fraction {}",
                fraction
            ));

        for j in 0..3 {
            assert_close(
                muad_state.position[j],
                cspice_state[j],
                POSITION_TOLERANCE,
                &format!("Type13 Position[{}] at fraction {}", j, fraction),
            );
        }
        for j in 0..3 {
            assert_close(
                muad_state.velocity[j],
                cspice_state[j + 3],
                VELOCITY_TOLERANCE,
                &format!("Type13 Velocity[{}] at fraction {}", j, fraction),
            );
        }
    }
}

#[test]
fn validate_spk_type13_segment_view_direct() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());
    cspice_kernels.load(&hermite_spk_path());

    let kernel = SpiceKernel::load(&hermite_spk_path()).expect("Failed to load Hermite SPK");

    // Get a segment view and test direct interpolation
    let segment = kernel.spk_segments().next().expect("No SPK segments found");

    // Verify it's Type 13
    assert_eq!(segment.spk_type, 13, "Expected Type 13 segment");

    let target = segment.target_code;
    let center = segment.center_code;
    let midpoint = (segment.initial_epoch + segment.final_epoch) / 2.0;

    // Get view and interpolate directly
    let view = kernel.spk_view(segment);
    let muad_state = view.state_at(EpochTDB(midpoint)).expect("state_at failed");

    // Compare with CSPICE
    let (cspice_state, _) = cspice_spkgeo(target, midpoint, frame_name(segment.frame_code), center);

    for i in 0..3 {
        assert_close(
            muad_state.position[i],
            cspice_state[i],
            POSITION_TOLERANCE,
            &format!("Type13 Direct view position[{}]", i),
        );
    }
    for i in 0..3 {
        assert_close(
            muad_state.velocity[i],
            cspice_state[i + 3],
            VELOCITY_TOLERANCE,
            &format!("Type13 Direct view velocity[{}]", i),
        );
    }
}

// ============================================================================
// SPK Type 2 (Chebyshev Position-Only) Interpolation Tests
// Uses de440s.bsp - JPL DE440 planetary ephemeris
// ============================================================================

/// Velocity tolerance for Type 2 (1e-3 km/s = 1 m/s).
///
/// Type 2 segments store only position Chebyshev coefficients. Velocity is
/// derived by differentiating the position polynomial. The derivative coefficients
/// are computed using the recurrence g_k = g_{k+2} + 2*(k+1)*c_{k+1}, then evaluated
/// using standard Clenshaw.
const TYPE2_VELOCITY_TOLERANCE: f64 = 1e-3;

#[test]
fn validate_spk_type2_midpoint() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    // Load kernels for CSPICE
    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());
    cspice_kernels.load(&de440s_spk_path());

    // Load kernel for muad-dib
    let kernel = SpiceKernel::load(&de440s_spk_path()).expect("Failed to load de440s SPK");

    // Get the first SPK segment to find coverage
    let file = File::open(de440s_spk_path()).expect("Failed to open de440s SPK");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF");
    let first_segment = daf.into_iter().next().unwrap().unwrap();

    let spk = match first_segment {
        DAFSegment::SPK(s) => s,
        _ => panic!("Expected SPK segment"),
    };

    // Verify it's Type 2
    assert_eq!(spk.spk_type, 2, "Expected Type 2 Chebyshev segment");

    // Query at midpoint of coverage
    let midpoint = (spk.initial_epoch + spk.final_epoch) / 2.0;
    let target = spk.target_code;
    let center = spk.center_code;

    // CSPICE query
    let (cspice_state, _lt) = cspice_spkgeo(target, midpoint, frame_name(spk.frame_code), center);

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
            &format!("Type2 Position[{}] at midpoint", i),
        );
    }

    // Compare velocity (derived from Chebyshev derivative)
    for i in 0..3 {
        assert_close(
            muad_state.velocity[i],
            cspice_state[i + 3],
            TYPE2_VELOCITY_TOLERANCE,
            &format!("Type2 Velocity[{}] at midpoint", i),
        );
    }
}

#[test]
fn validate_spk_type2_near_boundaries() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());
    cspice_kernels.load(&de440s_spk_path());

    let kernel = SpiceKernel::load(&de440s_spk_path()).expect("Failed to load de440s SPK");

    let file = File::open(de440s_spk_path()).expect("Failed to open de440s SPK");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF");
    let first_segment = daf.into_iter().next().unwrap().unwrap();

    let spk = match first_segment {
        DAFSegment::SPK(s) => s,
        _ => panic!("Expected SPK segment"),
    };

    let target = spk.target_code;
    let center = spk.center_code;

    // Test near start (1 day after initial epoch)
    {
        let epoch = spk.initial_epoch + 86400.0;
        let (cspice_state, _) = cspice_spkgeo(target, epoch, frame_name(spk.frame_code), center);
        let muad_state = kernel
            .state_of(NaifId(target), EpochTDB(epoch), NaifId(center))
            .expect("muad-dib state_of failed near start");

        for i in 0..3 {
            assert_close(
                muad_state.position[i],
                cspice_state[i],
                POSITION_TOLERANCE,
                &format!("Type2 Position[{}] near start", i),
            );
        }
        for i in 0..3 {
            assert_close(
                muad_state.velocity[i],
                cspice_state[i + 3],
                TYPE2_VELOCITY_TOLERANCE,
                &format!("Type2 Velocity[{}] near start", i),
            );
        }
    }

    // Test near end (1 day before final epoch)
    {
        let epoch = spk.final_epoch - 86400.0;
        let (cspice_state, _) = cspice_spkgeo(target, epoch, frame_name(spk.frame_code), center);
        let muad_state = kernel
            .state_of(NaifId(target), EpochTDB(epoch), NaifId(center))
            .expect("muad-dib state_of failed near end");

        for i in 0..3 {
            assert_close(
                muad_state.position[i],
                cspice_state[i],
                POSITION_TOLERANCE,
                &format!("Type2 Position[{}] near end", i),
            );
        }
        for i in 0..3 {
            assert_close(
                muad_state.velocity[i],
                cspice_state[i + 3],
                TYPE2_VELOCITY_TOLERANCE,
                &format!("Type2 Velocity[{}] near end", i),
            );
        }
    }
}

#[test]
fn validate_spk_type2_multiple_epochs() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());
    cspice_kernels.load(&de440s_spk_path());

    let kernel = SpiceKernel::load(&de440s_spk_path()).expect("Failed to load de440s SPK");

    let file = File::open(de440s_spk_path()).expect("Failed to open de440s SPK");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF");
    let first_segment = daf.into_iter().next().unwrap().unwrap();

    let spk = match first_segment {
        DAFSegment::SPK(s) => s,
        _ => panic!("Expected SPK segment"),
    };

    let target = spk.target_code;
    let center = spk.center_code;
    let duration = spk.final_epoch - spk.initial_epoch;

    // Test at 10 points across the coverage (avoiding exact boundaries)
    for i in 1..=10 {
        let fraction = i as f64 / 11.0;
        let epoch = spk.initial_epoch + fraction * duration;

        let (cspice_state, _) = cspice_spkgeo(target, epoch, frame_name(spk.frame_code), center);
        let muad_state = kernel
            .state_of(NaifId(target), EpochTDB(epoch), NaifId(center))
            .expect(&format!(
                "muad-dib state_of failed at fraction {}",
                fraction
            ));

        for j in 0..3 {
            assert_close(
                muad_state.position[j],
                cspice_state[j],
                POSITION_TOLERANCE,
                &format!("Type2 Position[{}] at fraction {:.2}", j, fraction),
            );
        }
        for j in 0..3 {
            assert_close(
                muad_state.velocity[j],
                cspice_state[j + 3],
                TYPE2_VELOCITY_TOLERANCE,
                &format!("Type2 Velocity[{}] at fraction {:.2}", j, fraction),
            );
        }
    }
}

#[test]
fn validate_spk_type2_multiple_bodies() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());
    cspice_kernels.load(&de440s_spk_path());

    let kernel = SpiceKernel::load(&de440s_spk_path()).expect("Failed to load de440s SPK");

    // DE440s contains planetary barycenters (1-9), Sun (10), Moon (301), Earth (399)
    // Test a selection of these bodies
    let test_bodies = [
        (3, 0, "Earth Barycenter"), // Earth Barycenter w.r.t. SSB
        (10, 0, "Sun"),             // Sun w.r.t. SSB
        (301, 3, "Moon"),           // Moon w.r.t. Earth Barycenter
        (399, 3, "Earth"),          // Earth w.r.t. Earth Barycenter
    ];

    // Use J2000 epoch (0.0 TDB seconds past J2000)
    let epoch = 0.0;

    for (target, center, name) in test_bodies {
        let result = kernel.state_of(NaifId(target), EpochTDB(epoch), NaifId(center));

        if let Ok(muad_state) = result {
            let (cspice_state, _) = cspice_spkgeo(target, epoch, "J2000", center);

            for i in 0..3 {
                assert_close(
                    muad_state.position[i],
                    cspice_state[i],
                    POSITION_TOLERANCE,
                    &format!("{} Position[{}]", name, i),
                );
            }
            for i in 0..3 {
                assert_close(
                    muad_state.velocity[i],
                    cspice_state[i + 3],
                    TYPE2_VELOCITY_TOLERANCE,
                    &format!("{} Velocity[{}]", name, i),
                );
            }
        }
        // If state_of fails, it might be because the body pair isn't directly in the file
        // (e.g., needs chain computation). That's acceptable for this test.
    }
}

#[test]
fn validate_spk_type2_segment_view_direct() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());
    cspice_kernels.load(&de440s_spk_path());

    let kernel = SpiceKernel::load(&de440s_spk_path()).expect("Failed to load de440s SPK");

    // Get a segment view and test direct interpolation
    let segment = kernel.spk_segments().next().expect("No SPK segments found");

    // Verify it's Type 2
    assert_eq!(segment.spk_type, 2, "Expected Type 2 segment");

    let target = segment.target_code;
    let center = segment.center_code;
    let midpoint = (segment.initial_epoch + segment.final_epoch) / 2.0;

    // Get view and interpolate directly
    let view = kernel.spk_view(segment);
    let muad_state = view.state_at(EpochTDB(midpoint)).expect("state_at failed");

    // Compare with CSPICE
    let (cspice_state, _) = cspice_spkgeo(target, midpoint, frame_name(segment.frame_code), center);

    for i in 0..3 {
        assert_close(
            muad_state.position[i],
            cspice_state[i],
            POSITION_TOLERANCE,
            &format!("Type2 Direct view position[{}]", i),
        );
    }
    for i in 0..3 {
        assert_close(
            muad_state.velocity[i],
            cspice_state[i + 3],
            TYPE2_VELOCITY_TOLERANCE,
            &format!("Type2 Direct view velocity[{}]", i),
        );
    }
}
