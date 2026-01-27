//! SPK writer module for reconstructing DAF/SPK files from parsed data.

use crate::daf_source::DAFSource;
use crate::prelude::*;
use crate::{DAFMetadata, DAFSegment, Endian};
use std::io::{BufWriter, Write};

/// FTP validation string used in DAF files.
/// This is a fixed string that SPICE uses to detect FTP corruption.
const FTP_STR: &[u8] =
    b"FTPSTR:\r:\n:\r\n:\r\x00:\x81:\x10\x00\x00\x00\x00:\x80\x00:\x08\x00:ENDFTP";

/// Write a DAF source back to an SPK file.
pub fn write_spk(path: &std::path::Path, source: &DAFSource) -> Result<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);

    let metadata = &source.metadata;
    let segments: Vec<_> = source
        .segments
        .iter()
        .filter_map(|s| match s {
            DAFSegment::SPK(spk) => Some(spk),
            _ => None,
        })
        .collect();

    if segments.is_empty() {
        return Err(Error::EmptyData {
            context: "No SPK segments to write".into(),
        });
    }

    // Calculate file structure
    // Record 1: File record (1024 bytes)
    // Record 2: Summary record (1024 bytes) - we'll use a single summary record
    // Record 3: Name record (1024 bytes)
    // Record 4+: Data records

    let _num_segments = segments.len();
    let summary_record = 2u64; // Summary starts at record 2
    let _name_record = 3u64; // Names start at record 3
    let data_start_record = 4u64; // Data starts at record 4

    // Calculate data addresses
    // DAF addresses are 1-indexed 8-byte (double word) offsets
    // Record 4 starts at byte 3072, which is DAF address 385 ((3072/8)+1)
    let mut current_daf_addr = (data_start_record - 1) * 128 + 1; // 128 doubles per 1024-byte record

    // Build segment info with new addresses
    let mut segment_addrs: Vec<(u64, u64)> = Vec::new();
    for seg in &segments {
        let data_len = seg.data.len() as u64;
        let start_addr = current_daf_addr;
        let end_addr = current_daf_addr + data_len - 1;
        segment_addrs.push((start_addr, end_addr));
        current_daf_addr = end_addr + 1;
    }

    // Free address is one past the last data element
    let free_address = current_daf_addr;

    // Write file record (Record 1)
    write_file_record(
        &mut writer,
        &source.header.name,
        metadata,
        summary_record,
        summary_record, // bward = fward for single summary record
        free_address,
    )?;

    // Write summary record (Record 2)
    write_summary_record(
        &mut writer,
        &segments,
        &segment_addrs,
        metadata.endian,
        0, // next record (0 = none)
        0, // prev record (0 = none)
    )?;

    // Write name record (Record 3)
    write_name_record(&mut writer, &segments)?;

    // Write data records (Record 4+)
    for seg in &segments {
        write_segment_data(&mut writer, &seg.data, metadata.endian)?;
    }

    // Pad to complete the last record if needed
    let total_bytes_written = 3 * 1024 + segments.iter().map(|s| s.data.len() * 8).sum::<usize>();
    let padding_needed = (1024 - (total_bytes_written % 1024)) % 1024;
    if padding_needed > 0 {
        writer.write_all(&vec![0u8; padding_needed])?;
    }

    writer.flush()?;

    Ok(())
}

fn write_file_record(
    writer: &mut BufWriter<File>,
    internal_name: &str,
    metadata: &DAFMetadata,
    fward: u64,
    bward: u64,
    free_address: u64,
) -> Result<()> {
    let mut record = [0u8; 1024];

    // LOCIDW: "DAF/SPK " (8 bytes at offset 0)
    record[0..8].copy_from_slice(b"DAF/SPK ");

    // ND (4 bytes at offset 8) - SPK uses ND=2
    let nd = 2i32;
    write_i32_to_buf(&mut record[8..12], nd, metadata.endian);

    // NI (4 bytes at offset 12) - SPK uses NI=6
    let ni = 6i32;
    write_i32_to_buf(&mut record[12..16], ni, metadata.endian);

    // LOCIFN: internal name (60 bytes at offset 16)
    let name_bytes = internal_name.as_bytes();
    let copy_len = name_bytes.len().min(60);
    record[16..16 + copy_len].copy_from_slice(&name_bytes[..copy_len]);

    // FWARD (4 bytes at offset 76)
    write_i32_to_buf(&mut record[76..80], fward as i32, metadata.endian);

    // BWARD (4 bytes at offset 80)
    write_i32_to_buf(&mut record[80..84], bward as i32, metadata.endian);

    // FREE (4 bytes at offset 84)
    write_i32_to_buf(&mut record[84..88], free_address as i32, metadata.endian);

    // LOCFMT (8 bytes at offset 88)
    let locfmt = metadata.endian.locfmt();
    record[88..88 + locfmt.len()].copy_from_slice(locfmt.as_bytes());

    // FTPSTR (28 bytes at offset 699)
    let ftp_len = FTP_STR.len().min(28);
    record[699..699 + ftp_len].copy_from_slice(&FTP_STR[..ftp_len]);

    writer.write_all(&record)?;

    Ok(())
}

fn write_summary_record(
    writer: &mut BufWriter<File>,
    segments: &[&crate::SPKSegment],
    addrs: &[(u64, u64)],
    endian: Endian,
    next_record: u64,
    prev_record: u64,
) -> Result<()> {
    let mut record = [0u8; 1024];

    // Summary record header (24 bytes):
    // - next summary record (f64 at offset 0)
    // - prev summary record (f64 at offset 8)
    // - number of summaries in this record (f64 at offset 16)
    write_f64_to_buf(&mut record[0..8], next_record as f64, endian);
    write_f64_to_buf(&mut record[8..16], prev_record as f64, endian);
    write_f64_to_buf(&mut record[16..24], segments.len() as f64, endian);

    // Each SPK summary is 40 bytes (ND=2 f64s + NI=6 i32s = 16 + 24 = 40 bytes)
    // Layout per summary:
    // - initial_epoch (f64)
    // - final_epoch (f64)
    // - target_code (i32)
    // - center_code (i32)
    // - frame_code (i32)
    // - spk_type (i32)
    // - data_start (i32)
    // - data_end (i32)

    let mut offset = 24;
    for (i, seg) in segments.iter().enumerate() {
        let (data_start, data_end) = addrs[i];

        write_f64_to_buf(&mut record[offset..offset + 8], seg.initial_epoch, endian);
        offset += 8;
        write_f64_to_buf(&mut record[offset..offset + 8], seg.final_epoch, endian);
        offset += 8;
        write_i32_to_buf(&mut record[offset..offset + 4], seg.target_code.0, endian);
        offset += 4;
        write_i32_to_buf(&mut record[offset..offset + 4], seg.center_code.0, endian);
        offset += 4;
        write_i32_to_buf(&mut record[offset..offset + 4], seg.frame_code.0, endian);
        offset += 4;
        write_i32_to_buf(&mut record[offset..offset + 4], seg.spk_type, endian);
        offset += 4;
        write_i32_to_buf(&mut record[offset..offset + 4], data_start as i32, endian);
        offset += 4;
        write_i32_to_buf(&mut record[offset..offset + 4], data_end as i32, endian);
        offset += 4;
    }

    writer.write_all(&record)?;

    Ok(())
}

fn write_name_record(writer: &mut BufWriter<File>, segments: &[&crate::SPKSegment]) -> Result<()> {
    let mut record = [0u8; 1024];

    // Each segment name is stored as a fixed-width field
    // NC = 8 * (ND + (NI+1)/2) = 8 * (2 + 3) = 40 bytes per name for SPK
    let nc = 40usize;

    for (i, seg) in segments.iter().enumerate() {
        let offset = i * nc;
        if offset + nc > 1024 {
            break; // Can't fit more names in this record
        }
        let name_bytes = seg.name.as_bytes();
        let copy_len = name_bytes.len().min(nc);
        record[offset..offset + copy_len].copy_from_slice(&name_bytes[..copy_len]);
    }

    writer.write_all(&record)?;

    Ok(())
}

fn write_segment_data(writer: &mut BufWriter<File>, data: &[f64], endian: Endian) -> Result<()> {
    for &val in data {
        let bytes = match endian {
            Endian::Little => val.to_le_bytes(),
            Endian::Big => val.to_be_bytes(),
        };
        writer.write_all(&bytes)?;
    }
    Ok(())
}

fn write_i32_to_buf(buf: &mut [u8], val: i32, endian: Endian) {
    let bytes = match endian {
        Endian::Little => val.to_le_bytes(),
        Endian::Big => val.to_be_bytes(),
    };
    buf[..4].copy_from_slice(&bytes);
}

fn write_f64_to_buf(buf: &mut [u8], val: f64, endian: Endian) {
    let bytes = match endian {
        Endian::Little => val.to_le_bytes(),
        Endian::Big => val.to_be_bytes(),
    };
    buf[..8].copy_from_slice(&bytes);
}
