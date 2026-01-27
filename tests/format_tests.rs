//! Format-specific round-trip tests.
//!
//! Tests that each output format can write and read data correctly,
//! preserving segment counts and data through the round-trip.

use muad_dib::formats::arrow::read_arrow;
use muad_dib::formats::bson::read_bson;
use muad_dib::formats::msgpack::read_msgpack;
use muad_dib::formats::parquet::read_parquet;
use muad_dib::formats::get_format;
use muad_dib::hdf5_output::DAFSource;
use muad_dib::{DAFFile, DAFSegment};
use std::fs::File;

const TEST_FILE: &str = "test_data/test.bsp";

/// Helper to create a DAFSource from test file
fn load_test_source() -> DAFSource {
    let file = File::open(TEST_FILE).expect("Could not open test file");
    let mut daf = DAFFile::from_file(file).expect("Failed to parse DAF file");
    let header = daf.daf_header().expect("Failed to get header");
    let metadata = daf.daf_metadata();
    let segments: Vec<DAFSegment> = daf.filter_map(|r| r.ok()).collect();

    DAFSource {
        filename: TEST_FILE.to_string(),
        header,
        metadata,
        segments,
    }
}

/// Test Parquet format round-trip
#[test]
fn test_parquet_round_trip() {
    let temp_dir = std::env::temp_dir();
    let parquet_path = temp_dir.join("format_test.parquet");

    let source = load_test_source();
    let original_segment_count = source.segments.len();

    // Write to Parquet
    let format = get_format("parquet").expect("Parquet format should exist");
    format
        .write(&parquet_path, &[source.clone()])
        .expect("Failed to write Parquet");

    // Read back
    let sources = read_parquet(&parquet_path).expect("Failed to read Parquet");

    assert_eq!(sources.len(), 1, "Should have one source");
    assert_eq!(
        sources[0].segments.len(),
        original_segment_count,
        "Segment count should match"
    );
    assert_eq!(sources[0].header.kind, "SPK", "Kind should be preserved");

    // Verify first segment data matches
    let original_spk = match &source.segments[0] {
        DAFSegment::SPK(spk) => spk,
        _ => panic!("Expected SPK segment"),
    };
    let restored_spk = match &sources[0].segments[0] {
        DAFSegment::SPK(spk) => spk,
        _ => panic!("Expected SPK segment"),
    };

    assert_eq!(original_spk.target_code, restored_spk.target_code);
    assert_eq!(original_spk.data.len(), restored_spk.data.len());

    // Cleanup
    let _ = std::fs::remove_file(&parquet_path);
}

/// Test Arrow IPC format round-trip
#[test]
fn test_arrow_round_trip() {
    let temp_dir = std::env::temp_dir();
    let arrow_path = temp_dir.join("format_test.arrow");

    let source = load_test_source();
    let original_segment_count = source.segments.len();

    // Write to Arrow
    let format = get_format("arrow").expect("Arrow format should exist");
    format
        .write(&arrow_path, &[source.clone()])
        .expect("Failed to write Arrow");

    // Read back
    let sources = read_arrow(&arrow_path).expect("Failed to read Arrow");

    assert_eq!(sources.len(), 1, "Should have one source");
    assert_eq!(
        sources[0].segments.len(),
        original_segment_count,
        "Segment count should match"
    );
    assert_eq!(sources[0].header.kind, "SPK", "Kind should be preserved");

    // Verify first segment data matches
    let original_spk = match &source.segments[0] {
        DAFSegment::SPK(spk) => spk,
        _ => panic!("Expected SPK segment"),
    };
    let restored_spk = match &sources[0].segments[0] {
        DAFSegment::SPK(spk) => spk,
        _ => panic!("Expected SPK segment"),
    };

    assert_eq!(original_spk.target_code, restored_spk.target_code);
    assert_eq!(original_spk.data.len(), restored_spk.data.len());

    // Cleanup
    let _ = std::fs::remove_file(&arrow_path);
}

/// Test MessagePack format round-trip
#[test]
fn test_msgpack_round_trip() {
    let temp_dir = std::env::temp_dir();
    let msgpack_path = temp_dir.join("format_test.msgpack");

    let source = load_test_source();
    let original_segment_count = source.segments.len();

    // Write to MessagePack
    let format = get_format("msgpack").expect("MsgPack format should exist");
    format
        .write(&msgpack_path, &[source.clone()])
        .expect("Failed to write MessagePack");

    // Read back
    let sources = read_msgpack(&msgpack_path).expect("Failed to read MessagePack");

    assert_eq!(sources.len(), 1, "Should have one source");
    assert_eq!(
        sources[0].segments.len(),
        original_segment_count,
        "Segment count should match"
    );
    assert_eq!(sources[0].header.kind, "SPK", "Kind should be preserved");

    // Verify first segment data matches
    let original_spk = match &source.segments[0] {
        DAFSegment::SPK(spk) => spk,
        _ => panic!("Expected SPK segment"),
    };
    let restored_spk = match &sources[0].segments[0] {
        DAFSegment::SPK(spk) => spk,
        _ => panic!("Expected SPK segment"),
    };

    assert_eq!(original_spk.target_code, restored_spk.target_code);
    assert_eq!(original_spk.data.len(), restored_spk.data.len());

    // Cleanup
    let _ = std::fs::remove_file(&msgpack_path);
}

/// Test BSON format round-trip
#[test]
fn test_bson_round_trip() {
    let temp_dir = std::env::temp_dir();
    let bson_path = temp_dir.join("format_test.bson");

    let source = load_test_source();
    let original_segment_count = source.segments.len();

    // Write to BSON
    let format = get_format("bson").expect("BSON format should exist");
    format
        .write(&bson_path, &[source.clone()])
        .expect("Failed to write BSON");

    // Read back
    let sources = read_bson(&bson_path).expect("Failed to read BSON");

    assert_eq!(sources.len(), 1, "Should have one source");
    assert_eq!(
        sources[0].segments.len(),
        original_segment_count,
        "Segment count should match"
    );
    assert_eq!(sources[0].header.kind, "SPK", "Kind should be preserved");

    // Verify first segment data matches
    let original_spk = match &source.segments[0] {
        DAFSegment::SPK(spk) => spk,
        _ => panic!("Expected SPK segment"),
    };
    let restored_spk = match &sources[0].segments[0] {
        DAFSegment::SPK(spk) => spk,
        _ => panic!("Expected SPK segment"),
    };

    assert_eq!(original_spk.target_code, restored_spk.target_code);
    assert_eq!(original_spk.data.len(), restored_spk.data.len());

    // Cleanup
    let _ = std::fs::remove_file(&bson_path);
}
