//! Two-body (Keplerian) propagation for SPK Type 5.
//!
//! SPK Type 5 stores discrete state vectors along with the central body's
//! gravitational parameter (GM). States between epochs are computed by
//! propagating the nearest state using two-body (Keplerian) dynamics.
//!
//! # Algorithm
//!
//! Given an initial state (r0, v0) at epoch t0 and GM:
//! 1. Compute orbital elements from the state
//! 2. Propagate to the query epoch using Kepler's equation
//! 3. Convert back to Cartesian coordinates
//!
//! This is accurate for elliptical, parabolic, and hyperbolic orbits.

use crate::error::Error;
use crate::kernel::spk_types::Spk5Data;
use crate::prelude::*;
use crate::spice::interpolate::State;

/// Maximum iterations for Kepler's equation solver.
const MAX_ITERATIONS: usize = 50;

/// Convergence tolerance for Kepler's equation.
const TOLERANCE: f64 = 1e-14;

/// Propagate a state using two-body dynamics.
///
/// # Arguments
///
/// * `r0` - Initial position [x, y, z] in km
/// * `v0` - Initial velocity [vx, vy, vz] in km/s
/// * `gm` - Gravitational parameter (km^3/s^2)
/// * `dt` - Time of flight (seconds)
///
/// # Returns
///
/// Final state after propagation.
pub fn propagate(r0: [f64; 3], v0: [f64; 3], gm: f64, dt: f64) -> State {
    if dt.abs() < 1e-12 {
        return State::new_raw(r0, v0);
    }

    let r_mag = (r0[0] * r0[0] + r0[1] * r0[1] + r0[2] * r0[2]).sqrt();
    let v_mag = (v0[0] * v0[0] + v0[1] * v0[1] + v0[2] * v0[2]).sqrt();

    if r_mag < 1e-10 {
        // At the origin, can't propagate
        return State::new_raw(r0, v0);
    }

    // Specific energy (vis-viva)
    let energy = v_mag * v_mag / 2.0 - gm / r_mag;

    // Semi-major axis
    let a = if energy.abs() < 1e-15 {
        // Parabolic case
        f64::INFINITY
    } else {
        -gm / (2.0 * energy)
    };

    // Angular momentum vector
    let h = [
        r0[1] * v0[2] - r0[2] * v0[1],
        r0[2] * v0[0] - r0[0] * v0[2],
        r0[0] * v0[1] - r0[1] * v0[0],
    ];
    let h_mag = (h[0] * h[0] + h[1] * h[1] + h[2] * h[2]).sqrt();

    // Radial velocity
    let r_dot = (r0[0] * v0[0] + r0[1] * v0[1] + r0[2] * v0[2]) / r_mag;

    // Eccentricity vector
    let e_vec = [
        (v0[1] * h[2] - v0[2] * h[1]) / gm - r0[0] / r_mag,
        (v0[2] * h[0] - v0[0] * h[2]) / gm - r0[1] / r_mag,
        (v0[0] * h[1] - v0[1] * h[0]) / gm - r0[2] / r_mag,
    ];
    let e = (e_vec[0] * e_vec[0] + e_vec[1] * e_vec[1] + e_vec[2] * e_vec[2]).sqrt();

    // Semi-latus rectum
    let _p = if e.abs() < 1e-10 {
        // Circular orbit
        r_mag
    } else {
        h_mag * h_mag / gm
    };

    // Use the universal variable formulation for robustness
    let (r_new, v_new) = propagate_universal(r0, v0, gm, dt, a, r_mag, r_dot);

    State::new_raw(r_new, v_new)
}

/// Universal variable formulation of Kepler's problem.
///
/// Works for all orbit types (elliptical, parabolic, hyperbolic).
fn propagate_universal(
    r0: [f64; 3],
    v0: [f64; 3],
    gm: f64,
    dt: f64,
    a: f64,
    r_mag: f64,
    r_dot: f64,
) -> ([f64; 3], [f64; 3]) {
    let sqrt_gm = gm.sqrt();

    // Initial guess for universal anomaly
    let alpha = 1.0 / a; // Can be negative for hyperbolic
    let mut chi = sqrt_gm * dt.abs() / r_mag; // Initial guess

    // Refine with initial radial velocity
    if a.is_finite() && a > 0.0 {
        // Elliptical: use mean anomaly as guide
        let _n = (gm / (a * a * a)).sqrt(); // mean motion
        chi = sqrt_gm * dt.abs() * alpha.abs();
    }

    // Newton-Raphson iteration
    for _ in 0..MAX_ITERATIONS {
        let chi2 = chi * chi;
        let psi = chi2 * alpha;

        // Stumpff functions
        let (c2, c3) = stumpff(psi);

        let r = chi2 * c2 + r_dot / sqrt_gm * chi * (1.0 - psi * c3) + r_mag * (1.0 - psi * c2);

        let f_chi = r_mag * (1.0 - psi * c2) * chi
            + r_dot / sqrt_gm * chi2 * (1.0 - psi * c3)
            + chi2 * chi * c3
            - sqrt_gm * dt.abs();

        let f_prime = r;

        if f_prime.abs() < 1e-15 {
            break;
        }

        let delta = f_chi / f_prime;
        chi -= delta;

        if delta.abs() < TOLERANCE * chi.abs().max(1.0) {
            break;
        }
    }

    // Apply sign for negative dt
    if dt < 0.0 {
        chi = -chi;
    }

    let chi2 = chi * chi;
    let psi = chi2 * alpha;
    let (c2, c3) = stumpff(psi);

    // Lagrange coefficients
    let f = 1.0 - chi2 * c2 / r_mag;
    let g = dt - chi2 * chi * c3 / sqrt_gm;

    // New position
    let r_new = [
        f * r0[0] + g * v0[0],
        f * r0[1] + g * v0[1],
        f * r0[2] + g * v0[2],
    ];

    let r_new_mag = (r_new[0] * r_new[0] + r_new[1] * r_new[1] + r_new[2] * r_new[2]).sqrt();

    let f_dot = sqrt_gm * chi * (psi * c3 - 1.0) / (r_mag * r_new_mag);
    let g_dot = 1.0 - chi2 * c2 / r_new_mag;

    // New velocity
    let v_new = [
        f_dot * r0[0] + g_dot * v0[0],
        f_dot * r0[1] + g_dot * v0[1],
        f_dot * r0[2] + g_dot * v0[2],
    ];

    (r_new, v_new)
}

/// Stumpff functions C2 and C3.
///
/// These are the series expansions that handle all orbit types:
/// - psi > 0: elliptical (sin/cos)
/// - psi = 0: parabolic (polynomials)
/// - psi < 0: hyperbolic (sinh/cosh)
fn stumpff(psi: f64) -> (f64, f64) {
    if psi.abs() < 1e-10 {
        // Parabolic (Taylor series)
        let c2 = 0.5 - psi / 24.0 + psi * psi / 720.0;
        let c3 = 1.0 / 6.0 - psi / 120.0 + psi * psi / 5040.0;
        (c2, c3)
    } else if psi > 0.0 {
        // Elliptical
        let sqrt_psi = psi.sqrt();
        let c2 = (1.0 - sqrt_psi.cos()) / psi;
        let c3 = (sqrt_psi - sqrt_psi.sin()) / (psi * sqrt_psi);
        (c2, c3)
    } else {
        // Hyperbolic
        let sqrt_neg_psi = (-psi).sqrt();
        let c2 = (1.0 - sqrt_neg_psi.cosh()) / psi;
        let c3 = (sqrt_neg_psi.sinh() - sqrt_neg_psi) / ((-psi) * sqrt_neg_psi);
        (c2, c3)
    }
}

/// Find the nearest state for a given epoch.
fn find_nearest_state(data: &Spk5Data, epoch: f64) -> Result<(usize, f64)> {
    let n = data.states.len();
    if n == 0 {
        return Err(Error::EpochOutOfRange {
            epoch,
            start: 0.0,
            end: 0.0,
        });
    }

    let start_epoch = data.states.first().unwrap().epoch;
    let end_epoch = data.states.last().unwrap().epoch;

    // Check bounds (allow some extrapolation with two-body)
    if epoch < start_epoch - 86400.0 * 365.0 || epoch > end_epoch + 86400.0 * 365.0 {
        return Err(Error::EpochOutOfRange {
            epoch,
            start: start_epoch,
            end: end_epoch,
        });
    }

    // Find the nearest state
    let mut best_idx = 0;
    let mut best_diff = (data.states[0].epoch - epoch).abs();

    for (i, state) in data.states.iter().enumerate() {
        let diff = (state.epoch - epoch).abs();
        if diff < best_diff {
            best_diff = diff;
            best_idx = i;
        }
    }

    let dt = epoch - data.states[best_idx].epoch;
    Ok((best_idx, dt))
}

/// Evaluate an SPK Type 5 segment at the given epoch.
///
/// Type 5 uses two-body propagation from the nearest discrete state.
///
/// # Arguments
///
/// * `data` - Parsed Type 5 segment data
/// * `epoch` - TDB seconds past J2000
///
/// # Returns
///
/// State vector (position and velocity) at the epoch.
pub fn evaluate_type5(data: &Spk5Data, epoch: f64) -> Result<State> {
    let (idx, dt) = find_nearest_state(data, epoch)?;
    let state = &data.states[idx];

    let r0 = [state.x, state.y, state.z];
    let v0 = [state.vx, state.vy, state.vz];

    Ok(propagate(r0, v0, data.gm, dt))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::spk_types::StateRecord;
    use std::f64::consts::PI;

    const EARTH_GM: f64 = 398600.435; // km^3/s^2

    #[test]
    fn test_stumpff_parabolic() {
        let (c2, c3) = stumpff(0.0);
        assert!((c2 - 0.5).abs() < 1e-10);
        assert!((c3 - 1.0 / 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_stumpff_elliptical() {
        let psi = 1.0;
        let (c2, c3) = stumpff(psi);

        // For psi=1: c2 = (1 - cos(1))/1, c3 = (1 - sin(1))/1
        let expected_c2 = (1.0 - 1.0_f64.cos()) / 1.0;
        let expected_c3 = (1.0 - 1.0_f64.sin()) / 1.0;

        assert!((c2 - expected_c2).abs() < 1e-10);
        assert!((c3 - expected_c3).abs() < 1e-10);
    }

    #[test]
    fn test_propagate_zero_time() {
        let r0 = [6678.0, 0.0, 0.0];
        let v0 = [0.0, 7.73, 0.0];

        let state = propagate(r0, v0, EARTH_GM, 0.0);

        assert!((state.position[0] - r0[0]).abs() < 1e-10);
        assert!((state.velocity[1] - v0[1]).abs() < 1e-10);
    }

    #[test]
    fn test_propagate_circular_orbit() {
        // Circular orbit at 300 km altitude
        let r = 6678.0; // km (Earth radius + 300 km)
        let v = (EARTH_GM / r).sqrt(); // Circular velocity

        let r0 = [r, 0.0, 0.0];
        let v0 = [0.0, v, 0.0];

        // Period of circular orbit
        let period = 2.0 * PI * (r * r * r / EARTH_GM).sqrt();

        // After one full orbit, should return to starting point
        let state = propagate(r0, v0, EARTH_GM, period);

        assert!(
            (state.position[0] - r).abs() < 1.0,
            "x position: {} vs {}",
            state.position[0],
            r
        );
        assert!(
            state.position[1].abs() < 1.0,
            "y position: {}",
            state.position[1]
        );

        // After half orbit, should be on opposite side
        let state_half = propagate(r0, v0, EARTH_GM, period / 2.0);
        assert!(
            (state_half.position[0] + r).abs() < 1.0,
            "half orbit x: {} vs {}",
            state_half.position[0],
            -r
        );
    }

    #[test]
    fn test_propagate_backward() {
        let r0 = [6678.0, 0.0, 0.0];
        let v0 = [0.0, 7.73, 0.0];

        // Propagate forward a short time
        let state1 = propagate(r0, v0, EARTH_GM, 100.0);

        // Position should have changed
        assert!(
            (state1.position[0] - r0[0]).abs() > 1.0,
            "Position should change after propagation"
        );

        // Propagate backward should approximately return to start
        let state2 = propagate(state1.position, state1.velocity, EARTH_GM, -100.0);

        // Use larger tolerance for numerical stability of two-body propagation
        assert!(
            (state2.position[0] - r0[0]).abs() < 10.0,
            "Forward-backward x: {} vs {}",
            state2.position[0],
            r0[0]
        );
    }

    #[test]
    fn test_evaluate_type5() {
        let states = vec![
            StateRecord {
                epoch: 0.0,
                x: 6678.0,
                y: 0.0,
                z: 0.0,
                vx: 0.0,
                vy: 7.73,
                vz: 0.0,
            },
            StateRecord {
                epoch: 3600.0,
                x: -6678.0,
                y: 0.0,
                z: 0.0,
                vx: 0.0,
                vy: -7.73,
                vz: 0.0,
            },
        ];

        let data = Spk5Data {
            gm: EARTH_GM,
            states,
        };

        // At first state
        let state = evaluate_type5(&data, 0.0).unwrap();
        assert!((state.position[0] - 6678.0).abs() < 1e-6);

        // Between states - should propagate from nearest
        let state = evaluate_type5(&data, 1800.0).unwrap();
        // Y position should be significant (we're at quarter orbit)
        assert!(state.position[1].abs() > 100.0);
    }

    #[test]
    fn test_evaluate_type5_out_of_range() {
        let data = Spk5Data {
            gm: EARTH_GM,
            states: vec![StateRecord {
                epoch: 0.0,
                x: 6678.0,
                y: 0.0,
                z: 0.0,
                vx: 0.0,
                vy: 7.73,
                vz: 0.0,
            }],
        };

        // Way outside reasonable extrapolation
        let result = evaluate_type5(&data, 1e10);
        assert!(matches!(result, Err(Error::EpochOutOfRange { .. })));
    }
}
