//! Brief utility - Display time coverage summaries for DAF files.
//!
//! This module provides functionality similar to the NAIF `brief` utility,
//! displaying coverage intervals for bodies/frames in SPK, CK, and BPCK files.
//!
//! Note: Text PCK files are not supported by brief as they don't have time coverage.
//! Brief is only for binary DAF files (SPK, CK, BPCK).

pub mod display;
pub mod names;
pub mod time;

use crate::daf_source::DAFSource;
use crate::error::Error;
use crate::formats::arrow::read_arrow;
use crate::formats::bson::read_bson;
use crate::formats::msgpack::read_msgpack;
use crate::formats::parquet::read_parquet;
#[cfg(feature = "hdf5")]
use crate::hdf5_input::read_hdf5;
use crate::{DAFFile, DAFSegment};
use std::collections::HashMap;
use std::fs::File;
use std::path::Path;

/// Time kind - distinguishes TDB times from SCLK ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimeKind {
    /// TDB seconds past J2000 (SPK, BPCK)
    #[default]
    TDB,
    /// Encoded spacecraft clock ticks (CK)
    SCLK,
}

/// Coverage interval for a body/frame.
#[derive(Debug, Clone)]
pub struct CoverageInterval {
    /// Start time (TDB seconds past J2000 for SPK/BPCK, SCLK ticks for CK)
    pub start: f64,
    /// End time (TDB seconds past J2000 for SPK/BPCK, SCLK ticks for CK)
    pub end: f64,
    /// SPK segment type (2, 3, 9, 13, etc.), None for CK/BPCK
    pub spk_type: Option<i32>,
    /// CK segment type (1-6), None for SPK/BPCK
    pub ck_type: Option<i32>,
    /// BPCK segment type, None for SPK/CK
    pub bpck_type: Option<i32>,
    /// Whether angular velocity data is present, None for SPK/BPCK
    pub has_rates: Option<bool>,
}

/// File type classification for binary DAF files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    SPK,
    CK,
    BPCK,
}

impl std::fmt::Display for FileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileType::SPK => write!(f, "SPK"),
            FileType::CK => write!(f, "CK"),
            FileType::BPCK => write!(f, "BPCK"),
        }
    }
}

/// Summary entry for a single object (body or frame).
#[derive(Debug, Clone)]
pub struct ObjectSummary {
    /// NAIF ID of the object
    pub id: i32,
    /// Center ID (SPK) or base frame (BPCK), None for CK
    pub center: Option<i32>,
    /// Coverage intervals
    pub intervals: Vec<CoverageInterval>,
    /// File type this object came from
    pub file_type: FileType,
    /// Time kind (TDB or SCLK)
    pub time_kind: TimeKind,
    /// Reference frame code (CK only)
    pub frame_code: Option<i32>,
}

/// Time format for output.
#[derive(Debug, Clone, Copy, Default)]
pub enum TimeFormat {
    /// Calendar ET: "YYYY MON DD HR:MN:SC.DDD"
    #[default]
    CalendarET,
    /// Calendar UTC: "YYYY-MON-DD HR:MN:SC.DDD"
    CalendarUTC,
    /// UTC day-of-year: "YYYY-DOY // HR:MN:SC.DDD"
    DoyUTC,
    /// ET seconds past J2000: "SSSSSSSS.SSSSSS"
    SecondsET,
}

/// Options for brief output.
#[derive(Debug, Clone, Default)]
pub struct BriefOptions {
    /// Use tabular output format
    pub tabular: bool,
    /// Show centers-of-motion (SPK) or base frames (BPCK)
    pub show_centers: bool,
    /// Combine all files into single summary
    pub combine_all: bool,
    /// Show numeric IDs only (no name lookup)
    pub numeric_only: bool,
    /// Sort tabular output by start time
    pub sort_by_time: bool,
    /// Group by identical coverage
    pub group_coverage: bool,
    /// Time format for output
    pub time_format: TimeFormat,
    /// Show reference frame column (CK files)
    pub show_rel_frame: bool,
    /// Show segment data types (SPK/CK/BPCK type numbers)
    pub show_types: bool,
}

/// File summary containing header info and object summaries.
#[derive(Debug, Clone)]
pub struct FileSummary {
    /// Original filename
    pub filename: String,
    /// Internal name from file header
    pub internal_name: String,
    /// File type (SPK, CK, BPCK)
    pub file_type: FileType,
    /// Object summaries (bodies/frames)
    pub objects: Vec<ObjectSummary>,
}

/// Collect summaries from a file path.
/// Automatically detects file type (DAF vs serialized) by extension.
///
/// Note: Text PCK files (.tpc, .pck) are not supported as they don't have time coverage.
pub fn collect_summaries(path: &Path) -> Result<Vec<FileSummary>, Error> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        // Native DAF formats
        "bsp" | "spk" => collect_from_daf(path),
        "bc" | "ck" => collect_from_daf(path),
        "bpc" | "bpck" => collect_from_daf(path),
        // Serialized formats
        #[cfg(feature = "hdf5")]
        "hdf5" | "h5" => collect_from_serialized(path, read_hdf5),
        #[cfg(not(feature = "hdf5"))]
        "hdf5" | "h5" => Err(Error::Format(
            "HDF5 support not enabled. Rebuild with: cargo build --features hdf5".to_string(),
        )),
        "parquet" | "pq" => collect_from_serialized(path, read_parquet),
        "arrow" | "feather" => collect_from_serialized(path, read_arrow),
        "msgpack" | "mp" => collect_from_serialized(path, read_msgpack),
        "bson" => collect_from_serialized(path, read_bson),
        // Text PCK files don't have time coverage
        "tpc" | "pck" => Err(Error::Format(
            "Text PCK files don't have time coverage and cannot be summarized by brief".to_string(),
        )),
        _ => Err(Error::UnknownFormat {
            format: format!(
                "file format '{}'. Supported: bsp, bc, bpc, hdf5, parquet, arrow, msgpack, bson",
                ext
            ),
        }),
    }
}

/// Collect summaries from a native DAF file.
fn collect_from_daf(path: &Path) -> Result<Vec<FileSummary>, Error> {
    let file = File::open(path)?;
    let mut daf = DAFFile::from_file(file)?;
    let header = daf.daf_header()?;

    let file_type = match header.kind.as_str() {
        "SPK" => FileType::SPK,
        "CK" => FileType::CK,
        "BPCK" => FileType::BPCK,
        _ => {
            return Err(Error::UnknownFormat {
                format: format!("DAF type: {}", header.kind),
            })
        }
    };

    // Collect segments and merge intervals per object
    let mut object_map: HashMap<i32, ObjectSummary> = HashMap::new();

    for seg_result in daf {
        let segment = seg_result?;
        let info = extract_segment_info(&segment);

        object_map
            .entry(info.id)
            .and_modify(|obj| obj.intervals.push(info.interval.clone()))
            .or_insert_with(|| ObjectSummary {
                id: info.id,
                center: info.center,
                intervals: vec![info.interval],
                file_type,
                time_kind: info.time_kind,
                frame_code: info.frame_code,
            });
    }

    let mut objects: Vec<ObjectSummary> = object_map.into_values().collect();
    objects.sort_by_key(|o| o.id);

    Ok(vec![FileSummary {
        filename: path.display().to_string(),
        internal_name: header.name,
        file_type,
        objects,
    }])
}

/// Collect summaries from a serialized format file.
fn collect_from_serialized<F>(path: &Path, reader: F) -> Result<Vec<FileSummary>, Error>
where
    F: Fn(&Path) -> Result<Vec<DAFSource>, Error>,
{
    let sources = reader(path)?;
    let mut summaries = Vec::new();

    for source in sources {
        let file_type = match source.header.kind.as_str() {
            "SPK" => FileType::SPK,
            "CK" => FileType::CK,
            "BPCK" => FileType::BPCK,
            _ => continue, // Skip unknown types (including PCK which doesn't have time coverage)
        };

        let mut object_map: HashMap<i32, ObjectSummary> = HashMap::new();

        for segment in &source.segments {
            let info = extract_segment_info(segment);

            object_map
                .entry(info.id)
                .and_modify(|obj| obj.intervals.push(info.interval.clone()))
                .or_insert_with(|| ObjectSummary {
                    id: info.id,
                    center: info.center,
                    intervals: vec![info.interval],
                    file_type,
                    time_kind: info.time_kind,
                    frame_code: info.frame_code,
                });
        }

        let mut objects: Vec<ObjectSummary> = object_map.into_values().collect();
        objects.sort_by_key(|o| o.id);

        summaries.push(FileSummary {
            filename: source.filename,
            internal_name: source.header.name,
            file_type,
            objects,
        });
    }

    Ok(summaries)
}

/// Information extracted from a segment.
struct SegmentInfo {
    id: i32,
    center: Option<i32>,
    interval: CoverageInterval,
    time_kind: TimeKind,
    frame_code: Option<i32>,
}

/// Extract object info from a segment.
fn extract_segment_info(segment: &DAFSegment) -> SegmentInfo {
    match segment {
        DAFSegment::SPK(spk) => SegmentInfo {
            id: spk.target_code,
            center: Some(spk.center_code),
            interval: CoverageInterval {
                start: spk.initial_epoch,
                end: spk.final_epoch,
                spk_type: Some(spk.spk_type),
                ck_type: None,
                bpck_type: None,
                has_rates: None,
            },
            time_kind: TimeKind::TDB,
            frame_code: None,
        },
        DAFSegment::CK(ck) => SegmentInfo {
            id: ck.instrument_code,
            center: None, // CK doesn't have a center concept
            interval: CoverageInterval {
                start: ck.initial_sclk,
                end: ck.final_sclk,
                spk_type: None,
                ck_type: Some(ck.ck_type),
                bpck_type: None,
                has_rates: Some(ck.rates),
            },
            time_kind: TimeKind::SCLK,
            frame_code: Some(ck.frame_code),
        },
        DAFSegment::BPCK(bpck) => SegmentInfo {
            id: bpck.frame_id,
            center: Some(bpck.base_frame),
            interval: CoverageInterval {
                start: bpck.initial_epoch,
                end: bpck.final_epoch,
                spk_type: None,
                ck_type: None,
                bpck_type: Some(bpck.bpck_type),
                has_rates: None,
            },
            time_kind: TimeKind::TDB,
            frame_code: None,
        },
    }
}
