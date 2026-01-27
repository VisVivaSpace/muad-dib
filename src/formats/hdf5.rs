//! HDF5 output format implementation.

use super::OutputFormat;
use crate::hdf5_output::{write_hdf5, DAFSource};
use crate::prelude::*;
use std::path::Path;

/// HDF5 output format.
pub struct Hdf5Format;

impl OutputFormat for Hdf5Format {
    fn name(&self) -> &'static str {
        "hdf5"
    }

    fn extension(&self) -> &'static str {
        "hdf5"
    }

    fn write(&self, path: &Path, sources: &[DAFSource]) -> Result<()> {
        write_hdf5(path, sources.to_vec())
    }
}
