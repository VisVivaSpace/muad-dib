//! # muad-dib
//!
//! Parser and I/O library for NAIF SPICE Double Precision Array Files (DAF).
//!
//! This library reads DAF binary files (SPK, CK, BPCK), loads text kernels
//! (LSK, PCK, FK, SCLK), and converts between SPICE and modern data formats
//! (HDF5, Parquet, Arrow, MessagePack, BSON).
//!
//! Interpolation and computation have moved to the `understated` crate.
//!
//! ## Kernel Loading
//!
//! Use [`kernel::SpiceKernel`] to load one or more kernel files:
//!
//! ```ignore
//! use muad_dib::kernel::SpiceKernel;
//! use muad_dib::types::NaifId;
//!
//! let kernel = SpiceKernel::load("de440.bsp")?;
//! let bodies = kernel.spk_bodies();
//! let segments = kernel.spk_segments();
//! ```
//!
//! ## Type-Safe Newtypes
//!
//! The [`types`] module provides newtypes with `Display` implementations for
//! cleaner output:
//!
//! - [`types::NaifId`] - NAIF body/frame identifiers
//! - [`types::EpochTDB`] - TDB seconds past J2000 (displays as "123.45 TDB")
//! - [`types::DafAddress`] - DAF 1-indexed double-word addresses
//!
//! ## Low-Level DAF Access
//!
//! For direct segment iteration, use [`DAFFile`]:
//!
//! ```no_run
//! use muad_dib::{DAFFile, DAFSegment};
//! use std::fs::File;
//!
//! let file = File::open("ephemeris.bsp").unwrap();
//! let daf = DAFFile::from_file(file).unwrap();
//!
//! for segment in daf {
//!     if let Ok(DAFSegment::SPK(spk)) = segment {
//!         println!("Target {}: {} to {}", spk.target_code, spk.initial_epoch, spk.final_epoch);
//!     }
//! }
//! ```

use crate::prelude::*;
use crate::types::NaifId;

pub mod brief;
pub mod daf_source;
pub mod error;
pub mod formats;
#[cfg(feature = "hdf5")]
pub mod hdf5_input;
#[cfg(feature = "hdf5")]
pub mod hdf5_output;
pub mod inspector;
pub mod kernel;
pub mod pck_writer;
mod prelude;
pub mod spice;
pub mod spk_writer;
pub mod text_pck;
pub mod types;

// Re-export NAIF ID utilities for convenient access
pub use brief::names::{
    body_name, format_frame_id, format_id, format_instrument_id, frame_name, spacecraft_name,
};

// Re-export time formatting utilities
pub use brief::time::{format_sclk_ticks, format_time, format_time_for_display};
pub use brief::TimeFormat;

// Re-export coverage types for kernel inspection
pub use brief::{
    collect_summaries, CoverageInterval, FileSummary, FileType, ObjectSummary, TimeKind,
};

#[cfg(target_endian = "big")]
pub const NATIVE_ENDIAN: Endian = Endian::Big;

#[cfg(target_endian = "little")]
pub const NATIVE_ENDIAN: Endian = Endian::Little;

// DAF file record byte offsets
const DAF_OFFSET_TYPE: u64 = 4; // File type character (S=SPK, C=CK, P=BPCK)
const DAF_OFFSET_ND: u64 = 8; // Number of double components per summary
const DAF_OFFSET_NI: u64 = 12; // Number of integer components per summary
const DAF_OFFSET_LOCIFN: u64 = 16; // Internal filename (60 chars)
const DAF_OFFSET_FWARD: u64 = 76; // Forward pointer to first summary record
const DAF_OFFSET_BWARD: u64 = 80; // Backward pointer to last summary record
const DAF_OFFSET_FREE: u64 = 84; // First free address
const DAF_OFFSET_LOCFMT: u64 = 88; // Endianness indicator
const DAF_OFFSET_FTPSTR: u64 = 699; // FTP validation string

// DAF summary field offsets (relative to summary pointer)
const SUMMARY_DC0: u64 = 0; // First double (initial_epoch / initial_sclk)
const SUMMARY_DC1: u64 = 8; // Second double (final_epoch / final_sclk)
const SUMMARY_IC0: u64 = 16; // First integer (target_code / instrument_code / frame_id)
const SUMMARY_IC1: u64 = 20; // Second integer (center_code / frame_code / base_frame)
const SUMMARY_IC2: u64 = 24; // Third integer (frame_code / ck_type / bpck_type)
const SUMMARY_IC3: u64 = 28; // Fourth integer (spk_type / rates / data_start for BPCK)
const SUMMARY_IC4: u64 = 32; // Fifth integer (data_start / data_start / data_end for BPCK)
const SUMMARY_IC5: u64 = 36; // Sixth integer (data_end / data_end)

/// Low-level binary reader for DAF files.
///
/// Wraps a `File` handle and `Endian` setting, providing type-safe reads
/// at arbitrary byte offsets with correct endian conversion.
#[derive(Debug)]
struct DafReader {
    file: File,
    endian: Endian,
}

impl DafReader {
    fn new(file: File) -> Result<Self> {
        let endian = Self::detect_endian(&file)?;
        Ok(Self { file, endian })
    }

    fn detect_endian(f: &File) -> Result<Endian> {
        let endian_char = Self::read_char_at(f, DAF_OFFSET_LOCFMT)?;
        match endian_char {
            'B' | 'b' => Ok(Endian::Big),
            'L' | 'l' => Ok(Endian::Little),
            _ => Err(Error::InvalidEndian { found: endian_char }),
        }
    }

    fn read_char_at(f: &File, offset: u64) -> Result<char> {
        let mut f = f;
        f.seek(SeekFrom::Start(offset))?;
        let mut buf: [u8; 1] = [0];
        f.read_exact(&mut buf)?;
        if buf[0].is_ascii() {
            Ok(buf[0] as char)
        } else {
            Err(Error::InvalidHeader(format!(
                "non-ASCII byte 0x{:02X} at offset {}",
                buf[0], offset
            )))
        }
    }

    fn read_f64(&self, offset: u64) -> Result<f64> {
        let mut f = &self.file;
        f.seek(SeekFrom::Start(offset))?;
        let mut buf: [u8; 8] = [0; 8];
        f.read_exact(&mut buf)?;
        match self.endian {
            Endian::Little => Ok(f64::from_le_bytes(buf)),
            Endian::Big => Ok(f64::from_be_bytes(buf)),
        }
    }

    fn read_f64vec(&self, daf_addr1: u64, daf_addr2: u64) -> Result<Vec<f64>> {
        // DAF addresses are 1-indexed double-word (8-byte) indices
        // daf_addr2 is inclusive, so we have (daf_addr2 - daf_addr1 + 1) elements
        let num_elements = (daf_addr2 - daf_addr1 + 1) as usize;
        let byte_offset = (daf_addr1 - 1) * 8;
        let num_bytes = num_elements * 8;

        // Bulk read: single seek + single read for all elements
        let mut f = &self.file;
        f.seek(SeekFrom::Start(byte_offset))?;
        let mut buf = vec![0u8; num_bytes];
        f.read_exact(&mut buf)?;

        let mut vectr = Vec::with_capacity(num_elements);
        let convert: fn([u8; 8]) -> f64 = match self.endian {
            Endian::Little => f64::from_le_bytes,
            Endian::Big => f64::from_be_bytes,
        };
        for chunk in buf.chunks_exact(8) {
            let bytes: [u8; 8] = chunk.try_into().unwrap();
            vectr.push(convert(bytes));
        }
        Ok(vectr)
    }

    fn read_char(&self, offset: u64) -> Result<char> {
        Self::read_char_at(&self.file, offset)
    }

    fn read_i32(&self, offset: u64) -> Result<i32> {
        let mut f = &self.file;
        f.seek(SeekFrom::Start(offset))?;
        let mut buf: [u8; 4] = [0; 4];
        f.read_exact(&mut buf)?;
        match self.endian {
            Endian::Little => Ok(i32::from_le_bytes(buf)),
            Endian::Big => Ok(i32::from_be_bytes(buf)),
        }
    }

    fn read_string(&self, offset: u64, maxlen: u64) -> Result<String> {
        let mut reader = std::io::BufReader::new(&self.file);
        reader.seek(SeekFrom::Start(offset))?;
        let mut byte = reader.bytes();
        let mut string_out = String::with_capacity(maxlen as usize);

        for _ in 0..maxlen {
            let b = byte.next();
            match b {
                None => break,
                Some(Err(error)) => return Err(Error::IO(error)),
                Some(Ok(0u8)) => continue,
                Some(Ok(4u8)) => break,
                Some(Ok(c)) => {
                    if c.is_ascii() {
                        string_out.push(c as char);
                    }
                }
            };
        }
        Ok(string_out.trim().to_string())
    }
}

use serde::{Deserialize, Serialize};

/// Byte order for multi-byte values in DAF files.
///
/// DAF files store an endianness indicator at byte offset 88, which this
/// library uses to correctly interpret all numeric values in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Endian {
    /// Big-endian byte order (most significant byte first)
    Big,
    /// Little-endian byte order (least significant byte first)
    Little,
}

impl Endian {

    /// Returns the LOCFMT string for this endianness ("LTL-IEEE" or "BIG-IEEE")
    pub fn locfmt(&self) -> &'static str {
        match self {
            Endian::Little => "LTL-IEEE",
            Endian::Big => "BIG-IEEE",
        }
    }
}

/// Metadata from a DAF file header needed for round-trip reconstruction.
///
/// This structure captures all the information from the 1024-byte DAF file record
/// that is necessary to reconstruct a byte-identical SPK file from exported data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DAFMetadata {
    /// Number of double precision components in each array summary (ND).
    /// For SPK files this is 2 (initial and final epoch).
    pub nd: u64,
    /// Number of integer components in each array summary (NI).
    /// For SPK files this is 6 (target, center, frame, type, data_start, data_end).
    pub ni: u64,
    /// File endianness, determined from the LOCFMT field at offset 88.
    pub endian: Endian,
    /// Record number of initial summary record (FWARD).
    pub fward: u64,
    /// Record number of final summary record (BWARD).
    pub bward: u64,
    /// First free address in the file (FREE).
    pub free_address: u64,
    /// FTP validation string at offset 699.
    pub ftpstr: String,
}

/*

# DAF File Record Structure

From: [https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/req/daf.html#The%20File%20Record]

The file record is always the first physical record in a DAF. The record size is 1024 bytes (for platforms with one byte char size, and four bytes integer size). The items listed in the File Record:

1. LOCIDW (8 characters, 8 bytes): An identification word (`DAF/xxxx').
  The 'xxxx' substring is a string of four characters or less indicating the type of data stored in the DAF file. This is used by the SPICELIB subroutines to verify that a particular file is in fact a DAF and not merely a direct access file with the same record length. When a DAF is opened, an error signals if this keyword is not present. [Address 0]
2. ND ( 1 integer, 4 bytes): The number of double precision components in each array summary. [Address 8]
3. NI ( 1 integer, 4 bytes): The number of integer components in each array summary. [Address 12]
4. LOCIFN (60 characters, 60 bytes): The internal name or description of the array file. [Address 16]
5. FWARD ( 1 integer, 4 bytes): The record number of the initial summary record in the file. [Address 76]
6. BWARD ( 1 integer, 4 bytes): The record number of the final summary record in the file. [Address 80]
7. FREE ( 1 integer, 4 bytes): The first free address in the file. This is the address at which the first element of the next array to be added to the file will be stored. [Address 84]
8. LOCFMT ( 8 characters, 8 bytes): The character string that indicates the numeric binary format of the DAF. The string has value either "LTL-IEEE" or "BIG-IEEE." [Address 88]
9. PRENUL ( 603 characters, 603 bytes): A block of nulls to pad between the last character of LOCFMT and the first character of FTPSTR to keep FTPSTR at character 700 (address 699) in a 1024 byte record. [Address 96]
10. FTPSTR ( 28 characters, 28 bytes): The FTP validation string.
  This string is assembled using components returned from the SPICELIB private routine ZZFTPSTR. [Address 699]
11. PSTNUL ( 297 characters, 297 bytes): A block of nulls to pad from the last character of FTPSTR to the end of the file record. Note: this value enforces the length of the file record as 1024 bytes. [Address 727]

*/

type SegReader = fn(&mut DAFFile, u64) -> Result<DAFSegment>;

/// Parser for NAIF Double Precision Array Files (DAF).
///
/// `DAFFile` reads the binary structure of SPK, CK, and BPCK files, providing
/// iteration over segments. It handles endian detection automatically.
///
/// # Example
///
/// ```no_run
/// use muad_dib::DAFFile;
/// use std::fs::File;
///
/// let file = File::open("de430.bsp").unwrap();
/// let mut daf = DAFFile::from_file(file).unwrap();
///
/// // Get file header info
/// let header = daf.daf_header().unwrap();
/// println!("File: {}", header.name);
///
/// // Iterate segments
/// for segment in daf {
///     println!("{:?}", segment);
/// }
/// ```
#[derive(Debug)]
pub struct DAFFile {
    reader: DafReader,
    pub endian: Endian,
    daf_type: char,
    seg_reader: SegReader,
    nd: u64,
    ni: u64,
    locifn: String,
    fward: u64,
    bward: u64,
    free_address: u64,
    pub ftpstr: String,
    current_record: u64,
    namerec_offset: u64,
    next_record: u64,
    sum_size: u64,
    nc: u64,
    current_segment: u64,
    nsum: u64,
}

impl DAFFile {
    pub fn from_file(file: File) -> Result<DAFFile> {
        let reader = DafReader::new(file)?;
        let endian = reader.endian;
        let daf_type = reader.read_char(DAF_OFFSET_TYPE)?;
        let nd = reader.read_i32(DAF_OFFSET_ND)? as u64;
        let ni = reader.read_i32(DAF_OFFSET_NI)? as u64;
        let locifn = reader.read_string(DAF_OFFSET_LOCIFN, 60)?;
        let fward = reader.read_i32(DAF_OFFSET_FWARD)? as u64;
        let bward = reader.read_i32(DAF_OFFSET_BWARD)? as u64;
        let free_address = reader.read_i32(DAF_OFFSET_FREE)? as u64;
        let ftpstr = reader.read_string(DAF_OFFSET_FTPSTR, 28)?;
        let current_record = fward;

        let namerec_offset = 1024 * (bward - fward + 1);
        // DAF packs NI integers into ceil(NI/2) doubles, so summary size is (ND + ceil(NI/2)) * 8
        let sum_size = 8 * (nd + ni.div_ceil(2));
        let nc = 8 * (nd + ni.div_ceil(2));

        let next_record = reader.read_f64(1024 * (current_record - 1))? as u64;
        let nsum = reader.read_f64(1024 * (current_record - 1) + 16)? as u64;

        let current_segment: u64 = 0;

        let seg_reader = match daf_type {
            'S' => SPKSegment::reader,
            'C' => CKSegment::reader,
            'P' => BPCKSegment::reader,
            _ => {
                return Err(Error::UnsupportedType { daf_type });
            }
        };

        Ok(DAFFile {
            reader,
            endian,
            daf_type,
            seg_reader,
            nd,
            ni,
            locifn,
            fward,
            bward,
            free_address,
            ftpstr,
            current_record,
            namerec_offset,
            next_record,
            sum_size,
            nc,
            current_segment,
            nsum,
        })
    }

    pub fn read_f64(&mut self, offset: u64) -> Result<f64> {
        self.reader.read_f64(offset)
    }

    pub fn read_f64vec(&mut self, offset1: u64, offset2: u64) -> Result<Vec<f64>> {
        self.reader.read_f64vec(offset1, offset2)
    }

    pub fn read_char(&mut self, offset: u64) -> Result<char> {
        self.reader.read_char(offset)
    }

    pub fn read_i32(&mut self, offset: u64) -> Result<i32> {
        self.reader.read_i32(offset)
    }

    pub fn read_string(&mut self, offset: u64, maxlen: u64) -> Result<String> {
        self.reader.read_string(offset, maxlen)
    }

    pub fn comment(&mut self) -> Result<String> {
        if self.fward > 2 {
            let offset: u64 = 1024; // DAF comments start at record 2 (address 1024)
            let maxlen: u64 = 1024 * (self.fward - 1); // DAF comments end at the summary record
            let comment = self.read_string(offset, maxlen)?;

            return Ok(comment);
        }

        // if summaries start at record 2 there are no comments;
        Ok("".to_string())
    }

    pub fn daf_header(&mut self) -> Result<DAFHeader> {
        Ok(DAFHeader {
            name: self.locifn.clone(),
            comment: self.comment()?,
            kind: match self.daf_type {
                'S' => "SPK".to_string(),
                'C' => "CK".to_string(),
                'P' => "BPCK".to_string(),
                _ => "unknown".to_string(),
            },
        })
    }

    /// Returns metadata needed for round-trip DAF reconstruction.
    pub fn daf_metadata(&self) -> DAFMetadata {
        DAFMetadata {
            nd: self.nd,
            ni: self.ni,
            endian: self.endian,
            fward: self.fward,
            bward: self.bward,
            free_address: self.free_address,
            ftpstr: self.ftpstr.clone(),
        }
    }

    pub fn segment_reader(&mut self, offset: u64) -> Result<DAFSegment> {
        (self.seg_reader)(self, offset)
    }

    pub fn current_ptr(&mut self) -> u64 {
        1024 * (self.current_record - 1) + 24 + self.current_segment * self.sum_size
    }

    fn advance_record(&mut self) -> Option<Result<u64>> {
        if self.next_record == 0 {
            None
        } else {
            let offset = (self.next_record - 1) * 1024;
            let new_next = match self.read_f64(offset) {
                Ok(s) => s as u64,
                Err(e) => return Some(Err(e)),
            };
            let new_nsum = match self.read_f64(offset + 16) {
                Ok(s) => s as u64,
                Err(e) => return Some(Err(e)),
            };
            self.current_record = self.next_record;
            self.next_record = new_next;
            self.nsum = new_nsum;
            self.current_segment = 0;
            Some(Ok(self.current_ptr()))
        }
    }

    fn advance_segment(&mut self) -> Option<Result<u64>> {
        if self.current_segment < self.nsum {
            let ptr = self.current_ptr();
            self.current_segment += 1;
            Some(Ok(ptr))
        } else {
            self.advance_record()
        }
    }
}

impl Iterator for DAFFile {
    type Item = Result<DAFSegment>;
    //change to return DAFSegment ...

    fn next(&mut self) -> Option<Self::Item> {
        match self.advance_segment() {
            Some(Ok(s)) => Some(self.segment_reader(s)),
            Some(Err(e)) => Some(Err(e)),
            None => None,
        }
    }
}

// Placeholder structs for different SPK segment types.
// These are parsed but not yet used for data access.
#[allow(dead_code)]
#[derive(Debug)]
pub struct SPK1 {
    epochs: Vec<f64>,
    records: Vec<Vec<f64>>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct SPK2 {
    init_epoch: f64,
    tstep: f64,
    midpoints: Vec<f64>,
    radii: Vec<f64>,
    rx_coefficients: Vec<Vec<f64>>,
    ry_coefficients: Vec<Vec<f64>>,
    rz_coefficients: Vec<Vec<f64>>,
    degree: u32,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct SPK3 {
    init_epoch: f64,
    tstep: f64,
    midpoints: Vec<f64>,
    radii: Vec<f64>,
    rx_coefficients: Vec<Vec<f64>>,
    ry_coefficients: Vec<Vec<f64>>,
    rz_coefficients: Vec<Vec<f64>>,
    vx_coefficients: Vec<Vec<f64>>,
    vy_coefficients: Vec<Vec<f64>>,
    vz_coefficients: Vec<Vec<f64>>,
    degree: u32,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct SPK5 {
    gm: f64,
    epochs: Vec<f64>,
    states: Vec<Vec<f64>>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct SPK8 {
    init_epoch: f64,
    tstep: f64,
    rx_coefficients: Vec<Vec<f64>>,
    ry_coefficients: Vec<Vec<f64>>,
    rz_coefficients: Vec<Vec<f64>>,
    vx_coefficients: Vec<Vec<f64>>,
    vy_coefficients: Vec<Vec<f64>>,
    vz_coefficients: Vec<Vec<f64>>,
    degree: u32,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct SPK9 {
    epochs: Vec<f64>,
    rx_coefficients: Vec<Vec<f64>>,
    ry_coefficients: Vec<Vec<f64>>,
    rz_coefficients: Vec<Vec<f64>>,
    vx_coefficients: Vec<Vec<f64>>,
    vy_coefficients: Vec<Vec<f64>>,
    vz_coefficients: Vec<Vec<f64>>,
    degree: u32,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct SPK10 {
    data: Vec<f64>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct SPK12 {
    data: Vec<f64>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct SPK13 {
    data: Vec<f64>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct SPK14 {
    data: Vec<f64>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct SPK15 {
    data: Vec<f64>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct SPK17 {
    data: Vec<f64>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct SPK18 {
    data: Vec<f64>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct SPK19 {
    data: Vec<f64>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct SPK20 {
    data: Vec<f64>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct SPK21 {
    data: Vec<f64>,
}

/// SPK (Spacecraft and Planet Kernel) ephemeris segment.
///
/// Contains position and velocity data for a target body relative to a center body
/// in a specified reference frame. Epochs are in TDB seconds past J2000.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SPKSegment {
    /// Segment descriptor name (up to 40 characters)
    pub name: String,
    /// Start of coverage interval (TDB seconds past J2000)
    pub initial_epoch: f64,
    /// End of coverage interval (TDB seconds past J2000)
    pub final_epoch: f64,
    /// NAIF ID of the target body
    pub target_code: NaifId,
    /// NAIF ID of the center body (origin for position vectors)
    pub center_code: NaifId,
    /// NAIF ID of the reference frame
    pub frame_code: NaifId,
    /// SPK segment type (1-21, determines data format)
    pub spk_type: i32,
    /// DAF address where segment data begins (1-indexed double-word)
    pub data_start: u64,
    /// DAF address where segment data ends (1-indexed double-word, inclusive)
    pub data_end: u64,
    /// Raw coefficient/state data (interpretation depends on spk_type)
    pub data: Vec<f64>,
}
// impl to give SPKSegment from file and pointer to Summary rec

impl SPKSegment {
    fn reader(daf: &mut DAFFile, sumptr: u64) -> Result<DAFSegment> {
        let nameptr = sumptr + daf.namerec_offset;
        let data_start = daf.read_i32(sumptr + SUMMARY_IC4)? as u64;
        let data_end = daf.read_i32(sumptr + SUMMARY_IC5)? as u64;
        Ok(DAFSegment::SPK(SPKSegment {
            name: daf.read_string(nameptr, daf.nc)?,
            initial_epoch: daf.read_f64(sumptr + SUMMARY_DC0)?,
            final_epoch: daf.read_f64(sumptr + SUMMARY_DC1)?,
            target_code: NaifId(daf.read_i32(sumptr + SUMMARY_IC0)?),
            center_code: NaifId(daf.read_i32(sumptr + SUMMARY_IC1)?),
            frame_code: NaifId(daf.read_i32(sumptr + SUMMARY_IC2)?),
            spk_type: daf.read_i32(sumptr + SUMMARY_IC3)?,
            data_start,
            data_end,
            data: daf.read_f64vec(data_start, data_end)?,
        }))
    }
}

/// CK (C-Kernel) pointing/orientation segment.
///
/// Contains attitude data (quaternions) for a spacecraft instrument or structure
/// relative to a reference frame. Times are in spacecraft clock (SCLK) ticks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CKSegment {
    /// Segment descriptor name (up to 40 characters)
    pub name: String,
    /// Start of coverage interval (encoded SCLK ticks)
    pub initial_sclk: f64,
    /// End of coverage interval (encoded SCLK ticks)
    pub final_sclk: f64,
    /// NAIF ID of the instrument or structure
    pub instrument_code: NaifId,
    /// NAIF ID of the reference frame
    pub frame_code: NaifId,
    /// CK segment type (1-6, determines data format)
    pub ck_type: i32,
    /// Whether angular velocity data is included
    pub rates: bool,
    /// DAF address where segment data begins (1-indexed double-word)
    pub data_start: u64,
    /// DAF address where segment data ends (1-indexed double-word, inclusive)
    pub data_end: u64,
    /// Raw quaternion/rate data (interpretation depends on ck_type)
    pub data: Vec<f64>,
}

impl CKSegment {
    fn reader(daf: &mut DAFFile, sumptr: u64) -> Result<DAFSegment> {
        let nameptr = sumptr + daf.namerec_offset;
        let data_start = daf.read_i32(sumptr + SUMMARY_IC4)? as u64;
        let data_end = daf.read_i32(sumptr + SUMMARY_IC5)? as u64;
        Ok(DAFSegment::CK(CKSegment {
            name: daf.read_string(nameptr, daf.nc)?,
            initial_sclk: daf.read_f64(sumptr + SUMMARY_DC0)?,
            final_sclk: daf.read_f64(sumptr + SUMMARY_DC1)?,
            instrument_code: NaifId(daf.read_i32(sumptr + SUMMARY_IC0)?),
            frame_code: NaifId(daf.read_i32(sumptr + SUMMARY_IC1)?),
            ck_type: daf.read_i32(sumptr + SUMMARY_IC2)?,
            rates: (daf.read_i32(sumptr + SUMMARY_IC3)? == 1),
            data_start,
            data_end,
            data: daf.read_f64vec(data_start, data_end)?,
        }))
    }
}

/// BPCK (Binary PCK) planetary constants segment.
///
/// Contains orientation data for natural bodies (planets, moons) as Euler angles
/// or nutation/precession coefficients. Epochs are in TDB seconds past J2000.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BPCKSegment {
    /// Segment descriptor name (up to 40 characters)
    pub name: String,
    /// Start of coverage interval (TDB seconds past J2000)
    pub initial_epoch: f64,
    /// End of coverage interval (TDB seconds past J2000)
    pub final_epoch: f64,
    /// NAIF frame ID for the body-fixed frame being defined
    pub frame_id: NaifId,
    /// NAIF frame ID of the base/inertial reference frame
    pub base_frame: NaifId,
    /// BPCK segment type (determines data format)
    pub bpck_type: i32,
    /// DAF address where segment data begins (1-indexed double-word)
    pub data_start: u64,
    /// DAF address where segment data ends (1-indexed double-word, inclusive)
    pub data_end: u64,
    /// Raw orientation data (interpretation depends on bpck_type)
    pub data: Vec<f64>,
}

impl BPCKSegment {
    fn reader(daf: &mut DAFFile, sumptr: u64) -> Result<DAFSegment> {
        let nameptr = sumptr + daf.namerec_offset;
        let data_start = daf.read_i32(sumptr + SUMMARY_IC3)? as u64;
        let data_end = daf.read_i32(sumptr + SUMMARY_IC4)? as u64;
        Ok(DAFSegment::BPCK(BPCKSegment {
            name: daf.read_string(nameptr, daf.nc)?,
            initial_epoch: daf.read_f64(sumptr + SUMMARY_DC0)?,
            final_epoch: daf.read_f64(sumptr + SUMMARY_DC1)?,
            frame_id: NaifId(daf.read_i32(sumptr + SUMMARY_IC0)?),
            base_frame: NaifId(daf.read_i32(sumptr + SUMMARY_IC1)?),
            bpck_type: daf.read_i32(sumptr + SUMMARY_IC2)?,
            data_start,
            data_end,
            data: daf.read_f64vec(data_start, data_end)?,
        }))
    }
}

/// A segment from a DAF binary file, wrapping the specific segment type.
///
/// The variant indicates the type of DAF file the segment came from:
/// - `SPK`: Spacecraft/planetary ephemeris (position/velocity)
/// - `CK`: C-kernel pointing data (orientation/attitude)
/// - `BPCK`: Binary PCK (planetary orientation constants)
///
/// Note: Text PCK files are NOT DAF files and use the separate `PCKSource` type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DAFSegment {
    /// Ephemeris segment from an SPK file
    SPK(SPKSegment),
    /// Pointing segment from a CK file
    CK(CKSegment),
    /// Planetary constants segment from a binary PCK file
    BPCK(BPCKSegment),
}

/// Header information from a DAF file.
///
/// Contains human-readable metadata about the file contents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DAFHeader {
    /// Internal file name (LOCIFN, 60 characters max)
    pub name: String,
    /// Comment area contents (records 2 through FWARD-1)
    pub comment: String,
    /// File type: "SPK", "CK", or "BPCK"
    pub kind: String,
}

// TODO: add asserts to verify file data

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Seek, Write};
    use tempfile::tempfile;

    fn make_reader(file: File, endian: Endian) -> DafReader {
        DafReader { file, endian }
    }

    /// Test f64 reading with little endian
    #[test]
    fn test_read_f64_little_endian() {
        let mut file = tempfile().expect("Could not create temp file");
        let value: f64 = 123.456;
        file.write_all(&value.to_le_bytes())
            .expect("Could not write");
        file.seek(SeekFrom::Start(0)).expect("Could not seek");

        let reader = make_reader(file, Endian::Little);
        let result = reader.read_f64(0).expect("Failed to read f64");
        assert!((result - value).abs() < 1e-10, "Little endian f64 mismatch");
    }

    /// Test f64 reading with big endian
    #[test]
    fn test_read_f64_big_endian() {
        let mut file = tempfile().expect("Could not create temp file");
        let value: f64 = 789.012;
        file.write_all(&value.to_be_bytes())
            .expect("Could not write");
        file.seek(SeekFrom::Start(0)).expect("Could not seek");

        let reader = make_reader(file, Endian::Big);
        let result = reader.read_f64(0).expect("Failed to read f64");
        assert!((result - value).abs() < 1e-10, "Big endian f64 mismatch");
    }

    /// Test i32 reading with both endians
    #[test]
    fn test_read_i32_conversion() {
        let mut file = tempfile().expect("Could not create temp file");
        let value: i32 = 42;
        file.write_all(&value.to_le_bytes())
            .expect("Could not write");
        file.seek(SeekFrom::Start(0)).expect("Could not seek");

        let reader = make_reader(file, Endian::Little);
        let result = reader.read_i32(0).expect("Failed to read i32");
        assert_eq!(result, value, "Little endian i32 mismatch");

        // Test big endian
        let mut file2 = tempfile().expect("Could not create temp file");
        file2
            .write_all(&value.to_be_bytes())
            .expect("Could not write");
        file2.seek(SeekFrom::Start(0)).expect("Could not seek");

        let reader2 = make_reader(file2, Endian::Big);
        let result2 = reader2.read_i32(0).expect("Failed to read i32");
        assert_eq!(result2, value, "Big endian i32 mismatch");
    }

    /// Test string parsing with null padding
    #[test]
    fn test_string_parsing() {
        let mut file = tempfile().expect("Could not create temp file");
        // Write "HELLO" with null padding
        let data = b"HELLO\0\0\0\0\0";
        file.write_all(data).expect("Could not write");
        file.seek(SeekFrom::Start(0)).expect("Could not seek");

        let reader = make_reader(file, Endian::Little);
        let result = reader.read_string(0, 10).expect("Failed to read string");
        assert_eq!(result, "HELLO", "String parsing mismatch");
    }

    /// Test Endian locfmt method
    #[test]
    fn test_endian_locfmt() {
        assert_eq!(Endian::Little.locfmt(), "LTL-IEEE");
        assert_eq!(Endian::Big.locfmt(), "BIG-IEEE");
    }
}
