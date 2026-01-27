//! Output format modules for writing DAF data to various formats.
//!
//! This module provides a unified interface for writing DAF segment data to
//! multiple file formats. Each format has trade-offs:
//!
//! | Format | Best For |
//! |--------|----------|
//! | HDF5 | Scientific workflows, hierarchical data |
//! | Parquet | Analytics, columnar queries, large datasets |
//! | Arrow | In-memory interchange, zero-copy reads |
//! | MessagePack | Compact storage, fast serialization |
//! | BSON | MongoDB integration, document storage |
//!
//! # Example
//!
//! ```no_run
//! use muad_dib::formats::get_format;
//!
//! let format = get_format("parquet").unwrap();
//! println!("Writing to {}.{}", "output", format.extension());
//! ```

pub mod arrow;
pub mod bson;
#[cfg(feature = "hdf5")]
pub mod hdf5;
pub mod msgpack;
pub mod parquet;

use crate::daf_source::DAFSource;
#[cfg(feature = "hdf5")]
use crate::hdf5_input::read_hdf5;
use crate::prelude::*;
use std::path::Path;

/// Trait for output format implementations.
pub trait OutputFormat {
    /// The name of the format (e.g., "hdf5", "msgpack").
    fn name(&self) -> &'static str;

    /// The file extension (e.g., "hdf5", "msgpack").
    fn extension(&self) -> &'static str;

    /// Write sources to a file.
    fn write(&self, path: &Path, sources: &[DAFSource]) -> Result<()>;
}

/// Get an output format by name.
pub fn get_format(name: &str) -> Option<Box<dyn OutputFormat>> {
    match name.to_lowercase().as_str() {
        #[cfg(feature = "hdf5")]
        "hdf5" | "h5" => Some(Box::new(hdf5::Hdf5Format)),
        "msgpack" | "mp" => Some(Box::new(msgpack::MsgPackFormat)),
        "bson" => Some(Box::new(bson::BsonFormat)),
        "parquet" | "pq" => Some(Box::new(parquet::ParquetFormat)),
        "arrow" | "arr" | "feather" => Some(Box::new(arrow::ArrowFormat)),
        _ => None,
    }
}

/// Get all available format names.
pub fn available_formats() -> Vec<&'static str> {
    #[cfg(feature = "hdf5")]
    let formats = vec!["hdf5", "parquet", "arrow", "msgpack", "bson"];
    #[cfg(not(feature = "hdf5"))]
    let formats = vec!["parquet", "arrow", "msgpack", "bson"];
    formats
}

/// Read DAF sources from any supported serialized format (auto-detected by extension).
///
/// Supports: HDF5 (with feature), Parquet, Arrow, MessagePack, BSON
///
/// # Example
///
/// ```no_run
/// use muad_dib::formats::read_sources;
/// use std::path::Path;
///
/// let sources = read_sources(Path::new("data.parquet")).unwrap();
/// for source in &sources {
///     println!("File: {} ({} segments)", source.filename, source.segments.len());
/// }
/// ```
pub fn read_sources(path: &Path) -> Result<Vec<DAFSource>> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        #[cfg(feature = "hdf5")]
        "hdf5" | "h5" => read_hdf5(path),
        #[cfg(not(feature = "hdf5"))]
        "hdf5" | "h5" => Err(crate::error::Error::Format(
            "HDF5 support not enabled. Rebuild with: cargo build --features hdf5".to_string(),
        )),
        "parquet" | "pq" => parquet::read_parquet(path),
        "arrow" | "feather" => arrow::read_arrow(path),
        "msgpack" | "mp" => msgpack::read_msgpack(path),
        "bson" => bson::read_bson(path),
        _ => Err(crate::error::Error::UnknownFormat {
            format: format!(
                "file format '{}'. Supported: {}, parquet, arrow, msgpack, bson",
                ext,
                if cfg!(feature = "hdf5") {
                    "hdf5"
                } else {
                    "hdf5 (disabled)"
                }
            ),
        }),
    }
}
