//! Lagrange polynomial interpolation for SPK Types 8 and 9.
//!
//! This module implements Lagrange interpolation for state vectors.
//!
//! # SPK Type 8
//!
//! Type 8 stores states at equally spaced epochs with a sliding window
//! of states for interpolation.
//!
//! # SPK Type 9
//!
//! Type 9 stores states at unequally spaced epochs, also using a
//! sliding window approach.
//!
//! # Algorithm
//!
//! Lagrange interpolation finds the unique polynomial of degree n-1
//! passing through n data points. For each component (x, y, z, vx, vy, vz),
//! we independently interpolate to get the value at the query epoch.

use crate::error::Error;
use crate::kernel::spk_types::{Spk8Data, Spk9Data, StateRecord};
use crate::prelude::*;
use crate::spice::interpolate::State;

/// Evaluate Lagrange interpolating polynomial at a point.
///
/// Given n data points (x_i, y_i), computes the value of the unique
/// polynomial of degree n-1 passing through all points at x.
///
/// # Arguments
///
/// * `x_values` - Array of x coordinates (epochs)
/// * `y_values` - Array of y coordinates (component values)
/// * `x` - Point at which to evaluate
///
/// # Returns
///
/// Value of the interpolating polynomial at x.
fn lagrange_interpolate(x_values: &[f64], y_values: &[f64], x: f64) -> f64 {
    let n = x_values.len();
    debug_assert_eq!(n, y_values.len());

    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        return y_values[0];
    }

    let mut result = 0.0;

    for i in 0..n {
        let mut basis = 1.0;

        for j in 0..n {
            if i != j {
                // basis *= (x - x_j) / (x_i - x_j)
                let denom = x_values[i] - x_values[j];
                if denom.abs() > 1e-15 {
                    basis *= (x - x_values[j]) / denom;
                }
            }
        }

        result += y_values[i] * basis;
    }

    result
}

/// Select the interpolation window for Type 8 (equally spaced).
///
/// Returns the indices of states to use for interpolation.
///
/// This implements the CSPICE algorithm from spkr08.c:
/// - For ODD window size: center around the NEAREST epoch to the query
/// - For EVEN window size: use the LOWER bracketing epoch
// Allow: The branches handle semantically different cases (edge vs normal) that happen
// to return the same value. This matches CSPICE's window selection algorithm exactly.
#[allow(clippy::if_same_then_else)]
fn select_window_type8(data: &Spk8Data, epoch: f64) -> Result<(usize, usize)> {
    let n = data.states.len();
    if n == 0 {
        return Err(Error::EpochOutOfRange {
            epoch,
            start: data.start_epoch,
            end: data.start_epoch,
        });
    }

    let end_epoch = data.start_epoch + (n - 1) as f64 * data.step_size;

    if epoch < data.start_epoch || epoch > end_epoch {
        return Err(Error::EpochOutOfRange {
            epoch,
            start: data.start_epoch,
            end: end_epoch,
        });
    }

    // Find the bracketing indices (lower and high)
    let normalized = (epoch - data.start_epoch) / data.step_size;
    let lower = normalized.floor() as usize;
    let high = (lower + 1).min(n - 1);

    // window_size = degree + 1
    let wndsiz = data.window_size as usize;
    let degree = wndsiz - 1;

    // CSPICE algorithm: different strategies for odd vs even window sizes
    let first = if wndsiz % 2 == 1 {
        // ODD window size: center around NEAREST epoch
        let lower_epoch = data.start_epoch + lower as f64 * data.step_size;
        let high_epoch = data.start_epoch + high as f64 * data.step_size;
        let near = if lower == 0 {
            lower
        } else if high >= n {
            lower
        } else if (epoch - lower_epoch).abs() <= (high_epoch - epoch).abs() {
            lower
        } else {
            high
        };
        // first = max(near - degree/2, 0), clamped to valid range
        let half = degree / 2;
        if near < half {
            0
        } else if near > n - 1 - (degree - half) {
            n - wndsiz
        } else {
            near - half
        }
    } else {
        // EVEN window size: use LOWER bracket
        let half = degree / 2;
        if lower < half {
            0
        } else if lower > n - 1 - (degree - half) {
            n - wndsiz
        } else {
            lower - half
        }
    };

    let last = first + degree;
    Ok((first, last + 1)) // Return as half-open interval [first, last+1)
}

/// Select the interpolation window for Type 9 (unequally spaced).
///
/// Returns the indices of states to use for interpolation.
///
/// This implements the CSPICE algorithm from spkr09.c:
/// - For ODD window size: center around the NEAREST epoch to the query
/// - For EVEN window size: use the LOWER bracketing epoch
// Allow: The branches handle semantically different cases (edge vs normal) that happen
// to return the same value. This matches CSPICE's window selection algorithm exactly.
#[allow(clippy::if_same_then_else)]
fn select_window_type9(data: &Spk9Data, epoch: f64) -> Result<(usize, usize)> {
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

    // Binary search to find the bracketing indices
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
    let high = lower + 1;

    // window_size = degree + 1
    let wndsiz = data.window_size as usize;
    let degree = wndsiz - 1;

    // CSPICE algorithm: different strategies for odd vs even window sizes
    let first = if wndsiz % 2 == 1 {
        // ODD window size: center around NEAREST epoch
        let near = if lower == 0 {
            lower
        } else if high >= n {
            lower
        } else if (epoch - data.states[lower].epoch).abs()
            <= (data.states[high].epoch - epoch).abs()
        {
            lower
        } else {
            high
        };
        // first = max(near - degree/2, 0), clamped to valid range
        let half = degree / 2;
        if near < half {
            0
        } else if near > n - 1 - (degree - half) {
            n - wndsiz
        } else {
            near - half
        }
    } else {
        // EVEN window size: use LOWER bracket
        let half = degree / 2;
        if lower < half {
            0
        } else if lower > n - 1 - (degree - half) {
            n - wndsiz
        } else {
            lower - half
        }
    };

    let last = first + degree;
    Ok((first, last + 1)) // Return as half-open interval [first, last+1)
}

/// Interpolate a single component across states.
fn interpolate_component<F>(states: &[StateRecord], epochs: &[f64], epoch: f64, extractor: F) -> f64
where
    F: Fn(&StateRecord) -> f64,
{
    let values: Vec<f64> = states.iter().map(&extractor).collect();
    lagrange_interpolate(epochs, &values, epoch)
}

/// Evaluate an SPK Type 8 segment at the given epoch.
///
/// Type 8 uses Lagrange interpolation with equally spaced states.
///
/// # Arguments
///
/// * `data` - Parsed Type 8 segment data
/// * `epoch` - TDB seconds past J2000
///
/// # Returns
///
/// State vector (position and velocity) at the epoch.
pub fn evaluate_type8(data: &Spk8Data, epoch: f64) -> Result<State> {
    let (start_idx, end_idx) = select_window_type8(data, epoch)?;
    let window_states = &data.states[start_idx..end_idx];

    // Build epochs array for the window
    let epochs: Vec<f64> = (start_idx..end_idx)
        .map(|i| data.start_epoch + (i as f64) * data.step_size)
        .collect();

    // Interpolate each component
    let x = interpolate_component(window_states, &epochs, epoch, |s| s.x);
    let y = interpolate_component(window_states, &epochs, epoch, |s| s.y);
    let z = interpolate_component(window_states, &epochs, epoch, |s| s.z);
    let vx = interpolate_component(window_states, &epochs, epoch, |s| s.vx);
    let vy = interpolate_component(window_states, &epochs, epoch, |s| s.vy);
    let vz = interpolate_component(window_states, &epochs, epoch, |s| s.vz);

    Ok(State::new_raw([x, y, z], [vx, vy, vz]))
}

/// Evaluate an SPK Type 9 segment at the given epoch.
///
/// Type 9 uses Lagrange interpolation with unequally spaced states.
///
/// # Arguments
///
/// * `data` - Parsed Type 9 segment data
/// * `epoch` - TDB seconds past J2000
///
/// # Returns
///
/// State vector (position and velocity) at the epoch.
pub fn evaluate_type9(data: &Spk9Data, epoch: f64) -> Result<State> {
    let (start_idx, end_idx) = select_window_type9(data, epoch)?;
    let window_states = &data.states[start_idx..end_idx];

    // Get epochs from the states themselves
    let epochs: Vec<f64> = window_states.iter().map(|s| s.epoch).collect();

    // Interpolate each component
    let x = interpolate_component(window_states, &epochs, epoch, |s| s.x);
    let y = interpolate_component(window_states, &epochs, epoch, |s| s.y);
    let z = interpolate_component(window_states, &epochs, epoch, |s| s.z);
    let vx = interpolate_component(window_states, &epochs, epoch, |s| s.vx);
    let vy = interpolate_component(window_states, &epochs, epoch, |s| s.vy);
    let vz = interpolate_component(window_states, &epochs, epoch, |s| s.vz);

    Ok(State::new_raw([x, y, z], [vx, vy, vz]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lagrange_single_point() {
        let x = [1.0];
        let y = [5.0];
        assert!((lagrange_interpolate(&x, &y, 1.0) - 5.0).abs() < 1e-10);
        // Extrapolation just returns the constant
        assert!((lagrange_interpolate(&x, &y, 2.0) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_lagrange_linear() {
        // Two points define a line: y = 2x + 1
        let x = [0.0, 1.0];
        let y = [1.0, 3.0];

        assert!((lagrange_interpolate(&x, &y, 0.0) - 1.0).abs() < 1e-10);
        assert!((lagrange_interpolate(&x, &y, 1.0) - 3.0).abs() < 1e-10);
        assert!((lagrange_interpolate(&x, &y, 0.5) - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_lagrange_quadratic() {
        // Three points: (0,0), (1,1), (2,4) -> y = x^2
        let x = [0.0, 1.0, 2.0];
        let y = [0.0, 1.0, 4.0];

        assert!((lagrange_interpolate(&x, &y, 0.0) - 0.0).abs() < 1e-10);
        assert!((lagrange_interpolate(&x, &y, 1.0) - 1.0).abs() < 1e-10);
        assert!((lagrange_interpolate(&x, &y, 2.0) - 4.0).abs() < 1e-10);
        assert!((lagrange_interpolate(&x, &y, 1.5) - 2.25).abs() < 1e-10);
    }

    #[test]
    fn test_evaluate_type8() {
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
            StateRecord {
                epoch: 20.0,
                x: 20.0,
                y: 0.0,
                z: 0.0,
                vx: 1.0,
                vy: 0.0,
                vz: 0.0,
            },
            StateRecord {
                epoch: 30.0,
                x: 30.0,
                y: 0.0,
                z: 0.0,
                vx: 1.0,
                vy: 0.0,
                vz: 0.0,
            },
        ];

        let data = Spk8Data {
            start_epoch: 0.0,
            step_size: 10.0,
            window_size: 4,
            states,
        };

        // At exactly a data point
        let state = evaluate_type8(&data, 10.0).unwrap();
        assert!((state.position[0] - 10.0).abs() < 1e-6);

        // Midpoint interpolation
        let state = evaluate_type8(&data, 15.0).unwrap();
        assert!((state.position[0] - 15.0).abs() < 1e-6);
    }

    #[test]
    fn test_evaluate_type8_out_of_range() {
        let data = Spk8Data {
            start_epoch: 0.0,
            step_size: 10.0,
            window_size: 2,
            states: vec![
                StateRecord {
                    epoch: 0.0,
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    vx: 0.0,
                    vy: 0.0,
                    vz: 0.0,
                },
                StateRecord {
                    epoch: 10.0,
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
            evaluate_type8(&data, -5.0),
            Err(Error::EpochOutOfRange { .. })
        ));
        assert!(matches!(
            evaluate_type8(&data, 15.0),
            Err(Error::EpochOutOfRange { .. })
        ));
    }

    #[test]
    fn test_evaluate_type9() {
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
                epoch: 5.0,
                x: 5.0,
                y: 0.0,
                z: 0.0,
                vx: 1.0,
                vy: 0.0,
                vz: 0.0,
            },
            StateRecord {
                epoch: 15.0, // Unequal spacing
                x: 15.0,
                y: 0.0,
                z: 0.0,
                vx: 1.0,
                vy: 0.0,
                vz: 0.0,
            },
            StateRecord {
                epoch: 30.0,
                x: 30.0,
                y: 0.0,
                z: 0.0,
                vx: 1.0,
                vy: 0.0,
                vz: 0.0,
            },
        ];

        let data = Spk9Data {
            window_size: 4,
            states,
        };

        // At a data point
        let state = evaluate_type9(&data, 5.0).unwrap();
        assert!((state.position[0] - 5.0).abs() < 1e-6);

        // Interpolated
        let state = evaluate_type9(&data, 10.0).unwrap();
        assert!((state.position[0] - 10.0).abs() < 1.0); // Lagrange should interpolate reasonably
    }

    #[test]
    fn test_evaluate_type9_out_of_range() {
        let data = Spk9Data {
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
            evaluate_type9(&data, 5.0),
            Err(Error::EpochOutOfRange { .. })
        ));
        assert!(matches!(
            evaluate_type9(&data, 25.0),
            Err(Error::EpochOutOfRange { .. })
        ));
    }
}
