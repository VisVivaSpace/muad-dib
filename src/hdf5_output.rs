//! HDF5 output module for writing DAF and PCK data to HDF5 format.

pub use crate::daf_source::DAFSource;
use crate::prelude::*;
use crate::text_pck::{KernelValue, PCKBlock, PCKSource, PCKVariable};
use crate::{BPCKSegment, CKSegment, DAFSegment, SPKSegment};
use hdf5::types::VarLenUnicode;
use hdf5::File as H5File;
use ndarray::Array1;
use std::str::FromStr;

/// Write multiple DAF sources to a single HDF5 file.
pub fn write_hdf5(path: &std::path::Path, sources: Vec<DAFSource>) -> Result<()> {
    let file = H5File::create(path).map_err(|e| Error::Hdf5 {
        operation: "create".into(),
        message: e.to_string(),
    })?;

    let sources_group = file.create_group("sources").map_err(|e| Error::Hdf5 {
        operation: "group".into(),
        message: e.to_string(),
    })?;

    for (src_idx, source) in sources.iter().enumerate() {
        let src_name = format!("source_{:03}", src_idx);
        let src_group = sources_group
            .create_group(&src_name)
            .map_err(|e| Error::Hdf5 {
                operation: "group".into(),
                message: e.to_string(),
            })?;

        // Add header attributes
        write_string_attr(&src_group, "name", &source.header.name)?;
        write_string_attr(&src_group, "kind", &source.header.kind)?;
        write_string_attr(&src_group, "comment", &source.header.comment)?;
        write_string_attr(&src_group, "filename", &source.filename)?;

        // Add metadata attributes for round-trip support
        write_u64_attr(&src_group, "nd", source.metadata.nd)?;
        write_u64_attr(&src_group, "ni", source.metadata.ni)?;
        write_string_attr(&src_group, "endian", source.metadata.endian.locfmt())?;
        write_u64_attr(&src_group, "fward", source.metadata.fward)?;
        write_u64_attr(&src_group, "bward", source.metadata.bward)?;
        write_u64_attr(&src_group, "free_address", source.metadata.free_address)?;
        write_string_attr(&src_group, "ftpstr", &source.metadata.ftpstr)?;

        // Create segments group
        let segs_group = src_group
            .create_group("segments")
            .map_err(|e| Error::Hdf5 {
                operation: "group".into(),
                message: e.to_string(),
            })?;

        for (seg_idx, segment) in source.segments.iter().enumerate() {
            let seg_name = format!("segment_{:03}", seg_idx);
            let seg_group = segs_group
                .create_group(&seg_name)
                .map_err(|e| Error::Hdf5 {
                    operation: "group".into(),
                    message: e.to_string(),
                })?;

            match segment {
                DAFSegment::SPK(spk) => write_spk_segment(&seg_group, spk)?,
                DAFSegment::CK(ck) => write_ck_segment(&seg_group, ck)?,
                DAFSegment::BPCK(bpck) => write_bpck_segment(&seg_group, bpck)?,
            }
        }
    }

    Ok(())
}

fn write_spk_segment(group: &hdf5::Group, spk: &SPKSegment) -> Result<()> {
    // Segment type attribute
    write_string_attr(group, "segment_type", "SPK")?;
    write_string_attr(group, "name", &spk.name)?;
    write_i32_attr(group, "spk_type", spk.spk_type)?;
    write_i32_attr(group, "target_code", spk.target_code)?;
    write_i32_attr(group, "center_code", spk.center_code)?;
    write_i32_attr(group, "frame_code", spk.frame_code)?;
    write_f64_attr(group, "initial_epoch", spk.initial_epoch)?;
    write_f64_attr(group, "final_epoch", spk.final_epoch)?;
    // Data offsets for round-trip support
    write_u64_attr(group, "data_start", spk.data_start)?;
    write_u64_attr(group, "data_end", spk.data_end)?;

    // Data dataset
    let data_arr = Array1::from_vec(spk.data.clone());
    let dataset = group
        .new_dataset::<f64>()
        .shape([spk.data.len()])
        .create("data")
        .map_err(|e| Error::Hdf5 {
            operation: "dataset".into(),
            message: e.to_string(),
        })?;
    dataset.write(&data_arr).map_err(|e| Error::Hdf5 {
        operation: "write".into(),
        message: e.to_string(),
    })?;

    Ok(())
}

fn write_ck_segment(group: &hdf5::Group, ck: &CKSegment) -> Result<()> {
    write_string_attr(group, "segment_type", "CK")?;
    write_string_attr(group, "name", &ck.name)?;
    write_i32_attr(group, "ck_type", ck.ck_type)?;
    write_i32_attr(group, "instrument_code", ck.instrument_code)?;
    write_i32_attr(group, "frame_code", ck.frame_code)?;
    write_f64_attr(group, "initial_sclk", ck.initial_sclk)?;
    write_f64_attr(group, "final_sclk", ck.final_sclk)?;
    // Data offsets for round-trip support
    write_u64_attr(group, "data_start", ck.data_start)?;
    write_u64_attr(group, "data_end", ck.data_end)?;

    let attr = group
        .new_attr::<bool>()
        .shape(())
        .create("rates")
        .map_err(|e| Error::Hdf5 {
            operation: "attr".into(),
            message: e.to_string(),
        })?;
    attr.write_scalar(&ck.rates).map_err(|e| Error::Hdf5 {
        operation: "write".into(),
        message: e.to_string(),
    })?;

    let data_arr = Array1::from_vec(ck.data.clone());
    let dataset = group
        .new_dataset::<f64>()
        .shape([ck.data.len()])
        .create("data")
        .map_err(|e| Error::Hdf5 {
            operation: "dataset".into(),
            message: e.to_string(),
        })?;
    dataset.write(&data_arr).map_err(|e| Error::Hdf5 {
        operation: "write".into(),
        message: e.to_string(),
    })?;

    Ok(())
}

fn write_bpck_segment(group: &hdf5::Group, bpck: &BPCKSegment) -> Result<()> {
    write_string_attr(group, "segment_type", "BPCK")?;
    write_string_attr(group, "name", &bpck.name)?;
    write_i32_attr(group, "bpck_type", bpck.bpck_type)?;
    write_i32_attr(group, "frame_id", bpck.frame_id)?;
    write_i32_attr(group, "base_frame", bpck.base_frame)?;
    write_f64_attr(group, "initial_epoch", bpck.initial_epoch)?;
    write_f64_attr(group, "final_epoch", bpck.final_epoch)?;
    // Data offsets for round-trip support
    write_u64_attr(group, "data_start", bpck.data_start)?;
    write_u64_attr(group, "data_end", bpck.data_end)?;

    let data_arr = Array1::from_vec(bpck.data.clone());
    let dataset = group
        .new_dataset::<f64>()
        .shape([bpck.data.len()])
        .create("data")
        .map_err(|e| Error::Hdf5 {
            operation: "dataset".into(),
            message: e.to_string(),
        })?;
    dataset.write(&data_arr).map_err(|e| Error::Hdf5 {
        operation: "write".into(),
        message: e.to_string(),
    })?;

    Ok(())
}

/// Write PCK sources to an HDF5 file.
///
/// PCK sources are stored in a "pck" group, separate from DAF sources.
/// Each source is stored with its filename and blocks (text/data).
pub fn write_pck_sources(file: &H5File, sources: &[PCKSource]) -> Result<()> {
    if sources.is_empty() {
        return Ok(());
    }

    let pck_group = file.create_group("pck").map_err(|e| Error::Hdf5 {
        operation: "group".into(),
        message: e.to_string(),
    })?;

    for (src_idx, source) in sources.iter().enumerate() {
        let src_name = format!("source_{:03}", src_idx);
        let src_group = pck_group.create_group(&src_name).map_err(|e| Error::Hdf5 {
            operation: "group".into(),
            message: e.to_string(),
        })?;

        // Store filename
        write_string_attr(&src_group, "filename", &source.filename)?;
        let block_count = i32::try_from(source.blocks.len()).map_err(|_| Error::Hdf5 {
            operation: "attr".into(),
            message: format!("block count {} exceeds i32 range", source.blocks.len()),
        })?;
        write_i32_attr(&src_group, "block_count", block_count)?;

        // Store blocks as indexed groups
        for (i, block) in source.blocks.iter().enumerate() {
            let block_group = src_group
                .create_group(&format!("block_{:03}", i))
                .map_err(|e| Error::Hdf5 {
                    operation: "group".into(),
                    message: e.to_string(),
                })?;

            match block {
                PCKBlock::Text(text) => {
                    write_string_attr(&block_group, "type", "text")?;
                    write_string_attr(&block_group, "content", text)?;
                }
                PCKBlock::Data(vars) => {
                    write_string_attr(&block_group, "type", "data")?;
                    write_pck_variables(&block_group, vars)?;
                }
            }
        }
    }

    Ok(())
}

/// Write PCK variables to an HDF5 group.
///
/// Each variable stores values with type information to support mixed Numeric/Epoch/Text values.
/// Format:
/// - value_count: number of values
/// - For each value: type ("numeric", "epoch", "text") + data (f64 or string)
fn write_pck_variables(group: &hdf5::Group, vars: &[PCKVariable]) -> Result<()> {
    let var_count = i32::try_from(vars.len()).map_err(|_| Error::Hdf5 {
        operation: "attr".into(),
        message: format!("variable count {} exceeds i32 range", vars.len()),
    })?;
    write_i32_attr(group, "variable_count", var_count)?;

    for (i, var) in vars.iter().enumerate() {
        let var_group = group
            .create_group(&format!("var_{:03}", i))
            .map_err(|e| Error::Hdf5 {
                operation: "group".into(),
                message: e.to_string(),
            })?;

        write_string_attr(&var_group, "name", &var.name)?;
        let val_count = i32::try_from(var.values.len()).map_err(|_| Error::Hdf5 {
            operation: "attr".into(),
            message: format!("value count {} exceeds i32 range", var.values.len()),
        })?;
        write_i32_attr(&var_group, "value_count", val_count)?;

        // Check if all values are numeric (optimization for common case)
        let all_numeric = var
            .values
            .iter()
            .all(|v| matches!(v, KernelValue::Numeric(_)));
        write_string_attr(
            &var_group,
            "value_format",
            if all_numeric {
                "numeric_array"
            } else {
                "mixed"
            },
        )?;

        if all_numeric {
            // Optimized storage for all-numeric variables (common case)
            let floats: Vec<f64> = var
                .values
                .iter()
                .filter_map(|v| {
                    if let KernelValue::Numeric(f) = v {
                        Some(*f)
                    } else {
                        None
                    }
                })
                .collect();
            let data_arr = Array1::from_vec(floats);
            let dataset = var_group
                .new_dataset::<f64>()
                .shape([var.values.len()])
                .create("values")
                .map_err(|e| Error::Hdf5 {
                    operation: "dataset".into(),
                    message: e.to_string(),
                })?;
            dataset.write(&data_arr).map_err(|e| Error::Hdf5 {
                operation: "write".into(),
                message: e.to_string(),
            })?;
        } else {
            // Mixed values - store each with type
            for (j, value) in var.values.iter().enumerate() {
                let val_group = var_group
                    .create_group(&format!("val_{:03}", j))
                    .map_err(|e| Error::Hdf5 {
                        operation: "group".into(),
                        message: e.to_string(),
                    })?;

                match value {
                    KernelValue::Numeric(f) => {
                        write_string_attr(&val_group, "type", "numeric")?;
                        write_f64_attr(&val_group, "data", *f)?;
                    }
                    KernelValue::Epoch(s) => {
                        write_string_attr(&val_group, "type", "epoch")?;
                        write_string_attr(&val_group, "data", s)?;
                    }
                    KernelValue::Text(s) => {
                        write_string_attr(&val_group, "type", "text")?;
                        write_string_attr(&val_group, "data", s)?;
                    }
                }
            }
        }
    }

    Ok(())
}

fn write_string_attr(group: &hdf5::Group, name: &str, value: &str) -> Result<()> {
    let attr = group
        .new_attr::<VarLenUnicode>()
        .shape(())
        .create(name)
        .map_err(|e| Error::Hdf5 {
            operation: "attr".into(),
            message: e.to_string(),
        })?;
    let unicode_val = VarLenUnicode::from_str(value).map_err(|e| Error::Hdf5 {
        operation: "string".into(),
        message: e.to_string(),
    })?;
    attr.write_scalar(&unicode_val).map_err(|e| Error::Hdf5 {
        operation: "write".into(),
        message: e.to_string(),
    })?;
    Ok(())
}

fn write_i32_attr(group: &hdf5::Group, name: &str, value: i32) -> Result<()> {
    let attr = group
        .new_attr::<i32>()
        .shape(())
        .create(name)
        .map_err(|e| Error::Hdf5 {
            operation: "attr".into(),
            message: e.to_string(),
        })?;
    attr.write_scalar(&value).map_err(|e| Error::Hdf5 {
        operation: "write".into(),
        message: e.to_string(),
    })?;
    Ok(())
}

fn write_u64_attr(group: &hdf5::Group, name: &str, value: u64) -> Result<()> {
    let attr = group
        .new_attr::<u64>()
        .shape(())
        .create(name)
        .map_err(|e| Error::Hdf5 {
            operation: "attr".into(),
            message: e.to_string(),
        })?;
    attr.write_scalar(&value).map_err(|e| Error::Hdf5 {
        operation: "write".into(),
        message: e.to_string(),
    })?;
    Ok(())
}

fn write_f64_attr(group: &hdf5::Group, name: &str, value: f64) -> Result<()> {
    let attr = group
        .new_attr::<f64>()
        .shape(())
        .create(name)
        .map_err(|e| Error::Hdf5 {
            operation: "attr".into(),
            message: e.to_string(),
        })?;
    attr.write_scalar(&value).map_err(|e| Error::Hdf5 {
        operation: "write".into(),
        message: e.to_string(),
    })?;
    Ok(())
}
