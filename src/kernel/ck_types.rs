//! CK segment type definitions with type-specific data structures.
//!
//! CK (C-Kernel) files store orientation/pointing data as quaternions:
//! - Type 1: Discrete pointing instances
//! - Type 2: Constant angular velocity segments
//! - Type 3: Linear interpolation between pointing instances
//!
//! Quaternion convention: SPICE uses scalar-first quaternions (q0, q1, q2, q3)
//! where q0 is the scalar component and (q1, q2, q3) is the vector component.

use serde::{Deserialize, Serialize};

/// Parsed CK segment data, with type-specific structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CkData {
    /// CK Type 1: Discrete pointing instances
    Type1(Ck1Data),
    /// CK Type 3: Linear interpolation between pointing instances
    Type3(Ck3Data),
    /// Raw/unparsed data for unsupported CK types
    Raw {
        ck_type: i32,
        has_rates: bool,
        data: Vec<f64>,
    },
}

impl CkData {
    /// Get the CK type code.
    pub fn ck_type(&self) -> i32 {
        match self {
            CkData::Type1(_) => 1,
            CkData::Type3(_) => 3,
            CkData::Raw { ck_type, .. } => *ck_type,
        }
    }

    /// Try to get Type 1 data.
    pub fn as_type1(&self) -> Option<&Ck1Data> {
        match self {
            CkData::Type1(data) => Some(data),
            _ => None,
        }
    }

    /// Try to get Type 3 data.
    pub fn as_type3(&self) -> Option<&Ck3Data> {
        match self {
            CkData::Type3(data) => Some(data),
            _ => None,
        }
    }
}

// ============================================================================
// CK Type 1: Discrete Pointing Instances
// ============================================================================

/// CK Type 1: Discrete pointing instances.
///
/// Type 1 stores quaternions (and optionally angular velocity) at discrete
/// spacecraft clock times. No interpolation is performed; the returned
/// attitude is from the most recent pointing record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ck1Data {
    /// Whether angular velocity data is included
    pub has_rates: bool,
    /// Pointing records
    pub records: Vec<PointingRecord>,
}

/// A single pointing record with quaternion and optional angular velocity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointingRecord {
    /// Encoded spacecraft clock time
    pub sclk: f64,
    /// Quaternion scalar component (q0)
    pub q0: f64,
    /// Quaternion i component (q1)
    pub q1: f64,
    /// Quaternion j component (q2)
    pub q2: f64,
    /// Quaternion k component (q3)
    pub q3: f64,
    /// Angular velocity X component (rad/s), if present
    pub av_x: Option<f64>,
    /// Angular velocity Y component (rad/s), if present
    pub av_y: Option<f64>,
    /// Angular velocity Z component (rad/s), if present
    pub av_z: Option<f64>,
}

impl PointingRecord {
    /// Get the quaternion as an array [q0, q1, q2, q3].
    pub fn quaternion(&self) -> [f64; 4] {
        [self.q0, self.q1, self.q2, self.q3]
    }

    /// Get the angular velocity as an array [av_x, av_y, av_z], if present.
    pub fn angular_velocity(&self) -> Option<[f64; 3]> {
        match (self.av_x, self.av_y, self.av_z) {
            (Some(x), Some(y), Some(z)) => Some([x, y, z]),
            _ => None,
        }
    }
}

// ============================================================================
// CK Type 3: Linear Interpolation
// ============================================================================

/// CK Type 3: Linear interpolation between pointing instances.
///
/// Type 3 stores quaternions at discrete times, partitioned into interpolation
/// intervals. Linear interpolation (SLERP) is performed between adjacent
/// records within an interval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ck3Data {
    /// Whether angular velocity data is included
    pub has_rates: bool,
    /// Pointing records
    pub records: Vec<PointingRecord>,
    /// Interval start times (indices into records)
    pub interval_starts: Vec<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ck_data_type_code() {
        let data = CkData::Type1(Ck1Data {
            has_rates: false,
            records: vec![],
        });
        assert_eq!(data.ck_type(), 1);

        let raw = CkData::Raw {
            ck_type: 5,
            has_rates: true,
            data: vec![],
        };
        assert_eq!(raw.ck_type(), 5);
    }

    #[test]
    fn test_pointing_record() {
        let rec = PointingRecord {
            sclk: 1000.0,
            q0: 1.0,
            q1: 0.0,
            q2: 0.0,
            q3: 0.0,
            av_x: Some(0.1),
            av_y: Some(0.2),
            av_z: Some(0.3),
        };

        assert_eq!(rec.quaternion(), [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(rec.angular_velocity(), Some([0.1, 0.2, 0.3]));
    }

    #[test]
    fn test_pointing_record_no_rates() {
        let rec = PointingRecord {
            sclk: 1000.0,
            q0: 1.0,
            q1: 0.0,
            q2: 0.0,
            q3: 0.0,
            av_x: None,
            av_y: None,
            av_z: None,
        };

        assert_eq!(rec.angular_velocity(), None);
    }
}
