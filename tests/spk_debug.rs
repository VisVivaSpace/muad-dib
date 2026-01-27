//! Diagnostic tests for SPK parsing.
//!
//! This file contains tests to debug the SPK Type 9 parsing difference.

#![cfg(all(feature = "cspice", feature = "test-data"))]

mod cspice_common;

use cspice_common::{
    cspice_spkgeo, hermite_spk_path, lsk_path, spk_path, CspiceKernels, CSPICE_LOCK,
};
use muad_dib::kernel::spk_parse::parse_spk_data;
use muad_dib::kernel::spk_types::SpkData;
use muad_dib::spice::SpkInterpolateExt;
use muad_dib::{DAFFile, DAFSegment};
use std::fs::File;

/// Convert NAIF frame code to CSPICE frame name string.
fn frame_name(frame_code: i32) -> &'static str {
    match frame_code {
        1 => "J2000",
        17 => "ECLIPJ2000",
        _ => "J2000", // fallback
    }
}

#[test]
fn debug_spk_type9_data() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    // Load kernel for CSPICE
    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());
    cspice_kernels.load(&spk_path());

    // Get the first SPK segment
    let file = File::open(spk_path()).expect("Failed to open SPK");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF");
    let first_segment = daf.into_iter().next().unwrap().unwrap();

    let spk = match first_segment {
        DAFSegment::SPK(s) => s,
        _ => panic!("Expected SPK segment"),
    };

    println!("\n=== Segment Info ===");
    println!("SPK Type: {}", spk.spk_type);
    println!(
        "Target: {}, Center: {}, Frame: {}",
        spk.target_code, spk.center_code, spk.frame_code
    );
    println!("Initial epoch: {}", spk.initial_epoch);
    println!("Final epoch: {}", spk.final_epoch);

    // Parse the raw data
    let parsed = parse_spk_data(spk.spk_type, spk.data.clone());

    match parsed {
        SpkData::Type9(type9) => {
            println!("\n=== Type 9 Data ===");
            println!("Window size: {}", type9.window_size);
            println!("Number of states: {}", type9.states.len());

            // Print first few states
            println!("\n=== First 5 States (muad-dib) ===");
            for (i, state) in type9.states.iter().take(5).enumerate() {
                println!(
                    "State {}: epoch={:.3} pos=[{:.3}, {:.3}, {:.3}] vel=[{:.6}, {:.6}, {:.6}]",
                    i, state.epoch, state.x, state.y, state.z, state.vx, state.vy, state.vz
                );
            }

            // Query CSPICE at first state's epoch
            let first_epoch = type9.states[0].epoch;

            // Query in J2000 frame (frame 1)
            println!("\n=== CSPICE at first epoch ({}) in J2000 ===", first_epoch);
            let (cspice_state_j2000, _lt) =
                cspice_spkgeo(spk.target_code, first_epoch, "J2000", spk.center_code);
            println!(
                "CSPICE J2000: pos=[{:.3}, {:.3}, {:.3}] vel=[{:.6}, {:.6}, {:.6}]",
                cspice_state_j2000[0],
                cspice_state_j2000[1],
                cspice_state_j2000[2],
                cspice_state_j2000[3],
                cspice_state_j2000[4],
                cspice_state_j2000[5]
            );

            // Query in ECLIPJ2000 frame (frame 17, which matches the segment)
            println!(
                "\n=== CSPICE at first epoch ({}) in ECLIPJ2000 ===",
                first_epoch
            );
            let (cspice_state_eclip, _lt) =
                cspice_spkgeo(spk.target_code, first_epoch, "ECLIPJ2000", spk.center_code);
            println!(
                "CSPICE ECLIPJ2000: pos=[{:.3}, {:.3}, {:.3}] vel=[{:.6}, {:.6}, {:.6}]",
                cspice_state_eclip[0],
                cspice_state_eclip[1],
                cspice_state_eclip[2],
                cspice_state_eclip[3],
                cspice_state_eclip[4],
                cspice_state_eclip[5]
            );

            // Compare differences in ECLIPJ2000 (should be near zero)
            println!("\n=== Differences at first epoch (in ECLIPJ2000) ===");
            let our_state = &type9.states[0];
            println!(
                "X diff: {:.10} km",
                (our_state.x - cspice_state_eclip[0]).abs()
            );
            println!(
                "Y diff: {:.10} km",
                (our_state.y - cspice_state_eclip[1]).abs()
            );
            println!(
                "Z diff: {:.10} km",
                (our_state.z - cspice_state_eclip[2]).abs()
            );
            println!(
                "VX diff: {:.15} km/s",
                (our_state.vx - cspice_state_eclip[3]).abs()
            );
            println!(
                "VY diff: {:.15} km/s",
                (our_state.vy - cspice_state_eclip[4]).abs()
            );
            println!(
                "VZ diff: {:.15} km/s",
                (our_state.vz - cspice_state_eclip[5]).abs()
            );
        }
        SpkData::Type13(type13) => {
            println!("\n=== Type 13 Data ===");
            println!("Window size: {}", type13.window_size);
            println!("Number of states: {}", type13.states.len());
        }
        other => {
            println!("\n=== Other type: {} ===", other.spk_type());
        }
    }
}

#[test]
fn debug_spk_type13_hermite_data() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    // Load kernel for CSPICE
    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());
    cspice_kernels.load(&hermite_spk_path());

    // Get the first SPK segment
    let file = File::open(hermite_spk_path()).expect("Failed to open Hermite SPK");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF");
    let first_segment = daf.into_iter().next().unwrap().unwrap();

    let spk = match first_segment {
        DAFSegment::SPK(s) => s,
        _ => panic!("Expected SPK segment"),
    };

    println!("\n=== Hermite Segment Info ===");
    println!("SPK Type: {}", spk.spk_type);
    println!(
        "Target: {}, Center: {}, Frame: {} ({})",
        spk.target_code,
        spk.center_code,
        spk.frame_code,
        frame_name(spk.frame_code)
    );
    println!("Initial epoch: {}", spk.initial_epoch);
    println!("Final epoch: {}", spk.final_epoch);

    // Parse the raw data
    let parsed = parse_spk_data(spk.spk_type, spk.data.clone());

    match parsed {
        SpkData::Type13(type13) => {
            println!("\n=== Type 13 Data ===");
            println!("Window size: {}", type13.window_size);
            println!("Number of states: {}", type13.states.len());

            // Print first state
            let first_state = &type13.states[0];
            println!("\n=== First State (muad-dib) ===");
            println!(
                "epoch={:.3} pos=[{:.3}, {:.3}, {:.3}]",
                first_state.epoch, first_state.x, first_state.y, first_state.z
            );

            // Query CSPICE at first state's epoch in the segment's frame
            let first_epoch = first_state.epoch;
            let frame = frame_name(spk.frame_code);
            println!("\n=== CSPICE at first epoch in {} ===", frame);
            let (cspice_state, _lt) =
                cspice_spkgeo(spk.target_code, first_epoch, frame, spk.center_code);
            println!(
                "CSPICE {}: pos=[{:.3}, {:.3}, {:.3}]",
                frame, cspice_state[0], cspice_state[1], cspice_state[2]
            );

            // Compare differences
            println!("\n=== Differences at first epoch ===");
            println!("X diff: {:.10} km", (first_state.x - cspice_state[0]).abs());
            println!("Y diff: {:.10} km", (first_state.y - cspice_state[1]).abs());
            println!("Z diff: {:.10} km", (first_state.z - cspice_state[2]).abs());
        }
        other => {
            println!("\n=== Unexpected type: {} ===", other.spk_type());
        }
    }
}

#[test]
fn debug_spk_boundary_interpolation() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    // Load kernel for CSPICE
    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());
    cspice_kernels.load(&spk_path());

    // Load our kernel
    let kernel = muad_dib::kernel::SpiceKernel::load(&spk_path()).expect("Failed to load SPK");

    // Get the first SPK segment
    let file = File::open(spk_path()).expect("Failed to open SPK");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF");
    let first_segment = daf.into_iter().next().unwrap().unwrap();

    let spk = match first_segment {
        DAFSegment::SPK(s) => s,
        _ => panic!("Expected SPK segment"),
    };

    let parsed = parse_spk_data(spk.spk_type, spk.data.clone());
    let type9 = match &parsed {
        SpkData::Type9(t) => t,
        _ => panic!("Expected Type 9"),
    };

    // Print boundary information with high precision
    println!("\n=== Boundary Analysis ===");
    println!("Segment initial_epoch: {:.15}", spk.initial_epoch);
    println!("Segment final_epoch:   {:.15}", spk.final_epoch);
    println!(
        "First state epoch:     {:.15}",
        type9.states.first().unwrap().epoch
    );
    println!(
        "Last state epoch:      {:.15}",
        type9.states.last().unwrap().epoch
    );
    println!("Window size: {}", type9.window_size);
    println!();
    println!(
        "final_epoch == last state epoch? {}",
        spk.final_epoch == type9.states.last().unwrap().epoch
    );
    println!("final_epoch bits: {:016x}", spk.final_epoch.to_bits());
    println!(
        "last state bits:  {:016x}",
        type9.states.last().unwrap().epoch.to_bits()
    );

    // Test at fraction 1 (the exact end)
    let epoch = spk.final_epoch;
    let target = spk.target_code;
    let center = spk.center_code;

    println!("\n=== Query at fraction 1 (epoch={:.6}) ===", epoch);

    // Our result
    let muad_state = kernel
        .state_of(
            muad_dib::types::NaifId(target),
            muad_dib::types::EpochTDB(epoch),
            muad_dib::types::NaifId(center),
        )
        .expect("state_of failed");
    println!(
        "muad-dib velocity: [{:.10}, {:.10}, {:.10}]",
        muad_state.velocity[0], muad_state.velocity[1], muad_state.velocity[2]
    );

    // CSPICE result
    let (cspice_state, _) = cspice_spkgeo(target, epoch, frame_name(spk.frame_code), center);
    println!(
        "CSPICE velocity: [{:.10}, {:.10}, {:.10}]",
        cspice_state[3], cspice_state[4], cspice_state[5]
    );

    // Difference
    println!("\nVelocity difference:");
    for i in 0..3 {
        println!(
            "  [{}]: diff = {:.10}",
            i,
            (muad_state.velocity[i] - cspice_state[i + 3]).abs()
        );
    }

    // Check at a slightly earlier epoch
    let earlier = epoch - 0.001;
    println!("\n=== Query 1ms earlier (epoch={:.6}) ===", earlier);

    let muad_earlier = kernel
        .state_of(
            muad_dib::types::NaifId(target),
            muad_dib::types::EpochTDB(earlier),
            muad_dib::types::NaifId(center),
        )
        .expect("state_of failed");
    let (cspice_earlier, _) = cspice_spkgeo(target, earlier, frame_name(spk.frame_code), center);

    println!("Velocity difference at earlier epoch:");
    for i in 0..3 {
        println!(
            "  [{}]: diff = {:.10}",
            i,
            (muad_earlier.velocity[i] - cspice_earlier[i + 3]).abs()
        );
    }

    // Check at fraction 0 (start boundary)
    let start_epoch = spk.initial_epoch;
    println!("\n=== Query at fraction 0 (epoch={:.6}) ===", start_epoch);

    let muad_start = kernel
        .state_of(
            muad_dib::types::NaifId(target),
            muad_dib::types::EpochTDB(start_epoch),
            muad_dib::types::NaifId(center),
        )
        .expect("state_of failed");
    let (cspice_start, _) = cspice_spkgeo(target, start_epoch, frame_name(spk.frame_code), center);

    println!("Velocity at start:");
    println!(
        "  muad-dib: [{:.10}, {:.10}, {:.10}]",
        muad_start.velocity[0], muad_start.velocity[1], muad_start.velocity[2]
    );
    println!(
        "  CSPICE:   [{:.10}, {:.10}, {:.10}]",
        cspice_start[3], cspice_start[4], cspice_start[5]
    );
    println!("Velocity difference at start:");
    for i in 0..3 {
        println!(
            "  [{}]: diff = {:.10}",
            i,
            (muad_start.velocity[i] - cspice_start[i + 3]).abs()
        );
    }

    // Print raw stored velocities at first and last states
    println!("\n=== Raw stored velocities ===");
    let first_state = type9.states.first().unwrap();
    let last_state = type9.states.last().unwrap();
    println!(
        "First state (idx 0) velocity: [{:.10}, {:.10}, {:.10}]",
        first_state.vx, first_state.vy, first_state.vz
    );
    println!(
        "Last state (idx {}) velocity: [{:.10}, {:.10}, {:.10}]",
        type9.states.len() - 1,
        last_state.vx,
        last_state.vy,
        last_state.vz
    );

    // CSPICE at last state's exact epoch (should return raw values)
    println!("\n=== CSPICE returns at exact last state epoch ===");
    println!(
        "CSPICE velocity: [{:.10}, {:.10}, {:.10}]",
        cspice_state[3], cspice_state[4], cspice_state[5]
    );

    // Our interpolation at last state (should also return raw or near-raw values)
    println!(
        "Our interpolated velocity: [{:.10}, {:.10}, {:.10}]",
        muad_state.velocity[0], muad_state.velocity[1], muad_state.velocity[2]
    );

    // Test at tiny epsilon before the last epoch
    for eps_exp in [9, 6, 3] {
        let eps = 10.0_f64.powi(-eps_exp);
        let test_epoch = spk.final_epoch - eps;
        let muad_eps = kernel
            .state_of(
                muad_dib::types::NaifId(target),
                muad_dib::types::EpochTDB(test_epoch),
                muad_dib::types::NaifId(center),
            )
            .expect("state_of failed");
        let (cspice_eps, _) = cspice_spkgeo(target, test_epoch, frame_name(spk.frame_code), center);

        println!("\n=== At epoch - 1e-{} seconds ===", eps_exp);
        println!(
            "VX diff: {:.15}",
            (muad_eps.velocity[0] - cspice_eps[3]).abs()
        );
    }
}

#[test]
fn debug_spk_raw_segment_data() {
    let file = File::open(spk_path()).expect("Failed to open SPK");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF");
    let first_segment = daf.into_iter().next().unwrap().unwrap();

    let spk = match first_segment {
        DAFSegment::SPK(s) => s,
        _ => panic!("Expected SPK segment"),
    };

    println!("\n=== Raw Segment Data ===");
    println!("SPK Type: {}", spk.spk_type);
    println!("Raw data length: {}", spk.data.len());

    let n = spk.data.len();
    if n >= 2 {
        let num_states = spk.data[n - 1] as usize;
        let window_size = spk.data[n - 2] as u32;
        println!("N (last element): {}", num_states);
        println!("Window size (second-to-last): {}", window_size);

        // For Type 9/13, the layout is:
        // states (6*N f64s) + epochs (N f64s) + epoch_dir (if N > 100) + window_size + N
        let state_size = 6;
        let epoch_dir_size = if num_states > 100 {
            (num_states - 1) / 100
        } else {
            0
        };
        let expected_len = num_states * state_size + num_states + epoch_dir_size + 2;
        println!("Expected data length: {}", expected_len);

        // Print first few raw values
        println!("\n=== First 24 raw values (first 4 states) ===");
        for i in 0..24.min(n) {
            println!("data[{}] = {:.6}", i, spk.data[i]);
        }

        // Print epochs (after states)
        let epochs_start = num_states * state_size;
        println!(
            "\n=== First 5 epochs (starting at index {}) ===",
            epochs_start
        );
        for i in 0..5.min(num_states) {
            if epochs_start + i < n - 2 - epoch_dir_size {
                println!("epoch[{}] = {:.3}", i, spk.data[epochs_start + i]);
            }
        }
    }
}
