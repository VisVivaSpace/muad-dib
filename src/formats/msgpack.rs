//! MessagePack output format implementation.

use super::OutputFormat;
use crate::hdf5_output::DAFSource;
use crate::prelude::*;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

/// MessagePack output format.
pub struct MsgPackFormat;

impl OutputFormat for MsgPackFormat {
    fn name(&self) -> &'static str {
        "msgpack"
    }

    fn extension(&self) -> &'static str {
        "msgpack"
    }

    fn write(&self, path: &Path, sources: &[DAFSource]) -> Result<()> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);

        rmp_serde::encode::write(&mut std::io::BufWriter::new(writer), sources)
            .map_err(|e| Error::Serialization { format: "MessagePack".into(), message: e.to_string() })?;

        Ok(())
    }
}

/// Read sources from a MessagePack file.
pub fn read_msgpack(path: &Path) -> Result<Vec<DAFSource>> {
    let file = File::open(path)?;

    let sources: Vec<DAFSource> = rmp_serde::decode::from_read(file)
        .map_err(|e| Error::Serialization { format: "MessagePack".into(), message: e.to_string() })?;

    Ok(sources)
}
