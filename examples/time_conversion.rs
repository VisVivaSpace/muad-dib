//! Example: Time string parsing and conversion
//!
//! Demonstrates parsing various time string formats to TDB epochs,
//! and converting between TDB and calendar representations.
//!
//! Run with: cargo run --example time_conversion

use muad_dib::spice::{format_calendar, format_iso8601, tdb_to_calendar, EpochTDB, TimeFormat};

fn main() {
    println!("=== Time Conversion Examples ===\n");

    // Parse different time string formats
    println!("Parsing time strings to TDB:\n");

    // ISO 8601 format
    let iso_str = "2020-06-15T12:30:00";
    match EpochTDB::parse(iso_str) {
        Ok(epoch) => {
            println!("  ISO 8601: \"{}\"", iso_str);
            println!("    -> TDB: {:.3} seconds past J2000", epoch.0);
        }
        Err(e) => println!("  Error parsing {}: {:?}", iso_str, e),
    }
    println!();

    // Calendar format (SPICE-style)
    let cal_str = "2020 JUN 15 12:30:00";
    match EpochTDB::parse_with_format(cal_str, TimeFormat::Calendar) {
        Ok(epoch) => {
            println!("  Calendar: \"{}\"", cal_str);
            println!("    -> TDB: {:.3} seconds past J2000", epoch.0);
        }
        Err(e) => println!("  Error parsing {}: {:?}", cal_str, e),
    }
    println!();

    // Julian Date format
    let jd_str = "JD 2459016.0208333";
    match EpochTDB::parse_with_format(jd_str, TimeFormat::JulianDate) {
        Ok(epoch) => {
            println!("  Julian Date: \"{}\"", jd_str);
            println!("    -> TDB: {:.3} seconds past J2000", epoch.0);
        }
        Err(e) => println!("  Error parsing {}: {:?}", jd_str, e),
    }
    println!();

    // Notable epochs
    println!("Notable epochs:\n");

    let epochs = [
        ("J2000.0 epoch", EpochTDB(0.0)),
        ("One day after J2000", EpochTDB(86400.0)),
        ("One year after J2000", EpochTDB(366.0 * 86400.0)), // 2000 is leap year
    ];

    for (name, epoch) in epochs {
        // tdb_to_calendar returns tuple: (year, month, day, hour, minute, second)
        let (year, month, day, hour, minute, second) = tdb_to_calendar(epoch.0);
        println!("  {}: TDB = {:.1} s", name, epoch.0);
        println!(
            "    Calendar: {:04}-{:02}-{:02} {:02}:{:02}:{:06.3}",
            year, month, day, hour, minute, second
        );
        println!("    ISO 8601: {}", format_iso8601(epoch.0));
        println!("    SPICE-style: {}", format_calendar(epoch.0));
        println!();
    }

    // Round-trip demonstration
    println!("Round-trip demonstration:\n");

    let original = "2025-12-31T23:59:59";
    if let Ok(epoch) = EpochTDB::parse(original) {
        let formatted = format_iso8601(epoch.0);
        println!("  Original:   \"{}\"", original);
        println!("  TDB value:  {:.3} seconds", epoch.0);
        println!("  Formatted:  \"{}\"", formatted);
    }
}
