//! CK segment view with lazy type-specific parsing.
//!
//! This module provides `CkSegmentView` which wraps a `CKSegment` and
//! adds lazy parsing of type-specific data on first access.

use std::cell::OnceCell;

use super::ck_parse::parse_ck_data;
use super::ck_types::CkData;
use crate::types::NaifId;
use crate::CKSegment;

/// A view over a CK segment with lazy type-specific data parsing.
///
/// The underlying segment data is parsed into a type-specific structure
/// (e.g., `Ck1Data`, `Ck3Data`) on first access, and cached for subsequent
/// accesses.
///
/// # Example
///
/// ```ignore
/// use despice::kernel::ck::CkSegmentView;
///
/// let view = CkSegmentView::new(&segment);
///
/// // Access metadata immediately
/// println!("Instrument: {}", view.instrument().0);
/// println!("Coverage: {} to {} SCLK", view.initial_sclk(), view.final_sclk());
///
/// // Parse type-specific data on first access
/// if let Some(type1) = view.data().as_type1() {
///     for rec in &type1.records {
///         println!("SCLK {}: q={:?}", rec.sclk, rec.quaternion());
///     }
/// }
/// ```
pub struct CkSegmentView<'a> {
    segment: &'a CKSegment,
    parsed_data: OnceCell<CkData>,
}

impl<'a> CkSegmentView<'a> {
    /// Create a new view over a CK segment.
    pub fn new(segment: &'a CKSegment) -> Self {
        Self {
            segment,
            parsed_data: OnceCell::new(),
        }
    }

    /// Get the underlying segment reference.
    pub fn segment(&self) -> &CKSegment {
        self.segment
    }

    /// Get the segment name.
    pub fn name(&self) -> &str {
        &self.segment.name
    }

    /// Get the instrument NAIF ID.
    pub fn instrument(&self) -> NaifId {
        NaifId(self.segment.instrument_code)
    }

    /// Get the reference frame code.
    pub fn frame(&self) -> i32 {
        self.segment.frame_code
    }

    /// Get the CK segment type (1-6).
    pub fn ck_type(&self) -> i32 {
        self.segment.ck_type
    }

    /// Check if angular velocity data is present.
    pub fn has_rates(&self) -> bool {
        self.segment.rates
    }

    /// Get the initial SCLK time (encoded spacecraft clock ticks).
    pub fn initial_sclk(&self) -> f64 {
        self.segment.initial_sclk
    }

    /// Get the final SCLK time (encoded spacecraft clock ticks).
    pub fn final_sclk(&self) -> f64 {
        self.segment.final_sclk
    }

    /// Check if an SCLK time is within this segment's coverage.
    pub fn covers_sclk(&self, sclk: f64) -> bool {
        self.segment.initial_sclk <= sclk && sclk <= self.segment.final_sclk
    }

    /// Get the parsed type-specific data.
    ///
    /// The data is parsed on first access and cached.
    pub fn data(&self) -> &CkData {
        self.parsed_data.get_or_init(|| {
            parse_ck_data(
                self.segment.ck_type,
                self.segment.rates,
                self.segment.data.clone(),
            )
        })
    }

    /// Get the raw data without parsing.
    pub fn raw_data(&self) -> &[f64] {
        &self.segment.data
    }
}

impl<'a> std::fmt::Debug for CkSegmentView<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CkSegmentView")
            .field("name", &self.segment.name)
            .field("instrument", &self.segment.instrument_code)
            .field("ck_type", &self.segment.ck_type)
            .field("rates", &self.segment.rates)
            .field("initial_sclk", &self.segment.initial_sclk)
            .field("final_sclk", &self.segment.final_sclk)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_segment() -> CKSegment {
        // Create a minimal Type 1 segment with 2 records, no rates
        CKSegment {
            name: "TEST CK SEGMENT".to_string(),
            initial_sclk: 1000.0,
            final_sclk: 2000.0,
            instrument_code: -82000,
            frame_code: 1,
            ck_type: 1,
            rates: false,
            data_start: 1,
            data_end: 11,
            data: vec![
                // Pointing records (4 elements each)
                1.0, 0.0, 0.0, 0.0, // Record 1: identity quaternion
                0.707, 0.707, 0.0, 0.0, // Record 2
                // SCLK times
                1000.0, 2000.0, // NPREC
                2.0,
            ],
        }
    }

    #[test]
    fn test_view_metadata() {
        let segment = make_test_segment();
        let view = CkSegmentView::new(&segment);

        assert_eq!(view.name(), "TEST CK SEGMENT");
        assert_eq!(view.instrument(), NaifId(-82000));
        assert_eq!(view.frame(), 1);
        assert_eq!(view.ck_type(), 1);
        assert!(!view.has_rates());
        assert_eq!(view.initial_sclk(), 1000.0);
        assert_eq!(view.final_sclk(), 2000.0);
    }

    #[test]
    fn test_view_covers_sclk() {
        let segment = make_test_segment();
        let view = CkSegmentView::new(&segment);

        assert!(view.covers_sclk(1000.0));
        assert!(view.covers_sclk(1500.0));
        assert!(view.covers_sclk(2000.0));
        assert!(!view.covers_sclk(999.0));
        assert!(!view.covers_sclk(2001.0));
    }

    #[test]
    fn test_view_lazy_parsing() {
        let segment = make_test_segment();
        let view = CkSegmentView::new(&segment);

        // First access parses the data
        let data = view.data();
        assert_eq!(data.ck_type(), 1);

        // Second access returns cached data
        let data2 = view.data();
        assert_eq!(data2.ck_type(), 1);
    }

    #[test]
    fn test_view_type1_data() {
        let segment = make_test_segment();
        let view = CkSegmentView::new(&segment);

        let type1 = view.data().as_type1().expect("Should be Type1");
        assert!(!type1.has_rates);
        assert_eq!(type1.records.len(), 2);
        assert_eq!(type1.records[0].quaternion(), [1.0, 0.0, 0.0, 0.0]);
    }
}
