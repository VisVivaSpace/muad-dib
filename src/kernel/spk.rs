//! SPK segment view with lazy type-specific parsing.
//!
//! This module provides `SpkSegmentView` which wraps an `SPKSegment` and
//! adds lazy parsing of type-specific data on first access.

use std::cell::OnceCell;

use super::spk_parse::parse_spk_data;
use super::spk_types::SpkData;
use crate::types::NaifId;
use crate::SPKSegment;

/// A view over an SPK segment with lazy type-specific data parsing.
///
/// The underlying segment data is parsed into a type-specific structure
/// (e.g., `Spk2Data`, `Spk5Data`) on first access, and cached for subsequent
/// accesses.
///
/// # Example
///
/// ```ignore
/// use despice::kernel::spk::SpkSegmentView;
///
/// let view = SpkSegmentView::new(&segment);
///
/// // Access metadata immediately
/// println!("Target: {}", view.target());
/// println!("Coverage: {} to {}", view.initial_epoch(), view.final_epoch());
///
/// // Parse type-specific data on first access
/// if let Some(type2) = view.data().as_type2() {
///     println!("Chebyshev degree: {}", type2.degree);
/// }
/// ```
pub struct SpkSegmentView<'a> {
    segment: &'a SPKSegment,
    parsed_data: OnceCell<SpkData>,
}

impl<'a> SpkSegmentView<'a> {
    /// Create a new view over an SPK segment.
    pub fn new(segment: &'a SPKSegment) -> Self {
        Self {
            segment,
            parsed_data: OnceCell::new(),
        }
    }

    /// Get the underlying segment reference.
    pub fn segment(&self) -> &SPKSegment {
        self.segment
    }

    /// Get the segment name.
    pub fn name(&self) -> &str {
        &self.segment.name
    }

    /// Get the target body NAIF ID.
    pub fn target(&self) -> NaifId {
        self.segment.target_code
    }

    /// Get the center body NAIF ID.
    pub fn center(&self) -> NaifId {
        self.segment.center_code
    }

    /// Get the reference frame NAIF ID.
    pub fn frame(&self) -> NaifId {
        self.segment.frame_code
    }

    /// Get the SPK segment type (1-21).
    pub fn spk_type(&self) -> i32 {
        self.segment.spk_type
    }

    /// Get the initial epoch (TDB seconds past J2000).
    pub fn initial_epoch(&self) -> f64 {
        self.segment.initial_epoch
    }

    /// Get the final epoch (TDB seconds past J2000).
    pub fn final_epoch(&self) -> f64 {
        self.segment.final_epoch
    }

    /// Check if an epoch is within this segment's coverage.
    pub fn covers_epoch(&self, epoch: f64) -> bool {
        self.segment.initial_epoch <= epoch && epoch <= self.segment.final_epoch
    }

    /// Get the parsed type-specific data.
    ///
    /// The data is parsed on first access and cached.
    pub fn data(&self) -> &SpkData {
        self.parsed_data
            .get_or_init(|| parse_spk_data(self.segment.spk_type, self.segment.data.clone()))
    }

    /// Get the raw data without parsing.
    pub fn raw_data(&self) -> &[f64] {
        &self.segment.data
    }
}

impl<'a> std::fmt::Debug for SpkSegmentView<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpkSegmentView")
            .field("name", &self.segment.name)
            .field("target", &self.segment.target_code)
            .field("center", &self.segment.center_code)
            .field("spk_type", &self.segment.spk_type)
            .field("initial_epoch", &self.segment.initial_epoch)
            .field("final_epoch", &self.segment.final_epoch)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_segment() -> SPKSegment {
        // Create a minimal Type 2 segment
        SPKSegment {
            name: "TEST SEGMENT".to_string(),
            initial_epoch: 0.0,
            final_epoch: 86400.0,
            target_code: NaifId(399),
            center_code: NaifId(3),
            frame_code: NaifId(1),
            spk_type: 2,
            data_start: 1,
            data_end: 12,
            data: vec![
                // Record 1 (degree 1, 2 coefficients per axis)
                100.0, // MID
                50.0,  // RADIUS
                1.0, 2.0, // X coefficients
                3.0, 4.0, // Y coefficients
                5.0, 6.0, // Z coefficients
                // Directory
                0.0,   // INIT
                100.0, // INTLEN
                8.0,   // RSIZE
                1.0,   // N
            ],
        }
    }

    #[test]
    fn test_view_metadata() {
        let segment = make_test_segment();
        let view = SpkSegmentView::new(&segment);

        assert_eq!(view.name(), "TEST SEGMENT");
        assert_eq!(view.target(), NaifId(399));
        assert_eq!(view.center(), NaifId(3));
        assert_eq!(view.frame(), NaifId(1));
        assert_eq!(view.spk_type(), 2);
        assert_eq!(view.initial_epoch(), 0.0);
        assert_eq!(view.final_epoch(), 86400.0);
    }

    #[test]
    fn test_view_covers_epoch() {
        let segment = make_test_segment();
        let view = SpkSegmentView::new(&segment);

        assert!(view.covers_epoch(0.0));
        assert!(view.covers_epoch(43200.0));
        assert!(view.covers_epoch(86400.0));
        assert!(!view.covers_epoch(-1.0));
        assert!(!view.covers_epoch(86401.0));
    }

    #[test]
    fn test_view_lazy_parsing() {
        let segment = make_test_segment();
        let view = SpkSegmentView::new(&segment);

        // First access parses the data
        let data = view.data();
        assert_eq!(data.spk_type(), 2);

        // Second access returns cached data
        let data2 = view.data();
        assert_eq!(data2.spk_type(), 2);
    }

    #[test]
    fn test_view_type2_data() {
        let segment = make_test_segment();
        let view = SpkSegmentView::new(&segment);

        let type2 = view.data().as_type2().expect("Should be Type2");
        assert_eq!(type2.degree, 1);
        assert_eq!(type2.records.len(), 1);
        assert_eq!(type2.records[0].x_coeffs, vec![1.0, 2.0]);
    }
}
