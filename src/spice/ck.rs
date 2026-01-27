//! High-level CK pointing evaluation API.
//!
//! This module provides `pointing_at()` methods for querying orientation data,
//! equivalent to CSPICE's `ckgp()` function.
//!
//! # SLERP Interpolation
//!
//! For CK Type 3 segments, quaternion interpolation uses SLERP (Spherical
//! Linear Interpolation) which provides constant angular velocity rotation
//! between two orientations.
//!
//! # Example
//!
//! ```ignore
//! use muad_dib::kernel::SpiceKernel;
//! use muad_dib::spice::CkInterpolateExt;
//! use muad_dib::types::{NaifId, Sclk};
//!
//! let kernel = SpiceKernel::load("spacecraft.bc")?;
//!
//! let sclk = Sclk(123456789.0);
//! let pointing = kernel.pointing_of(NaifId(-82), sclk)?;
//! println!("Quaternion: {:?}", pointing.quaternion);
//! ```

use crate::error::Error;
use crate::kernel::ck_types::{Ck1Data, Ck3Data, CkData, PointingRecord};
use crate::kernel::{CkSegmentView, SpiceKernel};
use crate::prelude::*;
use crate::spice::interpolate::Pointing;
use crate::types::{NaifId, Sclk};
use crate::CKSegment;

/// Spherical Linear Interpolation (SLERP) for quaternions.
///
/// Interpolates between two quaternions along the shortest path on the
/// 4D unit sphere, providing constant angular velocity.
///
/// # Arguments
///
/// * `q0` - Start quaternion [scalar, i, j, k]
/// * `q1` - End quaternion [scalar, i, j, k]
/// * `t` - Interpolation parameter [0, 1]
///
/// # Returns
///
/// Interpolated quaternion.
pub fn slerp(q0: &[f64; 4], q1: &[f64; 4], t: f64) -> [f64; 4] {
    // Helper to normalize quaternion
    fn normalize_quat(q: [f64; 4]) -> [f64; 4] {
        let norm = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
        if norm < 1e-15 {
            return q;
        }
        [q[0] / norm, q[1] / norm, q[2] / norm, q[3] / norm]
    }

    // Handle edge cases
    if t <= 0.0 {
        return normalize_quat(*q0);
    }
    if t >= 1.0 {
        return normalize_quat(*q1);
    }

    // Compute dot product (cosine of angle)
    let mut dot = q0[0] * q1[0] + q0[1] * q1[1] + q0[2] * q1[2] + q0[3] * q1[3];

    // If dot is negative, negate one quaternion to take shorter path
    let mut q1_adj = *q1;
    if dot < 0.0 {
        q1_adj = [-q1[0], -q1[1], -q1[2], -q1[3]];
        dot = -dot;
    }

    // If quaternions are very close, use linear interpolation
    if dot > 0.9995 {
        let result = [
            q0[0] + t * (q1_adj[0] - q0[0]),
            q0[1] + t * (q1_adj[1] - q0[1]),
            q0[2] + t * (q1_adj[2] - q0[2]),
            q0[3] + t * (q1_adj[3] - q0[3]),
        ];

        // Normalize
        let norm = (result[0] * result[0]
            + result[1] * result[1]
            + result[2] * result[2]
            + result[3] * result[3])
            .sqrt();
        return [
            result[0] / norm,
            result[1] / norm,
            result[2] / norm,
            result[3] / norm,
        ];
    }

    // Standard SLERP
    let theta_0 = dot.acos();
    let theta = theta_0 * t;

    let sin_theta = theta.sin();
    let sin_theta_0 = theta_0.sin();

    let s0 = theta.cos() - dot * sin_theta / sin_theta_0;
    let s1 = sin_theta / sin_theta_0;

    let result = [
        s0 * q0[0] + s1 * q1_adj[0],
        s0 * q0[1] + s1 * q1_adj[1],
        s0 * q0[2] + s1 * q1_adj[2],
        s0 * q0[3] + s1 * q1_adj[3],
    ];

    // Normalize result for numerical stability
    let norm = (result[0] * result[0]
        + result[1] * result[1]
        + result[2] * result[2]
        + result[3] * result[3])
        .sqrt();

    [
        result[0] / norm,
        result[1] / norm,
        result[2] / norm,
        result[3] / norm,
    ]
}

/// Interpolate angular velocity between two records.
fn interpolate_angular_velocity(
    rec0: &PointingRecord,
    rec1: &PointingRecord,
    t: f64,
) -> Option<[f64; 3]> {
    let av0 = rec0.angular_velocity()?;
    let av1 = rec1.angular_velocity()?;

    Some([
        av0[0] + t * (av1[0] - av0[0]),
        av0[1] + t * (av1[1] - av0[1]),
        av0[2] + t * (av1[2] - av0[2]),
    ])
}

/// Evaluate CK Type 1 data at the given SCLK.
///
/// Type 1 returns the pointing from the most recent record at or before
/// the query time (no interpolation).
fn evaluate_type1(data: &Ck1Data, sclk: f64) -> Result<Pointing> {
    if data.records.is_empty() {
        return Err(Error::EpochOutOfRange {
            epoch: sclk,
            start: 0.0,
            end: 0.0,
        });
    }

    // Find the most recent record at or before sclk
    let mut best_idx = None;
    for (i, record) in data.records.iter().enumerate() {
        if record.sclk <= sclk {
            best_idx = Some(i);
        } else {
            break;
        }
    }

    let idx = best_idx.ok_or(Error::EpochOutOfRange {
        epoch: sclk,
        start: data.records.first().unwrap().sclk,
        end: data.records.last().unwrap().sclk,
    })?;

    let record = &data.records[idx];
    Ok(Pointing::new_raw(
        record.quaternion(),
        record.angular_velocity(),
    ))
}

/// Evaluate CK Type 3 data at the given SCLK.
///
/// Type 3 uses SLERP interpolation between bracketing records.
fn evaluate_type3(data: &Ck3Data, sclk: f64) -> Result<Pointing> {
    if data.records.is_empty() {
        return Err(Error::EpochOutOfRange {
            epoch: sclk,
            start: 0.0,
            end: 0.0,
        });
    }

    let start_sclk = data.records.first().unwrap().sclk;
    let end_sclk = data.records.last().unwrap().sclk;

    if sclk < start_sclk || sclk > end_sclk {
        return Err(Error::EpochOutOfRange {
            epoch: sclk,
            start: start_sclk,
            end: end_sclk,
        });
    }

    // Find bracketing records
    let mut lower_idx = 0;
    for (i, record) in data.records.iter().enumerate() {
        if record.sclk <= sclk {
            lower_idx = i;
        } else {
            break;
        }
    }

    // Exact match
    if (data.records[lower_idx].sclk - sclk).abs() < 1e-10 {
        let record = &data.records[lower_idx];
        return Ok(Pointing::new_raw(
            record.quaternion(),
            record.angular_velocity(),
        ));
    }

    // Need to interpolate
    let upper_idx = (lower_idx + 1).min(data.records.len() - 1);
    if upper_idx == lower_idx {
        // At the last record
        let record = &data.records[lower_idx];
        return Ok(Pointing::new_raw(
            record.quaternion(),
            record.angular_velocity(),
        ));
    }

    let rec0 = &data.records[lower_idx];
    let rec1 = &data.records[upper_idx];

    // Compute interpolation parameter
    let dt = rec1.sclk - rec0.sclk;
    let t = if dt.abs() < 1e-15 {
        0.0
    } else {
        (sclk - rec0.sclk) / dt
    };

    // SLERP for quaternion
    let q = slerp(&rec0.quaternion(), &rec1.quaternion(), t);

    // Linear interpolation for angular velocity
    let av = interpolate_angular_velocity(rec0, rec1, t);

    Ok(Pointing::new_raw(q, av))
}

/// Extension trait for CK segment views with interpolation.
pub trait CkSegmentViewInterpolate {
    /// Evaluate pointing (quaternion + angular velocity) at the given SCLK.
    ///
    /// # Arguments
    ///
    /// * `sclk` - Spacecraft clock time
    ///
    /// # Returns
    ///
    /// Pointing data (quaternion and optional angular velocity).
    fn pointing_at(&self, sclk: Sclk) -> Result<Pointing>;
}

impl CkSegmentViewInterpolate for CkSegmentView<'_> {
    fn pointing_at(&self, sclk: Sclk) -> Result<Pointing> {
        let data = self.data();
        let sclk_ticks = sclk.as_ticks();

        // Compute raw quaternion/angular velocity from interpolation
        let mut pointing = match data {
            CkData::Type1(d) => evaluate_type1(d, sclk_ticks),
            CkData::Type3(d) => evaluate_type3(d, sclk_ticks),
            CkData::Raw { ck_type, .. } => Err(Error::UnsupportedCkType { ck_type: *ck_type }),
        }?;

        // Add frame context from the segment
        pointing.frame = self.frame();

        Ok(pointing)
    }
}

/// Extension trait for CK pointing evaluation on SpiceKernel.
pub trait CkInterpolateExt {
    /// Get pointing for an instrument at the given SCLK time.
    ///
    /// Searches loaded CK data for segments providing coverage and
    /// evaluates the appropriate interpolation algorithm.
    ///
    /// # Arguments
    ///
    /// * `instrument` - NAIF ID of instrument/structure
    /// * `sclk` - Spacecraft clock time
    ///
    /// # Returns
    ///
    /// Pointing data (quaternion and optional angular velocity).
    fn pointing_of(&self, instrument: NaifId, sclk: Sclk) -> Result<Pointing>;

    /// Evaluate a specific CK segment at the given SCLK.
    fn evaluate_ck_segment(&self, segment: &CKSegment, sclk: Sclk) -> Result<Pointing>;
}

impl CkInterpolateExt for SpiceKernel {
    fn pointing_of(&self, instrument: NaifId, sclk: Sclk) -> Result<Pointing> {
        let sclk_ticks = sclk.as_ticks();
        // Find a segment for the instrument that covers this SCLK
        let segment = self
            .ck_segments_for(instrument)
            .find(|seg| seg.initial_sclk <= sclk_ticks && sclk_ticks <= seg.final_sclk)
            .ok_or(Error::NoCoverage {
                body: instrument,
                epoch: sclk_ticks,
            })?;

        self.evaluate_ck_segment(segment, sclk)
    }

    fn evaluate_ck_segment(&self, segment: &CKSegment, sclk: Sclk) -> Result<Pointing> {
        let view = self.ck_view(segment);
        view.pointing_at(sclk)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn test_slerp_endpoints() {
        let q0 = [1.0, 0.0, 0.0, 0.0]; // Identity (already normalized)
                                       // Use properly normalized quaternion for 90 degree rotation around X
        let half_angle = std::f64::consts::FRAC_PI_4; // 45 degrees in radians
        let q1 = [half_angle.cos(), half_angle.sin(), 0.0, 0.0];

        // At t=0, should return q0 (normalized)
        let result = slerp(&q0, &q1, 0.0);
        assert!((result[0] - q0[0]).abs() < 1e-10);
        assert!((result[1] - q0[1]).abs() < 1e-10);

        // At t=1, should return q1 (normalized)
        let result = slerp(&q0, &q1, 1.0);
        assert!((result[0] - q1[0]).abs() < 1e-10);
        assert!((result[1] - q1[1]).abs() < 1e-10);
    }

    #[test]
    fn test_slerp_midpoint() {
        let q0 = [1.0, 0.0, 0.0, 0.0]; // Identity
        let q1 = [0.0, 1.0, 0.0, 0.0]; // 180 degree rotation around X

        // At t=0.5, should be 90 degree rotation around X
        let result = slerp(&q0, &q1, 0.5);

        // 90 degree around X: [cos(45), sin(45), 0, 0]
        let expected_scalar = (PI / 4.0).cos();
        let expected_i = (PI / 4.0).sin();

        assert!((result[0] - expected_scalar).abs() < 1e-6);
        assert!((result[1] - expected_i).abs() < 1e-6);
    }

    #[test]
    fn test_slerp_normalization() {
        let q0 = [1.0, 0.0, 0.0, 0.0];
        // Use properly normalized quaternion
        let half_angle = std::f64::consts::FRAC_PI_4;
        let q1 = [half_angle.cos(), half_angle.sin(), 0.0, 0.0];

        for i in 0..=10 {
            let t = i as f64 / 10.0;
            let result = slerp(&q0, &q1, t);

            let norm = (result[0] * result[0]
                + result[1] * result[1]
                + result[2] * result[2]
                + result[3] * result[3])
                .sqrt();

            assert!(
                (norm - 1.0).abs() < 1e-10,
                "SLERP result not normalized at t={}: norm={}",
                t,
                norm
            );
        }
    }

    #[test]
    fn test_slerp_antipodal() {
        // Antipodal quaternions (represent same rotation)
        let q0 = [1.0, 0.0, 0.0, 0.0];
        let q1 = [-1.0, 0.0, 0.0, 0.0];

        // Should take the short path (stay at identity)
        let result = slerp(&q0, &q1, 0.5);

        // Should be close to identity (both q0 and -q0 represent same rotation)
        let norm = (result[0] * result[0]
            + result[1] * result[1]
            + result[2] * result[2]
            + result[3] * result[3])
            .sqrt();
        assert!((norm - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_evaluate_type1() {
        let data = Ck1Data {
            has_rates: true,
            records: vec![
                PointingRecord {
                    sclk: 100.0,
                    q0: 1.0,
                    q1: 0.0,
                    q2: 0.0,
                    q3: 0.0,
                    av_x: Some(0.0),
                    av_y: Some(0.0),
                    av_z: Some(0.1),
                },
                PointingRecord {
                    sclk: 200.0,
                    q0: 0.707,
                    q1: 0.707,
                    q2: 0.0,
                    q3: 0.0,
                    av_x: Some(0.0),
                    av_y: Some(0.0),
                    av_z: Some(0.2),
                },
            ],
        };

        // At first record
        let pointing = evaluate_type1(&data, 100.0).unwrap();
        assert!((pointing.quaternion[0] - 1.0).abs() < 1e-10);

        // Between records - Type 1 returns most recent
        let pointing = evaluate_type1(&data, 150.0).unwrap();
        assert!((pointing.quaternion[0] - 1.0).abs() < 1e-10);

        // At second record
        let pointing = evaluate_type1(&data, 200.0).unwrap();
        assert!((pointing.quaternion[0] - 0.707).abs() < 1e-6);
    }

    #[test]
    fn test_evaluate_type3() {
        let data = Ck3Data {
            has_rates: false,
            records: vec![
                PointingRecord {
                    sclk: 100.0,
                    q0: 1.0,
                    q1: 0.0,
                    q2: 0.0,
                    q3: 0.0,
                    av_x: None,
                    av_y: None,
                    av_z: None,
                },
                PointingRecord {
                    sclk: 200.0,
                    q0: 0.0,
                    q1: 1.0,
                    q2: 0.0,
                    q3: 0.0,
                    av_x: None,
                    av_y: None,
                    av_z: None,
                },
            ],
            interval_starts: vec![0],
        };

        // At first record
        let pointing = evaluate_type3(&data, 100.0).unwrap();
        assert!((pointing.quaternion[0] - 1.0).abs() < 1e-10);

        // At midpoint - should be SLERP interpolated
        let pointing = evaluate_type3(&data, 150.0).unwrap();
        // Should be 90 degree rotation: [cos(45), sin(45), 0, 0]
        let expected = (PI / 4.0).cos();
        assert!(
            (pointing.quaternion[0] - expected).abs() < 1e-6,
            "Got {}, expected {}",
            pointing.quaternion[0],
            expected
        );
    }

    #[test]
    fn test_evaluate_type3_out_of_range() {
        let data = Ck3Data {
            has_rates: false,
            records: vec![PointingRecord {
                sclk: 100.0,
                q0: 1.0,
                q1: 0.0,
                q2: 0.0,
                q3: 0.0,
                av_x: None,
                av_y: None,
                av_z: None,
            }],
            interval_starts: vec![0],
        };

        let result = evaluate_type3(&data, 50.0);
        assert!(matches!(result, Err(Error::EpochOutOfRange { .. })));
    }
}
