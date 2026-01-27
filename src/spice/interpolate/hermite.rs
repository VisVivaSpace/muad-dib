//! Hermite polynomial interpolation for SPK Type 13.
//!
//! Hermite interpolation matches both position and velocity at each data point,
//! producing a smoother interpolant than Lagrange interpolation. This is
//! particularly important for trajectory data where continuity of velocity
//! is desirable.
//!
//! # Algorithm
//!
//! For n data points with positions and velocities, Hermite interpolation
//! constructs a polynomial of degree 2n-1 that exactly matches position and
//! velocity at each point.

use crate::error::Error;
use crate::kernel::spk_types::Spk13Data;
use crate::prelude::*;
use crate::spice::interpolate::State;

/// Hermite interpolation for a single component.
///
/// Given positions and derivatives at n points, evaluates the Hermite
/// interpolating polynomial at the query point.
///
/// # Arguments
///
/// * `epochs` - Time points
/// * `values` - Function values at each epoch
/// * `derivatives` - Function derivatives at each epoch
/// * `epoch` - Query point
///
/// # Returns
///
/// Interpolated value at the query epoch.
fn hermite_interpolate(
    epochs: &[f64],
    values: &[f64],
    derivatives: &[f64],
    epoch: f64,
) -> f64 {
    let n = epochs.len();
    debug_assert_eq!(n, values.len());
    debug_assert_eq!(n, derivatives.len());

    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        // Linear extrapolation from single point
        return values[0] + derivatives[0] * (epoch - epochs[0]);
    }

    // Build the divided difference table for Hermite interpolation
    // We duplicate each point to handle both value and derivative
    let m = 2 * n;
    let mut z = vec![0.0; m];
    let mut q = vec![vec![0.0; m]; m];

    // Fill z with duplicated epochs
    for i in 0..n {
        z[2 * i] = epochs[i];
        z[2 * i + 1] = epochs[i];
        q[2 * i][0] = values[i];
        q[2 * i + 1][0] = values[i];

        // First divided difference for duplicate points is the derivative
        q[2 * i + 1][1] = derivatives[i];

        if i > 0 {
            q[2 * i][1] = (q[2 * i][0] - q[2 * i - 1][0]) / (z[2 * i] - z[2 * i - 1]);
        }
    }

    // Fill the rest of the divided difference table
    for j in 2..m {
        for i in j..m {
            let denom = z[i] - z[i - j];
            if denom.abs() > 1e-15 {
                q[i][j] = (q[i][j - 1] - q[i - 1][j - 1]) / denom;
            }
        }
    }

    // Evaluate using Newton's form
    let mut result = q[m - 1][m - 1];
    for i in (0..m - 1).rev() {
        result = result * (epoch - z[i]) + q[i][i];
    }

    result
}

/// Hermite interpolation for derivative (velocity).
///
/// Computes the derivative of the Hermite interpolant at the query point.
fn hermite_interpolate_derivative(
    epochs: &[f64],
    values: &[f64],
    derivatives: &[f64],
    epoch: f64,
) -> f64 {
    let n = epochs.len();
    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        return derivatives[0];
    }

    // Build divided difference table (same as above)
    let m = 2 * n;
    let mut z = vec![0.0; m];
    let mut q = vec![vec![0.0; m]; m];

    for i in 0..n {
        z[2 * i] = epochs[i];
        z[2 * i + 1] = epochs[i];
        q[2 * i][0] = values[i];
        q[2 * i + 1][0] = values[i];
        q[2 * i + 1][1] = derivatives[i];
        if i > 0 {
            q[2 * i][1] = (q[2 * i][0] - q[2 * i - 1][0]) / (z[2 * i] - z[2 * i - 1]);
        }
    }

    for j in 2..m {
        for i in j..m {
            let denom = z[i] - z[i - j];
            if denom.abs() > 1e-15 {
                q[i][j] = (q[i][j - 1] - q[i - 1][j - 1]) / denom;
            }
        }
    }

    // Evaluate derivative using product rule on Newton's form
    // d/dt [c0 + c1*(t-z0) + c2*(t-z0)*(t-z1) + ...]
    let mut result = 0.0;
    let mut _prod = 1.0;

    for i in 1..m {
        // Derivative of (t-z0)*(t-z1)*...*(t-z_{i-1}) at t=epoch
        let mut d_prod = 0.0;
        for k in 0..i {
            let mut term = 1.0;
            for (j, &zj) in z[..i].iter().enumerate() {
                if j != k {
                    term *= epoch - zj;
                }
            }
            d_prod += term;
        }
        result += q[i][i] * d_prod;
        _prod *= epoch - z[i - 1];
    }

    result
}

/// Select interpolation window for Type 13 data.
fn select_window(data: &Spk13Data, epoch: f64) -> Result<(usize, usize)> {
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

    if epoch < start_epoch || epoch > end_epoch {
        return Err(Error::EpochOutOfRange {
            epoch,
            start: start_epoch,
            end: end_epoch,
        });
    }

    // Binary search to find bracketing indices
    let mut lower = 0;
    let mut upper = n - 1;

    while upper - lower > 1 {
        let mid = (lower + upper) / 2;
        if data.states[mid].epoch <= epoch {
            lower = mid;
        } else {
            upper = mid;
        }
    }

    // Select window centered around the query
    let window = data.window_size as usize;
    let half_window = window / 2;

    let start_idx = if lower < half_window {
        0
    } else if lower + half_window >= n {
        n.saturating_sub(window)
    } else {
        lower - half_window
    };

    let end_idx = (start_idx + window).min(n);

    Ok((start_idx, end_idx))
}

/// Evaluate an SPK Type 13 segment at the given epoch.
///
/// Type 13 uses Hermite interpolation, matching both position and velocity
/// at each data point for smoother interpolation.
///
/// # Arguments
///
/// * `data` - Parsed Type 13 segment data
/// * `epoch` - TDB seconds past J2000
///
/// # Returns
///
/// State vector (position and velocity) at the epoch.
pub fn evaluate_type13(data: &Spk13Data, epoch: f64) -> Result<State> {
    let (start_idx, end_idx) = select_window(data, epoch)?;
    let window_states = &data.states[start_idx..end_idx];

    let epochs: Vec<f64> = window_states.iter().map(|s| s.epoch).collect();
    let x_vals: Vec<f64> = window_states.iter().map(|s| s.x).collect();
    let y_vals: Vec<f64> = window_states.iter().map(|s| s.y).collect();
    let z_vals: Vec<f64> = window_states.iter().map(|s| s.z).collect();
    let vx_vals: Vec<f64> = window_states.iter().map(|s| s.vx).collect();
    let vy_vals: Vec<f64> = window_states.iter().map(|s| s.vy).collect();
    let vz_vals: Vec<f64> = window_states.iter().map(|s| s.vz).collect();

    // Interpolate position using Hermite
    let x = hermite_interpolate(&epochs, &x_vals, &vx_vals, epoch);
    let y = hermite_interpolate(&epochs, &y_vals, &vy_vals, epoch);
    let z = hermite_interpolate(&epochs, &z_vals, &vz_vals, epoch);

    // Interpolate velocity using derivative of Hermite polynomial
    let vx = hermite_interpolate_derivative(&epochs, &x_vals, &vx_vals, epoch);
    let vy = hermite_interpolate_derivative(&epochs, &y_vals, &vy_vals, epoch);
    let vz = hermite_interpolate_derivative(&epochs, &z_vals, &vz_vals, epoch);

    Ok(State::new_raw([x, y, z], [vx, vy, vz]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::spk_types::StateRecord;

    #[test]
    fn test_hermite_single_point() {
        // With one point, should do linear extrapolation
        let epochs = [0.0];
        let values = [10.0];
        let derivatives = [2.0];

        // At the point
        assert!((hermite_interpolate(&epochs, &values, &derivatives, 0.0) - 10.0).abs() < 1e-10);

        // Extrapolated
        assert!((hermite_interpolate(&epochs, &values, &derivatives, 1.0) - 12.0).abs() < 1e-10);
    }

    #[test]
    fn test_hermite_two_points() {
        // Two points with derivatives
        // f(0) = 0, f'(0) = 1
        // f(1) = 1, f'(1) = 1
        // This describes f(t) = t (linear)
        let epochs = [0.0, 1.0];
        let values = [0.0, 1.0];
        let derivatives = [1.0, 1.0];

        assert!((hermite_interpolate(&epochs, &values, &derivatives, 0.0) - 0.0).abs() < 1e-10);
        assert!((hermite_interpolate(&epochs, &values, &derivatives, 1.0) - 1.0).abs() < 1e-10);
        assert!((hermite_interpolate(&epochs, &values, &derivatives, 0.5) - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_hermite_cubic() {
        // For f(t) = t^3:
        // f(0) = 0, f'(0) = 0
        // f(1) = 1, f'(1) = 3
        let epochs = [0.0, 1.0];
        let values = [0.0, 1.0];
        let derivatives = [0.0, 3.0];

        // At midpoint, t^3 at t=0.5 is 0.125
        let mid_val = hermite_interpolate(&epochs, &values, &derivatives, 0.5);
        assert!((mid_val - 0.125).abs() < 0.1, "Got {}", mid_val);
    }

    #[test]
    fn test_evaluate_type13_at_data_points() {
        let states = vec![
            StateRecord {
                epoch: 0.0,
                x: 0.0,
                y: 0.0,
                z: 0.0,
                vx: 1.0,
                vy: 0.0,
                vz: 0.0,
            },
            StateRecord {
                epoch: 10.0,
                x: 10.0,
                y: 0.0,
                z: 0.0,
                vx: 1.0,
                vy: 0.0,
                vz: 0.0,
            },
        ];

        let data = Spk13Data {
            window_size: 2,
            states,
        };

        // At first data point
        let state = evaluate_type13(&data, 0.0).unwrap();
        assert!((state.position[0] - 0.0).abs() < 1e-6);
        assert!((state.velocity[0] - 1.0).abs() < 0.1);

        // At second data point
        let state = evaluate_type13(&data, 10.0).unwrap();
        assert!((state.position[0] - 10.0).abs() < 1e-6);
    }

    #[test]
    fn test_evaluate_type13_interpolated() {
        let states = vec![
            StateRecord {
                epoch: 0.0,
                x: 0.0,
                y: 0.0,
                z: 0.0,
                vx: 1.0,
                vy: 0.0,
                vz: 0.0,
            },
            StateRecord {
                epoch: 10.0,
                x: 10.0,
                y: 0.0,
                z: 0.0,
                vx: 1.0,
                vy: 0.0,
                vz: 0.0,
            },
        ];

        let data = Spk13Data {
            window_size: 2,
            states,
        };

        // At midpoint - with constant velocity, should be linear
        let state = evaluate_type13(&data, 5.0).unwrap();
        assert!((state.position[0] - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_evaluate_type13_out_of_range() {
        let data = Spk13Data {
            window_size: 2,
            states: vec![
                StateRecord {
                    epoch: 10.0,
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    vx: 0.0,
                    vy: 0.0,
                    vz: 0.0,
                },
                StateRecord {
                    epoch: 20.0,
                    x: 10.0,
                    y: 0.0,
                    z: 0.0,
                    vx: 0.0,
                    vy: 0.0,
                    vz: 0.0,
                },
            ],
        };

        assert!(matches!(
            evaluate_type13(&data, 5.0),
            Err(Error::EpochOutOfRange { .. })
        ));
    }
}
