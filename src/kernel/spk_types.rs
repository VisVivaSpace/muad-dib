//! SPK segment type definitions with type-specific data structures.
//!
//! SPK files contain different segment types for storing ephemeris data:
//! - Type 2: Chebyshev polynomials for position only (most common)
//! - Type 3: Chebyshev polynomials for position and velocity
//! - Type 5: Discrete states with two-body propagation
//! - Type 8: Lagrange interpolation (equal time steps)
//! - Type 9: Lagrange interpolation (unequal time steps)
//! - Type 13: Hermite interpolation (unequal time steps)
//! - Type 21: Extended Modified Difference Arrays
//!
//! Each type has a specific internal structure. This module provides
//! type-safe access to parsed segment data.

use serde::{Deserialize, Serialize};

/// Parsed SPK segment data, with type-specific structure.
///
/// Variants provide access to type-specific fields like Chebyshev coefficients
/// or discrete state vectors. The `Raw` variant is used as a fallback for
/// unsupported or less common SPK types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpkData {
    /// SPK Type 2: Chebyshev polynomials for position only
    Type2(Spk2Data),
    /// SPK Type 3: Chebyshev polynomials for position and velocity
    Type3(Spk3Data),
    /// SPK Type 5: Discrete states with two-body propagation (GM)
    Type5(Spk5Data),
    /// SPK Type 8: Lagrange interpolation (equally spaced states)
    Type8(Spk8Data),
    /// SPK Type 9: Lagrange interpolation (unequally spaced states)
    Type9(Spk9Data),
    /// SPK Type 13: Hermite interpolation (unequally spaced states)
    Type13(Spk13Data),
    /// Raw/unparsed data for unsupported SPK types
    Raw { spk_type: i32, data: Vec<f64> },
}

impl SpkData {
    /// Get the SPK type code.
    pub fn spk_type(&self) -> i32 {
        match self {
            SpkData::Type2(_) => 2,
            SpkData::Type3(_) => 3,
            SpkData::Type5(_) => 5,
            SpkData::Type8(_) => 8,
            SpkData::Type9(_) => 9,
            SpkData::Type13(_) => 13,
            SpkData::Raw { spk_type, .. } => *spk_type,
        }
    }

    /// Try to get Type 2 data.
    pub fn as_type2(&self) -> Option<&Spk2Data> {
        match self {
            SpkData::Type2(data) => Some(data),
            _ => None,
        }
    }

    /// Try to get Type 3 data.
    pub fn as_type3(&self) -> Option<&Spk3Data> {
        match self {
            SpkData::Type3(data) => Some(data),
            _ => None,
        }
    }

    /// Try to get Type 5 data.
    pub fn as_type5(&self) -> Option<&Spk5Data> {
        match self {
            SpkData::Type5(data) => Some(data),
            _ => None,
        }
    }

    /// Try to get Type 8 data.
    pub fn as_type8(&self) -> Option<&Spk8Data> {
        match self {
            SpkData::Type8(data) => Some(data),
            _ => None,
        }
    }

    /// Try to get Type 9 data.
    pub fn as_type9(&self) -> Option<&Spk9Data> {
        match self {
            SpkData::Type9(data) => Some(data),
            _ => None,
        }
    }

    /// Try to get Type 13 data.
    pub fn as_type13(&self) -> Option<&Spk13Data> {
        match self {
            SpkData::Type13(data) => Some(data),
            _ => None,
        }
    }
}

// ============================================================================
// SPK Type 2: Chebyshev Position Only
// ============================================================================

/// SPK Type 2: Chebyshev polynomials for position only.
///
/// Type 2 is the most common SPK format, used for planetary ephemerides
/// like DE430, DE440. Each record contains Chebyshev coefficients for
/// X, Y, Z position components over a fixed time interval.
///
/// Velocity is computed by differentiating the position polynomials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spk2Data {
    /// Initial epoch of the first record (TDB seconds past J2000)
    pub init_epoch: f64,
    /// Length of each record's time interval (seconds)
    pub interval_length: f64,
    /// Polynomial degree (number of coefficients - 1)
    pub degree: u32,
    /// Chebyshev coefficient records
    pub records: Vec<ChebyshevRecord>,
}

/// A single Chebyshev coefficient record for Type 2/3 segments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChebyshevRecord {
    /// Midpoint of the record's time interval (TDB seconds past J2000)
    pub midpoint: f64,
    /// Half-width of the record's time interval (seconds)
    pub radius: f64,
    /// Chebyshev coefficients for X position (km)
    pub x_coeffs: Vec<f64>,
    /// Chebyshev coefficients for Y position (km)
    pub y_coeffs: Vec<f64>,
    /// Chebyshev coefficients for Z position (km)
    pub z_coeffs: Vec<f64>,
}

// ============================================================================
// SPK Type 3: Chebyshev Position and Velocity
// ============================================================================

/// SPK Type 3: Chebyshev polynomials for position and velocity.
///
/// Similar to Type 2, but stores separate coefficients for velocity
/// instead of computing it by differentiation. More accurate for
/// velocity but requires more storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spk3Data {
    /// Initial epoch of the first record (TDB seconds past J2000)
    pub init_epoch: f64,
    /// Length of each record's time interval (seconds)
    pub interval_length: f64,
    /// Polynomial degree (number of coefficients - 1)
    pub degree: u32,
    /// Chebyshev coefficient records with velocity
    pub records: Vec<ChebyshevRecordWithVelocity>,
}

/// A single Chebyshev coefficient record with position and velocity (Type 3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChebyshevRecordWithVelocity {
    /// Midpoint of the record's time interval (TDB seconds past J2000)
    pub midpoint: f64,
    /// Half-width of the record's time interval (seconds)
    pub radius: f64,
    /// Chebyshev coefficients for X position (km)
    pub x_coeffs: Vec<f64>,
    /// Chebyshev coefficients for Y position (km)
    pub y_coeffs: Vec<f64>,
    /// Chebyshev coefficients for Z position (km)
    pub z_coeffs: Vec<f64>,
    /// Chebyshev coefficients for X velocity (km/s)
    pub vx_coeffs: Vec<f64>,
    /// Chebyshev coefficients for Y velocity (km/s)
    pub vy_coeffs: Vec<f64>,
    /// Chebyshev coefficients for Z velocity (km/s)
    pub vz_coeffs: Vec<f64>,
}

// ============================================================================
// SPK Type 5: Discrete States with Two-Body Propagation
// ============================================================================

/// SPK Type 5: Discrete states with gravitational parameter.
///
/// Type 5 stores state vectors at discrete epochs along with
/// the central body's GM value. States between epochs are
/// computed using two-body (Keplerian) propagation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spk5Data {
    /// Gravitational parameter of central body (km^3/s^2)
    pub gm: f64,
    /// Discrete state records
    pub states: Vec<StateRecord>,
}

/// A single state vector at a discrete epoch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateRecord {
    /// Epoch of this state (TDB seconds past J2000)
    pub epoch: f64,
    /// X position (km)
    pub x: f64,
    /// Y position (km)
    pub y: f64,
    /// Z position (km)
    pub z: f64,
    /// X velocity (km/s)
    pub vx: f64,
    /// Y velocity (km/s)
    pub vy: f64,
    /// Z velocity (km/s)
    pub vz: f64,
}

// ============================================================================
// SPK Type 8: Lagrange Interpolation (Equal Time Steps)
// ============================================================================

/// SPK Type 8: Lagrange interpolation with equally spaced states.
///
/// Type 8 stores state vectors at equally spaced time intervals.
/// Interpolation uses Lagrange polynomials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spk8Data {
    /// Epoch of the first state (TDB seconds past J2000)
    pub start_epoch: f64,
    /// Time step between states (seconds)
    pub step_size: f64,
    /// Interpolation window size (number of states used)
    pub window_size: u32,
    /// State vectors
    pub states: Vec<StateRecord>,
}

// ============================================================================
// SPK Type 9: Lagrange Interpolation (Unequal Time Steps)
// ============================================================================

/// SPK Type 9: Lagrange interpolation with unequally spaced states.
///
/// Type 9 stores state vectors at arbitrary epochs.
/// Interpolation uses Lagrange polynomials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spk9Data {
    /// Interpolation window size (number of states used)
    pub window_size: u32,
    /// State vectors with their epochs
    pub states: Vec<StateRecord>,
}

// ============================================================================
// SPK Type 13: Hermite Interpolation (Unequal Time Steps)
// ============================================================================

/// SPK Type 13: Hermite interpolation with unequally spaced states.
///
/// Type 13 stores state vectors at arbitrary epochs.
/// Interpolation uses Hermite polynomials which match both position
/// and velocity at interpolation points for smoother results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spk13Data {
    /// Interpolation window size (number of states used)
    pub window_size: u32,
    /// State vectors with their epochs
    pub states: Vec<StateRecord>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spk_data_type_code() {
        let data = SpkData::Type2(Spk2Data {
            init_epoch: 0.0,
            interval_length: 86400.0,
            degree: 10,
            records: vec![],
        });
        assert_eq!(data.spk_type(), 2);

        let raw = SpkData::Raw {
            spk_type: 21,
            data: vec![],
        };
        assert_eq!(raw.spk_type(), 21);
    }

    #[test]
    fn test_as_type2() {
        let data = SpkData::Type2(Spk2Data {
            init_epoch: 0.0,
            interval_length: 86400.0,
            degree: 10,
            records: vec![],
        });
        assert!(data.as_type2().is_some());
        assert!(data.as_type5().is_none());
    }
}
