//! Example: Kernel pool access
//!
//! Demonstrates reading kernel pool variables from loaded text PCK files.
//! These contain planetary constants like body radii, GM values, and
//! rotation parameters.
//!
//! Run with: cargo run --example kernel_pool -- path/to/pck00010.tpc
//!
//! Example PCK files can be obtained from NAIF:
//! https://naif.jpl.nasa.gov/pub/naif/generic_kernels/pck/

use muad_dib::kernel::SpiceKernel;
use muad_dib::spice::KernelPoolExt;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("=== Kernel Pool Access Example ===\n");
        println!("Usage: {} <pck_file.tpc>", args[0]);
        println!();
        println!("This example demonstrates reading kernel pool variables.");
        println!("Download a PCK file from NAIF, e.g.:");
        println!("  wget https://naif.jpl.nasa.gov/pub/naif/generic_kernels/pck/pck00010.tpc");
        println!();

        // Show API usage without a file
        println!("API Overview:\n");
        println!("  kernel.get_f64(name)       -> Option<Vec<f64>>");
        println!("  kernel.get_f64_scalar(name)-> Option<f64>");
        println!("  kernel.get_i32(name)       -> Option<Vec<i32>>");
        println!("  kernel.get_strings(name)   -> Option<Vec<String>>");
        println!("  kernel.pool_has(name)      -> bool");
        println!("  kernel.pool_count(name)    -> Option<usize>");
        println!();
        println!("Common variable names:");
        println!("  BODY399_RADII    - Earth's triaxial radii [km]");
        println!("  BODY399_GM       - Earth's gravitational parameter [km^3/s^2]");
        println!("  BODY301_RADII    - Moon's triaxial radii [km]");
        println!("  BODY10_GM        - Sun's gravitational parameter");

        return Ok(());
    }

    let pck_path = &args[1];
    println!("=== Kernel Pool Access Example ===\n");
    println!("Loading PCK file: {}\n", pck_path);

    // Load the kernel
    let kernel = SpiceKernel::load(pck_path)?;

    // Query Earth data (NAIF ID 399)
    println!("Earth (NAIF ID 399):");

    if let Some(radii) = kernel.get_f64("BODY399_RADII") {
        println!("  Radii: {:?} km", radii);
        if radii.len() >= 3 {
            println!("    Equatorial (a): {:.3} km", radii[0]);
            println!("    Equatorial (b): {:.3} km", radii[1]);
            println!("    Polar (c):      {:.3} km", radii[2]);
        }
    } else {
        println!("  Radii: not found");
    }

    if let Some(gm) = kernel.get_f64_scalar("BODY399_GM") {
        println!("  GM: {:.6} km^3/s^2", gm);
    }

    println!();

    // Query Moon data (NAIF ID 301)
    println!("Moon (NAIF ID 301):");

    if let Some(radii) = kernel.get_f64("BODY301_RADII") {
        println!("  Radii: {:?} km", radii);
    } else {
        println!("  Radii: not found");
    }

    if let Some(gm) = kernel.get_f64_scalar("BODY301_GM") {
        println!("  GM: {:.6} km^3/s^2", gm);
    } else {
        println!("  GM: not found");
    }

    println!();

    // Query Sun data (NAIF ID 10)
    println!("Sun (NAIF ID 10):");

    if let Some(gm) = kernel.get_f64_scalar("BODY10_GM") {
        println!("  GM: {:.6e} km^3/s^2", gm);
    } else {
        println!("  GM: not found");
    }

    println!();

    // Show some utility functions
    println!("Pool utilities:");
    println!(
        "  pool_has(\"BODY399_RADII\"): {}",
        kernel.pool_has("BODY399_RADII")
    );
    println!(
        "  pool_count(\"BODY399_RADII\"): {:?}",
        kernel.pool_count("BODY399_RADII")
    );

    // Case-insensitive lookup
    println!();
    println!("Case-insensitive lookup:");
    println!(
        "  pool_has(\"body399_radii\"): {}",
        kernel.pool_has("body399_radii")
    );

    Ok(())
}
