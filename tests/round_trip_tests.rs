//! Round-trip tests for SPK -> HDF5 -> SPK conversion.

#![cfg(feature = "test-data")]

use muad_dib::formats::arrow::read_arrow;
use muad_dib::formats::bson::read_bson;
use muad_dib::formats::get_format;
use muad_dib::formats::msgpack::read_msgpack;
use muad_dib::formats::parquet::read_parquet;
use muad_dib::hdf5_input::read_hdf5;
use muad_dib::hdf5_output::{write_hdf5, DAFSource};
use muad_dib::spk_writer::write_spk;
use muad_dib::{DAFFile, DAFSegment};
use std::fs::File;

const TEST_FILE: &str = "test_data/test.bsp";

/// Test that we can convert SPK to HDF5 and back, preserving segment count and metadata.
#[test]
fn test_round_trip_preserves_segments() {
    let temp_dir = std::env::temp_dir();
    let hdf5_path = temp_dir.join("round_trip_test.hdf5");
    let spk_path = temp_dir.join("round_trip_test.bsp");

    // Step 1: Read original SPK
    let file = File::open(TEST_FILE).expect("Could not open test file");
    let mut daf = DAFFile::from_file(file).expect("Failed to parse DAF file");

    let header = daf.daf_header().expect("Failed to get header");
    let metadata = daf.daf_metadata();
    let original_segments: Vec<DAFSegment> = daf.filter_map(|r| r.ok()).collect();
    let original_count = original_segments.len();

    // Step 2: Write to HDF5
    let source = DAFSource {
        filename: TEST_FILE.to_string(),
        header,
        metadata,
        segments: original_segments,
    };
    write_hdf5(&hdf5_path, vec![source]).expect("Failed to write HDF5");

    // Step 3: Read from HDF5
    let sources = read_hdf5(&hdf5_path).expect("Failed to read HDF5");
    assert_eq!(sources.len(), 1, "Should have one source");
    assert_eq!(
        sources[0].segments.len(),
        original_count,
        "HDF5 should preserve segment count"
    );

    // Step 4: Write back to SPK
    write_spk(&spk_path, &sources[0]).expect("Failed to write SPK");

    // Step 5: Read the reconstructed SPK
    let file2 = File::open(&spk_path).expect("Could not open reconstructed file");
    let daf2 = DAFFile::from_file(file2).expect("Failed to parse reconstructed DAF file");
    let reconstructed_segments: Vec<DAFSegment> = daf2.filter_map(|r| r.ok()).collect();

    assert_eq!(
        reconstructed_segments.len(),
        original_count,
        "Reconstructed SPK should have same segment count"
    );

    // Cleanup
    let _ = std::fs::remove_file(&hdf5_path);
    let _ = std::fs::remove_file(&spk_path);
}

/// Test that segment metadata is preserved through round-trip.
#[test]
fn test_round_trip_preserves_metadata() {
    let temp_dir = std::env::temp_dir();
    let hdf5_path = temp_dir.join("round_trip_metadata_test.hdf5");
    let spk_path = temp_dir.join("round_trip_metadata_test.bsp");

    // Read original
    let file = File::open(TEST_FILE).expect("Could not open test file");
    let mut daf = DAFFile::from_file(file).expect("Failed to parse DAF file");

    let header = daf.daf_header().expect("Failed to get header");
    let metadata = daf.daf_metadata();
    let original_segments: Vec<DAFSegment> = daf.filter_map(|r| r.ok()).collect();

    // Get first segment's metadata
    let original_spk = match &original_segments[0] {
        DAFSegment::SPK(spk) => spk,
        _ => panic!("Expected SPK segment"),
    };
    let original_target = original_spk.target_code;
    let original_center = original_spk.center_code;
    let original_frame = original_spk.frame_code;
    let original_type = original_spk.spk_type;
    let original_initial_epoch = original_spk.initial_epoch;
    let original_final_epoch = original_spk.final_epoch;

    // Round-trip
    let source = DAFSource {
        filename: TEST_FILE.to_string(),
        header,
        metadata,
        segments: original_segments,
    };
    write_hdf5(&hdf5_path, vec![source]).expect("Failed to write HDF5");
    let sources = read_hdf5(&hdf5_path).expect("Failed to read HDF5");
    write_spk(&spk_path, &sources[0]).expect("Failed to write SPK");

    // Read reconstructed
    let file2 = File::open(&spk_path).expect("Could not open reconstructed file");
    let daf2 = DAFFile::from_file(file2).expect("Failed to parse reconstructed DAF file");
    let reconstructed_segments: Vec<DAFSegment> = daf2.filter_map(|r| r.ok()).collect();

    let reconstructed_spk = match &reconstructed_segments[0] {
        DAFSegment::SPK(spk) => spk,
        _ => panic!("Expected SPK segment"),
    };

    assert_eq!(reconstructed_spk.target_code, original_target, "Target code mismatch");
    assert_eq!(reconstructed_spk.center_code, original_center, "Center code mismatch");
    assert_eq!(reconstructed_spk.frame_code, original_frame, "Frame code mismatch");
    assert_eq!(reconstructed_spk.spk_type, original_type, "SPK type mismatch");
    assert!(
        (reconstructed_spk.initial_epoch - original_initial_epoch).abs() < 1e-10,
        "Initial epoch mismatch"
    );
    assert!(
        (reconstructed_spk.final_epoch - original_final_epoch).abs() < 1e-10,
        "Final epoch mismatch"
    );

    // Cleanup
    let _ = std::fs::remove_file(&hdf5_path);
    let _ = std::fs::remove_file(&spk_path);
}

/// Test that segment data is preserved through round-trip.
#[test]
fn test_round_trip_preserves_data() {
    let temp_dir = std::env::temp_dir();
    let hdf5_path = temp_dir.join("round_trip_data_test.hdf5");
    let spk_path = temp_dir.join("round_trip_data_test.bsp");

    // Read original
    let file = File::open(TEST_FILE).expect("Could not open test file");
    let mut daf = DAFFile::from_file(file).expect("Failed to parse DAF file");

    let header = daf.daf_header().expect("Failed to get header");
    let metadata = daf.daf_metadata();
    let original_segments: Vec<DAFSegment> = daf.filter_map(|r| r.ok()).collect();

    // Get first segment's data
    let original_spk = match &original_segments[0] {
        DAFSegment::SPK(spk) => spk,
        _ => panic!("Expected SPK segment"),
    };
    let original_data = original_spk.data.clone();
    let original_data_len = original_data.len();

    // Round-trip
    let source = DAFSource {
        filename: TEST_FILE.to_string(),
        header,
        metadata,
        segments: original_segments,
    };
    write_hdf5(&hdf5_path, vec![source]).expect("Failed to write HDF5");
    let sources = read_hdf5(&hdf5_path).expect("Failed to read HDF5");
    write_spk(&spk_path, &sources[0]).expect("Failed to write SPK");

    // Read reconstructed
    let file2 = File::open(&spk_path).expect("Could not open reconstructed file");
    let daf2 = DAFFile::from_file(file2).expect("Failed to parse reconstructed DAF file");
    let reconstructed_segments: Vec<DAFSegment> = daf2.filter_map(|r| r.ok()).collect();

    let reconstructed_spk = match &reconstructed_segments[0] {
        DAFSegment::SPK(spk) => spk,
        _ => panic!("Expected SPK segment"),
    };

    assert_eq!(
        reconstructed_spk.data.len(),
        original_data_len,
        "Data length mismatch"
    );

    // Check data values (allow for floating point tolerance)
    for (i, (orig, recon)) in original_data.iter().zip(reconstructed_spk.data.iter()).enumerate() {
        assert!(
            (orig - recon).abs() < 1e-10,
            "Data mismatch at index {}: original={}, reconstructed={}",
            i,
            orig,
            recon
        );
    }

    // Cleanup
    let _ = std::fs::remove_file(&hdf5_path);
    let _ = std::fs::remove_file(&spk_path);
}

/// Test that DAF header info is preserved.
#[test]
fn test_round_trip_preserves_header() {
    let temp_dir = std::env::temp_dir();
    let hdf5_path = temp_dir.join("round_trip_header_test.hdf5");
    let spk_path = temp_dir.join("round_trip_header_test.bsp");

    // Read original
    let file = File::open(TEST_FILE).expect("Could not open test file");
    let mut daf = DAFFile::from_file(file).expect("Failed to parse DAF file");

    let original_header = daf.daf_header().expect("Failed to get header");
    let original_name = original_header.name.clone();
    let original_kind = original_header.kind.clone();
    let metadata = daf.daf_metadata();
    let segments: Vec<DAFSegment> = daf.filter_map(|r| r.ok()).collect();

    // Round-trip
    let source = DAFSource {
        filename: TEST_FILE.to_string(),
        header: original_header,
        metadata,
        segments,
    };
    write_hdf5(&hdf5_path, vec![source]).expect("Failed to write HDF5");
    let sources = read_hdf5(&hdf5_path).expect("Failed to read HDF5");
    write_spk(&spk_path, &sources[0]).expect("Failed to write SPK");

    // Read reconstructed
    let file2 = File::open(&spk_path).expect("Could not open reconstructed file");
    let mut daf2 = DAFFile::from_file(file2).expect("Failed to parse reconstructed DAF file");
    let reconstructed_header = daf2.daf_header().expect("Failed to get header");

    assert_eq!(reconstructed_header.name, original_name, "Header name mismatch");
    assert_eq!(reconstructed_header.kind, original_kind, "Header kind mismatch");

    // Cleanup
    let _ = std::fs::remove_file(&hdf5_path);
    let _ = std::fs::remove_file(&spk_path);
}

/// Test that comments are preserved through round-trip for all formats.
///
/// Note: test.bsp has an empty comment (fward=2, so no comment records).
/// This test verifies empty string preservation across all formats.
#[test]
fn test_round_trip_preserves_comments() {
    let temp_dir = std::env::temp_dir();

    // Read original and capture comment
    let file = File::open(TEST_FILE).expect("Could not open test file");
    let mut daf = DAFFile::from_file(file).expect("Failed to parse DAF file");
    let header = daf.daf_header().expect("Failed to get header");
    let metadata = daf.daf_metadata();
    let original_comment = header.comment.clone();
    let segments: Vec<DAFSegment> = daf.filter_map(|r| r.ok()).collect();

    let source = DAFSource {
        filename: TEST_FILE.to_string(),
        header,
        metadata,
        segments,
    };

    // Test HDF5 format
    {
        let path = temp_dir.join("comment_test.hdf5");
        write_hdf5(&path, vec![source.clone()]).expect("Failed to write HDF5");
        let sources = read_hdf5(&path).expect("Failed to read HDF5");
        assert_eq!(
            sources[0].header.comment, original_comment,
            "HDF5: comment mismatch"
        );
        let _ = std::fs::remove_file(&path);
    }

    // Test MessagePack format
    {
        let path = temp_dir.join("comment_test.msgpack");
        let format = get_format("msgpack").expect("MsgPack format should exist");
        format
            .write(&path, &[source.clone()])
            .expect("Failed to write MessagePack");
        let sources = read_msgpack(&path).expect("Failed to read MessagePack");
        assert_eq!(
            sources[0].header.comment, original_comment,
            "MessagePack: comment mismatch"
        );
        let _ = std::fs::remove_file(&path);
    }

    // Test BSON format
    {
        let path = temp_dir.join("comment_test.bson");
        let format = get_format("bson").expect("BSON format should exist");
        format
            .write(&path, &[source.clone()])
            .expect("Failed to write BSON");
        let sources = read_bson(&path).expect("Failed to read BSON");
        assert_eq!(
            sources[0].header.comment, original_comment,
            "BSON: comment mismatch"
        );
        let _ = std::fs::remove_file(&path);
    }

    // Test Arrow IPC format
    {
        let path = temp_dir.join("comment_test.arrow");
        let format = get_format("arrow").expect("Arrow format should exist");
        format
            .write(&path, &[source.clone()])
            .expect("Failed to write Arrow");
        let sources = read_arrow(&path).expect("Failed to read Arrow");
        assert_eq!(
            sources[0].header.comment, original_comment,
            "Arrow: comment mismatch"
        );
        let _ = std::fs::remove_file(&path);
    }

    // Test Parquet format
    {
        let path = temp_dir.join("comment_test.parquet");
        let format = get_format("parquet").expect("Parquet format should exist");
        format
            .write(&path, &[source.clone()])
            .expect("Failed to write Parquet");
        let sources = read_parquet(&path).expect("Failed to read Parquet");
        assert_eq!(
            sources[0].header.comment, original_comment,
            "Parquet: comment mismatch"
        );
        let _ = std::fs::remove_file(&path);
    }
}
