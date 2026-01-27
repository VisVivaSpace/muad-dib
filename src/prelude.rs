//! Crate prelude
//!
//! Provides the core types and Result alias for the crate.

pub use crate::error::Error;

/// Crate-wide Result type alias.
pub type Result<T> = core::result::Result<T, Error>;

// Standard library re-exports used across the crate
pub use std::fs::File;
pub use std::io::prelude::*;
pub use std::io::Read;
pub use std::io::SeekFrom;
