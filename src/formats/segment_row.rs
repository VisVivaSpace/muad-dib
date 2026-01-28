//! Shared segment row type for Parquet and Arrow formats.
//!
//! Each DAF segment is flattened into a `SegmentRow` with source/metadata
//! info embedded, since Parquet/Arrow don't support nested union types.

use crate::daf_source::DAFSource;
use crate::error::Error;
use crate::prelude::*;
use crate::types::NaifId;
use crate::{BPCKSegment, CKSegment, DAFHeader, DAFMetadata, DAFSegment, Endian, SPKSegment};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A flattened segment row for columnar output formats.
/// Each segment becomes one row with source/metadata info embedded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentRow {
    // Source info
    pub source_filename: String,
    pub source_name: String,
    pub source_comment: String,
    pub source_kind: String,

    // Metadata
    pub meta_nd: u64,
    pub meta_ni: u64,
    pub meta_endian: String,
    pub meta_fward: u64,
    pub meta_bward: u64,
    pub meta_free_address: u64,
    pub meta_ftpstr: String,

    // Segment common fields
    pub segment_type: String, // "SPK", "CK", "BPCK"
    pub segment_name: String,
    pub data_start: u64,
    pub data_end: u64,

    // SPK-specific (None for CK/BPCK)
    pub initial_epoch: Option<f64>,
    pub final_epoch: Option<f64>,
    pub target_code: Option<i32>,
    pub center_code: Option<i32>,
    pub frame_code: Option<i32>,
    pub spk_type: Option<i32>,

    // CK-specific (None for SPK/BPCK)
    pub initial_sclk: Option<f64>,
    pub final_sclk: Option<f64>,
    pub instrument_code: Option<i32>,
    pub ck_frame_code: Option<i32>,
    pub ck_type: Option<i32>,
    pub rates: Option<bool>,

    // BPCK-specific (None for SPK/CK)
    pub bpck_initial_epoch: Option<f64>,
    pub bpck_final_epoch: Option<f64>,
    pub frame_id: Option<i32>,
    pub base_frame: Option<i32>,
    pub bpck_type: Option<i32>,

    // Data (common to all)
    pub data: Vec<f64>,
}

impl SegmentRow {
    pub fn base(
        source: &DAFSource,
        segment_type: &str,
        segment_name: &str,
        data_start: u64,
        data_end: u64,
        data: Vec<f64>,
    ) -> Self {
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
            segment_type: segment_type.to_string(),
            segment_name: segment_name.to_string(),
            data_start,
            data_end,
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
            bpck_initial_epoch: None,
            bpck_final_epoch: None,
            frame_id: None,
            base_frame: None,
            bpck_type: None,
            data,
        }
    }

    pub fn from_spk(source: &DAFSource, seg: &SPKSegment) -> Self {
        let mut row = Self::base(source, "SPK", &seg.name, seg.data_start, seg.data_end, seg.data.clone());
        row.initial_epoch = Some(seg.initial_epoch);
        row.final_epoch = Some(seg.final_epoch);
        row.target_code = Some(seg.target_code.0);
        row.center_code = Some(seg.center_code.0);
        row.frame_code = Some(seg.frame_code.0);
        row.spk_type = Some(seg.spk_type);
        row
    }

    pub fn from_ck(source: &DAFSource, seg: &CKSegment) -> Self {
        let mut row = Self::base(source, "CK", &seg.name, seg.data_start, seg.data_end, seg.data.clone());
        row.initial_sclk = Some(seg.initial_sclk);
        row.final_sclk = Some(seg.final_sclk);
        row.instrument_code = Some(seg.instrument_code.0);
        row.ck_frame_code = Some(seg.frame_code.0);
        row.ck_type = Some(seg.ck_type);
        row.rates = Some(seg.rates);
        row
    }

    pub fn from_bpck(source: &DAFSource, seg: &BPCKSegment) -> Self {
        let mut row = Self::base(source, "BPCK", &seg.name, seg.data_start, seg.data_end, seg.data.clone());
        row.bpck_initial_epoch = Some(seg.initial_epoch);
        row.bpck_final_epoch = Some(seg.final_epoch);
        row.frame_id = Some(seg.frame_id.0);
        row.base_frame = Some(seg.base_frame.0);
        row.bpck_type = Some(seg.bpck_type);
        row
    }
}

/// Flatten DAF sources into segment rows for columnar output.
pub fn sources_to_rows(sources: &[DAFSource]) -> Vec<SegmentRow> {
    sources
        .iter()
        .flat_map(|source| {
            source.segments.iter().map(move |seg| match seg {
                DAFSegment::SPK(spk) => SegmentRow::from_spk(source, spk),
                DAFSegment::CK(ck) => SegmentRow::from_ck(source, ck),
                DAFSegment::BPCK(bpck) => SegmentRow::from_bpck(source, bpck),
            })
        })
        .collect()
}

/// Reconstruct DAF sources from segment rows.
///
/// Returns an error if required type-specific fields are missing.
pub fn rows_to_sources(rows: Vec<SegmentRow>) -> Result<Vec<DAFSource>> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    // Group rows by source filename to reconstruct DAFSource structures
    let mut source_map: HashMap<String, Vec<SegmentRow>> = HashMap::new();
    for row in rows {
        source_map
            .entry(row.source_filename.clone())
            .or_default()
            .push(row);
    }

    let mut sources: Vec<DAFSource> = Vec::new();

    for (filename, rows) in source_map {
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

        let mut segments: Vec<DAFSegment> = Vec::new();
        for row in rows {
            let segment = match row.segment_type.as_str() {
                "SPK" => DAFSegment::SPK(SPKSegment {
                    name: row.segment_name,
                    initial_epoch: required_field(row.initial_epoch, "initial_epoch", "SPK")?,
                    final_epoch: required_field(row.final_epoch, "final_epoch", "SPK")?,
                    target_code: NaifId(required_field(row.target_code, "target_code", "SPK")?),
                    center_code: NaifId(required_field(row.center_code, "center_code", "SPK")?),
                    frame_code: NaifId(required_field(row.frame_code, "frame_code", "SPK")?),
                    spk_type: required_field(row.spk_type, "spk_type", "SPK")?,
                    data_start: row.data_start,
                    data_end: row.data_end,
                    data: row.data,
                }),
                "CK" => DAFSegment::CK(CKSegment {
                    name: row.segment_name,
                    initial_sclk: required_field(row.initial_sclk, "initial_sclk", "CK")?,
                    final_sclk: required_field(row.final_sclk, "final_sclk", "CK")?,
                    instrument_code: NaifId(required_field(row.instrument_code, "instrument_code", "CK")?),
                    frame_code: NaifId(required_field(row.ck_frame_code, "ck_frame_code", "CK")?),
                    ck_type: required_field(row.ck_type, "ck_type", "CK")?,
                    rates: required_field(row.rates, "rates", "CK")?,
                    data_start: row.data_start,
                    data_end: row.data_end,
                    data: row.data,
                }),
                _ => DAFSegment::BPCK(BPCKSegment {
                    name: row.segment_name,
                    initial_epoch: required_field(row.bpck_initial_epoch, "bpck_initial_epoch", "BPCK")?,
                    final_epoch: required_field(row.bpck_final_epoch, "bpck_final_epoch", "BPCK")?,
                    frame_id: NaifId(required_field(row.frame_id, "frame_id", "BPCK")?),
                    base_frame: NaifId(required_field(row.base_frame, "base_frame", "BPCK")?),
                    bpck_type: required_field(row.bpck_type, "bpck_type", "BPCK")?,
                    data_start: row.data_start,
                    data_end: row.data_end,
                    data: row.data,
                }),
            };
            segments.push(segment);
        }

        sources.push(DAFSource {
            filename,
            header,
            metadata,
            segments,
        });
    }

    Ok(sources)
}

/// Extract a required field from an Option, returning an error if missing.
fn required_field<T>(value: Option<T>, field_name: &str, segment_type: &str) -> Result<T> {
    value.ok_or_else(|| Error::Serialization {
        format: segment_type.to_string(),
        message: format!("missing required field '{}' for {} segment", field_name, segment_type),
    })
}
