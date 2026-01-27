//! Time string parsing for TDB epochs.
//!
//! Provides parsing of common time string formats into TDB seconds past J2000.
//! Equivalent to CSPICE's `str2et()` function.
//!
//! # Supported Formats
//!
//! - **ISO 8601**: `"2000-01-01T12:00:00"`, `"2000-01-01T12:00:00.123"`
//! - **Calendar**: `"2000 JAN 01 12:00:00"`, `"2000-JAN-01 12:00:00.000"`
//! - **Julian Date**: `"JD 2451545.0"`, `"JD2451545.0"`
//!
//! Note: For accurate TDB/UTC conversion, use `utc_to_tdb()` with leap second data.
//! The `EpochTDB::parse()` method assumes the input is already in TDB.
//!
//! # Example
//!
//! ```
//! use muad_dib::types::EpochTDB;
//! use muad_dib::spice::time::TimeFormat;
//!
//! // J2000 epoch in ISO format
//! let epoch = EpochTDB::parse("2000-01-01T12:00:00").unwrap();
//! assert!(epoch.0.abs() < 1.0);  // Should be very close to 0 TDB
//!
//! // Julian Date format
//! let epoch = EpochTDB::parse("JD 2451545.0").unwrap();
//! assert!(epoch.0.abs() < 1.0);  // J2000 Julian Date
//! ```

use crate::error::Error;
use crate::prelude::*;
use crate::types::EpochTDB;

/// Time format hint for parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeFormat {
    /// ISO 8601 format: "2000-01-01T12:00:00"
    Iso8601,
    /// Calendar format: "2000 JAN 01 12:00:00"
    Calendar,
    /// Julian Date: "JD 2451545.0"
    JulianDate,
}

impl TimeFormat {
    /// Detect the format of a time string.
    pub fn detect(s: &str) -> Option<TimeFormat> {
        let s = s.trim().to_uppercase();

        if s.starts_with("JD") {
            return Some(TimeFormat::JulianDate);
        }

        // ISO 8601 has 'T' separator between date and time
        if s.contains('T') && s.chars().filter(|c| *c == '-').count() >= 2 {
            return Some(TimeFormat::Iso8601);
        }

        // Calendar format has month abbreviation
        if contains_month_abbrev(&s) {
            return Some(TimeFormat::Calendar);
        }

        // Could be ISO without T (space-separated)
        if s.chars().filter(|c| *c == '-').count() >= 2 && s.contains(':') {
            return Some(TimeFormat::Iso8601);
        }

        None
    }
}

/// Check if string contains a month abbreviation.
fn contains_month_abbrev(s: &str) -> bool {
    const MONTHS: [&str; 12] = [
        "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
    ];
    MONTHS.iter().any(|m| s.contains(m))
}

/// Convert month abbreviation to month number (1-12).
fn month_from_abbrev(abbrev: &str) -> Option<u32> {
    match abbrev.to_uppercase().as_str() {
        "JAN" => Some(1),
        "FEB" => Some(2),
        "MAR" => Some(3),
        "APR" => Some(4),
        "MAY" => Some(5),
        "JUN" => Some(6),
        "JUL" => Some(7),
        "AUG" => Some(8),
        "SEP" => Some(9),
        "OCT" => Some(10),
        "NOV" => Some(11),
        "DEC" => Some(12),
        _ => None,
    }
}

/// J2000 epoch as Julian Date.
const J2000_JD: f64 = 2451545.0;

/// Seconds per day.
const SECONDS_PER_DAY: f64 = 86400.0;

impl EpochTDB {
    /// Parse a time string to TDB seconds past J2000.
    ///
    /// Automatically detects the format. For UTC strings, use `utc_to_tdb()`
    /// instead to properly account for leap seconds.
    ///
    /// # Supported Formats
    ///
    /// - ISO 8601: `"2000-01-01T12:00:00"`, `"2000-01-01T12:00:00.123"`
    /// - Calendar: `"2000 JAN 01 12:00:00"`, `"2000-JAN-01 12:00:00"`
    /// - Julian Date: `"JD 2451545.0"`
    ///
    /// # Example
    ///
    /// ```
    /// use muad_dib::types::EpochTDB;
    ///
    /// let epoch = EpochTDB::parse("2000-01-01T12:00:00").unwrap();
    /// assert!(epoch.0.abs() < 1.0);
    /// ```
    pub fn parse(time_str: &str) -> Result<EpochTDB> {
        let trimmed = time_str.trim();

        match TimeFormat::detect(trimmed) {
            Some(TimeFormat::JulianDate) => parse_julian_date(trimmed),
            Some(TimeFormat::Iso8601) => parse_iso8601(trimmed),
            Some(TimeFormat::Calendar) => parse_calendar(trimmed),
            None => Err(Error::TimeParseError {
                input: time_str.to_string(),
            }),
        }
    }

    /// Parse a time string with an explicit format hint.
    pub fn parse_with_format(time_str: &str, format: TimeFormat) -> Result<EpochTDB> {
        let trimmed = time_str.trim();

        match format {
            TimeFormat::JulianDate => parse_julian_date(trimmed),
            TimeFormat::Iso8601 => parse_iso8601(trimmed),
            TimeFormat::Calendar => parse_calendar(trimmed),
        }
    }
}

/// Parse Julian Date format: "JD 2451545.0" or "JD2451545.0"
fn parse_julian_date(s: &str) -> Result<EpochTDB> {
    let upper = s.to_uppercase();
    let jd_str = upper
        .strip_prefix("JD")
        .ok_or_else(|| Error::TimeParseError {
            input: s.to_string(),
        })?
        .trim();

    let jd: f64 = jd_str.parse().map_err(|_| Error::TimeParseError {
        input: s.to_string(),
    })?;

    // Convert JD to TDB seconds past J2000
    let tdb = (jd - J2000_JD) * SECONDS_PER_DAY;
    Ok(EpochTDB(tdb))
}

/// Parse ISO 8601 format: "2000-01-01T12:00:00" or "2000-01-01 12:00:00"
fn parse_iso8601(s: &str) -> Result<EpochTDB> {
    // Replace T with space for uniform parsing
    let normalized = s.replace(['T', 't'], " ");
    let parts: Vec<&str> = normalized.split_whitespace().collect();

    if parts.is_empty() {
        return Err(Error::TimeParseError {
            input: s.to_string(),
        });
    }

    // Parse date part
    let date_parts: Vec<&str> = parts[0].split('-').collect();
    if date_parts.len() != 3 {
        return Err(Error::TimeParseError {
            input: s.to_string(),
        });
    }

    let year: i32 = date_parts[0].parse().map_err(|_| Error::TimeParseError {
        input: s.to_string(),
    })?;
    let month: u32 = date_parts[1].parse().map_err(|_| Error::TimeParseError {
        input: s.to_string(),
    })?;
    let day: u32 = date_parts[2].parse().map_err(|_| Error::TimeParseError {
        input: s.to_string(),
    })?;

    // Parse time part (optional)
    let (hour, minute, second) = if parts.len() > 1 {
        parse_time_component(parts[1])?
    } else {
        (0, 0, 0.0)
    };

    calendar_to_tdb(year, month, day, hour, minute, second)
}

/// Parse calendar format: "2000 JAN 01 12:00:00" or "2000-JAN-01 12:00:00.000"
fn parse_calendar(s: &str) -> Result<EpochTDB> {
    // Normalize separators
    let normalized = s.replace('-', " ");
    let parts: Vec<&str> = normalized.split_whitespace().collect();

    if parts.len() < 3 {
        return Err(Error::TimeParseError {
            input: s.to_string(),
        });
    }

    // Determine order: could be "2000 JAN 01" or "01 JAN 2000"
    let (year, month, day) = if let Some(m) = month_from_abbrev(parts[1]) {
        // Format: YEAR MON DAY or DAY MON YEAR
        let first: i32 = parts[0].parse().map_err(|_| Error::TimeParseError {
            input: s.to_string(),
        })?;
        let third: i32 = parts[2].parse().map_err(|_| Error::TimeParseError {
            input: s.to_string(),
        })?;

        if first > 31 {
            // YEAR MON DAY
            (first, m, third as u32)
        } else {
            // DAY MON YEAR
            (third, m, first as u32)
        }
    } else {
        return Err(Error::TimeParseError {
            input: s.to_string(),
        });
    };

    // Parse time part (optional)
    let (hour, minute, second) = if parts.len() > 3 {
        parse_time_component(parts[3])?
    } else {
        (0, 0, 0.0)
    };

    calendar_to_tdb(year, month, day, hour, minute, second)
}

/// Parse time component "HH:MM:SS" or "HH:MM:SS.sss"
fn parse_time_component(s: &str) -> Result<(u32, u32, f64)> {
    let parts: Vec<&str> = s.split(':').collect();

    let hour: u32 = if !parts.is_empty() {
        parts[0].parse().unwrap_or(0)
    } else {
        0
    };

    let minute: u32 = if parts.len() > 1 {
        parts[1].parse().unwrap_or(0)
    } else {
        0
    };

    let second: f64 = if parts.len() > 2 {
        parts[2].parse().unwrap_or(0.0)
    } else {
        0.0
    };

    Ok((hour, minute, second))
}

/// Convert calendar date/time to TDB seconds past J2000.
///
/// Uses a simplified algorithm (no leap seconds, no TDB-TT difference).
/// For accurate conversion, use `utc_to_tdb()` with leap second data.
fn calendar_to_tdb(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: f64) -> Result<EpochTDB> {
    // Compute Julian Date using the algorithm from "Astronomical Algorithms" by Jean Meeus
    let y = if month <= 2 {
        year - 1
    } else {
        year
    } as f64;

    let m = if month <= 2 {
        month + 12
    } else {
        month
    } as f64;

    let d = day as f64;

    // Julian Day Number
    let a = (y / 100.0).floor();
    let b = 2.0 - a + (a / 4.0).floor();

    let jd = (365.25 * (y + 4716.0)).floor()
        + (30.6001 * (m + 1.0)).floor()
        + d
        + b
        - 1524.5;

    // Add time of day
    let day_fraction = (hour as f64) / 24.0 + (minute as f64) / 1440.0 + second / 86400.0;
    let jd_full = jd + day_fraction;

    // Convert to TDB seconds past J2000
    let tdb = (jd_full - J2000_JD) * SECONDS_PER_DAY;
    Ok(EpochTDB(tdb))
}

/// Convert TDB seconds past J2000 to calendar components.
///
/// Returns (year, month, day, hour, minute, second).
pub fn tdb_to_calendar(tdb: f64) -> (i32, u32, u32, u32, u32, f64) {
    // Convert to Julian Date
    let jd = tdb / SECONDS_PER_DAY + J2000_JD;

    // Algorithm from "Astronomical Algorithms" by Jean Meeus
    let z = (jd + 0.5).floor();
    let f = jd + 0.5 - z;

    let a = if z < 2299161.0 {
        z
    } else {
        let alpha = ((z - 1867216.25) / 36524.25).floor();
        z + 1.0 + alpha - (alpha / 4.0).floor()
    };

    let b = a + 1524.0;
    let c = ((b - 122.1) / 365.25).floor();
    let d = (365.25 * c).floor();
    let e = ((b - d) / 30.6001).floor();

    let day = (b - d - (30.6001 * e).floor()) as u32;

    let month = if e < 14.0 {
        (e - 1.0) as u32
    } else {
        (e - 13.0) as u32
    };

    let year = if month > 2 {
        (c - 4716.0) as i32
    } else {
        (c - 4715.0) as i32
    };

    // Extract time of day
    let day_frac = f;
    let total_seconds = day_frac * SECONDS_PER_DAY;
    let hour = (total_seconds / 3600.0) as u32;
    let minute = ((total_seconds % 3600.0) / 60.0) as u32;
    let second = total_seconds % 60.0;

    (year, month, day, hour, minute, second)
}

/// Format TDB epoch as ISO 8601 string.
pub fn format_iso8601(tdb: f64) -> String {
    let (year, month, day, hour, minute, second) = tdb_to_calendar(tdb);
    let whole_sec = second as u32;
    let frac = second - whole_sec as f64;

    if frac > 1e-9 {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}",
            year,
            month,
            day,
            hour,
            minute,
            whole_sec,
            (frac * 1000.0).round() as u32
        )
    } else {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            year, month, day, hour, minute, whole_sec
        )
    }
}

/// Format TDB epoch as calendar string.
pub fn format_calendar(tdb: f64) -> String {
    const MONTHS: [&str; 12] = [
        "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
    ];

    let (year, month, day, hour, minute, second) = tdb_to_calendar(tdb);
    let month_str = MONTHS[(month - 1) as usize];
    let whole_sec = second as u32;
    let frac = second - whole_sec as f64;

    if frac > 1e-9 {
        format!(
            "{:04} {} {:02} {:02}:{:02}:{:02}.{:03}",
            year,
            month_str,
            day,
            hour,
            minute,
            whole_sec,
            (frac * 1000.0).round() as u32
        )
    } else {
        format!(
            "{:04} {} {:02} {:02}:{:02}:{:02}",
            year, month_str, day, hour, minute, whole_sec
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1.0; // 1 second tolerance for simplified algorithm

    #[test]
    fn test_parse_j2000_iso() {
        // J2000 epoch: 2000-01-01T12:00:00 TDB
        let epoch = EpochTDB::parse("2000-01-01T12:00:00").unwrap();
        assert!(epoch.0.abs() < EPSILON, "J2000 should be near 0 TDB: {}", epoch.0);
    }

    #[test]
    fn test_parse_j2000_calendar() {
        let epoch = EpochTDB::parse("2000 JAN 01 12:00:00").unwrap();
        assert!(epoch.0.abs() < EPSILON, "J2000 should be near 0 TDB: {}", epoch.0);
    }

    #[test]
    fn test_parse_julian_date() {
        // J2000 Julian Date
        let epoch = EpochTDB::parse("JD 2451545.0").unwrap();
        assert!(epoch.0.abs() < EPSILON, "JD 2451545.0 should be near 0 TDB: {}", epoch.0);

        // Without space
        let epoch2 = EpochTDB::parse("JD2451545.0").unwrap();
        assert!((epoch.0 - epoch2.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_with_fractional_seconds() {
        let epoch1 = EpochTDB::parse("2000-01-01T12:00:00.500").unwrap();
        let epoch2 = EpochTDB::parse("2000-01-01T12:00:00").unwrap();
        assert!((epoch1.0 - epoch2.0 - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_parse_various_calendar_formats() {
        // With dashes
        let e1 = EpochTDB::parse("2000-JAN-01 12:00:00").unwrap();
        let e2 = EpochTDB::parse("2000 JAN 01 12:00:00").unwrap();
        assert!((e1.0 - e2.0).abs() < 0.01);

        // Day-Month-Year order
        let e3 = EpochTDB::parse("01 JAN 2000 12:00:00").unwrap();
        assert!((e1.0 - e3.0).abs() < 0.01);
    }

    #[test]
    fn test_time_format_detection() {
        assert_eq!(TimeFormat::detect("2000-01-01T12:00:00"), Some(TimeFormat::Iso8601));
        assert_eq!(TimeFormat::detect("2000 JAN 01 12:00:00"), Some(TimeFormat::Calendar));
        assert_eq!(TimeFormat::detect("JD 2451545.0"), Some(TimeFormat::JulianDate));
        assert_eq!(TimeFormat::detect("garbage"), None);
    }

    #[test]
    fn test_round_trip() {
        let original = 123456789.0; // Some arbitrary TDB
        let iso = format_iso8601(original);
        let parsed = EpochTDB::parse(&iso).unwrap();
        assert!((original - parsed.0).abs() < 1.0, "Round-trip failed: {} vs {}", original, parsed.0);
    }

    #[test]
    fn test_tdb_to_calendar() {
        // J2000 epoch
        let (year, month, day, hour, minute, _second) = tdb_to_calendar(0.0);
        assert_eq!(year, 2000);
        assert_eq!(month, 1);
        assert_eq!(day, 1);
        assert_eq!(hour, 12);
        assert_eq!(minute, 0);
    }

    #[test]
    fn test_format_iso8601() {
        let s = format_iso8601(0.0);
        assert!(s.starts_with("2000-01-01T12:00:00"));
    }

    #[test]
    fn test_format_calendar() {
        let s = format_calendar(0.0);
        assert!(s.contains("JAN"));
        assert!(s.contains("2000"));
    }

    #[test]
    fn test_parse_error() {
        let result = EpochTDB::parse("not a date");
        assert!(result.is_err());
    }

    #[test]
    fn test_one_year_later() {
        // 2001-01-01T12:00:00 should be about 366 days later (2000 is a leap year)
        let epoch = EpochTDB::parse("2001-01-01T12:00:00").unwrap();
        let expected = 366.0 * SECONDS_PER_DAY; // 2000 is a leap year
        assert!((epoch.0 - expected).abs() < SECONDS_PER_DAY, "One year: {} vs {}", epoch.0, expected);
    }
}
