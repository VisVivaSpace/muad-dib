//! SPICE kernel pool access, leap second data, and time parsing.
//!
//! This module provides functionality equivalent to common NAIF CSPICE routines:
//!
//! - **Kernel Pool Access**: `get_f64()`, `get_i32()`, `get_string()` for typed variable lookup
//! - **Time Parsing**: `EpochTDB::parse()` for converting time strings to TDB
//! - **TDB/UTC Conversion**: `utc_to_tdb()`, `tdb_to_utc()` using leap second data
//!
//! # Example
//!
//! ```ignore
//! use muad_dib::kernel::SpiceKernel;
//! use muad_dib::spice::KernelPoolExt;
//! use muad_dib::types::{EpochTDB, NaifId};
//!
//! let kernel = SpiceKernel::load("de440.bsp")?;
//!
//! // Parse a time string
//! let epoch = EpochTDB::parse("2020-01-01T00:00:00")?;
//! ```

pub mod lsk;
pub mod pool;
pub mod time;

pub use lsk::{tdb_to_utc, utc_to_tdb, EpochType, LeapSecondData, LeapSecondExt};
pub use pool::KernelPoolExt;
pub use time::{format_calendar, format_iso8601, tdb_to_calendar, TimeFormat};

pub use crate::types::EpochTDB;
