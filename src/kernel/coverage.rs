//! Coverage index for fast body/instrument lookups.
//!
//! This module provides efficient indexing of coverage intervals from
//! loaded SPICE kernels, enabling fast queries like "what bodies are covered?"
//! or "what's the coverage for body X?"

use crate::brief::CoverageInterval;
use crate::hdf5_output::DAFSource;
use crate::types::NaifId;
use crate::DAFSegment;
use std::collections::HashMap;

/// Coverage index for fast lookups by NAIF ID.
///
/// This struct builds sorted indexes of coverage intervals from loaded kernels,
/// enabling O(1) lookups by body ID.
#[derive(Debug, Default)]
pub struct CoverageIndex {
    /// SPK body ID -> coverage intervals
    spk_bodies: HashMap<NaifId, Vec<CoverageInterval>>,
    /// CK instrument ID -> coverage intervals
    ck_instruments: HashMap<NaifId, Vec<CoverageInterval>>,
    /// BPCK frame ID -> coverage intervals
    bpck_frames: HashMap<NaifId, Vec<CoverageInterval>>,
}

impl CoverageIndex {
    /// Create a new empty coverage index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build coverage index from DAF sources.
    pub fn from_daf_sources(sources: &[DAFSource]) -> Self {
        let mut index = Self::new();

        for source in sources {
            for segment in &source.segments {
                index.add_segment(segment);
            }
        }

        index
    }

    /// Add a single segment to the index.
    pub fn add_segment(&mut self, segment: &DAFSegment) {
        match segment {
            DAFSegment::SPK(spk) => {
                let interval = CoverageInterval {
                    start: spk.initial_epoch,
                    end: spk.final_epoch,
                    ck_type: None,
                    has_rates: None,
                };
                self.spk_bodies
                    .entry(NaifId(spk.target_code))
                    .or_default()
                    .push(interval);
            }
            DAFSegment::CK(ck) => {
                let interval = CoverageInterval {
                    start: ck.initial_sclk,
                    end: ck.final_sclk,
                    ck_type: Some(ck.ck_type),
                    has_rates: Some(ck.rates),
                };
                self.ck_instruments
                    .entry(NaifId(ck.instrument_code))
                    .or_default()
                    .push(interval);
            }
            DAFSegment::BPCK(bpck) => {
                let interval = CoverageInterval {
                    start: bpck.initial_epoch,
                    end: bpck.final_epoch,
                    ck_type: None,
                    has_rates: None,
                };
                self.bpck_frames
                    .entry(NaifId(bpck.frame_id))
                    .or_default()
                    .push(interval);
            }
        }
    }

    /// Get all SPK body IDs.
    pub fn spk_bodies(&self) -> Vec<NaifId> {
        let mut ids: Vec<_> = self.spk_bodies.keys().copied().collect();
        ids.sort_by_key(|n| n.0);
        ids
    }

    /// Get all CK instrument IDs.
    pub fn ck_instruments(&self) -> Vec<NaifId> {
        let mut ids: Vec<_> = self.ck_instruments.keys().copied().collect();
        ids.sort_by_key(|n| n.0);
        ids
    }

    /// Get all BPCK frame IDs.
    pub fn bpck_frames(&self) -> Vec<NaifId> {
        let mut ids: Vec<_> = self.bpck_frames.keys().copied().collect();
        ids.sort_by_key(|n| n.0);
        ids
    }

    /// Get SPK coverage intervals for a body.
    pub fn spk_coverage(&self, body: NaifId) -> Option<&[CoverageInterval]> {
        self.spk_bodies.get(&body).map(|v| v.as_slice())
    }

    /// Get CK coverage intervals for an instrument.
    pub fn ck_coverage(&self, instrument: NaifId) -> Option<&[CoverageInterval]> {
        self.ck_instruments.get(&instrument).map(|v| v.as_slice())
    }

    /// Get BPCK coverage intervals for a frame.
    pub fn bpck_coverage(&self, frame: NaifId) -> Option<&[CoverageInterval]> {
        self.bpck_frames.get(&frame).map(|v| v.as_slice())
    }

    /// Check if a body has SPK coverage at a given epoch (TDB seconds past J2000).
    pub fn spk_has_coverage(&self, body: NaifId, epoch: f64) -> bool {
        self.spk_bodies
            .get(&body)
            .map(|intervals| intervals.iter().any(|i| i.start <= epoch && epoch <= i.end))
            .unwrap_or(false)
    }

    /// Check if an instrument has CK coverage at a given SCLK tick.
    pub fn ck_has_coverage(&self, instrument: NaifId, sclk: f64) -> bool {
        self.ck_instruments
            .get(&instrument)
            .map(|intervals| intervals.iter().any(|i| i.start <= sclk && sclk <= i.end))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SPKSegment, CKSegment};

    #[test]
    fn test_coverage_index_empty() {
        let index = CoverageIndex::new();
        assert!(index.spk_bodies().is_empty());
        assert!(index.ck_instruments().is_empty());
        assert!(index.bpck_frames().is_empty());
    }

    #[test]
    fn test_coverage_index_add_spk() {
        let mut index = CoverageIndex::new();

        let spk = SPKSegment {
            name: "test".to_string(),
            initial_epoch: 0.0,
            final_epoch: 86400.0,
            target_code: 399,
            center_code: 3,
            frame_code: 1,
            spk_type: 2,
            data_start: 1,
            data_end: 10,
            data: vec![],
        };

        index.add_segment(&DAFSegment::SPK(spk));

        assert_eq!(index.spk_bodies(), vec![NaifId(399)]);
        assert!(index.spk_coverage(NaifId(399)).is_some());
        assert!(index.spk_has_coverage(NaifId(399), 1000.0));
        assert!(!index.spk_has_coverage(NaifId(399), -1000.0));
    }

    #[test]
    fn test_coverage_index_add_ck() {
        let mut index = CoverageIndex::new();

        let ck = CKSegment {
            name: "test".to_string(),
            initial_sclk: 1000.0,
            final_sclk: 2000.0,
            instrument_code: -82000,
            frame_code: 1,
            ck_type: 3,
            rates: true,
            data_start: 1,
            data_end: 10,
            data: vec![],
        };

        index.add_segment(&DAFSegment::CK(ck));

        assert_eq!(index.ck_instruments(), vec![NaifId(-82000)]);
        assert!(index.ck_coverage(NaifId(-82000)).is_some());
        assert!(index.ck_has_coverage(NaifId(-82000), 1500.0));
        assert!(!index.ck_has_coverage(NaifId(-82000), 500.0));
    }
}
