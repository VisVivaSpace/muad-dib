//! Example: Query ephemeris data from SPK files
//!
//! Demonstrates using the SPK interpolation API to query spacecraft/planetary
//! positions and velocities at arbitrary epochs.
//!
//! Run with: cargo run --example query_ephemeris -- path/to/ephemeris.bsp
//!
//! Example SPK files can be obtained from NAIF:
//! https://naif.jpl.nasa.gov/pub/naif/generic_kernels/spk/planets/

use muad_dib::kernel::SpiceKernel;
use muad_dib::spice::{
    format_iso8601, EpochTDB, Rectangular, SpkInterpolateExt, SpkSegmentViewInterpolate,
};
use muad_dib::types::NaifId;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("=== SPK Ephemeris Query Example ===\n");
        println!("Usage: {} <spk_file.bsp> [epoch]", args[0]);
        println!();
        println!("Arguments:");
        println!("  spk_file.bsp  - SPK ephemeris file");
        println!("  epoch         - Optional: time string (default: J2000.0)");
        println!();
        println!("Examples:");
        println!("  {} de440.bsp", args[0]);
        println!("  {} de440.bsp \"2020-06-15T12:00:00\"", args[0]);
        println!();
        println!("Download planetary ephemeris from NAIF:");
        println!("  wget https://naif.jpl.nasa.gov/pub/naif/generic_kernels/spk/planets/de440.bsp");
        println!();

        println!("API Overview:\n");
        println!("  // Direct segment evaluation");
        println!("  let state = segment_view.state_at(epoch)?;");
        println!();
        println!("  // High-level query with center body chaining");
        println!("  let state = kernel.state_of(target, epoch, center)?;");
        println!();
        println!("  // State contains full relativity context:");
        println!("  //   target:   NaifId      // Body this state describes");
        println!("  //   center:   NaifId      // Origin of coordinates");
        println!("  //   frame:    i32         // Reference frame (e.g., 1 = J2000)");
        println!("  //   position: [f64; 3]    // km");
        println!("  //   velocity: [f64; 3]    // km/s");

        return Ok(());
    }

    let spk_path = &args[1];
    let epoch = if args.len() > 2 {
        EpochTDB::parse(&args[2])?
    } else {
        EpochTDB(0.0) // J2000.0
    };

    println!("=== SPK Ephemeris Query Example ===\n");
    println!("SPK file: {}", spk_path);
    println!(
        "Epoch: {} (TDB = {:.3} s)\n",
        format_iso8601(epoch.0),
        epoch.0
    );

    // Load the kernel
    let kernel = SpiceKernel::load(spk_path)?;

    // List available segments
    println!("Available segments:\n");

    let segments: Vec<_> = kernel.spk_segments().collect();
    for seg in &segments {
        println!(
            "  Target {:>4} -> Center {:>4} | Type {:>2} | {} to {}",
            seg.target_code,
            seg.center_code,
            seg.spk_type,
            format_epoch_short(seg.initial_epoch),
            format_epoch_short(seg.final_epoch)
        );
    }
    println!();

    // Try to evaluate segments that cover our epoch
    println!("States at epoch:\n");

    for seg in &segments {
        // Check if segment covers our epoch
        if seg.initial_epoch <= epoch.0 && epoch.0 <= seg.final_epoch {
            let view = kernel.spk_view(seg);
            match view.state_at(epoch) {
                Ok(state) => {
                    let pos = Rectangular(state.position);
                    let lat = pos.to_latitudinal();

                    println!(
                        "  Target {} -> Center {} (frame {}):",
                        state.target, state.center, state.frame
                    );
                    println!(
                        "    Position: [{:>15.3}, {:>15.3}, {:>15.3}] km",
                        state.position[0], state.position[1], state.position[2]
                    );
                    println!(
                        "    Velocity: [{:>15.6}, {:>15.6}, {:>15.6}] km/s",
                        state.velocity[0], state.velocity[1], state.velocity[2]
                    );

                    let distance = (state.position[0].powi(2)
                        + state.position[1].powi(2)
                        + state.position[2].powi(2))
                    .sqrt();
                    let speed = (state.velocity[0].powi(2)
                        + state.velocity[1].powi(2)
                        + state.velocity[2].powi(2))
                    .sqrt();

                    println!(
                        "    Distance: {:.3} km ({:.6} AU)",
                        distance,
                        distance / 149597870.7
                    );
                    println!("    Speed: {:.6} km/s", speed);
                    println!(
                        "    Lat/Lon: {:.3} deg / {:.3} deg",
                        lat.latitude.to_degrees(),
                        lat.longitude.to_degrees()
                    );
                    println!();
                }
                Err(e) => {
                    println!("  Target {}: Error - {:?}\n", seg.target_code, e);
                }
            }
        }
    }

    // Try high-level API if we have Earth data
    println!("High-level API demonstration:");
    println!("  (Attempting to query Earth relative to SSB)\n");

    match kernel.state_of(NaifId::EARTH, epoch, NaifId::SSB) {
        Ok(state) => {
            println!(
                "  State context: target={}, center={}, frame={}",
                state.target, state.center, state.frame
            );
            println!(
                "  Earth-SSB distance: {:.3} km ({:.6} AU)",
                state.distance(),
                state.distance() / 149597870.7
            );
        }
        Err(e) => {
            println!("  Could not query Earth: {:?}", e);
            println!("  (This file may not contain Earth ephemeris data)");
        }
    }

    Ok(())
}

fn format_epoch_short(tdb: f64) -> String {
    let (year, month, day, _, _, _) = muad_dib::spice::tdb_to_calendar(tdb);
    format!("{:04}-{:02}-{:02}", year, month, day)
}
