//! Display formatting for brief output.
//!
//! Handles default (grouped) and tabular output formats.
//! CK files display SCLK times and use "Instruments" instead of "Bodies".

use super::names::{format_frame_id, format_id, format_instrument_id};
use super::time::format_time_for_display;
use super::{BriefOptions, CoverageInterval, FileSummary, FileType, ObjectSummary};
use std::collections::HashMap;

/// Display summaries for multiple files.
pub fn display_summaries(summaries: &[FileSummary], opts: &BriefOptions) {
    if opts.combine_all {
        display_combined(summaries, opts);
    } else {
        for summary in summaries {
            display_file_summary(summary, opts);
            println!();
        }
    }
}

/// Display a single file summary.
fn display_file_summary(summary: &FileSummary, opts: &BriefOptions) {
    // Header
    println!("Brief summary for: {}", summary.filename);
    if !summary.internal_name.is_empty() {
        println!("Internal name: {}", summary.internal_name);
    }
    println!("Type: {}", summary.file_type);
    println!();

    if opts.tabular {
        display_tabular(&summary.objects, summary.file_type, opts);
    } else {
        display_grouped(&summary.objects, summary.file_type, opts);
    }
}

/// Display combined summary from all files.
fn display_combined(summaries: &[FileSummary], opts: &BriefOptions) {
    println!("Combined summary for {} file(s)", summaries.len());
    println!();

    // Merge all objects
    let mut all_objects: Vec<ObjectSummary> = Vec::new();
    for summary in summaries {
        all_objects.extend(summary.objects.clone());
    }

    // Merge intervals for same objects
    let mut object_map: HashMap<i32, ObjectSummary> = HashMap::new();
    for obj in all_objects {
        object_map
            .entry(obj.id)
            .and_modify(|existing| existing.intervals.extend(obj.intervals.clone()))
            .or_insert(obj);
    }

    let mut objects: Vec<ObjectSummary> = object_map.into_values().collect();
    objects.sort_by_key(|o| o.id);

    // Determine file type from first object, default to SPK
    let file_type = objects.first().map(|o| o.file_type).unwrap_or(FileType::SPK);

    if opts.tabular {
        display_tabular(&objects, file_type, opts);
    } else {
        display_grouped(&objects, file_type, opts);
    }
}

/// Row data for tabular output.
struct TabularRow {
    id_str: String,
    frame_str: Option<String>,
    av_str: Option<String>,
    start: String,
    end: String,
    sort_key: f64,
}

/// Display in tabular format.
fn display_tabular(objects: &[ObjectSummary], file_type: FileType, opts: &BriefOptions) {
    let is_ck = file_type == FileType::CK;

    // Collect all rows
    let mut rows: Vec<TabularRow> = Vec::new();

    for obj in objects {
        let id_str = if is_ck {
            format_instrument_id(obj.id, opts.numeric_only)
        } else {
            format_id(obj.id, opts.numeric_only)
        };

        let frame_str = if opts.show_rel_frame {
            obj.frame_code.map(|fc| format_frame_id(fc, opts.numeric_only))
        } else {
            None
        };

        for interval in &obj.intervals {
            let start = format_time_for_display(interval.start, opts.time_format, obj.time_kind);
            let end = format_time_for_display(interval.end, opts.time_format, obj.time_kind);

            // For CK files, show angular velocity indicator
            let av_str = if is_ck {
                interval.has_rates.map(|r| if r { "Y" } else { "N" }.to_string())
            } else {
                None
            };

            rows.push(TabularRow {
                id_str: id_str.clone(),
                frame_str: frame_str.clone(),
                av_str,
                start,
                end,
                sort_key: interval.start,
            });
        }
    }

    // Sort by start time if requested
    if opts.sort_by_time {
        rows.sort_by(|a, b| a.sort_key.partial_cmp(&b.sort_key).unwrap());
    }

    // Calculate column widths
    let id_label = if is_ck { "Instruments" } else { "Bodies" };
    let time_label = if is_ck { "SCLK" } else { "ET" };

    let id_width = rows.iter().map(|r| r.id_str.len()).max().unwrap_or(6).max(id_label.len());
    let frame_width = if opts.show_rel_frame {
        rows.iter()
            .filter_map(|r| r.frame_str.as_ref())
            .map(|s| s.len())
            .max()
            .unwrap_or(10)
            .max(10) // "Rel. Frame"
    } else {
        0
    };
    let time_width = rows.iter().map(|r| r.start.len()).max().unwrap_or(24).max(24);

    // Print header
    let start_header = format!("Start of Interval ({})", time_label);
    let end_header = format!("End of Interval ({})", time_label);

    if opts.show_rel_frame && is_ck {
        println!(
            "{:<iw$}  {:<fw$}  {:>2}  {:>tw$}  {:>tw$}",
            id_label, "Rel. Frame", "AV", start_header, end_header,
            iw = id_width, fw = frame_width, tw = time_width
        );
        println!(
            "{:-<iw$}  {:-<fw$}  {:->2}  {:-<tw$}  {:-<tw$}",
            "", "", "", "", "",
            iw = id_width, fw = frame_width, tw = time_width
        );
    } else if is_ck {
        println!(
            "{:<iw$}  {:>2}  {:>tw$}  {:>tw$}",
            id_label, "AV", start_header, end_header,
            iw = id_width, tw = time_width
        );
        println!(
            "{:-<iw$}  {:->2}  {:-<tw$}  {:-<tw$}",
            "", "", "", "",
            iw = id_width, tw = time_width
        );
    } else {
        println!(
            "{:<iw$}  {:>tw$}  {:>tw$}",
            id_label, start_header, end_header,
            iw = id_width, tw = time_width
        );
        println!(
            "{:-<iw$}  {:-<tw$}  {:-<tw$}",
            "", "", "",
            iw = id_width, tw = time_width
        );
    }

    // Print rows
    for row in &rows {
        if opts.show_rel_frame && is_ck {
            println!(
                "{:<iw$}  {:<fw$}  {:>2}  {:>tw$}  {:>tw$}",
                row.id_str,
                row.frame_str.as_deref().unwrap_or(""),
                row.av_str.as_deref().unwrap_or(""),
                row.start,
                row.end,
                iw = id_width, fw = frame_width, tw = time_width
            );
        } else if is_ck {
            println!(
                "{:<iw$}  {:>2}  {:>tw$}  {:>tw$}",
                row.id_str,
                row.av_str.as_deref().unwrap_or(""),
                row.start,
                row.end,
                iw = id_width, tw = time_width
            );
        } else {
            println!(
                "{:<iw$}  {:>tw$}  {:>tw$}",
                row.id_str,
                row.start,
                row.end,
                iw = id_width, tw = time_width
            );
        }
    }
}

/// Display grouped by coverage.
fn display_grouped(objects: &[ObjectSummary], file_type: FileType, opts: &BriefOptions) {
    if objects.is_empty() {
        println!("  (no objects found)");
        return;
    }

    let is_ck = file_type == FileType::CK;
    let id_label = if is_ck { "Instruments" } else { "Bodies" };
    let time_label = if is_ck { "SCLK" } else { "ET" };

    // Group objects by their coverage intervals
    let groups = if opts.group_coverage {
        group_by_coverage(objects)
    } else {
        // Each object in its own group
        objects
            .iter()
            .map(|o| vec![o.clone()])
            .collect()
    };

    for group in groups {
        if group.is_empty() {
            continue;
        }

        // Get time kind from first object in group
        let time_kind = group[0].time_kind;

        // Print object names
        let names: Vec<String> = group
            .iter()
            .map(|o| {
                if is_ck {
                    format_instrument_id(o.id, opts.numeric_only)
                } else {
                    format_id(o.id, opts.numeric_only)
                }
            })
            .collect();
        print!("{}: ", id_label);
        let indent = id_label.len() + 2;
        print_wrapped(&names, indent, 72);

        // Get coverage intervals from first object (all should be same if grouped)
        let intervals = &group[0].intervals;

        // Print coverage header
        let start_header = format!("Start of Interval ({})", time_label);
        let end_header = format!("End of Interval ({})", time_label);
        println!(
            "        {:^27}     {:^27}",
            start_header, end_header
        );
        println!(
            "        {:->27}     {:->27}",
            "", ""
        );

        // Print each interval
        for interval in intervals {
            let start = format_time_for_display(interval.start, opts.time_format, time_kind);
            let end = format_time_for_display(interval.end, opts.time_format, time_kind);
            println!("        {:>27}     {:>27}", start, end);
        }
        println!();
    }
}

/// Group objects with identical coverage intervals.
fn group_by_coverage(objects: &[ObjectSummary]) -> Vec<Vec<ObjectSummary>> {
    let mut groups: Vec<Vec<ObjectSummary>> = Vec::new();

    'outer: for obj in objects {
        // Check if this object matches any existing group
        for group in &mut groups {
            if coverage_matches(&group[0].intervals, &obj.intervals) {
                group.push(obj.clone());
                continue 'outer;
            }
        }
        // No match found, create new group
        groups.push(vec![obj.clone()]);
    }

    groups
}

/// Check if two coverage interval lists are identical.
fn coverage_matches(a: &[CoverageInterval], b: &[CoverageInterval]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for (ia, ib) in a.iter().zip(b.iter()) {
        if (ia.start - ib.start).abs() > 1e-6 || (ia.end - ib.end).abs() > 1e-6 {
            return false;
        }
    }
    true
}

/// Print a list of names with wrapping.
fn print_wrapped(names: &[String], indent: usize, max_width: usize) {
    let mut current_width = 0;

    for (i, name) in names.iter().enumerate() {
        let needed = name.len() + if i > 0 { 2 } else { 0 }; // ", " separator

        if current_width > 0 && current_width + needed > max_width - indent {
            println!();
            print!("{:indent$}", "", indent = indent);
            current_width = 0;
        }

        if current_width > 0 {
            print!(", ");
            current_width += 2;
        }
        print!("{}", name);
        current_width += name.len();
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coverage_matches() {
        let a = vec![CoverageInterval {
            start: 0.0,
            end: 100.0,
            ck_type: None,
            has_rates: None,
        }];
        let b = vec![CoverageInterval {
            start: 0.0,
            end: 100.0,
            ck_type: None,
            has_rates: None,
        }];
        let c = vec![CoverageInterval {
            start: 0.0,
            end: 200.0,
            ck_type: None,
            has_rates: None,
        }];

        assert!(coverage_matches(&a, &b));
        assert!(!coverage_matches(&a, &c));
    }
}
