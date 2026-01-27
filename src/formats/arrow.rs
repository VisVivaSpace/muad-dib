//! Arrow IPC output format implementation.
//!
//! Uses the same flattened SegmentRow structure as Parquet for compatibility.
//! Arrow IPC (also known as Feather v2) is efficient for in-memory data exchange.
//!
//! Note: Text PCK files use a separate storage mechanism (PCKSource) and are not
//! stored in Arrow. Only DAF binary files (SPK, CK, BPCK) are stored here.

use super::OutputFormat;
use crate::hdf5_output::DAFSource;
use crate::prelude::*;
use crate::{BPCKSegment, CKSegment, DAFHeader, DAFMetadata, DAFSegment, Endian, SPKSegment};
use arrow::datatypes::FieldRef;
use arrow::ipc::reader::FileReader;
use arrow::ipc::writer::FileWriter;
use arrow::record_batch::RecordBatch;
use serde::{Deserialize, Serialize};
use serde_arrow::schema::{SchemaLike, TracingOptions};
use std::collections::HashMap;
use std::fs::File;
use std::path::Path;

/// A flattened segment row for Arrow IPC output.
/// Each segment becomes one row with source/metadata info embedded.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SegmentRow {
    // Source info
    source_filename: String,
    source_name: String,
    source_comment: String,
    source_kind: String,

    // Metadata
    meta_nd: u64,
    meta_ni: u64,
    meta_endian: String,
    meta_fward: u64,
    meta_bward: u64,
    meta_free_address: u64,
    meta_ftpstr: String,

    // Segment common fields
    segment_type: String, // "SPK", "CK", "BPCK"
    segment_name: String,
    data_start: u64,
    data_end: u64,

    // SPK-specific (None for CK/BPCK)
    initial_epoch: Option<f64>,
    final_epoch: Option<f64>,
    target_code: Option<i32>,
    center_code: Option<i32>,
    frame_code: Option<i32>,
    spk_type: Option<i32>,

    // CK-specific (None for SPK/BPCK)
    initial_sclk: Option<f64>,
    final_sclk: Option<f64>,
    instrument_code: Option<i32>,
    ck_frame_code: Option<i32>,
    ck_type: Option<i32>,
    rates: Option<bool>,

    // BPCK-specific (None for SPK/CK)
    bpck_initial_epoch: Option<f64>,
    bpck_final_epoch: Option<f64>,
    frame_id: Option<i32>,
    base_frame: Option<i32>,
    bpck_type: Option<i32>,

    // Data (common to all)
    data: Vec<f64>,
}

impl SegmentRow {
    fn from_spk(source: &DAFSource, seg: &SPKSegment) -> Self {
        Self {
            source_filename: source.filename.clone(),
            source_name: source.header.name.clone(),
            source_comment: source.header.comment.clone(),
            source_kind: source.header.kind.clone(),
            meta_nd: source.metadata.nd,
            meta_ni: source.metadata.ni,
            meta_endian: source.metadata.endian.locfmt().to_string(),
            meta_fward: source.metadata.fward,
            meta_bward: source.metadata.bward,
            meta_free_address: source.metadata.free_address,
            meta_ftpstr: source.metadata.ftpstr.clone(),
            segment_type: "SPK".to_string(),
            segment_name: seg.name.clone(),
            data_start: seg.data_start,
            data_end: seg.data_end,
            initial_epoch: Some(seg.initial_epoch),
            final_epoch: Some(seg.final_epoch),
            target_code: Some(seg.target_code),
            center_code: Some(seg.center_code),
            frame_code: Some(seg.frame_code),
            spk_type: Some(seg.spk_type),
            initial_sclk: None,
            final_sclk: None,
            instrument_code: None,
            ck_frame_code: None,
            ck_type: None,
            rates: None,
            bpck_initial_epoch: None,
            bpck_final_epoch: None,
            frame_id: None,
            base_frame: None,
            bpck_type: None,
            data: seg.data.clone(),
        }
    }

    fn from_ck(source: &DAFSource, seg: &CKSegment) -> Self {
        Self {
            source_filename: source.filename.clone(),
            source_name: source.header.name.clone(),
            source_comment: source.header.comment.clone(),
            source_kind: source.header.kind.clone(),
            meta_nd: source.metadata.nd,
            meta_ni: source.metadata.ni,
            meta_endian: source.metadata.endian.locfmt().to_string(),
            meta_fward: source.metadata.fward,
            meta_bward: source.metadata.bward,
            meta_free_address: source.metadata.free_address,
            meta_ftpstr: source.metadata.ftpstr.clone(),
            segment_type: "CK".to_string(),
            segment_name: seg.name.clone(),
            data_start: seg.data_start,
            data_end: seg.data_end,
            initial_epoch: None,
            final_epoch: None,
            target_code: None,
            center_code: None,
            frame_code: None,
            spk_type: None,
            initial_sclk: Some(seg.initial_sclk),
            final_sclk: Some(seg.final_sclk),
            instrument_code: Some(seg.instrument_code),
            ck_frame_code: Some(seg.frame_code),
            ck_type: Some(seg.ck_type),
            rates: Some(seg.rates),
            bpck_initial_epoch: None,
            bpck_final_epoch: None,
            frame_id: None,
            base_frame: None,
            bpck_type: None,
            data: seg.data.clone(),
        }
    }

    fn from_bpck(source: &DAFSource, seg: &BPCKSegment) -> Self {
        Self {
            source_filename: source.filename.clone(),
            source_name: source.header.name.clone(),
            source_comment: source.header.comment.clone(),
            source_kind: source.header.kind.clone(),
            meta_nd: source.metadata.nd,
            meta_ni: source.metadata.ni,
            meta_endian: source.metadata.endian.locfmt().to_string(),
            meta_fward: source.metadata.fward,
            meta_bward: source.metadata.bward,
            meta_free_address: source.metadata.free_address,
            meta_ftpstr: source.metadata.ftpstr.clone(),
            segment_type: "BPCK".to_string(),
            segment_name: seg.name.clone(),
            data_start: seg.data_start,
            data_end: seg.data_end,
            initial_epoch: None,
            final_epoch: None,
            target_code: None,
            center_code: None,
            frame_code: None,
            spk_type: None,
            initial_sclk: None,
            final_sclk: None,
            instrument_code: None,
            ck_frame_code: None,
            ck_type: None,
            rates: None,
            bpck_initial_epoch: Some(seg.initial_epoch),
            bpck_final_epoch: Some(seg.final_epoch),
            frame_id: Some(seg.frame_id),
            base_frame: Some(seg.base_frame),
            bpck_type: Some(seg.bpck_type),
            data: seg.data.clone(),
        }
    }
}

/// Arrow IPC output format.
pub struct ArrowFormat;

impl OutputFormat for ArrowFormat {
    fn name(&self) -> &'static str {
        "arrow"
    }

    fn extension(&self) -> &'static str {
        "arrow"
    }

    fn write(&self, path: &Path, sources: &[DAFSource]) -> Result<()> {
        // Flatten sources into segment rows
        let rows: Vec<SegmentRow> = sources
            .iter()
            .flat_map(|source| {
                source.segments.iter().map(move |seg| match seg {
                    DAFSegment::SPK(spk) => SegmentRow::from_spk(source, spk),
                    DAFSegment::CK(ck) => SegmentRow::from_ck(source, ck),
                    DAFSegment::BPCK(bpck) => SegmentRow::from_bpck(source, bpck),
                })
            })
            .collect();

        if rows.is_empty() {
            return Err(Error::EmptyData { context: "No segments to write".into() });
        }

        // Derive Arrow schema from the data
        let tracing_options = TracingOptions::default().allow_null_fields(true);
        let fields = Vec::<FieldRef>::from_samples(&rows, tracing_options)
            .map_err(|e| Error::Serialization { format: "Arrow".into(), message: e.to_string() })?;

        // Convert to Arrow RecordBatch via serde_arrow
        let batch: RecordBatch = serde_arrow::to_record_batch(&fields, &rows)
            .map_err(|e| Error::Serialization { format: "Arrow".into(), message: e.to_string() })?;

        let file =
            File::create(path)?;

        let mut writer = FileWriter::try_new(file, &batch.schema())
            .map_err(|e| Error::Serialization { format: "Arrow IPC".into(), message: e.to_string() })?;

        writer
            .write(&batch)
            .map_err(|e| Error::Serialization { format: "Arrow IPC".into(), message: e.to_string() })?;

        writer
            .finish()
            .map_err(|e| Error::Serialization { format: "Arrow IPC".into(), message: e.to_string() })?;

        Ok(())
    }
}

/// Read sources from an Arrow IPC file.
/// Reconstructs DAFSource structures from the flattened segment rows.
pub fn read_arrow(path: &Path) -> Result<Vec<DAFSource>> {
    let file = File::open(path)?;

    let reader =
        FileReader::try_new(file, None).map_err(|e| Error::Serialization { format: "Arrow IPC".into(), message: e.to_string() })?;

    let mut all_rows: Vec<SegmentRow> = Vec::new();
    for batch_result in reader {
        let batch =
            batch_result.map_err(|e| Error::Serialization { format: "Arrow IPC".into(), message: e.to_string() })?;
        let rows: Vec<SegmentRow> = serde_arrow::from_record_batch(&batch)
            .map_err(|e| Error::Serialization { format: "Arrow".into(), message: e.to_string() })?;
        all_rows.extend(rows);
    }

    if all_rows.is_empty() {
        return Ok(Vec::new());
    }

    // Group rows by source filename to reconstruct DAFSource structures
    let mut source_map: HashMap<String, Vec<SegmentRow>> = HashMap::new();
    for row in all_rows {
        source_map
            .entry(row.source_filename.clone())
            .or_default()
            .push(row);
    }

    // Reconstruct DAFSource from grouped rows
    let sources: Vec<DAFSource> = source_map
        .into_iter()
        .map(|(filename, rows)| {
            let first = &rows[0];

            let endian = match first.meta_endian.as_str() {
                "LTL-IEEE" => Endian::Little,
                _ => Endian::Big,
            };

            let header = DAFHeader {
                name: first.source_name.clone(),
                comment: first.source_comment.clone(),
                kind: first.source_kind.clone(),
            };

            let metadata = DAFMetadata {
                nd: first.meta_nd,
                ni: first.meta_ni,
                endian,
                fward: first.meta_fward,
                bward: first.meta_bward,
                free_address: first.meta_free_address,
                ftpstr: first.meta_ftpstr.clone(),
            };

            let segments: Vec<DAFSegment> = rows
                .into_iter()
                .map(|row| match row.segment_type.as_str() {
                    "SPK" => DAFSegment::SPK(SPKSegment {
                        name: row.segment_name,
                        initial_epoch: row.initial_epoch.unwrap_or(0.0),
                        final_epoch: row.final_epoch.unwrap_or(0.0),
                        target_code: row.target_code.unwrap_or(0),
                        center_code: row.center_code.unwrap_or(0),
                        frame_code: row.frame_code.unwrap_or(0),
                        spk_type: row.spk_type.unwrap_or(0),
                        data_start: row.data_start,
                        data_end: row.data_end,
                        data: row.data,
                    }),
                    "CK" => DAFSegment::CK(CKSegment {
                        name: row.segment_name,
                        initial_sclk: row.initial_sclk.unwrap_or(0.0),
                        final_sclk: row.final_sclk.unwrap_or(0.0),
                        instrument_code: row.instrument_code.unwrap_or(0),
                        frame_code: row.ck_frame_code.unwrap_or(0),
                        ck_type: row.ck_type.unwrap_or(0),
                        rates: row.rates.unwrap_or(false),
                        data_start: row.data_start,
                        data_end: row.data_end,
                        data: row.data,
                    }),
                    _ => DAFSegment::BPCK(BPCKSegment {
                        name: row.segment_name,
                        initial_epoch: row.bpck_initial_epoch.unwrap_or(0.0),
                        final_epoch: row.bpck_final_epoch.unwrap_or(0.0),
                        frame_id: row.frame_id.unwrap_or(0),
                        base_frame: row.base_frame.unwrap_or(0),
                        bpck_type: row.bpck_type.unwrap_or(0),
                        data_start: row.data_start,
                        data_end: row.data_end,
                        data: row.data,
                    }),
                })
                .collect();

            DAFSource {
                filename,
                header,
                metadata,
                segments,
            }
        })
        .collect();

    Ok(sources)
}
