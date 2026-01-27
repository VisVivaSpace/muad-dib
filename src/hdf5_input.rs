//! HDF5 input module for reading DAF and PCK data from HDF5 format.

use crate::hdf5_output::DAFSource;
use crate::prelude::*;
use crate::text_pck::{KernelValue, PCKBlock, PCKSource, PCKVariable};
use crate::{BPCKSegment, CKSegment, DAFHeader, DAFMetadata, DAFSegment, Endian, SPKSegment};
use hdf5::types::VarLenUnicode;
use hdf5::File as H5File;

/// Read multiple DAF sources from a single HDF5 file.
/// Returns an empty vector if the "sources" group doesn't exist (e.g., PCK-only files).
pub fn read_hdf5(path: &std::path::Path) -> Result<Vec<DAFSource>> {
    let file =
        H5File::open(path).map_err(|e| Error::Hdf5 { operation: "open".into(), message: e.to_string() })?;

    // Check if sources group exists - it may not if the file only contains PCK data
    let sources_group = match file.group("sources") {
        Ok(g) => g,
        Err(_) => return Ok(Vec::new()), // No DAF sources in this file
    };

    let mut sources = Vec::new();

    // Get all source groups
    let source_names = sources_group
        .member_names()
        .map_err(|e| Error::Hdf5 { operation: "member_names".into(), message: e.to_string() })?;

    for src_name in source_names {
        let src_group = sources_group
            .group(&src_name)
            .map_err(|e| Error::Hdf5 { operation: "group".into(), message: e.to_string() })?;

        // Read header attributes
        let name = read_string_attr(&src_group, "name")?;
        let kind = read_string_attr(&src_group, "kind")?;
        let comment = read_string_attr(&src_group, "comment")?;
        let filename = read_string_attr(&src_group, "filename")?;

        let header = DAFHeader { name, comment, kind };

        // Read metadata attributes
        let nd = read_u64_attr(&src_group, "nd")?;
        let ni = read_u64_attr(&src_group, "ni")?;
        let endian_str = read_string_attr(&src_group, "endian")?;
        let endian = match endian_str.as_str() {
            "LTL-IEEE" => Endian::Little,
            "BIG-IEEE" => Endian::Big,
            _ => {
                return Err(Error::UnknownFormat { format: endian_str })
            }
        };
        let fward = read_u64_attr(&src_group, "fward")?;
        let bward = read_u64_attr(&src_group, "bward")?;
        let free_address = read_u64_attr(&src_group, "free_address")?;
        let ftpstr = read_string_attr(&src_group, "ftpstr")?;

        let metadata = DAFMetadata {
            nd,
            ni,
            endian,
            fward,
            bward,
            free_address,
            ftpstr,
        };

        // Read segments
        let segs_group = src_group
            .group("segments")
            .map_err(|e| Error::Hdf5 { operation: "group".into(), message: e.to_string() })?;

        let segment_names = segs_group
            .member_names()
            .map_err(|e| Error::Hdf5 { operation: "member_names".into(), message: e.to_string() })?;

        let mut segments = Vec::new();
        for seg_name in segment_names {
            let seg_group = segs_group
                .group(&seg_name)
                .map_err(|e| Error::Hdf5 { operation: "group".into(), message: e.to_string() })?;

            let segment = read_segment(&seg_group)?;
            segments.push(segment);
        }

        sources.push(DAFSource {
            filename,
            header,
            metadata,
            segments,
        });
    }

    Ok(sources)
}

/// Read PCK sources from an HDF5 file.
///
/// PCK sources are stored in a "pck" group, separate from DAF sources.
pub fn read_pck_sources(path: &std::path::Path) -> Result<Vec<PCKSource>> {
    let file =
        H5File::open(path).map_err(|e| Error::Hdf5 { operation: "open".into(), message: e.to_string() })?;

    // Check if pck group exists
    let pck_group = match file.group("pck") {
        Ok(g) => g,
        Err(_) => return Ok(Vec::new()), // No PCK sources in this file
    };

    let mut sources = Vec::new();

    // Get all source groups
    let source_names = pck_group
        .member_names()
        .map_err(|e| Error::Hdf5 { operation: "member_names".into(), message: e.to_string() })?;

    for src_name in source_names {
        let src_group = pck_group
            .group(&src_name)
            .map_err(|e| Error::Hdf5 { operation: "group".into(), message: e.to_string() })?;

        let filename = read_string_attr(&src_group, "filename")?;
        let block_count = read_i32_attr(&src_group, "block_count")? as usize;

        let mut blocks = Vec::with_capacity(block_count);

        // Read blocks in order
        for i in 0..block_count {
            let block_group = src_group
                .group(&format!("block_{:03}", i))
                .map_err(|e| Error::Hdf5 { operation: "group".into(), message: e.to_string() })?;

            let block_type = read_string_attr(&block_group, "type")?;

            let block = match block_type.as_str() {
                "text" => {
                    let content = read_string_attr(&block_group, "content")?;
                    PCKBlock::Text(content)
                }
                "data" => {
                    let vars = read_pck_variables(&block_group)?;
                    PCKBlock::Data(vars)
                }
                _ => {
                    return Err(Error::UnknownFormat { format: format!("PCK block type: {}", block_type) })
                }
            };

            blocks.push(block);
        }

        sources.push(PCKSource { filename, blocks });
    }

    Ok(sources)
}

/// Read PCK variables from an HDF5 group.
///
/// Supports two formats:
/// - "numeric_array": All values are f64, stored in a single dataset
/// - "mixed": Each value stored individually with type info
fn read_pck_variables(group: &hdf5::Group) -> Result<Vec<PCKVariable>> {
    let var_count = read_i32_attr(group, "variable_count")? as usize;
    let mut vars = Vec::with_capacity(var_count);

    for i in 0..var_count {
        let var_group = group
            .group(&format!("var_{:03}", i))
            .map_err(|e| Error::Hdf5 { operation: "group".into(), message: e.to_string() })?;

        let name = read_string_attr(&var_group, "name")?;

        // Check format - default to "numeric_array" for backward compatibility
        let format = read_string_attr(&var_group, "value_format")
            .unwrap_or_else(|_| "numeric_array".to_string());

        let values = if format == "numeric_array" {
            // All values are numeric
            let floats = read_f64_dataset(&var_group, "values")?;
            floats.into_iter().map(KernelValue::Numeric).collect()
        } else {
            // Mixed values - read each individually
            let value_count = read_i32_attr(&var_group, "value_count")? as usize;
            let mut values = Vec::with_capacity(value_count);

            for j in 0..value_count {
                let val_group = var_group
                    .group(&format!("val_{:03}", j))
                    .map_err(|e| Error::Hdf5 { operation: "group".into(), message: e.to_string() })?;

                let val_type = read_string_attr(&val_group, "type")?;
                let value = match val_type.as_str() {
                    "numeric" => {
                        let f = read_f64_attr(&val_group, "data")?;
                        KernelValue::Numeric(f)
                    }
                    "epoch" => {
                        let s = read_string_attr(&val_group, "data")?;
                        KernelValue::Epoch(s)
                    }
                    "text" => {
                        let s = read_string_attr(&val_group, "data")?;
                        KernelValue::Text(s)
                    }
                    _ => {
                        return Err(Error::UnknownFormat { format: format!("value type: {}", val_type) })
                    }
                };
                values.push(value);
            }
            values
        };

        vars.push(PCKVariable { name, values });
    }

    Ok(vars)
}

fn read_segment(group: &hdf5::Group) -> Result<DAFSegment> {
    let segment_type = read_string_attr(group, "segment_type")?;

    match segment_type.as_str() {
        "SPK" => read_spk_segment(group),
        "CK" => read_ck_segment(group),
        "BPCK" => read_bpck_segment(group),
        _ => Err(Error::UnknownFormat { format: format!("segment type: {}", segment_type) }),
    }
}

fn read_spk_segment(group: &hdf5::Group) -> Result<DAFSegment> {
    let name = read_string_attr(group, "name")?;
    let spk_type = read_i32_attr(group, "spk_type")?;
    let target_code = read_i32_attr(group, "target_code")?;
    let center_code = read_i32_attr(group, "center_code")?;
    let frame_code = read_i32_attr(group, "frame_code")?;
    let initial_epoch = read_f64_attr(group, "initial_epoch")?;
    let final_epoch = read_f64_attr(group, "final_epoch")?;
    let data_start = read_u64_attr(group, "data_start")?;
    let data_end = read_u64_attr(group, "data_end")?;
    let data = read_f64_dataset(group, "data")?;

    Ok(DAFSegment::SPK(SPKSegment {
        name,
        initial_epoch,
        final_epoch,
        target_code,
        center_code,
        frame_code,
        spk_type,
        data_start,
        data_end,
        data,
    }))
}

fn read_ck_segment(group: &hdf5::Group) -> Result<DAFSegment> {
    let name = read_string_attr(group, "name")?;
    let ck_type = read_i32_attr(group, "ck_type")?;
    let instrument_code = read_i32_attr(group, "instrument_code")?;
    let frame_code = read_i32_attr(group, "frame_code")?;
    let initial_sclk = read_f64_attr(group, "initial_sclk")?;
    let final_sclk = read_f64_attr(group, "final_sclk")?;
    let data_start = read_u64_attr(group, "data_start")?;
    let data_end = read_u64_attr(group, "data_end")?;
    let rates = read_bool_attr(group, "rates")?;
    let data = read_f64_dataset(group, "data")?;

    Ok(DAFSegment::CK(CKSegment {
        name,
        initial_sclk,
        final_sclk,
        instrument_code,
        frame_code,
        ck_type,
        rates,
        data_start,
        data_end,
        data,
    }))
}

fn read_bpck_segment(group: &hdf5::Group) -> Result<DAFSegment> {
    let name = read_string_attr(group, "name")?;
    let bpck_type = read_i32_attr(group, "bpck_type")?;
    let frame_id = read_i32_attr(group, "frame_id")?;
    let base_frame = read_i32_attr(group, "base_frame")?;
    let initial_epoch = read_f64_attr(group, "initial_epoch")?;
    let final_epoch = read_f64_attr(group, "final_epoch")?;
    let data_start = read_u64_attr(group, "data_start")?;
    let data_end = read_u64_attr(group, "data_end")?;
    let data = read_f64_dataset(group, "data")?;

    Ok(DAFSegment::BPCK(BPCKSegment {
        name,
        initial_epoch,
        final_epoch,
        frame_id,
        base_frame,
        bpck_type,
        data_start,
        data_end,
        data,
    }))
}

fn read_string_attr(group: &hdf5::Group, name: &str) -> Result<String> {
    let attr = group
        .attr(name)
        .map_err(|e| Error::Hdf5 { operation: format!("attr '{}'", name), message: e.to_string() })?;
    let value: VarLenUnicode = attr
        .read_scalar()
        .map_err(|e| Error::Hdf5 { operation: format!("read '{}'", name), message: e.to_string() })?;
    Ok(value.to_string())
}

fn read_i32_attr(group: &hdf5::Group, name: &str) -> Result<i32> {
    let attr = group
        .attr(name)
        .map_err(|e| Error::Hdf5 { operation: format!("attr '{}'", name), message: e.to_string() })?;
    let value: i32 = attr
        .read_scalar()
        .map_err(|e| Error::Hdf5 { operation: format!("read '{}'", name), message: e.to_string() })?;
    Ok(value)
}

fn read_u64_attr(group: &hdf5::Group, name: &str) -> Result<u64> {
    let attr = group
        .attr(name)
        .map_err(|e| Error::Hdf5 { operation: format!("attr '{}'", name), message: e.to_string() })?;
    let value: u64 = attr
        .read_scalar()
        .map_err(|e| Error::Hdf5 { operation: format!("read '{}'", name), message: e.to_string() })?;
    Ok(value)
}

fn read_f64_attr(group: &hdf5::Group, name: &str) -> Result<f64> {
    let attr = group
        .attr(name)
        .map_err(|e| Error::Hdf5 { operation: format!("attr '{}'", name), message: e.to_string() })?;
    let value: f64 = attr
        .read_scalar()
        .map_err(|e| Error::Hdf5 { operation: format!("read '{}'", name), message: e.to_string() })?;
    Ok(value)
}

fn read_bool_attr(group: &hdf5::Group, name: &str) -> Result<bool> {
    let attr = group
        .attr(name)
        .map_err(|e| Error::Hdf5 { operation: format!("attr '{}'", name), message: e.to_string() })?;
    let value: bool = attr
        .read_scalar()
        .map_err(|e| Error::Hdf5 { operation: format!("read '{}'", name), message: e.to_string() })?;
    Ok(value)
}

fn read_f64_dataset(group: &hdf5::Group, name: &str) -> Result<Vec<f64>> {
    let dataset = group
        .dataset(name)
        .map_err(|e| Error::Hdf5 { operation: format!("dataset '{}'", name), message: e.to_string() })?;
    let data: Vec<f64> = dataset
        .read_raw()
        .map_err(|e| Error::Hdf5 { operation: format!("read '{}'", name), message: e.to_string() })?;
    Ok(data)
}
