//! Error handling tests for DAF parsing.
//!
//! Tests that the parser correctly handles malformed or invalid input files.

use muad_dib::DAFFile;
use std::fs::File;
use std::io::Write;

/// Test that opening a non-existent file returns an error
#[test]
fn test_nonexistent_file() {
    let result = File::open("definitely_does_not_exist_12345.bsp");
    assert!(result.is_err(), "Opening non-existent file should fail");
}

/// Test that a file with invalid DAF magic header is rejected
#[test]
fn test_invalid_daf_header() {
    let temp_dir = std::env::temp_dir();
    let bad_file_path = temp_dir.join("invalid_header.bsp");

    // Create a file with invalid header (not a DAF file)
    {
        let mut file = File::create(&bad_file_path).expect("Could not create temp file");
        // Write garbage data that isn't a valid DAF header
        file.write_all(b"NOT A DAF FILE - INVALID HEADER DATA")
            .expect("Could not write");
        // Pad to at least 1024 bytes to avoid truncation errors
        let padding = vec![0u8; 1024];
        file.write_all(&padding).expect("Could not write padding");
    }

    // Try to parse it
    let file = File::open(&bad_file_path).expect("Could not open temp file");
    let result = DAFFile::from_file(file);

    // Should fail because the endian byte at offset 88 won't be 'B' or 'L'
    assert!(result.is_err(), "Invalid DAF header should fail to parse");

    // Cleanup
    let _ = std::fs::remove_file(&bad_file_path);
}

/// Test that a truncated file (too short) returns an error
#[test]
fn test_truncated_file() {
    let temp_dir = std::env::temp_dir();
    let truncated_path = temp_dir.join("truncated.bsp");

    // Create a file that's too short to be a valid DAF
    {
        let mut file = File::create(&truncated_path).expect("Could not create temp file");
        // Only write 50 bytes - not enough for a DAF header
        file.write_all(&[0u8; 50]).expect("Could not write");
    }

    // Try to parse it
    let file = File::open(&truncated_path).expect("Could not open temp file");
    let result = DAFFile::from_file(file);

    // Should fail because file is too short
    assert!(result.is_err(), "Truncated file should fail to parse");

    // Cleanup
    let _ = std::fs::remove_file(&truncated_path);
}

/// Test that a file with wrong endian indicator is rejected
#[test]
fn test_wrong_endian_byte() {
    let temp_dir = std::env::temp_dir();
    let bad_endian_path = temp_dir.join("bad_endian.bsp");

    // Create a file with proper size but invalid endian byte
    {
        let mut file = File::create(&bad_endian_path).expect("Could not create temp file");
        // Create 1024+ byte file
        let mut data = vec![0u8; 1100];
        // Set something at offset 4 that looks like a type
        data[4] = b'S'; // SPK type
                        // Set invalid endian byte at offset 88 (not 'B' or 'L')
        data[88] = b'X'; // Invalid
        file.write_all(&data).expect("Could not write");
    }

    // Try to parse it
    let file = File::open(&bad_endian_path).expect("Could not open temp file");
    let result = DAFFile::from_file(file);

    // Should fail because endian byte is invalid
    assert!(result.is_err(), "Invalid endian byte should fail to parse");

    // Cleanup
    let _ = std::fs::remove_file(&bad_endian_path);
}

/// Test format detection with unknown extension
#[test]
fn test_unknown_format_extension() {
    use muad_dib::formats::get_format;

    // Unknown format should return None
    let format = get_format("xyz");
    assert!(format.is_none(), "Unknown format should return None");

    let format = get_format("json");
    assert!(format.is_none(), "JSON format should not exist");
}
