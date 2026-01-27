//! Time formatting functions for brief output.
//!
//! Converts TDB seconds past J2000 to various human-readable formats.
//! Also handles SCLK tick display for CK files.

use super::{TimeFormat, TimeKind};

/// J2000 epoch in Julian Date
const J2000_JD: f64 = 2451545.0;

/// Seconds per day
const SECONDS_PER_DAY: f64 = 86400.0;

/// Month names
const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN",
    "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

/// Format a time value according to the specified format.
pub fn format_time(tdb_seconds: f64, format: TimeFormat) -> String {
    match format {
        TimeFormat::CalendarET => format_calendar_et(tdb_seconds),
        TimeFormat::SecondsET => format_seconds_et(tdb_seconds),
        TimeFormat::CalendarUTC | TimeFormat::DoyUTC => {
            // UTC requires leap seconds - return ET with warning marker
            format!("{}*", format_calendar_et(tdb_seconds))
        }
    }
}

/// Format SCLK ticks for display.
/// Without an SCLK kernel, we can only display the raw encoded ticks.
pub fn format_sclk_ticks(sclk_ticks: f64) -> String {
    format!("{:.6} SCLK", sclk_ticks)
}

/// Format a time value for display, selecting format based on time kind.
/// For SCLK times, the format parameter is ignored and raw ticks are shown.
pub fn format_time_for_display(time: f64, format: TimeFormat, kind: TimeKind) -> String {
    match kind {
        TimeKind::SCLK => format_sclk_ticks(time),
        TimeKind::TDB => format_time(time, format),
    }
}

/// Format as Calendar ET: "YYYY MON DD HR:MN:SC.DDD"
fn format_calendar_et(tdb_seconds: f64) -> String {
    // Convert TDB seconds past J2000 to Julian Date
    let jd = J2000_JD + tdb_seconds / SECONDS_PER_DAY;

    // Convert JD to calendar date using standard algorithm
    let (year, month, day, hour, minute, second) = jd_to_calendar(jd);

    format!(
        "{:4} {} {:02} {:02}:{:02}:{:06.3}",
        year,
        MONTHS[(month - 1) as usize],
        day,
        hour,
        minute,
        second
    )
}

/// Format as ET seconds past J2000: "SSSSSSSSSS.SSSSSS"
fn format_seconds_et(tdb_seconds: f64) -> String {
    format!("{:.6}", tdb_seconds)
}

/// Convert Julian Date to calendar date.
/// Returns (year, month, day, hour, minute, second).
fn jd_to_calendar(jd: f64) -> (i32, i32, i32, i32, i32, f64) {
    // Standard algorithm for JD to Gregorian calendar
    let z = (jd + 0.5).floor() as i64;
    let f = jd + 0.5 - z as f64;

    let a = if z < 2299161 {
        z
    } else {
        let alpha = ((z as f64 - 1867216.25) / 36524.25).floor() as i64;
        z + 1 + alpha - alpha / 4
    };

    let b = a + 1524;
    let c = ((b as f64 - 122.1) / 365.25).floor() as i64;
    let d = (365.25 * c as f64).floor() as i64;
    let e = ((b - d) as f64 / 30.6001).floor() as i64;

    let day = (b - d - (30.6001 * e as f64).floor() as i64) as i32;
    let month = if e < 14 { e - 1 } else { e - 13 } as i32;
    let year = if month > 2 { c - 4716 } else { c - 4715 } as i32;

    // Convert fraction of day to time
    let day_fraction = f * 24.0;
    let hour = day_fraction.floor() as i32;
    let min_fraction = (day_fraction - hour as f64) * 60.0;
    let minute = min_fraction.floor() as i32;
    let second = (min_fraction - minute as f64) * 60.0;

    (year, month, day, hour, minute, second)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_j2000_epoch() {
        // J2000 = 2000 JAN 01 12:00:00.000 TDB
        let result = format_calendar_et(0.0);
        assert_eq!(result, "2000 JAN 01 12:00:00.000");
    }

    #[test]
    fn test_one_day_after_j2000() {
        // One day after J2000
        let result = format_calendar_et(86400.0);
        assert_eq!(result, "2000 JAN 02 12:00:00.000");
    }

    #[test]
    fn test_negative_time() {
        // One day before J2000
        let result = format_calendar_et(-86400.0);
        assert_eq!(result, "1999 DEC 31 12:00:00.000");
    }

    #[test]
    fn test_seconds_format() {
        let result = format_seconds_et(123456.789);
        assert_eq!(result, "123456.789000");
    }

    #[test]
    fn test_format_time_dispatcher() {
        let t = 0.0;
        assert_eq!(format_time(t, TimeFormat::CalendarET), "2000 JAN 01 12:00:00.000");
        assert_eq!(format_time(t, TimeFormat::SecondsET), "0.000000");
    }

    #[test]
    fn test_format_sclk_ticks() {
        assert_eq!(format_sclk_ticks(1287100360.885000), "1287100360.885000 SCLK");
        assert_eq!(format_sclk_ticks(0.0), "0.000000 SCLK");
    }

    #[test]
    fn test_format_time_for_display() {
        use super::TimeKind;

        // TDB uses format_time
        assert_eq!(
            format_time_for_display(0.0, TimeFormat::CalendarET, TimeKind::TDB),
            "2000 JAN 01 12:00:00.000"
        );
        assert_eq!(
            format_time_for_display(0.0, TimeFormat::SecondsET, TimeKind::TDB),
            "0.000000"
        );

        // SCLK ignores format and shows ticks
        assert_eq!(
            format_time_for_display(1287100360.885, TimeFormat::CalendarET, TimeKind::SCLK),
            "1287100360.885000 SCLK"
        );
        assert_eq!(
            format_time_for_display(1287100360.885, TimeFormat::SecondsET, TimeKind::SCLK),
            "1287100360.885000 SCLK"
        );
    }
}
