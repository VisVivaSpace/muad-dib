//! Example: Coordinate transformations
//!
//! Demonstrates converting between different coordinate systems:
//! - Rectangular (Cartesian)
//! - Latitudinal (geodetic-like)
//! - Spherical
//! - Cylindrical
//!
//! Run with: cargo run --example coordinate_transforms

use muad_dib::spice::Rectangular;
use std::f64::consts::PI;

fn main() {
    println!("=== Coordinate Transform Examples ===\n");

    // Start with a Cartesian position (e.g., a spacecraft 1000 km from Earth center)
    let position = Rectangular([500.0, 500.0, 707.107]);
    println!("Original Rectangular coordinates:");
    println!("  X: {:.3} km", position.0[0]);
    println!("  Y: {:.3} km", position.0[1]);
    println!("  Z: {:.3} km", position.0[2]);
    println!();

    // Convert to Latitudinal (useful for surface coordinates)
    let lat = position.to_latitudinal();
    println!("Latitudinal coordinates:");
    println!("  Radius:    {:.3} km", lat.radius);
    println!("  Longitude: {:.3} deg", lat.longitude * 180.0 / PI);
    println!("  Latitude:  {:.3} deg", lat.latitude * 180.0 / PI);
    println!();

    // Convert to Spherical (common in physics)
    let sph = position.to_spherical();
    println!("Spherical coordinates:");
    println!("  Radius:     {:.3} km", sph.radius);
    println!("  Colatitude: {:.3} deg", sph.colatitude * 180.0 / PI);
    println!("  Longitude:  {:.3} deg", sph.longitude * 180.0 / PI);
    println!();

    // Convert to Cylindrical (useful for axially symmetric problems)
    let cyl = position.to_cylindrical();
    println!("Cylindrical coordinates:");
    println!("  r (radial): {:.3} km", cyl.r);
    println!("  Longitude:  {:.3} deg", cyl.longitude * 180.0 / PI);
    println!("  Z:          {:.3} km", cyl.z);
    println!();

    // Round-trip back to Rectangular
    let back: Rectangular = lat.into();
    println!("Round-trip (Latitudinal -> Rectangular):");
    println!("  X: {:.3} km", back.0[0]);
    println!("  Y: {:.3} km", back.0[1]);
    println!("  Z: {:.3} km", back.0[2]);
    println!();

    // All coordinate systems can convert back to Rectangular
    let from_sph: Rectangular = sph.into();
    let from_cyl: Rectangular = cyl.into();

    // Verify round-trip accuracy
    let error_sph = ((from_sph.0[0] - position.0[0]).powi(2)
        + (from_sph.0[1] - position.0[1]).powi(2)
        + (from_sph.0[2] - position.0[2]).powi(2))
    .sqrt();

    let error_cyl = ((from_cyl.0[0] - position.0[0]).powi(2)
        + (from_cyl.0[1] - position.0[1]).powi(2)
        + (from_cyl.0[2] - position.0[2]).powi(2))
    .sqrt();

    println!("Round-trip errors:");
    println!("  Spherical:   {:.2e} km", error_sph);
    println!("  Cylindrical: {:.2e} km", error_cyl);
}
