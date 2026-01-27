//! Extension traits for convenient iteration and filtering.
//!
//! These traits add query methods to existing types without modifying them.

use crate::hdf5_output::DAFSource;
use crate::types::NaifId;
use crate::{DAFSegment, SPKSegment, CKSegment, BPCKSegment};

/// Extension trait for iterators over DAFSegments.
pub trait DAFSegmentIteratorExt<'a>: Iterator<Item = &'a DAFSegment> + Sized {
    /// Filter to only SPK segments.
    fn spk_only(self) -> impl Iterator<Item = &'a SPKSegment> {
        self.filter_map(|seg| match seg {
            DAFSegment::SPK(spk) => Some(spk),
            _ => None,
        })
    }

    /// Filter to only CK segments.
    fn ck_only(self) -> impl Iterator<Item = &'a CKSegment> {
        self.filter_map(|seg| match seg {
            DAFSegment::CK(ck) => Some(ck),
            _ => None,
        })
    }

    /// Filter to only BPCK segments.
    fn bpck_only(self) -> impl Iterator<Item = &'a BPCKSegment> {
        self.filter_map(|seg| match seg {
            DAFSegment::BPCK(bpck) => Some(bpck),
            _ => None,
        })
    }
}

impl<'a, I: Iterator<Item = &'a DAFSegment>> DAFSegmentIteratorExt<'a> for I {}

/// Extension trait for iterators over SPKSegments.
pub trait SpkIteratorExt<'a>: Iterator<Item = &'a SPKSegment> + Sized {
    /// Filter to segments for a specific target body.
    fn for_target(self, target: NaifId) -> impl Iterator<Item = &'a SPKSegment> {
        self.filter(move |spk| spk.target_code == target.0)
    }

    /// Filter to segments for a specific center body.
    fn for_center(self, center: NaifId) -> impl Iterator<Item = &'a SPKSegment> {
        self.filter(move |spk| spk.center_code == center.0)
    }

    /// Filter to segments covering a specific epoch.
    fn covering_epoch(self, epoch: f64) -> impl Iterator<Item = &'a SPKSegment> {
        self.filter(move |spk| spk.initial_epoch <= epoch && epoch <= spk.final_epoch)
    }

    /// Filter to segments of a specific SPK type.
    fn of_type(self, spk_type: i32) -> impl Iterator<Item = &'a SPKSegment> {
        self.filter(move |spk| spk.spk_type == spk_type)
    }
}

impl<'a, I: Iterator<Item = &'a SPKSegment>> SpkIteratorExt<'a> for I {}

/// Extension trait for iterators over CKSegments.
pub trait CkIteratorExt<'a>: Iterator<Item = &'a CKSegment> + Sized {
    /// Filter to segments for a specific instrument.
    fn for_instrument(self, instrument: NaifId) -> impl Iterator<Item = &'a CKSegment> {
        self.filter(move |ck| ck.instrument_code == instrument.0)
    }

    /// Filter to segments covering a specific SCLK time.
    fn covering_sclk(self, sclk: f64) -> impl Iterator<Item = &'a CKSegment> {
        self.filter(move |ck| ck.initial_sclk <= sclk && sclk <= ck.final_sclk)
    }

    /// Filter to segments with angular velocity data.
    fn with_rates(self) -> impl Iterator<Item = &'a CKSegment> {
        self.filter(|ck| ck.rates)
    }

    /// Filter to segments of a specific CK type.
    fn of_type(self, ck_type: i32) -> impl Iterator<Item = &'a CKSegment> {
        self.filter(move |ck| ck.ck_type == ck_type)
    }
}

impl<'a, I: Iterator<Item = &'a CKSegment>> CkIteratorExt<'a> for I {}

/// Extension trait for DAFSource with query methods.
pub trait DAFSourceExt {
    /// Get all unique target body IDs from SPK segments.
    fn spk_body_ids(&self) -> Vec<NaifId>;

    /// Get all unique instrument IDs from CK segments.
    fn ck_instrument_ids(&self) -> Vec<NaifId>;

    /// Get all unique frame IDs from BPCK segments.
    fn bpck_frame_ids(&self) -> Vec<NaifId>;

    /// Count segments by type.
    fn segment_counts(&self) -> (usize, usize, usize);
}

impl DAFSourceExt for DAFSource {
    fn spk_body_ids(&self) -> Vec<NaifId> {
        let mut ids: Vec<NaifId> = self
            .segments
            .iter()
            .filter_map(|seg| match seg {
                DAFSegment::SPK(spk) => Some(NaifId(spk.target_code)),
                _ => None,
            })
            .collect();
        ids.sort_by_key(|n| n.0);
        ids.dedup();
        ids
    }

    fn ck_instrument_ids(&self) -> Vec<NaifId> {
        let mut ids: Vec<NaifId> = self
            .segments
            .iter()
            .filter_map(|seg| match seg {
                DAFSegment::CK(ck) => Some(NaifId(ck.instrument_code)),
                _ => None,
            })
            .collect();
        ids.sort_by_key(|n| n.0);
        ids.dedup();
        ids
    }

    fn bpck_frame_ids(&self) -> Vec<NaifId> {
        let mut ids: Vec<NaifId> = self
            .segments
            .iter()
            .filter_map(|seg| match seg {
                DAFSegment::BPCK(bpck) => Some(NaifId(bpck.frame_id)),
                _ => None,
            })
            .collect();
        ids.sort_by_key(|n| n.0);
        ids.dedup();
        ids
    }

    fn segment_counts(&self) -> (usize, usize, usize) {
        let mut spk = 0;
        let mut ck = 0;
        let mut bpck = 0;
        for seg in &self.segments {
            match seg {
                DAFSegment::SPK(_) => spk += 1,
                DAFSegment::CK(_) => ck += 1,
                DAFSegment::BPCK(_) => bpck += 1,
            }
        }
        (spk, ck, bpck)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DAFHeader;
    use crate::DAFMetadata;
    use crate::Endian;

    fn make_test_source() -> DAFSource {
        DAFSource {
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
            segments: vec![
                DAFSegment::SPK(SPKSegment {
                    name: "Earth".to_string(),
                    initial_epoch: 0.0,
                    final_epoch: 86400.0,
                    target_code: 399,
                    center_code: 3,
                    frame_code: 1,
                    spk_type: 2,
                    data_start: 1,
                    data_end: 10,
                    data: vec![],
                }),
                DAFSegment::SPK(SPKSegment {
                    name: "Mars".to_string(),
                    initial_epoch: 0.0,
                    final_epoch: 86400.0,
                    target_code: 499,
                    center_code: 4,
                    frame_code: 1,
                    spk_type: 2,
                    data_start: 11,
                    data_end: 20,
                    data: vec![],
                }),
            ],
        }
    }

    #[test]
    fn test_daf_source_ext() {
        let source = make_test_source();

        let ids = source.spk_body_ids();
        assert_eq!(ids, vec![NaifId(399), NaifId(499)]);

        let (spk, ck, bpck) = source.segment_counts();
        assert_eq!(spk, 2);
        assert_eq!(ck, 0);
        assert_eq!(bpck, 0);
    }

    #[test]
    fn test_spk_iterator_ext() {
        let source = make_test_source();

        let segs: Vec<_> = source.segments.iter().spk_only().for_target(NaifId(399)).collect();
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].name, "Earth");
    }
}
