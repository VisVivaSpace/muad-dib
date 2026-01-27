//! High-level SPK state evaluation API.
//!
//! This module provides `state_at()` and `state_of()` methods for querying
//! ephemeris data, equivalent to CSPICE's `spkez()` function.
//!
//! # Example
//!
//! ```ignore
//! use muad_dib::kernel::SpiceKernel;
//! use muad_dib::spice::SpkInterpolateExt;
//! use muad_dib::types::{EpochTDB, NaifId};
//!
//! let kernel = SpiceKernel::load("de440.bsp")?;
//!
//! // Get Earth's state relative to SSB
//! let epoch = EpochTDB::parse("2020-01-01T00:00:00")?;
//! let state = kernel.state_of(NaifId::EARTH, epoch, NaifId::SSB)?;
//! println!("Earth position: {:?} km", state.position);
//! ```

use crate::error::Error;
use crate::kernel::spk_types::SpkData;
use crate::kernel::{SpiceKernel, SpkSegmentView};
use crate::prelude::*;
use crate::spice::interpolate::{chebyshev, hermite, lagrange, twobody, State};
use crate::types::{EpochTDB, NaifId};
use crate::SPKSegment;

/// Extension trait for SPK segment views with interpolation.
pub trait SpkSegmentViewInterpolate {
    /// Evaluate state (position + velocity) at the given epoch.
    ///
    /// # Arguments
    ///
    /// * `epoch` - TDB seconds past J2000
    ///
    /// # Returns
    ///
    /// State vector (position and velocity) in the segment's reference frame.
    fn state_at(&self, epoch: EpochTDB) -> Result<State>;
}

impl SpkSegmentViewInterpolate for SpkSegmentView<'_> {
    fn state_at(&self, epoch: EpochTDB) -> Result<State> {
        let data = self.data();

        // Compute raw position/velocity from interpolation
        let mut state = match data {
            SpkData::Type2(d) => chebyshev::evaluate_type2(d, epoch.0),
            SpkData::Type3(d) => chebyshev::evaluate_type3(d, epoch.0),
            SpkData::Type5(d) => twobody::evaluate_type5(d, epoch.0),
            SpkData::Type8(d) => lagrange::evaluate_type8(d, epoch.0),
            SpkData::Type9(d) => lagrange::evaluate_type9(d, epoch.0),
            SpkData::Type13(d) => hermite::evaluate_type13(d, epoch.0),
            SpkData::Raw { spk_type, .. } => Err(Error::UnsupportedSpkType { spk_type: *spk_type }),
        }?;

        // Add relativity context from the segment
        state.target = self.target();
        state.center = self.center();
        state.frame = self.frame();

        Ok(state)
    }
}

/// Extension trait for SPK state evaluation on SpiceKernel.
pub trait SpkInterpolateExt {
    /// Get state of target relative to center at the given epoch.
    ///
    /// Searches loaded SPK data for segments providing coverage and
    /// evaluates the appropriate interpolation algorithm.
    ///
    /// # Arguments
    ///
    /// * `target` - NAIF ID of target body
    /// * `epoch` - TDB epoch
    /// * `center` - NAIF ID of center body (origin for state vector)
    ///
    /// # Returns
    ///
    /// State vector (position and velocity) of target relative to center.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No coverage exists for the target at the requested epoch
    /// - The segment type is not supported for interpolation
    fn state_of(&self, target: NaifId, epoch: EpochTDB, center: NaifId) -> Result<State>;

    /// Evaluate a specific SPK segment at the given epoch.
    ///
    /// Lower-level API that directly evaluates a segment without searching.
    fn evaluate_segment(&self, segment: &SPKSegment, epoch: EpochTDB) -> Result<State>;
}

impl SpkInterpolateExt for SpiceKernel {
    fn state_of(&self, target: NaifId, epoch: EpochTDB, center: NaifId) -> Result<State> {
        // Find a segment for the target that covers this epoch
        let segment = self
            .spk_segments_for(target)
            .find(|seg| seg.initial_epoch <= epoch.0 && epoch.0 <= seg.final_epoch)
            .ok_or(Error::NoCoverage {
                body: target,
                epoch: epoch.0,
            })?;

        // Evaluate the segment
        let state = self.evaluate_segment(segment, epoch)?;

        // If the center matches the segment's center, we're done
        if segment.center_code == center.0 {
            return Ok(state);
        }

        // If target IS the center, return negated state
        if segment.target_code == center.0 {
            return Ok(-state);
        }

        // Otherwise, we need to chain: target→seg_center→center
        // First, get state of segment center relative to the requested center
        let center_state = self.state_of(NaifId(segment.center_code), epoch, center)?;

        // state_target_center = state_target_seg_center + state_seg_center_center
        Ok(state + center_state)
    }

    fn evaluate_segment(&self, segment: &SPKSegment, epoch: EpochTDB) -> Result<State> {
        let view = self.spk_view(segment);
        view.state_at(epoch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::CoverageIndex;
    use crate::{DAFHeader, DAFMetadata, DAFSegment, Endian, SPKSegment};
    use crate::hdf5_output::DAFSource;

    #[allow(dead_code)]
    fn make_test_kernel() -> SpiceKernel {
        // Create a simple Type 2 SPK segment
        let segment = SPKSegment {
            name: "Test".to_string(),
            initial_epoch: 0.0,
            final_epoch: 100.0,
            target_code: 399, // Earth
            center_code: 0,   // SSB
            frame_code: 1,
            spk_type: 2,
            data_start: 1,
            data_end: 100,
            data: vec![], // Will be parsed from here, but we use spk_types
        };

        let daf_source = DAFSource {
            filename: "test.bsp".to_string(),
            header: DAFHeader {
                name: "Test".to_string(),
                comment: "".to_string(),
                kind: "SPK".to_string(),
            },
            metadata: DAFMetadata {
                nd: 2,
                ni: 6,
                endian: Endian::Little,
                fward: 2,
                bward: 2,
                free_address: 100,
                ftpstr: "".to_string(),
            },
            segments: vec![DAFSegment::SPK(segment)],
        };

        let coverage = CoverageIndex::from_daf_sources(&[daf_source.clone()]);

        SpiceKernel {
            daf_sources: vec![daf_source],
            pck_sources: vec![],
            coverage_index: coverage,
        }
    }

    #[test]
    fn test_state_of_no_coverage() {
        let kernel = SpiceKernel::default();

        let result = kernel.state_of(NaifId::EARTH, EpochTDB(0.0), NaifId::SSB);
        assert!(matches!(result, Err(Error::NoCoverage { .. })));
    }

    #[test]
    fn test_state_add() {
        // Chain traversal: SSB→Earth + Earth→Moon = SSB→Moon
        let ssb_to_earth = State::new(NaifId::EARTH, NaifId::SSB, 1, [1.0, 2.0, 3.0], [0.1, 0.2, 0.3]);
        let earth_to_moon = State::new(NaifId::MOON, NaifId::EARTH, 1, [4.0, 5.0, 6.0], [0.4, 0.5, 0.6]);
        let sum = ssb_to_earth + earth_to_moon;

        assert!((sum.position[0] - 5.0).abs() < 1e-10);
        assert!((sum.position[1] - 7.0).abs() < 1e-10);
        assert!((sum.position[2] - 9.0).abs() < 1e-10);
        assert!((sum.velocity[0] - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_state_negate() {
        // Negation: -(SSB→Earth) = Earth→SSB
        let s = State::new(NaifId::EARTH, NaifId::SSB, 1, [1.0, -2.0, 3.0], [-0.1, 0.2, -0.3]);
        let neg = -s;

        assert!((neg.position[0] + 1.0).abs() < 1e-10);
        assert!((neg.position[1] - 2.0).abs() < 1e-10);
        assert!((neg.velocity[2] - 0.3).abs() < 1e-10);

        // Check metadata swap
        assert_eq!(neg.target, NaifId::SSB);
        assert_eq!(neg.center, NaifId::EARTH);
    }
}
