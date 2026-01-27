//! DAFSource — a parsed DAF file with header, metadata, and segments.
//!
//! This struct is the central data type used by all output formats.
//! It is format-independent and does not depend on HDF5.

use crate::{DAFHeader, DAFMetadata, DAFSegment};
use serde::{Deserialize, Serialize};

/// A source file with its header, metadata, and parsed segments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DAFSource {
    pub filename: String,
    pub header: DAFHeader,
    pub metadata: DAFMetadata,
    pub segments: Vec<DAFSegment>,
}
