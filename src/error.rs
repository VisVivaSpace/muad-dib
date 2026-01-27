//! Error types for DAF file parsing.

#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// I/O error from file operations.
    #[error(transparent)]
    IO(#[from] std::io::Error),

    /// HDF5 library error.
    #[cfg(feature = "hdf5")]
    #[error("HDF5 {operation}: {message}")]
    Hdf5 { operation: String, message: String },

    /// Serialization/deserialization error.
    #[error("{format} error: {message}")]
    Serialization { format: String, message: String },

    /// No data to process.
    #[error("No data: {context}")]
    EmptyData { context: String },

    /// Unknown or unsupported format.
    #[error("Unknown format: {format}")]
    UnknownFormat { format: String },

    /// Invalid DAF file header.
    #[error("Invalid DAF header: {0}")]
    InvalidHeader(String),

    /// Unsupported DAF file type.
    #[error(
        "Unsupported DAF type: '{daf_type}' (expected 'S' for SPK, 'C' for CK, or 'P' for BPCK)"
    )]
    UnsupportedType { daf_type: char },

    /// Invalid endianness indicator in DAF file.
    #[error("Invalid endian indicator at offset 88: expected 'B' or 'L', found '{found}'")]
    InvalidEndian { found: char },

    /// Error parsing a segment at a specific offset.
    #[error("Segment parse error at offset {offset}: {message}")]
    SegmentParse { offset: u64, message: String },

    /// Format-related error (output format issues).
    #[error("Format error: {0}")]
    Format(String),

    // ========== SPICE API Errors ==========
    /// Cannot parse time string.
    #[error("Cannot parse time string: '{input}'")]
    TimeParseError { input: String },

    /// Leap second kernel (LSK) data required for TDB/UTC conversion.
    #[error("Leap second kernel (LSK) data required for TDB/UTC conversion")]
    MissingLskData,

    /// Kernel pool variable not found.
    #[error("Kernel pool variable '{name}' not found")]
    VariableNotFound { name: String },

    /// Variable has wrong type (expected numeric, got text or vice versa).
    #[error("Variable '{name}' has wrong type: expected {expected}")]
    WrongVariableType { name: String, expected: String },
}
