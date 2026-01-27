//! Unified API for loading and querying SPICE kernel data.
//!
//! The `SpiceKernel` type provides a single entry point for working with
//! SPICE data from any format (SPK, CK, BPCK, text PCK, HDF5).
//!
//! # Basic Usage
//!
//! ```ignore
//! use muad_dib::kernel::SpiceKernel;
//! use muad_dib::types::NaifId;
//!
//! // Load a single file
//! let kernel = SpiceKernel::load("de430.bsp")?;
//!
//! // Query bodies with coverage
//! for body in kernel.spk_bodies() {
//!     println!("Body {}: {:?}", body.0, kernel.spk_coverage(body));
//! }
//! ```
//!
//! # Builder Pattern
//!
//! ```ignore
//! // Load multiple files of different types
//! let kernel = SpiceKernel::builder()
//!     .file("de430.bsp")
//!     .file("pck00010.tpc")
//!     .build()?;
//!
//! // PCK variable lookup
//! if let Some(var) = kernel.pck_lookup("BODY399_RADII") {
//!     println!("Earth radii: {:?}", var.values);
//! }
//! ```
//!
//! # Type-Specific Data Access
//!
//! SPK and CK segments contain type-specific data that can be parsed lazily:
//!
//! ```ignore
//! use muad_dib::kernel::SpiceKernel;
//! use muad_dib::types::NaifId;
//!
//! let kernel = SpiceKernel::load("de430.bsp")?;
//!
//! // Get views with lazy type-specific parsing
//! for view in kernel.spk_views_for(NaifId::EARTH) {
//!     println!("Segment: {} (Type {})", view.name(), view.spk_type());
//!
//!     // Parse Chebyshev coefficients for Type 2 segments
//!     if let Some(type2) = view.data().as_type2() {
//!         println!("  Chebyshev degree: {}", type2.degree);
//!         println!("  Records: {}", type2.records.len());
//!     }
//! }
//! ```
//!
//! # Supported SPK Types
//!
//! - **Type 2**: Chebyshev polynomials for position (most common)
//! - **Type 3**: Chebyshev polynomials for position and velocity
//! - **Type 5**: Discrete states with two-body propagation (GM)
//! - **Type 8**: Lagrange interpolation (equal time steps)
//! - **Type 9**: Lagrange interpolation (unequal time steps)
//! - **Type 13**: Hermite interpolation (unequal time steps)
//!
//! # Supported CK Types
//!
//! - **Type 1**: Discrete pointing instances (quaternions)
//! - **Type 3**: Linear interpolation between pointing instances

mod builder;
pub mod ck;
pub mod ck_parse;
pub mod ck_types;
mod convert;
mod coverage;
pub mod ext;
pub mod spk;
pub mod spk_parse;
pub mod spk_types;

pub use builder::SpiceKernelBuilder;
pub use ck::CkSegmentView;
pub use ck_types::{Ck1Data, Ck3Data, CkData, PointingRecord};
pub use coverage::CoverageIndex;
pub use ext::{CkIteratorExt, DAFSegmentIteratorExt, DAFSourceExt, SpkIteratorExt};
pub use spk::SpkSegmentView;
pub use spk_types::{
    ChebyshevRecord, ChebyshevRecordWithVelocity, Spk13Data, Spk2Data, Spk3Data, Spk5Data,
    Spk8Data, Spk9Data, SpkData, StateRecord,
};

use crate::brief::CoverageInterval;
use crate::hdf5_output::DAFSource;
use crate::prelude::*;
use crate::text_pck::{PCKSource, PCKVariable};
use crate::types::NaifId;
use crate::{BPCKSegment, CKSegment, DAFSegment, SPKSegment};
use std::path::Path;

/// Unified entry point for loading and querying SPICE kernel data.
///
/// `SpiceKernel` wraps loaded DAF sources (SPK/CK/BPCK) and text PCK sources,
/// providing query methods for bodies, coverage intervals, and PCK variables.
#[derive(Debug)]
pub struct SpiceKernel {
    pub(crate) daf_sources: Vec<DAFSource>,
    pub(crate) pck_sources: Vec<PCKSource>,
    pub(crate) coverage_index: CoverageIndex,
}

impl SpiceKernel {
    /// Load a single file of any supported format.
    ///
    /// Automatically detects file type by extension:
    /// - `.bsp`, `.spk` - SPK ephemeris
    /// - `.bc`, `.ck` - CK pointing
    /// - `.bpc`, `.bpck` - Binary PCK
    /// - `.tpc`, `.pck` - Text PCK
    /// - `.hdf5`, `.h5` - HDF5 (may contain multiple types)
    #[must_use = "loading a SpiceKernel returns a Result that should be handled"]
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        SpiceKernelBuilder::new().file(path).build()
    }

    /// Load multiple files of any supported format.
    #[must_use = "loading a SpiceKernel returns a Result that should be handled"]
    pub fn load_many<P: AsRef<Path>>(paths: &[P]) -> Result<Self> {
        let mut builder = SpiceKernelBuilder::new();
        for path in paths {
            builder = builder.file(path);
        }
        builder.build()
    }

    /// Create a builder for loading files.
    pub fn builder() -> SpiceKernelBuilder {
        SpiceKernelBuilder::new()
    }

    // ========== SPK Queries ==========

    /// Get all body IDs with SPK coverage.
    pub fn spk_bodies(&self) -> Vec<NaifId> {
        self.coverage_index.spk_bodies()
    }

    /// Get SPK coverage intervals for a body.
    pub fn spk_coverage(&self, body: NaifId) -> Option<&[CoverageInterval]> {
        self.coverage_index.spk_coverage(body)
    }

    /// Check if a body has SPK coverage at a given epoch (TDB seconds past J2000).
    pub fn spk_has_coverage(&self, body: NaifId, epoch: f64) -> bool {
        self.coverage_index.spk_has_coverage(body, epoch)
    }

    /// Iterate over all SPK segments.
    pub fn spk_segments(&self) -> impl Iterator<Item = &SPKSegment> {
        self.daf_sources
            .iter()
            .flat_map(|s| s.segments.iter())
            .filter_map(|seg| match seg {
                DAFSegment::SPK(spk) => Some(spk),
                _ => None,
            })
    }

    /// Iterate over SPK segments for a specific body.
    pub fn spk_segments_for(&self, body: NaifId) -> impl Iterator<Item = &SPKSegment> {
        self.spk_segments()
            .filter(move |spk| spk.target_code == body.0)
    }

    /// Get an SPK segment view with lazy type-specific data parsing.
    ///
    /// The returned `SpkSegmentView` provides access to type-specific data
    /// structures (e.g., Chebyshev coefficients for Type 2) that are parsed
    /// on first access.
    pub fn spk_view<'a>(&'a self, segment: &'a SPKSegment) -> SpkSegmentView<'a> {
        SpkSegmentView::new(segment)
    }

    /// Iterate over SPK segment views for a specific body.
    ///
    /// Each view provides lazy access to type-specific parsed data.
    pub fn spk_views_for(&self, body: NaifId) -> impl Iterator<Item = SpkSegmentView<'_>> {
        self.spk_segments_for(body).map(SpkSegmentView::new)
    }

    // ========== CK Queries ==========

    /// Get all instrument IDs with CK coverage.
    pub fn ck_instruments(&self) -> Vec<NaifId> {
        self.coverage_index.ck_instruments()
    }

    /// Get CK coverage intervals for an instrument.
    pub fn ck_coverage(&self, instrument: NaifId) -> Option<&[CoverageInterval]> {
        self.coverage_index.ck_coverage(instrument)
    }

    /// Check if an instrument has CK coverage at a given SCLK tick.
    pub fn ck_has_coverage(&self, instrument: NaifId, sclk: f64) -> bool {
        self.coverage_index.ck_has_coverage(instrument, sclk)
    }

    /// Iterate over all CK segments.
    pub fn ck_segments(&self) -> impl Iterator<Item = &CKSegment> {
        self.daf_sources
            .iter()
            .flat_map(|s| s.segments.iter())
            .filter_map(|seg| match seg {
                DAFSegment::CK(ck) => Some(ck),
                _ => None,
            })
    }

    /// Iterate over CK segments for a specific instrument.
    pub fn ck_segments_for(&self, instrument: NaifId) -> impl Iterator<Item = &CKSegment> {
        self.ck_segments()
            .filter(move |ck| ck.instrument_code == instrument.0)
    }

    /// Get a CK segment view with lazy type-specific data parsing.
    ///
    /// The returned `CkSegmentView` provides access to type-specific data
    /// structures (e.g., quaternions for Type 1) that are parsed on first access.
    pub fn ck_view<'a>(&'a self, segment: &'a CKSegment) -> CkSegmentView<'a> {
        CkSegmentView::new(segment)
    }

    /// Iterate over CK segment views for a specific instrument.
    ///
    /// Each view provides lazy access to type-specific parsed data.
    pub fn ck_views_for(&self, instrument: NaifId) -> impl Iterator<Item = CkSegmentView<'_>> {
        self.ck_segments_for(instrument).map(CkSegmentView::new)
    }

    // ========== BPCK Queries ==========

    /// Get all frame IDs with BPCK coverage.
    pub fn bpck_frames(&self) -> Vec<NaifId> {
        self.coverage_index.bpck_frames()
    }

    /// Get BPCK coverage intervals for a frame.
    pub fn bpck_coverage(&self, frame: NaifId) -> Option<&[CoverageInterval]> {
        self.coverage_index.bpck_coverage(frame)
    }

    /// Iterate over all BPCK segments.
    pub fn bpck_segments(&self) -> impl Iterator<Item = &BPCKSegment> {
        self.daf_sources
            .iter()
            .flat_map(|s| s.segments.iter())
            .filter_map(|seg| match seg {
                DAFSegment::BPCK(bpck) => Some(bpck),
                _ => None,
            })
    }

    // ========== PCK Queries ==========

    /// Get all PCK sources.
    pub fn pck_sources(&self) -> &[PCKSource] {
        &self.pck_sources
    }

    /// Lookup a PCK variable by name (case-insensitive).
    pub fn pck_lookup(&self, name: &str) -> Option<&PCKVariable> {
        let name_upper = name.to_uppercase();
        self.pck_sources
            .iter()
            .flat_map(|src| src.variables())
            .find(|v| v.name == name_upper)
    }

    /// Get all PCK variables for a specific body ID.
    pub fn pck_variables_for_body(&self, body_id: i32) -> Vec<&PCKVariable> {
        let prefix = format!("BODY{}_", body_id);
        self.pck_sources
            .iter()
            .flat_map(|src| src.variables())
            .filter(|v| v.name.starts_with(&prefix))
            .collect()
    }

    /// Get all unique body IDs from PCK variables.
    pub fn pck_body_ids(&self) -> Vec<i32> {
        let mut ids: Vec<i32> = self
            .pck_sources
            .iter()
            .flat_map(|src| src.body_ids())
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }

    // ========== General Queries ==========

    /// Get all DAF sources (SPK, CK, BPCK).
    pub fn daf_sources(&self) -> &[DAFSource] {
        &self.daf_sources
    }

    /// Get the total number of segments across all DAF sources.
    pub fn segment_count(&self) -> usize {
        self.daf_sources.iter().map(|s| s.segments.len()).sum()
    }

    /// Check if the kernel is empty (no loaded data).
    pub fn is_empty(&self) -> bool {
        self.daf_sources.is_empty() && self.pck_sources.is_empty()
    }
}

impl Default for SpiceKernel {
    fn default() -> Self {
        SpiceKernel {
            daf_sources: Vec::new(),
            pck_sources: Vec::new(),
            coverage_index: CoverageIndex::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_kernel() {
        let kernel = SpiceKernel::default();
        assert!(kernel.is_empty());
        assert_eq!(kernel.segment_count(), 0);
        assert!(kernel.spk_bodies().is_empty());
        assert!(kernel.ck_instruments().is_empty());
    }

    #[test]
    fn test_builder_returns_kernel() {
        let kernel = SpiceKernel::builder().build().unwrap();
        assert!(kernel.is_empty());
    }

    #[test]
    #[cfg(feature = "test-data")]
    fn test_doc_example_basic_usage() {
        // Mirrors the "Basic Usage" doc example
        let kernel = SpiceKernel::load("test_data/test.bsp").unwrap();

        // Query bodies with coverage
        for body in kernel.spk_bodies() {
            let _coverage = kernel.spk_coverage(body);
        }
    }

    #[test]
    #[cfg(feature = "test-data")]
    fn test_doc_example_type_specific_access() {
        // Mirrors the "Type-Specific Data Access" doc example
        let kernel = SpiceKernel::load("test_data/test.bsp").unwrap();

        for body in kernel.spk_bodies() {
            for view in kernel.spk_views_for(body) {
                let _name = view.name();
                let _spk_type = view.spk_type();

                // Parse type-specific data
                if let Some(type2) = view.data().as_type2() {
                    let _degree = type2.degree;
                    let _records = &type2.records;
                }
            }
        }
    }
}
