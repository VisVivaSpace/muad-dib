//! Time parsing and TDB/UTC conversion tests comparing muad-dib against CSPICE.
//!
//! Tests validate EpochTDB::parse(), utc_to_tdb(), and tdb_to_utc() functions.
//!
//! Requires: naif0012.tls (leap seconds kernel)
//! Run with: cargo test --test cspice_time_tests -- --test-threads=1

#![cfg(all(feature = "cspice", feature = "test-data"))]

mod cspice_common;

use cspice_common::{
    assert_close, cspice_et2utc, cspice_str2et, cspice_utc2et, lsk_path, CspiceKernels,
    CSPICE_LOCK,
};
use muad_dib::kernel::SpiceKernel;
use muad_dib::spice::{tdb_to_utc, utc_to_tdb, LeapSecondExt, TimeFormat};
use muad_dib::types::EpochTDB;

/// Tolerance for time parsing (microsecond precision).
const TIME_PARSE_TOLERANCE: f64 = 1e-6;

/// Tolerance for TDB/UTC conversion (millisecond precision due to leap second model differences).
const TDB_UTC_TOLERANCE: f64 = 1e-3;

// ============================================================================
// Time Parsing Tests (str2et equivalent)
// ============================================================================

#[test]
fn validate_time_parsing_j2000() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    // Load LSK for CSPICE
    let mut kernels = CspiceKernels::new();
    kernels.load(&lsk_path());

    // J2000 epoch should be ~0 TDB
    let time_str = "2000-01-01T12:00:00";
    let cspice_et = cspice_str2et(time_str);
    let muad_et = EpochTDB::parse(time_str).expect("Failed to parse time");

    assert_close(
        muad_et.0,
        cspice_et,
        TIME_PARSE_TOLERANCE,
        &format!("J2000 epoch: {}", time_str),
    );

    // J2000 should be very close to 0
    assert!(
        muad_et.0.abs() < 100.0,
        "J2000 should be within 100 seconds of 0 TDB"
    );
}

#[test]
fn validate_time_parsing_fractional_seconds() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut kernels = CspiceKernels::new();
    kernels.load(&lsk_path());

    let time_str = "2000-01-01T12:00:00.500";
    let cspice_et = cspice_str2et(time_str);
    let muad_et = EpochTDB::parse(time_str).expect("Failed to parse time");

    assert_close(
        muad_et.0,
        cspice_et,
        TIME_PARSE_TOLERANCE,
        "Fractional seconds",
    );

    // Should be 0.5 seconds after J2000
    let j2000_et = EpochTDB::parse("2000-01-01T12:00:00").unwrap();
    assert_close(
        muad_et.0 - j2000_et.0,
        0.5,
        TIME_PARSE_TOLERANCE,
        "Fractional offset",
    );
}

#[test]
fn validate_time_parsing_recent_date() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut kernels = CspiceKernels::new();
    kernels.load(&lsk_path());

    let time_str = "2020-06-15T14:30:00";
    let cspice_et = cspice_str2et(time_str);
    let muad_et = EpochTDB::parse(time_str).expect("Failed to parse time");

    assert_close(
        muad_et.0,
        cspice_et,
        TIME_PARSE_TOLERANCE,
        "Recent date 2020",
    );

    // Should be positive (after J2000)
    assert!(muad_et.0 > 0.0, "2020 date should be after J2000");
}

#[test]
fn validate_time_parsing_calendar_format() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut kernels = CspiceKernels::new();
    kernels.load(&lsk_path());

    let time_str = "2000 JAN 01 12:00:00";
    let cspice_et = cspice_str2et(time_str);
    let muad_et = EpochTDB::parse(time_str).expect("Failed to parse calendar format");

    assert_close(
        muad_et.0,
        cspice_et,
        TIME_PARSE_TOLERANCE,
        "Calendar format",
    );
}

#[test]
fn validate_time_parsing_julian_date() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut kernels = CspiceKernels::new();
    kernels.load(&lsk_path());

    // J2000 Julian Date
    let time_str = "JD 2451545.0";
    let cspice_et = cspice_str2et(time_str);
    let muad_et = EpochTDB::parse(time_str).expect("Failed to parse Julian Date");

    assert_close(
        muad_et.0,
        cspice_et,
        TIME_PARSE_TOLERANCE,
        "Julian Date J2000",
    );
}

#[test]
fn validate_time_parsing_various_formats() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut kernels = CspiceKernels::new();
    kernels.load(&lsk_path());

    let test_times = [
        "2000-01-01T12:00:00",
        "2000-01-01T12:00:00.500",
        "2020-06-15T14:30:00",
        "2000 JAN 01 12:00:00",
        "JD 2451545.0",
        "2010-03-15T00:00:00",
    ];

    for time_str in test_times.iter() {
        let cspice_et = cspice_str2et(time_str);
        let muad_et = EpochTDB::parse(time_str).expect(&format!("Failed to parse: {}", time_str));

        assert_close(
            muad_et.0,
            cspice_et,
            TIME_PARSE_TOLERANCE,
            &format!("Time string: {}", time_str),
        );
    }
}

// ============================================================================
// TDB/UTC Conversion Tests
// ============================================================================

#[test]
fn validate_utc_to_tdb_j2000() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    // Load LSK for both
    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());

    let kernel = SpiceKernel::load(&lsk_path()).expect("Failed to load LSK");
    assert!(kernel.has_lsk(), "Kernel should have LSK data");

    let utc_str = "2000-01-01T12:00:00";

    // CSPICE utc2et
    let cspice_et = cspice_utc2et(utc_str);

    // muad-dib utc_to_tdb
    let muad_tdb = utc_to_tdb(&kernel, utc_str).expect("Failed to convert UTC to TDB");

    assert_close(
        muad_tdb.0,
        cspice_et,
        TDB_UTC_TOLERANCE,
        "UTC to TDB at J2000",
    );
}

#[test]
fn validate_utc_to_tdb_recent() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());

    let kernel = SpiceKernel::load(&lsk_path()).expect("Failed to load LSK");

    let utc_str = "2020-06-15T14:30:00";

    let cspice_et = cspice_utc2et(utc_str);
    let muad_tdb = utc_to_tdb(&kernel, utc_str).expect("Failed to convert UTC to TDB");

    assert_close(
        muad_tdb.0,
        cspice_et,
        TDB_UTC_TOLERANCE,
        "UTC to TDB for 2020",
    );
}

#[test]
fn validate_tdb_to_utc_j2000() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());

    let kernel = SpiceKernel::load(&lsk_path()).expect("Failed to load LSK");

    // TDB = 0 is J2000 TDB epoch
    let tdb = 0.0;

    // CSPICE et2utc
    let cspice_utc = cspice_et2utc(tdb, "ISOC", 3);

    // muad-dib tdb_to_utc
    let muad_utc =
        tdb_to_utc(&kernel, EpochTDB(tdb), TimeFormat::Iso8601).expect("Failed to convert TDB to UTC");

    // Both should produce UTC times around 2000-01-01T11:58:55
    // (J2000 TDB is about 64 seconds ahead of UTC due to leap seconds)
    assert!(
        cspice_utc.starts_with("2000-01-01T11:58"),
        "CSPICE J2000 UTC should be ~11:58: {}",
        cspice_utc
    );
    assert!(
        muad_utc.starts_with("2000-01-01T11:58"),
        "muad-dib J2000 UTC should be ~11:58: {}",
        muad_utc
    );
}

#[test]
fn validate_utc_tdb_round_trip() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());

    let kernel = SpiceKernel::load(&lsk_path()).expect("Failed to load LSK");

    // Start with a UTC time
    let utc_str = "2015-07-04T12:00:00";

    // Convert to TDB
    let tdb = utc_to_tdb(&kernel, utc_str).expect("UTC to TDB failed");

    // Convert back to UTC
    let utc_back =
        tdb_to_utc(&kernel, tdb, TimeFormat::Iso8601).expect("TDB to UTC failed");

    // Parse the returned UTC string and convert again
    let tdb_again = utc_to_tdb(&kernel, &utc_back).expect("Second UTC to TDB failed");

    // Should be very close
    assert_close(
        tdb.0,
        tdb_again.0,
        TDB_UTC_TOLERANCE,
        "Round-trip UTC→TDB→UTC→TDB",
    );
}

#[test]
fn validate_leap_second_boundary() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());

    let kernel = SpiceKernel::load(&lsk_path()).expect("Failed to load LSK");

    // Test times around a leap second (2017-01-01 had a leap second)
    let before_leap = "2016-12-31T23:59:59";
    let after_leap = "2017-01-01T00:00:00";

    let cspice_before = cspice_utc2et(before_leap);
    let cspice_after = cspice_utc2et(after_leap);

    let muad_before = utc_to_tdb(&kernel, before_leap).expect("Before leap failed");
    let muad_after = utc_to_tdb(&kernel, after_leap).expect("After leap failed");

    // The difference should account for the leap second
    // In UTC, these are 1 second apart
    // In TDB, they should be 2 seconds apart (1 normal + 1 leap second)
    let cspice_diff = cspice_after - cspice_before;
    let muad_diff = muad_after.0 - muad_before.0;

    assert_close(muad_diff, cspice_diff, TDB_UTC_TOLERANCE, "Leap second difference");
}

#[test]
fn validate_multiple_epochs() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let mut cspice_kernels = CspiceKernels::new();
    cspice_kernels.load(&lsk_path());

    let kernel = SpiceKernel::load(&lsk_path()).expect("Failed to load LSK");

    let test_utc_times = [
        "1999-01-01T00:00:00",
        "2000-01-01T00:00:00",
        "2005-06-15T12:30:00",
        "2010-12-31T23:59:59",
        "2015-07-01T00:00:00",
        "2020-03-14T15:09:26", // Pi day
    ];

    for utc_str in test_utc_times.iter() {
        let cspice_et = cspice_utc2et(utc_str);
        let muad_tdb = utc_to_tdb(&kernel, utc_str)
            .expect(&format!("Failed to convert: {}", utc_str));

        assert_close(
            muad_tdb.0,
            cspice_et,
            TDB_UTC_TOLERANCE,
            &format!("UTC to TDB for {}", utc_str),
        );
    }
}

// ============================================================================
// Leap Second Data Tests
// ============================================================================

#[test]
fn validate_leap_second_data_loaded() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let kernel = SpiceKernel::load(&lsk_path()).expect("Failed to load LSK");

    assert!(kernel.has_lsk(), "Kernel should have LSK data");

    let lsk = kernel.lsk_data().expect("Should have LSK data");

    // Check standard constants
    assert!(
        (lsk.delta_t_a - 32.184).abs() < 1e-6,
        "DELTA_T_A should be 32.184"
    );
    assert!(lsk.leap_seconds.len() > 20, "Should have many leap seconds");

    // Check that leap seconds are sorted by epoch
    for i in 1..lsk.leap_seconds.len() {
        assert!(
            lsk.leap_seconds[i].1 > lsk.leap_seconds[i - 1].1,
            "Leap seconds should be sorted by epoch"
        );
    }
}

#[test]
fn validate_delta_at_lookup() {
    let _lock = CSPICE_LOCK.lock().unwrap();

    let kernel = SpiceKernel::load(&lsk_path()).expect("Failed to load LSK");
    let lsk = kernel.lsk_data().expect("Should have LSK data");

    // Before 1972: should return 0
    let ancient = -1e10;
    assert_eq!(lsk.delta_at(ancient), 0.0, "No leap seconds before 1972");

    // After 2017: should be 37 (as of naif0012.tls)
    let recent = 1e9; // Well after 2017
    assert!(
        lsk.delta_at(recent) >= 37.0,
        "Should have at least 37 leap seconds after 2017"
    );

    // Delta should increase over time
    let earlier = EpochTDB::parse("2000-01-01T12:00:00").unwrap();
    let later = EpochTDB::parse("2020-01-01T12:00:00").unwrap();
    assert!(
        lsk.delta_at(later.0) >= lsk.delta_at(earlier.0),
        "Leap seconds should not decrease over time"
    );
}
