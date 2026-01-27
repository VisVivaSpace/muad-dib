//! Brief - Display time coverage summaries for NAIF SPICE files.
//!
//! Similar to the NAIF `brief` utility, displays coverage intervals
//! for bodies/frames in SPK, CK, and BPCK files.

use clap::{value_parser, Arg, ArgAction, Command};
use muad_dib::brief::{collect_summaries, display, BriefOptions, TimeFormat};
use std::path::PathBuf;

fn main() {
    let app = Command::new("brief")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Display time coverage summary for NAIF SPICE files")
        .arg(
            Arg::new("files")
                .value_name("FILE")
                .value_parser(value_parser!(PathBuf))
                .required(true)
                .num_args(1..)
                .help("Input files (DAF or serialized formats)"),
        )
        .arg(
            Arg::new("tabular")
                .short('t')
                .long("tabular")
                .action(ArgAction::SetTrue)
                .help("Tabular format (body | start | end)"),
        )
        .arg(
            Arg::new("centers")
                .short('c')
                .long("centers")
                .action(ArgAction::SetTrue)
                .help("Show centers-of-motion (SPK) or base frames (BPCK)"),
        )
        .arg(
            Arg::new("all")
                .short('a')
                .long("all")
                .action(ArgAction::SetTrue)
                .help("Combine all files into single summary"),
        )
        .arg(
            Arg::new("numeric")
                .short('n')
                .long("numeric")
                .action(ArgAction::SetTrue)
                .help("Show numeric IDs only (no name lookup)"),
        )
        .arg(
            Arg::new("sort-time")
                .short('s')
                .long("sort-time")
                .action(ArgAction::SetTrue)
                .help("Sort tabular output by start time (requires -t)"),
        )
        .arg(
            Arg::new("group")
                .short('g')
                .long("group")
                .action(ArgAction::SetTrue)
                .help("Group by identical coverage"),
        )
        // Time format options (mutually exclusive)
        .arg(
            Arg::new("et")
                .long("et")
                .action(ArgAction::SetTrue)
                .help("Calendar ET format (default): \"YYYY MON DD HR:MN:SC.DDD\""),
        )
        .arg(
            Arg::new("utc")
                .long("utc")
                .action(ArgAction::SetTrue)
                .conflicts_with("et")
                .help("Calendar UTC format (requires leap seconds)"),
        )
        .arg(
            Arg::new("utc-doy")
                .long("utc-doy")
                .action(ArgAction::SetTrue)
                .conflicts_with_all(["et", "utc"])
                .help("UTC day-of-year format (requires leap seconds)"),
        )
        .arg(
            Arg::new("et-sec")
                .long("et-sec")
                .action(ArgAction::SetTrue)
                .conflicts_with_all(["et", "utc", "utc-doy"])
                .help("ET seconds past J2000"),
        )
        .arg(
            Arg::new("rel")
                .long("rel")
                .action(ArgAction::SetTrue)
                .help("Show reference frame column (CK files)"),
        )
        .arg(
            Arg::new("types")
                .short('y')
                .long("types")
                .action(ArgAction::SetTrue)
                .help("Show segment data types (SPK/CK/BPCK type numbers)"),
        );

    let matches = app.get_matches();

    // Get input files
    let files: Vec<_> = matches
        .get_many::<PathBuf>("files")
        .expect("Must specify at least one file")
        .collect();

    // Build options
    let time_format = if matches.get_flag("et-sec") {
        TimeFormat::SecondsET
    } else if matches.get_flag("utc") {
        eprintln!("Warning: UTC format requires leap seconds data (not implemented)");
        eprintln!("         Displaying ET times with '*' marker");
        TimeFormat::CalendarUTC
    } else if matches.get_flag("utc-doy") {
        eprintln!("Warning: UTC-DOY format requires leap seconds data (not implemented)");
        eprintln!("         Displaying ET times with '*' marker");
        TimeFormat::DoyUTC
    } else {
        TimeFormat::CalendarET
    };

    let opts = BriefOptions {
        tabular: matches.get_flag("tabular"),
        show_centers: matches.get_flag("centers"),
        combine_all: matches.get_flag("all"),
        numeric_only: matches.get_flag("numeric"),
        sort_by_time: matches.get_flag("sort-time"),
        group_coverage: matches.get_flag("group"),
        time_format,
        show_rel_frame: matches.get_flag("rel"),
        show_types: matches.get_flag("types"),
    };

    // Collect summaries from all files
    let mut all_summaries = Vec::new();

    for file_path in &files {
        match collect_summaries(file_path) {
            Ok(summaries) => all_summaries.extend(summaries),
            Err(e) => {
                eprintln!("Error reading {}: {}", file_path.display(), e);
                std::process::exit(1);
            }
        }
    }

    if all_summaries.is_empty() {
        eprintln!("No valid files found");
        std::process::exit(1);
    }

    // Display the summaries
    display::display_summaries(&all_summaries, &opts);
}
