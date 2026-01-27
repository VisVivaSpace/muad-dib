//! Validation tests comparing despice's DAF parsing against the anise crate.
//!
//! These tests ensure despice correctly parses DAF files by comparing results
//! with anise's well-tested implementation.

use anise::naif::daf::DAF;
use anise::naif::spk::summary::SPKSummaryRecord;
use muad_dib::{DAFFile, DAFSegment, Endian};
use std::fs::File;

const TEST_FILE: &str = "test_data/test.bsp";

/// Get the actual number of valid summaries from anise's DAF.
///
/// Note: anise's data_summaries() returns all records in the summary block,
/// not just the valid ones. We need to use daf_summary().num_summaries() to
/// get the actual count.
fn get_anise_segment_count(daf: &DAF<SPKSummaryRecord>) -> usize {
    daf.daf_summary().expect("Failed to get DAF summary").num_summaries()
}

/// Validate file record fields match between despice and anise.
///
/// Compares: ND, NI, endian, FWARD
#[test]
fn validate_file_record_matches_anise() {
    // Load with despice
    let file = File::open(TEST_FILE).expect("Could not open test file");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");
    let despice_meta = daf.daf_metadata();

    // Load with anise
    let anise_daf: DAF<SPKSummaryRecord> =
        DAF::load(TEST_FILE).expect("Anise failed to load DAF file");
    let anise_fr = anise_daf.file_record().expect("Failed to get anise file record");

    // Compare ND (number of double precision components)
    assert_eq!(
        despice_meta.nd as usize,
        anise_fr.nd(),
        "ND mismatch: despice={}, anise={}",
        despice_meta.nd,
        anise_fr.nd()
    );

    // Compare NI (number of integer components)
    assert_eq!(
        despice_meta.ni as usize,
        anise_fr.ni(),
        "NI mismatch: despice={}, anise={}",
        despice_meta.ni,
        anise_fr.ni()
    );

    // Compare FWARD (forward pointer to first summary record)
    assert_eq!(
        despice_meta.fward as usize,
        anise_fr.fwrd_idx(),
        "FWARD mismatch: despice={}, anise={}",
        despice_meta.fward,
        anise_fr.fwrd_idx()
    );

    // Compare endianness
    let anise_endian = anise_fr.endianness().expect("Failed to get anise endianness");
    let despice_endian_matches = match (despice_meta.endian, anise_endian) {
        (Endian::Little, anise::naif::Endian::Little) => true,
        (Endian::Big, anise::naif::Endian::Big) => true,
        _ => false,
    };
    assert!(
        despice_endian_matches,
        "Endian mismatch: despice={:?}, anise={:?}",
        despice_meta.endian, anise_endian
    );
}

/// Validate segment count matches between despice and anise.
#[test]
fn validate_segment_count_matches_anise() {
    // Load with despice
    let file = File::open(TEST_FILE).expect("Could not open test file");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");
    let despice_segments: Vec<_> = daf.collect();
    let despice_count = despice_segments.len();

    // Load with anise
    let anise_daf: DAF<SPKSummaryRecord> =
        DAF::load(TEST_FILE).expect("Anise failed to load DAF file");
    let anise_count = get_anise_segment_count(&anise_daf);

    assert_eq!(
        despice_count, anise_count,
        "Segment count mismatch: despice={}, anise={}",
        despice_count, anise_count
    );
}

/// Validate segment summary metadata matches between despice and anise.
///
/// For each segment, compares: target_id, center_id, frame_id, data_type,
/// initial_epoch, final_epoch, data_start, data_end
#[test]
fn validate_segment_summaries_match_anise() {
    // Load with despice
    let file = File::open(TEST_FILE).expect("Could not open test file");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");
    let despice_segments: Vec<_> = daf.filter_map(|s| s.ok()).collect();

    // Load with anise
    let anise_daf: DAF<SPKSummaryRecord> =
        DAF::load(TEST_FILE).expect("Anise failed to load DAF file");
    let num_summaries = get_anise_segment_count(&anise_daf);
    let anise_summaries = &anise_daf.data_summaries().expect("Failed to get anise summaries")[..num_summaries];

    assert_eq!(
        despice_segments.len(),
        anise_summaries.len(),
        "Segment count mismatch before comparison"
    );

    for (i, (despice_seg, anise_sum)) in
        despice_segments.iter().zip(anise_summaries.iter()).enumerate()
    {
        let spk = match despice_seg {
            DAFSegment::SPK(s) => s,
            _ => panic!("Segment {} is not SPK type", i),
        };

        // Compare target ID
        assert_eq!(
            spk.target_code, anise_sum.target_id,
            "Segment {}: target_id mismatch: despice={}, anise={}",
            i, spk.target_code, anise_sum.target_id
        );

        // Compare center ID
        assert_eq!(
            spk.center_code, anise_sum.center_id,
            "Segment {}: center_id mismatch: despice={}, anise={}",
            i, spk.center_code, anise_sum.center_id
        );

        // Compare frame ID
        assert_eq!(
            spk.frame_code, anise_sum.frame_id,
            "Segment {}: frame_id mismatch: despice={}, anise={}",
            i, spk.frame_code, anise_sum.frame_id
        );

        // Compare SPK type
        assert_eq!(
            spk.spk_type, anise_sum.data_type_i,
            "Segment {}: spk_type mismatch: despice={}, anise={}",
            i, spk.spk_type, anise_sum.data_type_i
        );

        // Compare epochs (TDB seconds past J2000)
        // These should be exact since both read the same bytes
        assert!(
            (spk.initial_epoch - anise_sum.start_epoch_et_s).abs() < 1e-10,
            "Segment {}: initial_epoch mismatch: despice={}, anise={}",
            i,
            spk.initial_epoch,
            anise_sum.start_epoch_et_s
        );

        assert!(
            (spk.final_epoch - anise_sum.end_epoch_et_s).abs() < 1e-10,
            "Segment {}: final_epoch mismatch: despice={}, anise={}",
            i,
            spk.final_epoch,
            anise_sum.end_epoch_et_s
        );

        // Compare data array indices
        // Note: anise uses i32 for indices, despice uses u64
        assert_eq!(
            spk.data_start as i32, anise_sum.start_idx,
            "Segment {}: data_start mismatch: despice={}, anise={}",
            i, spk.data_start, anise_sum.start_idx
        );

        assert_eq!(
            spk.data_end as i32, anise_sum.end_idx,
            "Segment {}: data_end mismatch: despice={}, anise={}",
            i, spk.data_end, anise_sum.end_idx
        );
    }
}

/// Validate data array sizes are consistent with indices.
///
/// For each segment, verifies that despice's data vector has the correct
/// number of elements based on the segment indices (end - start + 1).
#[test]
fn validate_segment_data_sizes() {
    // Load with despice
    let file = File::open(TEST_FILE).expect("Could not open test file");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");
    let despice_segments: Vec<_> = daf.filter_map(|s| s.ok()).collect();

    // Load with anise for index comparison
    let anise_daf: DAF<SPKSummaryRecord> =
        DAF::load(TEST_FILE).expect("Anise failed to load DAF file");
    let num_summaries = get_anise_segment_count(&anise_daf);
    let anise_summaries = &anise_daf.data_summaries().expect("Failed to get anise summaries")[..num_summaries];

    for (i, (despice_seg, anise_sum)) in
        despice_segments.iter().zip(anise_summaries.iter()).enumerate()
    {
        let spk = match despice_seg {
            DAFSegment::SPK(s) => s,
            _ => panic!("Segment {} is not SPK type", i),
        };

        // Calculate expected data length from indices
        // DAF indices are 1-indexed, end is inclusive
        let expected_len = (anise_sum.end_idx - anise_sum.start_idx + 1) as usize;

        assert_eq!(
            spk.data.len(),
            expected_len,
            "Segment {}: data length mismatch: despice={}, expected={}",
            i,
            spk.data.len(),
            expected_len
        );

        // Also verify despice's indices match what we'd compute from data
        let despice_expected_len = (spk.data_end - spk.data_start + 1) as usize;
        assert_eq!(
            spk.data.len(),
            despice_expected_len,
            "Segment {}: despice data length {} doesn't match its own indices (expected {})",
            i,
            spk.data.len(),
            despice_expected_len
        );
    }
}
