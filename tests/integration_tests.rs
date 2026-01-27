use muad_dib::{DAFFile, DAFSegment};
use std::fs::File;

const TEST_FILE: &str = "test_data/test.bsp";

#[test]
fn test_open_spk_file() {
    let file = File::open(TEST_FILE).expect("Could not open test file");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");

    // Verify endian detection worked (test file is little-endian)
    assert!(matches!(daf.endian, muad_dib::Endian::Little));
}

#[test]
fn test_read_all_segments() {
    let file = File::open(TEST_FILE).expect("Could not open test file");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");

    let segments: Vec<_> = daf.collect();

    // Test file has 9 segments
    assert_eq!(
        segments.len(),
        9,
        "Expected 9 segments, got {}",
        segments.len()
    );

    // All segments should parse successfully
    for (i, seg_result) in segments.iter().enumerate() {
        assert!(
            seg_result.is_ok(),
            "Segment {} failed to parse: {:?}",
            i,
            seg_result
        );
    }
}

#[test]
fn test_segment_metadata() {
    let file = File::open(TEST_FILE).expect("Could not open test file");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");

    let first_segment = daf.into_iter().next().unwrap().unwrap();

    match first_segment {
        DAFSegment::SPK(spk) => {
            // Verify SPK type 9 (Lagrange interpolation)
            assert_eq!(spk.spk_type, 9, "Expected SPK type 9");

            // Verify epochs are reasonable (should be around year 2040, ~1.2 billion seconds past J2000)
            assert!(
                spk.initial_epoch > 1_000_000_000.0,
                "Initial epoch too small"
            );
            assert!(
                spk.final_epoch > spk.initial_epoch,
                "Final epoch should be after initial"
            );

            // Verify target is spacecraft -82 (Cassini-like negative ID)
            assert_eq!(spk.target_code, -82);
        }
        _ => panic!("Expected SPK segment"),
    }
}

#[test]
fn test_segment_data_sanity() {
    // This test validates that the get_f64vec bug fix works correctly
    let file = File::open(TEST_FILE).expect("Could not open test file");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");

    let first_segment = daf.into_iter().next().unwrap().unwrap();

    match first_segment {
        DAFSegment::SPK(spk) => {
            // Data should not be empty
            assert!(!spk.data.is_empty(), "SPK data should not be empty");

            // Data values should be reasonable (not 1e93 which would indicate bug)
            // Positions are in km (expect millions of km for outer planet missions)
            // Velocities are in km/s (expect single digits)
            for (i, val) in spk.data.iter().take(6).enumerate() {
                assert!(
                    val.abs() < 1e12,
                    "Data value {} = {} is unreasonably large (indicates parsing bug)",
                    i,
                    val
                );
            }

            // First 3 values are position (x,y,z) in km - typically millions for outer planets
            let pos_magnitude =
                (spk.data[0].powi(2) + spk.data[1].powi(2) + spk.data[2].powi(2)).sqrt();
            assert!(
                pos_magnitude > 1_000_000.0 && pos_magnitude < 1e10,
                "Position magnitude {} km is outside expected range",
                pos_magnitude
            );

            // Values 3-5 are velocity (vx,vy,vz) in km/s - typically < 100 km/s
            let vel_magnitude =
                (spk.data[3].powi(2) + spk.data[4].powi(2) + spk.data[5].powi(2)).sqrt();
            assert!(
                vel_magnitude > 0.001 && vel_magnitude < 100.0,
                "Velocity magnitude {} km/s is outside expected range",
                vel_magnitude
            );
        }
        _ => panic!("Expected SPK segment"),
    }
}

#[test]
fn test_daf_header() {
    let file = File::open(TEST_FILE).expect("Could not open test file");
    let mut daf = DAFFile::from_file(file).expect("Failed to parse DAF file");

    let header = daf.daf_header().expect("Failed to get header");

    assert_eq!(header.kind, "SPK");
    assert_eq!(header.name, "test");
}
