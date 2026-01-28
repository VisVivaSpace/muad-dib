//! Parquet output format implementation.
//!
//! Parquet doesn't support Arrow Union types (Rust enums), so we flatten the
//! DAFSource structure into a segment-centric format where each segment is a row
//! with source and metadata info embedded. This is more idiomatic for Parquet anyway.
//!
//! Note: Text PCK files use a separate storage mechanism (PCKSource) and are not
//! stored in Parquet. Only DAF binary files (SPK, CK, BPCK) are stored here.

use super::segment_row::{rows_to_sources, sources_to_rows, SegmentRow};
use super::OutputFormat;
use crate::daf_source::DAFSource;
use crate::prelude::*;
use arrow::datatypes::FieldRef;
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;
use parquet::file::properties::WriterProperties;
use serde_arrow::schema::{SchemaLike, TracingOptions};
use std::fs::File;
use std::path::Path;

/// Parquet output format.
pub struct ParquetFormat;

impl OutputFormat for ParquetFormat {
    fn name(&self) -> &'static str {
        "parquet"
    }

    fn extension(&self) -> &'static str {
        "parquet"
    }

    fn write(&self, path: &Path, sources: &[DAFSource]) -> Result<()> {
        let rows = sources_to_rows(sources);

        if rows.is_empty() {
            return Err(Error::EmptyData {
                context: "No segments to write".into(),
            });
        }

        // Derive Arrow schema from the data
        let tracing_options = TracingOptions::default().allow_null_fields(true);
        let fields = Vec::<FieldRef>::from_samples(&rows, tracing_options).map_err(|e| {
            Error::Serialization {
                format: "Arrow".into(),
                message: e.to_string(),
            }
        })?;

        // Convert to Arrow RecordBatch via serde_arrow
        let batch: RecordBatch =
            serde_arrow::to_record_batch(&fields, &rows).map_err(|e| Error::Serialization {
                format: "Arrow".into(),
                message: e.to_string(),
            })?;

        let file = File::create(path)?;

        let props = WriterProperties::builder().build();
        let mut writer = ArrowWriter::try_new(file, batch.schema(), Some(props)).map_err(|e| {
            Error::Serialization {
                format: "Parquet".into(),
                message: e.to_string(),
            }
        })?;

        writer.write(&batch).map_err(|e| Error::Serialization {
            format: "Parquet".into(),
            message: e.to_string(),
        })?;

        writer.close().map_err(|e| Error::Serialization {
            format: "Parquet".into(),
            message: e.to_string(),
        })?;

        Ok(())
    }
}

/// Read sources from a Parquet file.
/// Reconstructs DAFSource structures from the flattened segment rows.
pub fn read_parquet(path: &Path) -> Result<Vec<DAFSource>> {
    let file = File::open(path)?;

    let reader = parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| Error::Serialization {
            format: "Parquet".into(),
            message: e.to_string(),
        })?
        .build()
        .map_err(|e| Error::Serialization {
            format: "Parquet".into(),
            message: e.to_string(),
        })?;

    let batches: Vec<RecordBatch> =
        reader
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Serialization {
                format: "Parquet".into(),
                message: e.to_string(),
            })?;

    if batches.is_empty() {
        return Ok(Vec::new());
    }

    // Combine all batches and decode
    let mut all_rows: Vec<SegmentRow> = Vec::new();
    for batch in &batches {
        let rows: Vec<SegmentRow> =
            serde_arrow::from_record_batch(batch).map_err(|e| Error::Serialization {
                format: "Arrow".into(),
                message: e.to_string(),
            })?;
        all_rows.extend(rows);
    }

    rows_to_sources(all_rows)
}
