//! From/Into implementations for SpiceKernel.
//!
//! These implementations allow convenient conversion from various source types
//! into a unified SpiceKernel.

use crate::hdf5_output::DAFSource;
use crate::text_pck::PCKSource;

use super::coverage::CoverageIndex;
use super::SpiceKernel;

impl From<DAFSource> for SpiceKernel {
    /// Create a SpiceKernel from a single DAFSource.
    fn from(source: DAFSource) -> Self {
        let coverage_index = CoverageIndex::from_daf_sources(std::slice::from_ref(&source));
        SpiceKernel {
            daf_sources: vec![source],
            pck_sources: vec![],
            coverage_index,
        }
    }
}

impl From<Vec<DAFSource>> for SpiceKernel {
    /// Create a SpiceKernel from multiple DAFSources.
    fn from(sources: Vec<DAFSource>) -> Self {
        let coverage_index = CoverageIndex::from_daf_sources(&sources);
        SpiceKernel {
            daf_sources: sources,
            pck_sources: vec![],
            coverage_index,
        }
    }
}

impl From<PCKSource> for SpiceKernel {
    /// Create a SpiceKernel from a single PCKSource.
    fn from(source: PCKSource) -> Self {
        SpiceKernel {
            daf_sources: vec![],
            pck_sources: vec![source],
            coverage_index: CoverageIndex::new(),
        }
    }
}

impl From<Vec<PCKSource>> for SpiceKernel {
    /// Create a SpiceKernel from multiple PCKSources.
    fn from(sources: Vec<PCKSource>) -> Self {
        SpiceKernel {
            daf_sources: vec![],
            pck_sources: sources,
            coverage_index: CoverageIndex::new(),
        }
    }
}

impl From<(Vec<DAFSource>, Vec<PCKSource>)> for SpiceKernel {
    /// Create a SpiceKernel from both DAF and PCK sources.
    fn from((daf_sources, pck_sources): (Vec<DAFSource>, Vec<PCKSource>)) -> Self {
        let coverage_index = CoverageIndex::from_daf_sources(&daf_sources);
        SpiceKernel {
            daf_sources,
            pck_sources,
            coverage_index,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DAFHeader, DAFMetadata, DAFSegment, Endian, SPKSegment};

    fn make_test_daf_source() -> DAFSource {
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
            segments: vec![DAFSegment::SPK(SPKSegment {
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
            })],
        }
    }

    fn make_test_pck_source() -> PCKSource {
        use crate::text_pck::{KernelValue, PCKBlock, PCKVariable};

        PCKSource {
            filename: "test.tpc".to_string(),
            blocks: vec![PCKBlock::Data(vec![PCKVariable {
                name: "BODY399_RADII".to_string(),
                values: vec![
                    KernelValue::Numeric(6378.14),
                    KernelValue::Numeric(6378.14),
                    KernelValue::Numeric(6356.75),
                ],
            }])],
        }
    }

    #[test]
    fn test_from_daf_source() {
        let source = make_test_daf_source();
        let kernel: SpiceKernel = source.into();

        assert_eq!(kernel.daf_sources.len(), 1);
        assert!(kernel.pck_sources.is_empty());
        assert_eq!(kernel.spk_bodies().len(), 1);
    }

    #[test]
    fn test_from_pck_source() {
        let source = make_test_pck_source();
        let kernel: SpiceKernel = source.into();

        assert!(kernel.daf_sources.is_empty());
        assert_eq!(kernel.pck_sources.len(), 1);
        assert!(kernel.pck_lookup("BODY399_RADII").is_some());
    }

    #[test]
    fn test_from_tuple() {
        let daf_sources = vec![make_test_daf_source()];
        let pck_sources = vec![make_test_pck_source()];

        let kernel: SpiceKernel = (daf_sources, pck_sources).into();

        assert_eq!(kernel.daf_sources.len(), 1);
        assert_eq!(kernel.pck_sources.len(), 1);
    }
}
