//! Builder pattern for SpiceKernel construction.
//!
//! The builder allows loading multiple files of different types into a single
//! unified kernel:
//!
//! ```ignore
//! let kernel = SpiceKernel::builder()
//!     .file("de430.bsp")
//!     .file("pck00010.tpc")
//!     .build()?;
//! ```

use crate::hdf5_input::{read_hdf5, read_pck_sources};
use crate::hdf5_output::DAFSource;
use crate::prelude::*;
use crate::text_pck::PCKSource;
use crate::DAFFile;
use std::path::{Path, PathBuf};

use super::coverage::CoverageIndex;
use super::SpiceKernel;

/// Builder for creating SpiceKernel instances.
///
/// Supports loading multiple files of different types (SPK, CK, BPCK, PCK, HDF5)
/// into a unified kernel.
#[derive(Debug, Default)]
pub struct SpiceKernelBuilder {
    files: Vec<PathBuf>,
}

impl SpiceKernelBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a file to load.
    pub fn file<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.files.push(path.as_ref().to_path_buf());
        self
    }

    /// Add multiple files to load.
    pub fn files<P: AsRef<Path>, I: IntoIterator<Item = P>>(mut self, paths: I) -> Self {
        for path in paths {
            self.files.push(path.as_ref().to_path_buf());
        }
        self
    }

    /// Build the SpiceKernel, loading all specified files.
    #[must_use = "building a SpiceKernel returns a Result that should be handled"]
    pub fn build(self) -> Result<SpiceKernel> {
        let mut daf_sources = Vec::new();
        let mut pck_sources = Vec::new();

        for path in &self.files {
            let (dafs, pcks) = load_file(path)?;
            daf_sources.extend(dafs);
            pck_sources.extend(pcks);
        }

        let coverage_index = CoverageIndex::from_daf_sources(&daf_sources);

        Ok(SpiceKernel {
            daf_sources,
            pck_sources,
            coverage_index,
        })
    }
}

/// Load a single file, returning DAF sources and PCK sources.
fn load_file(path: &Path) -> Result<(Vec<DAFSource>, Vec<PCKSource>)> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        // Native DAF formats
        "bsp" | "spk" | "bc" | "ck" | "bpc" | "bpck" => {
            let source = load_daf_file(path)?;
            Ok((vec![source], vec![]))
        }
        // Text kernels (PCK, LSK, SCLK, FK)
        "tpc" | "pck" | "tls" | "tsc" | "tf" => {
            let source = PCKSource::from_path(path)?;
            Ok((vec![], vec![source]))
        }
        // HDF5 (may contain both DAF and PCK)
        "hdf5" | "h5" => {
            let daf_sources = read_hdf5(path)?;
            let pck_sources = read_pck_sources(path).unwrap_or_default();
            Ok((daf_sources, pck_sources))
        }
        _ => Err(Error::UnknownFormat {
            format: format!("file extension: '{}'. Supported: bsp, spk, bc, ck, bpc, bpck, tpc, pck, tls, tsc, tf, hdf5, h5", ext)
        }),
    }
}

/// Load a native DAF file into a DAFSource.
fn load_daf_file(path: &Path) -> Result<DAFSource> {
    let file = File::open(path)?;
    let mut daf = DAFFile::from_file(file)?;

    let header = daf.daf_header()?;
    let metadata = daf.daf_metadata();

    let mut segments = Vec::new();
    for segment_result in daf {
        segments.push(segment_result?);
    }

    Ok(DAFSource {
        filename: path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string(),
        header,
        metadata,
        segments,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_empty() {
        let kernel = SpiceKernelBuilder::new().build().unwrap();
        assert!(kernel.daf_sources.is_empty());
        assert!(kernel.pck_sources.is_empty());
    }
}
