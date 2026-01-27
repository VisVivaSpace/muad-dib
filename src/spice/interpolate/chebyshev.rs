//! Chebyshev polynomial interpolation for SPK Types 2 and 3.
//!
//! This module implements Chebyshev polynomial evaluation using the Clenshaw
//! recurrence algorithm, which is numerically stable and efficient.
//!
//! # SPK Type 2
//!
//! Type 2 segments store Chebyshev coefficients for position only.
//! Velocity is computed by differentiating the position polynomials.
//!
//! # SPK Type 3
//!
//! Type 3 segments store separate Chebyshev coefficients for both
//! position and velocity.
//!
//! # Example
//!
//! ```ignore
//! use muad_dib::spice::interpolate::chebyshev;
//! use muad_dib::kernel::spk_types::Spk2Data;
//!
//! let state = chebyshev::evaluate_type2(&spk2_data, epoch)?;
//! println!("Position: {:?} km", state.position);
//! ```

use crate::error::Error;
use crate::kernel::spk_types::{Spk2Data, Spk3Data};
use crate::prelude::*;
use crate::spice::interpolate::State;

/// Evaluate Chebyshev polynomials using Clenshaw's recurrence.
///
/// Given coefficients [c0, c1, ..., cn] and normalized argument s ∈ [-1, 1],
/// computes the sum: c0*T0(s) + c1*T1(s) + ... + cn*Tn(s)
///
/// # Arguments
///
/// * `coeffs` - Chebyshev coefficients
/// * `s` - Normalized argument in [-1, 1]
///
/// # Returns
///
/// The value of the Chebyshev polynomial at s.
fn clenshaw(coeffs: &[f64], s: f64) -> f64 {
    let n = coeffs.len();
    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        return coeffs[0];
    }

    // Clenshaw recurrence: work backwards from highest order
    let s2 = 2.0 * s;
    let mut b_k = 0.0;
    let mut b_k1 = 0.0;

    for i in (1..n).rev() {
        let b_k2 = b_k1;
        b_k1 = b_k;
        b_k = s2 * b_k1 - b_k2 + coeffs[i];
    }

    // Final step: c0 + s*b1 - b2
    coeffs[0] + s * b_k - b_k1
}

/// Evaluate Chebyshev derivative using the T-polynomial coefficient method.
///
/// Computes the derivative of a Chebyshev series at normalized argument s.
/// Uses the recurrence to compute derivative coefficients g[] such that
/// P'(s) = sum(g_k * T_k(s)), then evaluates using standard Clenshaw.
///
/// The recurrence for derivative coefficients:
///   g_{n-1} = 2*n*c_n
///   g_k = g_{k+2} + 2*(k+1)*c_{k+1}  for k = n-2, ..., 0
///   g_0 = g_0 / 2  (halve zeroth coefficient)
///
/// Reference: Numerical Recipes Section 5.9, CSPICE chbint_c
///
/// # Arguments
///
/// * `coeffs` - Chebyshev coefficients [c_0, c_1, ..., c_{n-1}]
/// * `s` - Normalized argument in [-1, 1]
/// * `scale` - Scaling factor (1/radius for ds/dt conversion)
///
/// # Returns
///
/// The derivative of the Chebyshev polynomial at s, scaled.
fn clenshaw_derivative(coeffs: &[f64], s: f64, scale: f64) -> f64 {
    let n = coeffs.len();
    if n <= 1 {
        return 0.0;
    }

    // Compute derivative coefficients g[] such that P'(s) = sum(g_k * T_k(s))
    // g has length n-1 (one less than original polynomial)
    let mut g = vec![0.0; n - 1];

    // g_{n-2} = 2*(n-1)*c_{n-1}  (highest derivative coeff)
    g[n - 2] = 2.0 * (n - 1) as f64 * coeffs[n - 1];

    // Recurrence: g_k = g_{k+2} + 2*(k+1)*c_{k+1}
    // Working backwards from k = n-3 down to 0
    for k in (0..n - 2).rev() {
        let g_k_plus_2 = if k + 2 < g.len() { g[k + 2] } else { 0.0 };
        g[k] = g_k_plus_2 + 2.0 * (k + 1) as f64 * coeffs[k + 1];
    }

    // Halve the zeroth coefficient for Clenshaw evaluation
    if !g.is_empty() {
        g[0] /= 2.0;
    }

    // Evaluate P'(s) using standard Clenshaw on derivative coefficients
    scale * clenshaw(&g, s)
}

/// Find the record index containing the given epoch.
///
/// Returns the index and the normalized argument s ∈ [-1, 1].
fn find_record(data: &Spk2Data, epoch: f64) -> Result<(usize, f64)> {
    if data.records.is_empty() {
        return Err(Error::EpochOutOfRange {
            epoch,
            start: data.init_epoch,
            end: data.init_epoch,
        });
    }

    // Find which record contains this epoch
    for (i, record) in data.records.iter().enumerate() {
        let start = record.midpoint - record.radius;
        let end = record.midpoint + record.radius;

        if epoch >= start && epoch <= end {
            // Normalize epoch to [-1, 1] within this record
            let s = (epoch - record.midpoint) / record.radius;
            return Ok((i, s));
        }
    }

    // Epoch not in any record
    let first = &data.records[0];
    let last = &data.records[data.records.len() - 1];

    Err(Error::EpochOutOfRange {
        epoch,
        start: first.midpoint - first.radius,
        end: last.midpoint + last.radius,
    })
}

/// Find the record index for Type 3 data.
fn find_record_type3(data: &Spk3Data, epoch: f64) -> Result<(usize, f64)> {
    if data.records.is_empty() {
        return Err(Error::EpochOutOfRange {
            epoch,
            start: data.init_epoch,
            end: data.init_epoch,
        });
    }

    for (i, record) in data.records.iter().enumerate() {
        let start = record.midpoint - record.radius;
        let end = record.midpoint + record.radius;

        if epoch >= start && epoch <= end {
            let s = (epoch - record.midpoint) / record.radius;
            return Ok((i, s));
        }
    }

    let first = &data.records[0];
    let last = &data.records[data.records.len() - 1];

    Err(Error::EpochOutOfRange {
        epoch,
        start: first.midpoint - first.radius,
        end: last.midpoint + last.radius,
    })
}

/// Evaluate an SPK Type 2 segment at the given epoch.
///
/// Type 2 stores Chebyshev coefficients for position only.
/// Velocity is computed by differentiating the position polynomials.
///
/// # Arguments
///
/// * `data` - Parsed Type 2 segment data
/// * `epoch` - TDB seconds past J2000
///
/// # Returns
///
/// State vector (position and velocity) at the epoch.
pub fn evaluate_type2(data: &Spk2Data, epoch: f64) -> Result<State> {
    let (idx, s) = find_record(data, epoch)?;
    let record = &data.records[idx];

    // Evaluate position using Clenshaw
    let x = clenshaw(&record.x_coeffs, s);
    let y = clenshaw(&record.y_coeffs, s);
    let z = clenshaw(&record.z_coeffs, s);

    // Compute velocity by differentiating
    // Scale factor: derivative of s w.r.t. epoch is 1/radius
    let scale = 1.0 / record.radius;

    let vx = clenshaw_derivative(&record.x_coeffs, s, scale);
    let vy = clenshaw_derivative(&record.y_coeffs, s, scale);
    let vz = clenshaw_derivative(&record.z_coeffs, s, scale);

    Ok(State::new_raw([x, y, z], [vx, vy, vz]))
}

/// Evaluate an SPK Type 3 segment at the given epoch.
///
/// Type 3 stores separate Chebyshev coefficients for both position and velocity.
///
/// # Arguments
///
/// * `data` - Parsed Type 3 segment data
/// * `epoch` - TDB seconds past J2000
///
/// # Returns
///
/// State vector (position and velocity) at the epoch.
pub fn evaluate_type3(data: &Spk3Data, epoch: f64) -> Result<State> {
    let (idx, s) = find_record_type3(data, epoch)?;
    let record = &data.records[idx];

    // Evaluate position
    let x = clenshaw(&record.x_coeffs, s);
    let y = clenshaw(&record.y_coeffs, s);
    let z = clenshaw(&record.z_coeffs, s);

    // Evaluate velocity directly from stored coefficients
    let vx = clenshaw(&record.vx_coeffs, s);
    let vy = clenshaw(&record.vy_coeffs, s);
    let vz = clenshaw(&record.vz_coeffs, s);

    Ok(State::new_raw([x, y, z], [vx, vy, vz]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::spk_types::{ChebyshevRecord, ChebyshevRecordWithVelocity};

    #[test]
    fn test_clenshaw_constant() {
        // T_0(s) = 1, so coeffs [5.0] should give 5.0 for any s
        let coeffs = [5.0];
        assert!((clenshaw(&coeffs, 0.0) - 5.0).abs() < 1e-10);
        assert!((clenshaw(&coeffs, 0.5) - 5.0).abs() < 1e-10);
        assert!((clenshaw(&coeffs, -1.0) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_clenshaw_linear() {
        // T_0(s) = 1, T_1(s) = s
        // coeffs [3.0, 2.0] gives 3 + 2*s
        let coeffs = [3.0, 2.0];
        assert!((clenshaw(&coeffs, 0.0) - 3.0).abs() < 1e-10);
        assert!((clenshaw(&coeffs, 1.0) - 5.0).abs() < 1e-10);
        assert!((clenshaw(&coeffs, -1.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_clenshaw_quadratic() {
        // T_2(s) = 2s^2 - 1
        // coeffs [0, 0, 1] gives T_2(s) = 2s^2 - 1
        let coeffs = [0.0, 0.0, 1.0];
        assert!((clenshaw(&coeffs, 0.0) - (-1.0)).abs() < 1e-10); // 2*0 - 1 = -1
        assert!((clenshaw(&coeffs, 1.0) - 1.0).abs() < 1e-10); // 2*1 - 1 = 1
        assert!((clenshaw(&coeffs, -1.0) - 1.0).abs() < 1e-10); // 2*1 - 1 = 1
    }

    #[test]
    fn test_clenshaw_derivative_linear() {
        // f(s) = 3 + 2*s, f'(s) = 2
        let coeffs = [3.0, 2.0];
        let scale = 1.0;
        assert!((clenshaw_derivative(&coeffs, 0.0, scale) - 2.0).abs() < 1e-10);
        assert!((clenshaw_derivative(&coeffs, 0.5, scale) - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_clenshaw_derivative_quadratic() {
        // P(s) = c0*T0(s) + c1*T1(s) + c2*T2(s)
        //      = c0 + c1*s + c2*(2s^2 - 1)
        // P'(s) = c1 + 4*c2*s
        //
        // With coeffs [0.0, 3.0, 2.0]:
        //   P(s) = 0 + 3*s + 2*(2s^2 - 1) = 4s^2 + 3s - 2
        //   P'(s) = 8s + 3
        let coeffs = [0.0, 3.0, 2.0];
        let scale = 1.0;

        // P'(0) = 3
        assert!((clenshaw_derivative(&coeffs, 0.0, scale) - 3.0).abs() < 1e-10);

        // P'(0.5) = 8*0.5 + 3 = 7
        assert!((clenshaw_derivative(&coeffs, 0.5, scale) - 7.0).abs() < 1e-10);

        // P'(1.0) = 8*1 + 3 = 11
        assert!((clenshaw_derivative(&coeffs, 1.0, scale) - 11.0).abs() < 1e-10);

        // P'(-1.0) = 8*(-1) + 3 = -5
        assert!((clenshaw_derivative(&coeffs, -1.0, scale) - (-5.0)).abs() < 1e-10);
    }

    #[test]
    fn test_clenshaw_derivative_cubic() {
        // T3(s) = 4s^3 - 3s
        // P(s) = c3*T3(s) = c3*(4s^3 - 3s)
        // P'(s) = c3*(12s^2 - 3)
        //
        // With coeffs [0, 0, 0, 1]:
        //   P'(s) = 12s^2 - 3
        let coeffs = [0.0, 0.0, 0.0, 1.0];
        let scale = 1.0;

        // P'(0) = -3
        assert!((clenshaw_derivative(&coeffs, 0.0, scale) - (-3.0)).abs() < 1e-10);

        // P'(1) = 12 - 3 = 9
        assert!((clenshaw_derivative(&coeffs, 1.0, scale) - 9.0).abs() < 1e-10);

        // P'(-1) = 12 - 3 = 9
        assert!((clenshaw_derivative(&coeffs, -1.0, scale) - 9.0).abs() < 1e-10);
    }

    #[test]
    fn test_clenshaw_derivative_with_scale() {
        // Test that scaling works correctly
        // If interval has radius=50, scale = 1/50 = 0.02
        let coeffs = [1000.0, 100.0, 10.0];
        let scale = 0.02;

        // Unscaled P'(s) = c1 + 4*c2*s = 100 + 40s
        // At s=0: P'(0) = 100, scaled = 2.0
        // At s=0.5: P'(0.5) = 120, scaled = 2.4

        assert!((clenshaw_derivative(&coeffs, 0.0, scale) - 2.0).abs() < 1e-10);
        assert!((clenshaw_derivative(&coeffs, 0.5, scale) - 2.4).abs() < 1e-10);
    }

    #[test]
    fn test_evaluate_type2() {
        // Create simple test data
        let data = Spk2Data {
            init_epoch: 0.0,
            interval_length: 100.0,
            degree: 1,
            records: vec![ChebyshevRecord {
                midpoint: 50.0,
                radius: 50.0,
                x_coeffs: vec![1000.0, 10.0], // x = 1000 + 10*s
                y_coeffs: vec![2000.0, 20.0], // y = 2000 + 20*s
                z_coeffs: vec![3000.0, 30.0], // z = 3000 + 30*s
            }],
        };

        // At midpoint (s=0)
        let state = evaluate_type2(&data, 50.0).unwrap();
        assert!((state.position[0] - 1000.0).abs() < 1e-6);
        assert!((state.position[1] - 2000.0).abs() < 1e-6);
        assert!((state.position[2] - 3000.0).abs() < 1e-6);

        // At end (s=1)
        let state = evaluate_type2(&data, 100.0).unwrap();
        assert!((state.position[0] - 1010.0).abs() < 1e-6);

        // Velocity should be derivative scaled by 1/radius
        // d/ds(1000 + 10*s) = 10, scaled by 1/50 = 0.2 km/s
        assert!((state.velocity[0] - 0.2).abs() < 1e-6);
    }

    #[test]
    fn test_evaluate_type2_out_of_range() {
        let data = Spk2Data {
            init_epoch: 0.0,
            interval_length: 100.0,
            degree: 1,
            records: vec![ChebyshevRecord {
                midpoint: 50.0,
                radius: 50.0,
                x_coeffs: vec![1000.0],
                y_coeffs: vec![2000.0],
                z_coeffs: vec![3000.0],
            }],
        };

        // Before range
        let result = evaluate_type2(&data, -10.0);
        assert!(matches!(result, Err(Error::EpochOutOfRange { .. })));

        // After range
        let result = evaluate_type2(&data, 110.0);
        assert!(matches!(result, Err(Error::EpochOutOfRange { .. })));
    }

    #[test]
    fn test_evaluate_type3() {
        let data = Spk3Data {
            init_epoch: 0.0,
            interval_length: 100.0,
            degree: 1,
            records: vec![ChebyshevRecordWithVelocity {
                midpoint: 50.0,
                radius: 50.0,
                x_coeffs: vec![1000.0, 10.0],
                y_coeffs: vec![2000.0, 20.0],
                z_coeffs: vec![3000.0, 30.0],
                vx_coeffs: vec![1.0, 0.1],
                vy_coeffs: vec![2.0, 0.2],
                vz_coeffs: vec![3.0, 0.3],
            }],
        };

        let state = evaluate_type3(&data, 50.0).unwrap();

        // Position at s=0
        assert!((state.position[0] - 1000.0).abs() < 1e-6);

        // Velocity comes from stored coefficients, not derivative
        assert!((state.velocity[0] - 1.0).abs() < 1e-6);
        assert!((state.velocity[1] - 2.0).abs() < 1e-6);
        assert!((state.velocity[2] - 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_multiple_records() {
        let data = Spk2Data {
            init_epoch: 0.0,
            interval_length: 50.0,
            degree: 0,
            records: vec![
                ChebyshevRecord {
                    midpoint: 25.0,
                    radius: 25.0,
                    x_coeffs: vec![100.0],
                    y_coeffs: vec![0.0],
                    z_coeffs: vec![0.0],
                },
                ChebyshevRecord {
                    midpoint: 75.0,
                    radius: 25.0,
                    x_coeffs: vec![200.0],
                    y_coeffs: vec![0.0],
                    z_coeffs: vec![0.0],
                },
            ],
        };

        // First record
        let state = evaluate_type2(&data, 10.0).unwrap();
        assert!((state.position[0] - 100.0).abs() < 1e-6);

        // Second record
        let state = evaluate_type2(&data, 60.0).unwrap();
        assert!((state.position[0] - 200.0).abs() < 1e-6);
    }
}
