//! SPICE-compatible API for kernel queries and data interpolation.
//!
//! This module provides functionality equivalent to common NAIF CSPICE routines:
//!
//! - **Kernel Pool Access**: `get_f64()`, `get_i32()`, `get_string()` for typed variable lookup
//! - **Time Parsing**: `EpochTDB::parse()` for converting time strings to TDB
//! - **TDB/UTC Conversion**: `utc_to_tdb()`, `tdb_to_utc()` using leap second data
//! - **SPK State Evaluation**: `state_at()` for computing position/velocity at any epoch
//! - **CK Pointing Evaluation**: `pointing_at()` for computing orientation at any time
//! - **Coordinate Conversions**: Rectangular, latitudinal, spherical, cylindrical
//!
//! # Example
//!
//! ```ignore
//! use muad_dib::kernel::SpiceKernel;
//! use muad_dib::spice::{KernelPoolExt, State};
//! use muad_dib::types::{EpochTDB, NaifId};
//!
//! let kernel = SpiceKernel::load("de440.bsp")?;
//!
//! // Query ephemeris
//! let epoch = EpochTDB::parse("2020-01-01T00:00:00")?;
//! let state: State = kernel.state_of(NaifId::EARTH, epoch, NaifId::SSB)?;
//! println!("Earth position: {:?} km", state.position);
//! ```

pub mod coord;
pub mod interpolate;
pub mod lsk;
pub mod pool;
pub mod time;

mod ck;
mod spk;

// Re-exports for convenient access
pub use coord::{Cylindrical, Latitudinal, Rectangular, Spherical};
pub use interpolate::{Pointing, State};
pub use lsk::{tdb_to_utc, utc_to_tdb, LeapSecondData, LeapSecondExt};
pub use pool::KernelPoolExt;
pub use time::{format_calendar, format_iso8601, tdb_to_calendar, TimeFormat};

// Re-export extension traits for SPK/CK interpolation
pub use ck::{slerp, CkInterpolateExt, CkSegmentViewInterpolate};
pub use spk::{SpkInterpolateExt, SpkSegmentViewInterpolate};

// Re-export EpochTDB for convenience (primary definition is in types)
pub use crate::types::EpochTDB;
