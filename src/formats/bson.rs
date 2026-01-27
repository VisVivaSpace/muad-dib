//! BSON output format implementation.

use super::OutputFormat;
use crate::daf_source::DAFSource;
use crate::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;

/// BSON output format.
pub struct BsonFormat;

/// Wrapper struct for BSON serialization (BSON requires a document at top level).
#[derive(Serialize, Deserialize)]
struct BsonWrapper {
    sources: Vec<DAFSource>,
}

impl OutputFormat for BsonFormat {
    fn name(&self) -> &'static str {
        "bson"
    }

    fn extension(&self) -> &'static str {
        "bson"
    }

    fn write(&self, path: &Path, sources: &[DAFSource]) -> Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        let wrapper = BsonWrapper {
            sources: sources.to_vec(),
        };

        let doc = bson::to_document(&wrapper).map_err(|e| Error::Serialization {
            format: "BSON".into(),
            message: e.to_string(),
        })?;

        let bytes = bson::to_vec(&doc).map_err(|e| Error::Serialization {
            format: "BSON".into(),
            message: e.to_string(),
        })?;

        writer.write_all(&bytes)?;

        Ok(())
    }
}

/// Read sources from a BSON file.
pub fn read_bson(path: &Path) -> Result<Vec<DAFSource>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let doc: bson::Document =
        bson::Document::from_reader(reader).map_err(|e| Error::Serialization {
            format: "BSON".into(),
            message: e.to_string(),
        })?;

    let wrapper: BsonWrapper = bson::from_document(doc).map_err(|e| Error::Serialization {
        format: "BSON".into(),
        message: e.to_string(),
    })?;

    Ok(wrapper.sources)
}
