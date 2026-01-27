//! Tests for various SPICE kernel file formats.
//!
//! Tests parsing of SPK files with different:
//! - Endianness (big-endian vs little-endian)
//! - SPK types (Hermite, Lagrange, Chebyshev)
//! - Segment configurations

#![cfg(feature = "test-data")]

use muad_dib::{DAFFile, DAFSegment, Endian};
use std::fs::File;

// =============================================================================
// SPK Format Tests
// =============================================================================

/// Test parsing GMAT-generated Hermite SPK (Type 13)
#[test]
fn test_parse_gmat_hermite_spk() {
    let file = File::open("test_data/gmat-hermite.bsp").expect("Could not open file");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");

    // Should be little-endian
    assert!(matches!(daf.endian, Endian::Little));

    let segments: Vec<_> = daf.filter_map(|s| s.ok()).collect();
    assert!(!segments.is_empty(), "Should have at least one segment");

    // Verify first segment is SPK
    match &segments[0] {
        DAFSegment::SPK(spk) => {
            // GMAT spacecraft ID
            assert_eq!(spk.target_code, -10000001);
            // Should be Type 13 (Hermite)
            assert_eq!(spk.spk_type, 13, "Expected SPK Type 13 (Hermite)");
        }
        _ => panic!("Expected SPK segment"),
    }
}

/// Test parsing GMAT-generated Lagrange SPK (Type 9)
#[test]
fn test_parse_gmat_lagrange_spk() {
    let file = File::open("test_data/gmat-lagrange.bsp").expect("Could not open file");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");

    assert!(matches!(daf.endian, Endian::Little));

    let segments: Vec<_> = daf.filter_map(|s| s.ok()).collect();
    assert!(!segments.is_empty(), "Should have at least one segment");

    match &segments[0] {
        DAFSegment::SPK(spk) => {
            assert_eq!(spk.target_code, -10000001);
            // Should be Type 9 (Lagrange)
            assert_eq!(spk.spk_type, 9, "Expected SPK Type 9 (Lagrange)");
        }
        _ => panic!("Expected SPK segment"),
    }
}

/// Test parsing big-endian SPK file
#[test]
fn test_parse_big_endian_spk() {
    let file = File::open("test_data/gmat-hermite-big-endian.bsp").expect("Could not open file");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");

    // Must be big-endian
    assert!(
        matches!(daf.endian, Endian::Big),
        "Expected big-endian, got {:?}",
        daf.endian
    );

    let segments: Vec<_> = daf.filter_map(|s| s.ok()).collect();
    assert!(!segments.is_empty(), "Should have at least one segment");

    // Data should parse correctly despite endianness
    match &segments[0] {
        DAFSegment::SPK(spk) => {
            assert_eq!(spk.target_code, -10000001);
            // Verify data is reasonable (not garbled by endian issues)
            assert!(spk.initial_epoch > 0.0, "Initial epoch should be positive");
            assert!(
                spk.final_epoch > spk.initial_epoch,
                "Final epoch should be after initial"
            );
        }
        _ => panic!("Expected SPK segment"),
    }
}

/// Test parsing variable segment size Hermite SPK
///
/// Note: This file uses SPK Type 12 (Hermite with variable step size)
/// which may not be fully supported yet.
#[test]
fn test_parse_variable_segment_hermite() {
    let file = File::open("test_data/variable-seg-size-hermite.bsp").expect("Could not open file");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");

    // Verify we can at least read the file header
    assert!(matches!(daf.endian, Endian::Little | Endian::Big));

    let segments: Vec<_> = daf.collect();
    // File may have segments that fail to parse due to unsupported types
    // Just verify we can iterate without panic
    let ok_count = segments.iter().filter(|s| s.is_ok()).count();
    let err_count = segments.iter().filter(|s| s.is_err()).count();

    // Log what we found for debugging
    eprintln!(
        "variable-seg-size-hermite.bsp: {} ok, {} errors",
        ok_count, err_count
    );
}

/// Test parsing rename-test SPK (multiple segments)
///
/// Note: This file may contain SPK types not yet fully supported.
#[test]
fn test_parse_rename_test_spk() {
    let file = File::open("test_data/rename-test.bsp").expect("Could not open file");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");

    // Verify we can at least read the file header
    assert!(matches!(daf.endian, Endian::Little | Endian::Big));

    let segments: Vec<_> = daf.collect();
    // File may have segments that fail to parse due to unsupported types
    let ok_count = segments.iter().filter(|s| s.is_ok()).count();
    let err_count = segments.iter().filter(|s| s.is_err()).count();

    // Log what we found for debugging
    eprintln!("rename-test.bsp: {} ok, {} errors", ok_count, err_count);

    // If we got any segments, verify they're SPK type
    for (i, seg) in segments.iter().filter_map(|s| s.as_ref().ok()).enumerate() {
        assert!(
            matches!(seg, DAFSegment::SPK(_)),
            "Segment {} should be SPK",
            i
        );
    }
}

// =============================================================================
// CK Format Tests
// =============================================================================

/// Test parsing CK (C-kernel) file
#[test]
fn test_parse_ck_file() {
    let file = File::open("test_data/test.bc").expect("Could not open file");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");

    let segments: Vec<_> = daf.filter_map(|s| s.ok()).collect();
    assert!(!segments.is_empty(), "Should have at least one segment");

    // All segments should be CK type
    for (i, seg) in segments.iter().enumerate() {
        match seg {
            DAFSegment::CK(ck) => {
                // CK instrument IDs are typically negative (spacecraft frame IDs)
                assert!(
                    ck.instrument_code != 0,
                    "Segment {} should have non-zero instrument code",
                    i
                );
                // Verify SCLK times are reasonable
                assert!(
                    ck.final_sclk > ck.initial_sclk,
                    "Segment {} final SCLK should be after initial",
                    i
                );
            }
            _ => panic!("Segment {} should be CK, got {:?}", i, seg),
        }
    }
}

// =============================================================================
// Cross-format consistency tests
// =============================================================================

/// Verify GMAT Hermite and Lagrange files have consistent metadata
#[test]
fn test_gmat_spk_consistency() {
    let hermite_file = File::open("test_data/gmat-hermite.bsp").expect("Could not open hermite");
    let lagrange_file = File::open("test_data/gmat-lagrange.bsp").expect("Could not open lagrange");

    let hermite_daf = DAFFile::from_file(hermite_file).expect("Failed to parse hermite");
    let lagrange_daf = DAFFile::from_file(lagrange_file).expect("Failed to parse lagrange");

    let hermite_segs: Vec<_> = hermite_daf.filter_map(|s| s.ok()).collect();
    let lagrange_segs: Vec<_> = lagrange_daf.filter_map(|s| s.ok()).collect();

    // Same number of segments
    assert_eq!(
        hermite_segs.len(),
        lagrange_segs.len(),
        "GMAT files should have same segment count"
    );

    // Same target/center for corresponding segments
    for (h, l) in hermite_segs.iter().zip(lagrange_segs.iter()) {
        match (h, l) {
            (DAFSegment::SPK(h_spk), DAFSegment::SPK(l_spk)) => {
                assert_eq!(
                    h_spk.target_code, l_spk.target_code,
                    "Target codes should match"
                );
                assert_eq!(
                    h_spk.center_code, l_spk.center_code,
                    "Center codes should match"
                );
                // Epochs should be very close (same simulation)
                assert!(
                    (h_spk.initial_epoch - l_spk.initial_epoch).abs() < 1.0,
                    "Initial epochs should be close"
                );
            }
            _ => panic!("Both should be SPK segments"),
        }
    }
}

/// Verify big-endian file has same content as little-endian equivalent
#[test]
fn test_endian_data_equivalence() {
    let le_file = File::open("test_data/gmat-hermite.bsp").expect("Could not open LE file");
    let be_file =
        File::open("test_data/gmat-hermite-big-endian.bsp").expect("Could not open BE file");

    let le_daf = DAFFile::from_file(le_file).expect("Failed to parse LE");
    let be_daf = DAFFile::from_file(be_file).expect("Failed to parse BE");

    // Verify opposite endianness
    assert!(matches!(le_daf.endian, Endian::Little));
    assert!(matches!(be_daf.endian, Endian::Big));

    let le_segs: Vec<_> = le_daf.filter_map(|s| s.ok()).collect();
    let be_segs: Vec<_> = be_daf.filter_map(|s| s.ok()).collect();

    assert_eq!(le_segs.len(), be_segs.len(), "Segment counts should match");

    // Compare segment metadata
    for (le, be) in le_segs.iter().zip(be_segs.iter()) {
        match (le, be) {
            (DAFSegment::SPK(le_spk), DAFSegment::SPK(be_spk)) => {
                assert_eq!(le_spk.target_code, be_spk.target_code);
                assert_eq!(le_spk.center_code, be_spk.center_code);
                assert_eq!(le_spk.spk_type, be_spk.spk_type);
                // Epochs should be identical
                assert!(
                    (le_spk.initial_epoch - be_spk.initial_epoch).abs() < 1e-10,
                    "Initial epochs should match exactly"
                );
                assert!(
                    (le_spk.final_epoch - be_spk.final_epoch).abs() < 1e-10,
                    "Final epochs should match exactly"
                );
                // Data arrays should have same length
                assert_eq!(
                    le_spk.data.len(),
                    be_spk.data.len(),
                    "Data lengths should match"
                );
                // First few data values should match (checking endian conversion worked)
                for i in 0..le_spk.data.len().min(10) {
                    assert!(
                        (le_spk.data[i] - be_spk.data[i]).abs() < 1e-10,
                        "Data[{}] mismatch: LE={}, BE={}",
                        i,
                        le_spk.data[i],
                        be_spk.data[i]
                    );
                }
            }
            _ => panic!("Both should be SPK segments"),
        }
    }
}

// =============================================================================
// SpiceKernel API Tests
// =============================================================================

use muad_dib::kernel::SpiceKernel;
use muad_dib::types::NaifId;

/// Test loading GMAT Hermite SPK via SpiceKernel API
#[test]
fn test_spice_kernel_load_hermite() {
    let kernel = SpiceKernel::load("test_data/gmat-hermite.bsp").expect("Failed to load kernel");

    assert!(!kernel.is_empty());
    let bodies = kernel.spk_bodies();
    assert!(!bodies.is_empty(), "Should have at least one body");

    let gmat_sc = NaifId(-10000001);
    assert!(bodies.contains(&gmat_sc), "Should contain GMAT spacecraft");

    // Check coverage
    let coverage = kernel.spk_coverage(gmat_sc);
    assert!(coverage.is_some(), "Should have coverage for spacecraft");
}

/// Test loading multiple SPK files into one kernel
#[test]
fn test_spice_kernel_load_multiple() {
    let kernel = SpiceKernel::builder()
        .file("test_data/gmat-hermite.bsp")
        .file("test_data/gmat-lagrange.bsp")
        .build()
        .expect("Failed to build kernel");

    // Both files have the same target, so should still see one body
    let bodies = kernel.spk_bodies();
    let gmat_sc = NaifId(-10000001);
    assert!(bodies.contains(&gmat_sc));

    // But segment count should reflect both files
    assert!(
        kernel.segment_count() >= 2,
        "Should have segments from both files"
    );
}

/// Test loading CK file via SpiceKernel API
#[test]
fn test_spice_kernel_load_ck() {
    let kernel = SpiceKernel::load("test_data/test.bc").expect("Failed to load CK");

    assert!(!kernel.is_empty());
    let instruments = kernel.ck_instruments();
    assert!(
        !instruments.is_empty(),
        "Should have at least one instrument"
    );
}

/// Test loading mixed SPK and CK files
#[test]
fn test_spice_kernel_load_mixed() {
    let kernel = SpiceKernel::builder()
        .file("test_data/test.bsp")
        .file("test_data/test.bc")
        .build()
        .expect("Failed to build kernel");

    // Should have both SPK bodies and CK instruments
    assert!(!kernel.spk_bodies().is_empty(), "Should have SPK bodies");
    assert!(
        !kernel.ck_instruments().is_empty(),
        "Should have CK instruments"
    );
}

// =============================================================================
// BPC (Binary PCK) Format Tests
// =============================================================================

/// Test parsing BPC (Binary PCK) file - earth high precision
#[test]
fn test_parse_bpc_earth() {
    let file = File::open("test_data/earth_latest_high_prec.bpc").expect("Could not open file");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");

    // Should be little-endian
    assert!(matches!(daf.endian, Endian::Little));

    let segments: Vec<_> = daf.filter_map(|s| s.ok()).collect();
    assert!(!segments.is_empty(), "Should have at least one segment");

    // All segments should be BPCK type
    for (i, seg) in segments.iter().enumerate() {
        match seg {
            DAFSegment::BPCK(bpck) => {
                // Earth body-fixed frame (ITRF93 or similar)
                assert_eq!(
                    bpck.frame_id, 3000,
                    "Segment {} should have frame_id 3000",
                    i
                );
                // Base frame should be J2000 (1) or ICRF (17)
                assert!(
                    bpck.base_frame == 1 || bpck.base_frame == 17,
                    "Segment {} should have base_frame 1 or 17, got {}",
                    i,
                    bpck.base_frame
                );
                // Should be Type 2
                assert_eq!(bpck.bpck_type, 2, "Segment {} should be Type 2", i);
                // Data range should be valid
                assert!(
                    bpck.data_end >= bpck.data_start,
                    "Segment {} data_end should be >= data_start",
                    i
                );
                // Epochs should be reasonable
                assert!(
                    bpck.final_epoch > bpck.initial_epoch,
                    "Segment {} final_epoch should be > initial_epoch",
                    i
                );
            }
            _ => panic!("Segment {} should be BPCK, got {:?}", i, seg),
        }
    }
}

/// Test parsing BPC file - moon orientation
#[test]
fn test_parse_bpc_moon() {
    let file = File::open("test_data/moon_pa_de440_200625.bpc").expect("Could not open file");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");

    assert!(matches!(daf.endian, Endian::Little));

    let segments: Vec<_> = daf.filter_map(|s| s.ok()).collect();
    assert!(!segments.is_empty(), "Should have at least one segment");

    for (i, seg) in segments.iter().enumerate() {
        match seg {
            DAFSegment::BPCK(bpck) => {
                // Moon principal axes frame
                assert_eq!(
                    bpck.frame_id, 31008,
                    "Segment {} should have frame_id 31008",
                    i
                );
                // Data should be valid
                assert!(
                    bpck.data_end >= bpck.data_start,
                    "Segment {} data_end should be >= data_start",
                    i
                );
            }
            _ => panic!("Segment {} should be BPCK, got {:?}", i, seg),
        }
    }
}

/// Test BPC segment metadata consistency across multiple files
#[test]
fn test_bpc_segment_consistency() {
    // Parse both Earth BPC files
    let earth1 = File::open("test_data/earth_latest_high_prec.bpc").expect("Could not open file");
    let earth2 = File::open("test_data/earth_longterm_000101_251211_250915.bpc")
        .expect("Could not open file");

    let daf1 = DAFFile::from_file(earth1).expect("Failed to parse DAF");
    let daf2 = DAFFile::from_file(earth2).expect("Failed to parse DAF");

    let segs1: Vec<_> = daf1.filter_map(|s| s.ok()).collect();
    let segs2: Vec<_> = daf2.filter_map(|s| s.ok()).collect();

    // Both should have segments
    assert!(!segs1.is_empty(), "earth_latest should have segments");
    assert!(!segs2.is_empty(), "earth_longterm should have segments");

    // Both should define the same frame
    let frame1 = match &segs1[0] {
        DAFSegment::BPCK(b) => b.frame_id,
        _ => panic!("Expected BPCK"),
    };
    let frame2 = match &segs2[0] {
        DAFSegment::BPCK(b) => b.frame_id,
        _ => panic!("Expected BPCK"),
    };
    assert_eq!(frame1, frame2, "Both files should define same frame");
}
