//! Arrow IPC output format implementation.
//!
//! Uses the same flattened SegmentRow structure as Parquet for compatibility.
//! Arrow IPC (also known as Feather v2) is efficient for in-memory data exchange.
//!
//! Note: Text PCK files use a separate storage mechanism (PCKSource) and are not
//! stored in Arrow. Only DAF binary files (SPK, CK, BPCK) are stored here.

use super::segment_row::{rows_to_sources, sources_to_rows, SegmentRow};
use super::OutputFormat;
use crate::daf_source::DAFSource;
use crate::prelude::*;
use arrow::datatypes::FieldRef;
use arrow::ipc::reader::FileReader;
use arrow::ipc::writer::FileWriter;
use arrow::record_batch::RecordBatch;
use serde_arrow::schema::{SchemaLike, TracingOptions};
use std::fs::File;
use std::path::Path;

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

        let mut writer =
            FileWriter::try_new(file, &batch.schema()).map_err(|e| Error::Serialization {
                format: "Arrow IPC".into(),
                message: e.to_string(),
            })?;

        writer.write(&batch).map_err(|e| Error::Serialization {
            format: "Arrow IPC".into(),
            message: e.to_string(),
        })?;

        writer.finish().map_err(|e| Error::Serialization {
            format: "Arrow IPC".into(),
            message: e.to_string(),
        })?;

        Ok(())
    }
}

/// Read sources from an Arrow IPC file.
/// Reconstructs DAFSource structures from the flattened segment rows.
pub fn read_arrow(path: &Path) -> Result<Vec<DAFSource>> {
    let file = File::open(path)?;

    let reader = FileReader::try_new(file, None).map_err(|e| Error::Serialization {
        format: "Arrow IPC".into(),
        message: e.to_string(),
    })?;

    let mut all_rows: Vec<SegmentRow> = Vec::new();
    for batch_result in reader {
        let batch = batch_result.map_err(|e| Error::Serialization {
            format: "Arrow IPC".into(),
            message: e.to_string(),
        })?;
        let rows: Vec<SegmentRow> =
            serde_arrow::from_record_batch(&batch).map_err(|e| Error::Serialization {
                format: "Arrow".into(),
                message: e.to_string(),
            })?;
        all_rows.extend(rows);
    }

    rows_to_sources(all_rows)
}
