//! Leap second data extraction and TDB/UTC conversion.
//!
//! This module provides functionality for:
//! - Extracting leap second data from LSK (Leap Seconds Kernel) files
//! - Converting between TDB (Barycentric Dynamical Time) and UTC
//!
//! # CSPICE-Compatible DELTET
//!
//! Conversions use the `deltet` formula matching CSPICE's `deltet_` exactly:
//!
//! ```text
//! ET - UTC = DELTA_T_A + leap_seconds + K * sin(E)
//! ```
//!
//! where `E` is derived from Earth's mean anomaly. The periodic term is
//! evaluated at the nearest ET second to guarantee exact round-trip
//! reversibility (`UTC → TDB → UTC`).
//!
//! # Example
//!
//! ```ignore
//! use muad_dib::kernel::SpiceKernel;
//! use muad_dib::spice::{LeapSecondExt, utc_to_tdb, tdb_to_utc, TimeFormat};
//!
//! let kernel = SpiceKernel::load("naif0012.tls")?;
//!
//! // Convert UTC to TDB
//! let tdb = utc_to_tdb(&kernel, "2020-01-01T00:00:00")?;
//!
//! // Convert TDB back to UTC
//! let utc = tdb_to_utc(&kernel, tdb, TimeFormat::Iso8601)?;
//! ```

use crate::error::Error;
use crate::kernel::SpiceKernel;
use crate::prelude::*;
use crate::spice::pool::KernelPoolExt;
use crate::spice::time::{format_calendar, format_iso8601, TimeFormat};
use crate::text_pck::KernelValue;
use crate::types::EpochTDB;

/// Epoch type for `deltet` computation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpochType {
    /// Epoch is UTC seconds past J2000.
    Utc,
    /// Epoch is ET (TDB) seconds past J2000.
    Et,
}

/// Leap second data extracted from an LSK file.
///
/// Contains all the constants needed for TDB/UTC conversion.
#[derive(Debug, Clone)]
pub struct LeapSecondData {
    /// DELTET/DELTA_T_A: The TT-TAI offset (32.184 seconds).
    pub delta_t_a: f64,

    /// DELTET/K: A constant used in the TDB-TT relationship.
    pub k: f64,

    /// DELTET/EB: Earth's orbital eccentricity effect on TDB.
    pub eb: f64,

    /// DELTET/M: Constants for the mean anomaly calculation.
    /// `M = M[0] + M[1] * seconds_past_J2000`
    pub m: [f64; 2],

    /// Leap second entries: (TAI-UTC offset, epoch in TDB seconds past J2000).
    /// Sorted by epoch ascending.
    pub leap_seconds: Vec<(f64, f64)>,
}

impl LeapSecondData {
    /// Get the TAI-UTC offset (ΔAT) for a given epoch compared directly
    /// against the leap second table epochs.
    ///
    /// This is a raw lookup — for correct ET-aware lookup, use `deltet`.
    pub fn delta_at(&self, epoch: f64) -> f64 {
        let mut delta = 0.0;
        for (d, ep) in &self.leap_seconds {
            if epoch >= *ep {
                delta = *d;
            } else {
                break;
            }
        }
        delta
    }

    /// Compute ET - UTC (delta ET) for a given epoch.
    /// Matches CSPICE's `deltet_` exactly.
    ///
    /// `delta_ET = delta_T_A + leap_seconds + K * sin(E)`
    ///
    /// The periodic term is evaluated at the nearest ET second to ensure
    /// reversibility (UTC→ET→UTC round-trips exactly).
    pub fn deltet(&self, epoch: f64, epoch_type: EpochType) -> f64 {
        // 1. Determine leap seconds count
        let mut leaps = self
            .leap_seconds
            .first()
            .map_or(0.0, |(d, _)| d - 1.0);

        match epoch_type {
            EpochType::Utc => {
                // UTC case: compare epoch directly against table epochs
                for &(delta, ep) in &self.leap_seconds {
                    if epoch >= ep {
                        leaps = delta;
                    } else {
                        break;
                    }
                }
            }
            EpochType::Et => {
                // ET case: compute the actual ET at each leap second boundary
                for &(delta, ep) in &self.leap_seconds {
                    if epoch > ep {
                        let aet = (ep + self.delta_t_a + delta).round();
                        let ma = self.m[0] + self.m[1] * aet;
                        let ea = ma + self.eb * ma.sin();
                        let ettai = self.k * ea.sin();
                        let et_boundary = ep + self.delta_t_a + delta + ettai;
                        if epoch >= et_boundary {
                            leaps = delta;
                        }
                    }
                }
            }
        }

        // 2. Compute periodic term at nearest ET second (for reversibility)
        let aet = match epoch_type {
            EpochType::Et => epoch.round(),
            EpochType::Utc => (epoch + self.delta_t_a + leaps).round(),
        };
        let ma = self.m[0] + self.m[1] * aet;
        let ea = ma + self.eb * ma.sin();
        let ettai = self.k * ea.sin();

        // delta_ET = delta_T_A + leap_seconds + K*sin(E)
        self.delta_t_a + leaps + ettai
    }

    /// Convert UTC seconds past J2000 to TDB.
    pub fn utc_to_tdb_seconds(&self, utc: f64) -> f64 {
        utc + self.deltet(utc, EpochType::Utc)
    }

    /// Convert TDB to UTC (as seconds past J2000).
    pub fn tdb_to_utc_seconds(&self, tdb: f64) -> f64 {
        tdb - self.deltet(tdb, EpochType::Et)
    }
}

/// Extension trait for extracting leap second data from kernels.
pub trait LeapSecondExt {
    /// Extract leap second data from loaded kernels.
    ///
    /// Returns `None` if no LSK data is available.
    fn lsk_data(&self) -> Option<LeapSecondData>;

    /// Check if leap second data is available.
    fn has_lsk(&self) -> bool;
}

impl LeapSecondExt for SpiceKernel {
    fn lsk_data(&self) -> Option<LeapSecondData> {
        // Get the required DELTET variables
        let delta_t_a = self.get_f64_scalar("DELTET/DELTA_T_A")?;
        let k = self.get_f64_scalar("DELTET/K")?;
        let eb = self.get_f64_scalar("DELTET/EB")?;
        let m_values = self.get_f64("DELTET/M")?;

        if m_values.len() < 2 {
            return None;
        }

        let m = [m_values[0], m_values[1]];

        // Parse DELTET/DELTA_AT - alternating (TAI-UTC, epoch) pairs
        let delta_at_var = self.pck_lookup("DELTET/DELTA_AT")?;

        let mut leap_seconds = Vec::new();
        let values = &delta_at_var.values;

        let mut i = 0;
        while i + 1 < values.len() {
            // Each pair is (numeric TAI-UTC value, epoch string)
            let delta = match &values[i] {
                KernelValue::Numeric(n) => *n,
                _ => {
                    i += 1;
                    continue;
                }
            };

            let epoch_str = match &values[i + 1] {
                KernelValue::Epoch(s) => s.clone(),
                KernelValue::Text(s) => s.clone(),
                _ => {
                    i += 2;
                    continue;
                }
            };

            // Parse the epoch string to TDB
            if let Ok(epoch) = parse_lsk_epoch(&epoch_str) {
                leap_seconds.push((delta, epoch));
            }

            i += 2;
        }

        // Sort by epoch
        leap_seconds.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        Some(LeapSecondData {
            delta_t_a,
            k,
            eb,
            m,
            leap_seconds,
        })
    }

    fn has_lsk(&self) -> bool {
        self.pool_has("DELTET/DELTA_T_A")
            && self.pool_has("DELTET/K")
            && self.pool_has("DELTET/DELTA_AT")
    }
}

/// Parse an LSK epoch string like "@1972-JAN-1" to TDB seconds past J2000.
fn parse_lsk_epoch(s: &str) -> Result<f64> {
    let trimmed = s.trim().trim_start_matches('@');

    // Parse as a calendar date (without time, assume midnight)
    let epoch_str = format!("{} 00:00:00", trimmed);
    let epoch = EpochTDB::parse(&epoch_str)?;

    Ok(epoch.0)
}

/// Convert a UTC time string to TDB.
///
/// # Arguments
///
/// * `kernel` - SpiceKernel with loaded LSK data
/// * `utc_str` - UTC time string in a supported format
///
/// # Returns
///
/// TDB epoch corresponding to the input UTC time.
///
/// # Errors
///
/// Returns an error if:
/// - No LSK data is loaded
/// - The time string cannot be parsed
///
/// # Example
///
/// ```ignore
/// let kernel = SpiceKernel::load("naif0012.tls")?;
/// let tdb = utc_to_tdb(&kernel, "2020-01-01T00:00:00")?;
/// ```
pub fn utc_to_tdb(kernel: &SpiceKernel, utc_str: &str) -> Result<EpochTDB> {
    let lsk = kernel.lsk_data().ok_or(Error::MissingLskData)?;

    // Parse the UTC string to get "raw" seconds (treating it as TDB for parsing)
    let utc_parsed = EpochTDB::parse(utc_str)?;

    // Convert to true TDB
    let tdb = lsk.utc_to_tdb_seconds(utc_parsed.0);

    Ok(EpochTDB(tdb))
}

/// Convert a TDB epoch to a UTC time string.
///
/// # Arguments
///
/// * `kernel` - SpiceKernel with loaded LSK data
/// * `tdb` - TDB epoch
/// * `format` - Output format (Iso8601 or Calendar)
///
/// # Returns
///
/// UTC time string in the requested format.
///
/// # Errors
///
/// Returns an error if no LSK data is loaded.
///
/// # Example
///
/// ```ignore
/// let kernel = SpiceKernel::load("naif0012.tls")?;
/// let utc = tdb_to_utc(&kernel, EpochTDB(0.0), TimeFormat::Iso8601)?;
/// // Returns approximately "2000-01-01T11:58:55" (J2000 in UTC)
/// ```
pub fn tdb_to_utc(kernel: &SpiceKernel, tdb: EpochTDB, format: TimeFormat) -> Result<String> {
    let lsk = kernel.lsk_data().ok_or(Error::MissingLskData)?;

    // Convert TDB to UTC seconds
    let utc_seconds = lsk.tdb_to_utc_seconds(tdb.0);

    // Format the result
    let formatted = match format {
        TimeFormat::Iso8601 => format_iso8601(utc_seconds),
        TimeFormat::Calendar => format_calendar(utc_seconds),
        TimeFormat::JulianDate => {
            let jd = utc_seconds / 86400.0 + 2451545.0;
            format!("JD {:.6}", jd)
        }
    };

    Ok(formatted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::CoverageIndex;
    use crate::text_pck::{PCKBlock, PCKSource, PCKVariable};

    /// Create a test kernel with minimal LSK data.
    fn make_lsk_kernel() -> SpiceKernel {
        let pck = PCKSource {
            filename: "test.tls".to_string(),
            blocks: vec![PCKBlock::Data(vec![
                PCKVariable {
                    name: "DELTET/DELTA_T_A".to_string(),
                    values: vec![KernelValue::Numeric(32.184)],
                },
                PCKVariable {
                    name: "DELTET/K".to_string(),
                    values: vec![KernelValue::Numeric(1.657e-3)],
                },
                PCKVariable {
                    name: "DELTET/EB".to_string(),
                    values: vec![KernelValue::Numeric(1.671e-2)],
                },
                PCKVariable {
                    name: "DELTET/M".to_string(),
                    values: vec![
                        KernelValue::Numeric(6.239996),
                        KernelValue::Numeric(1.99096871e-7),
                    ],
                },
                PCKVariable {
                    name: "DELTET/DELTA_AT".to_string(),
                    values: vec![
                        KernelValue::Numeric(10.0),
                        KernelValue::Epoch("@1972-JAN-1".to_string()),
                        KernelValue::Numeric(11.0),
                        KernelValue::Epoch("@1972-JUL-1".to_string()),
                        KernelValue::Numeric(37.0),
                        KernelValue::Epoch("@2017-JAN-1".to_string()),
                    ],
                },
            ])],
        };

        SpiceKernel {
            daf_sources: Vec::new(),
            pck_sources: vec![pck],
            coverage_index: CoverageIndex::new(),
        }
    }

    #[test]
    fn test_lsk_data_extraction() {
        let kernel = make_lsk_kernel();

        assert!(kernel.has_lsk());

        let lsk = kernel.lsk_data().unwrap();
        assert!((lsk.delta_t_a - 32.184).abs() < 1e-6);
        assert!((lsk.k - 1.657e-3).abs() < 1e-9);
        assert!((lsk.eb - 1.671e-2).abs() < 1e-9);
        assert_eq!(lsk.leap_seconds.len(), 3);
    }

    #[test]
    fn test_delta_at_lookup() {
        let kernel = make_lsk_kernel();
        let lsk = kernel.lsk_data().unwrap();

        // Before 1972: should return 0 (no leap seconds)
        let ancient = -1e10;
        assert_eq!(lsk.delta_at(ancient), 0.0);

        // After 2017: should return 37
        let recent = 1e9;
        assert_eq!(lsk.delta_at(recent), 37.0);
    }

    #[test]
    fn test_deltet_reversibility() {
        let kernel = make_lsk_kernel();
        let lsk = kernel.lsk_data().unwrap();

        // CSPICE guarantees: deltet(utc, UTC) == deltet(utc + delta, ET)
        for utc in &[0.0, 1e8, -5e8, 5e8] {
            let delta1 = lsk.deltet(*utc, EpochType::Utc);
            let delta2 = lsk.deltet(*utc + delta1, EpochType::Et);
            assert_eq!(
                delta1, delta2,
                "deltet reversibility failed for utc={}",
                utc
            );
        }
    }

    #[test]
    fn test_utc_tdb_round_trip() {
        let kernel = make_lsk_kernel();
        let lsk = kernel.lsk_data().unwrap();

        // Convert UTC=0 to TDB and back
        let utc_seconds = 0.0;
        let tdb = lsk.utc_to_tdb_seconds(utc_seconds);
        let utc_back = lsk.tdb_to_utc_seconds(tdb);

        assert!(
            (utc_seconds - utc_back).abs() < 1e-10,
            "Round-trip error: {} vs {} (diff={})",
            utc_seconds,
            utc_back,
            (utc_seconds - utc_back).abs()
        );
    }

    #[test]
    fn test_utc_to_tdb_function() {
        let kernel = make_lsk_kernel();

        // J2000 in UTC should give a TDB slightly earlier
        // (because TDB = TT = TAI + 32.184, and TAI = UTC + leap_seconds)
        let tdb = utc_to_tdb(&kernel, "2000-01-01T12:00:00").unwrap();

        // The offset should be roughly delta_t_a + leap_seconds ≈ 64 seconds
        // J2000 TDB is 2000-01-01T12:00:00 TDB, which is about 2000-01-01T11:58:55 UTC
        // So UTC 12:00:00 should map to TDB ~64 seconds later
        assert!(tdb.0 > 0.0, "TDB should be positive for UTC noon J2000");
        assert!(tdb.0 < 100.0, "TDB should be less than 100s past J2000");
    }

    #[test]
    fn test_tdb_to_utc_function() {
        let kernel = make_lsk_kernel();

        // J2000 TDB (0.0) should give a UTC string around 11:58:55
        let utc = tdb_to_utc(&kernel, EpochTDB(0.0), TimeFormat::Iso8601).unwrap();

        assert!(utc.starts_with("2000-01-01T"));
        // The hour should be 11 (not 12) due to leap seconds
    }

    #[test]
    fn test_parse_lsk_epoch() {
        // 1972-JAN-1 should be significantly before J2000
        let epoch = parse_lsk_epoch("@1972-JAN-1").unwrap();
        assert!(epoch < 0.0, "1972 should be before J2000");

        // 2017-JAN-1 should be after J2000
        let epoch2 = parse_lsk_epoch("@2017-JAN-1").unwrap();
        assert!(epoch2 > 0.0, "2017 should be after J2000");
    }

    #[test]
    fn test_missing_lsk_error() {
        let kernel = SpiceKernel::default();

        let result = utc_to_tdb(&kernel, "2000-01-01T12:00:00");
        assert!(matches!(result, Err(Error::MissingLskData)));
    }
}
