//! Validation tests comparing despice's DAF parsing against CSPICE.
//!
//! These tests ensure despice correctly parses DAF files by comparing results
//! with the official CSPICE library implementation.
//!
//! CSPICE is linked as a static library during test builds only.
//! Set CSPICE_LIB environment variable to point to your CSPICE lib directory.
//!
//! Note: CSPICE uses global state, so tests in this file must run sequentially.
//! The test harness is invoked with --test-threads=1 via the test wrapper.

#![cfg(all(feature = "cspice", feature = "test-data"))]

use muad_dib::{DAFFile, DAFSegment};
use std::ffi::{CStr, CString};
use std::fs::File;
use std::sync::Mutex;

// Mutex to ensure CSPICE calls are serialized across tests
static CSPICE_LOCK: Mutex<()> = Mutex::new(());

/// Get the absolute path to the test file
fn test_file_path() -> String {
    std::env::current_dir()
        .expect("Could not get current directory")
        .join("test_data/test.bsp")
        .to_string_lossy()
        .into_owned()
}

// ============================================================================
// CSPICE FFI Bindings
// ============================================================================

#[link(name = "cspice")]
extern "C" {
    /// Open a DAF for read access
    fn dafopr_c(fname: *const libc::c_char, handle: *mut libc::c_int);

    /// Close a DAF
    fn dafcls_c(handle: libc::c_int);

    /// Read DAF file record
    fn dafrfr_c(
        handle: libc::c_int,
        lenout: libc::c_int,
        nd: *mut libc::c_int,
        ni: *mut libc::c_int,
        ifname: *mut libc::c_char,
        fward: *mut libc::c_int,
        bward: *mut libc::c_int,
        free: *mut libc::c_int,
    );

    /// Begin forward search for arrays
    fn dafbfs_c(handle: libc::c_int);

    /// Find next array in forward search
    fn daffna_c(found: *mut libc::c_int);

    /// Get array summary for current array
    fn dafgs_c(sum: *mut libc::c_double);

    /// Unpack an array summary
    fn dafus_c(
        sum: *const libc::c_double,
        nd: libc::c_int,
        ni: libc::c_int,
        dc: *mut libc::c_double,
        ic: *mut libc::c_int,
    );

    /// Read elements from the data area of a DAF
    fn dafgda_c(
        handle: libc::c_int,
        begin: libc::c_int,
        end: libc::c_int,
        data: *mut libc::c_double,
    );

    /// Extract comments from a DAF
    fn dafec_c(
        handle: libc::c_int,
        bufsiz: libc::c_int,
        lenout: libc::c_int,
        n: *mut libc::c_int,
        buffer: *mut libc::c_char,
        done: *mut libc::c_int,
    );

    /// Reset the CSPICE error status
    fn reset_c();

    /// Check if an error has occurred
    fn failed_c() -> libc::c_int;
}

// ============================================================================
// Safe Rust Wrappers
// ============================================================================

/// RAII wrapper for a CSPICE DAF handle
struct CspiceDAF {
    handle: libc::c_int,
}

impl CspiceDAF {
    /// Open a DAF file for reading
    fn open(path: &str) -> Result<Self, String> {
        let c_path = CString::new(path).map_err(|e| format!("Invalid path: {}", e))?;
        let mut handle: libc::c_int = 0;

        unsafe {
            reset_c();
            dafopr_c(c_path.as_ptr(), &mut handle);

            if failed_c() != 0 {
                reset_c();
                return Err(format!("CSPICE failed to open: {}", path));
            }
        }

        Ok(CspiceDAF { handle })
    }

    /// Read the file record (ND, NI, IFNAME, FWARD, BWARD, FREE)
    fn read_file_record(&self) -> FileRecord {
        let mut nd: libc::c_int = 0;
        let mut ni: libc::c_int = 0;
        let mut ifname = [0i8; 61]; // LOCIFN is 60 chars + null
        let mut fward: libc::c_int = 0;
        let mut bward: libc::c_int = 0;
        let mut free: libc::c_int = 0;

        unsafe {
            dafrfr_c(
                self.handle,
                61, // lenout: size of ifname buffer
                &mut nd,
                &mut ni,
                ifname.as_mut_ptr(),
                &mut fward,
                &mut bward,
                &mut free,
            );
        }

        let internal_name = unsafe { CStr::from_ptr(ifname.as_ptr()) }
            .to_string_lossy()
            .into_owned();

        FileRecord {
            nd,
            ni,
            internal_name,
            fward,
            bward,
            free,
        }
    }

    /// Iterate over segments and collect summaries
    fn get_segments(&self, nd: i32, ni: i32) -> Vec<SegmentSummary> {
        let mut segments = Vec::new();
        let sum_size = nd + (ni + 1) / 2; // Size of summary in doubles

        unsafe {
            dafbfs_c(self.handle);

            loop {
                let mut found: libc::c_int = 0;
                daffna_c(&mut found);

                if found == 0 {
                    break;
                }

                // Get packed summary
                let mut sum = vec![0.0f64; sum_size as usize];
                dafgs_c(sum.as_mut_ptr());

                // Unpack summary
                let mut dc = vec![0.0f64; nd as usize];
                let mut ic = vec![0i32; ni as usize];
                dafus_c(
                    sum.as_ptr(),
                    nd,
                    ni,
                    dc.as_mut_ptr(),
                    ic.as_mut_ptr(),
                );

                segments.push(SegmentSummary { dc, ic });
            }
        }

        segments
    }

    /// Read data array between given DAF addresses (1-indexed, inclusive)
    fn read_data(&self, begin: i32, end: i32) -> Vec<f64> {
        let count = (end - begin + 1) as usize;
        let mut data = vec![0.0f64; count];

        unsafe {
            dafgda_c(self.handle, begin, end, data.as_mut_ptr());
        }

        data
    }

    /// Extract comments from the DAF
    fn extract_comments(&self) -> String {
        const BUF_SIZE: usize = 1024;
        const LINE_LEN: usize = 256;
        let mut buffer = vec![0i8; BUF_SIZE * LINE_LEN];
        let mut n: libc::c_int = 0;
        let mut done: libc::c_int = 0;
        let mut all_comments = String::new();

        unsafe {
            while done == 0 {
                dafec_c(
                    self.handle,
                    BUF_SIZE as libc::c_int,
                    LINE_LEN as libc::c_int,
                    &mut n,
                    buffer.as_mut_ptr(),
                    &mut done,
                );

                // Extract each line
                for i in 0..n as usize {
                    let line_ptr = buffer.as_ptr().add(i * LINE_LEN);
                    if let Ok(line) = CStr::from_ptr(line_ptr).to_str() {
                        if !all_comments.is_empty() && !line.is_empty() {
                            all_comments.push('\n');
                        }
                        all_comments.push_str(line.trim_end());
                    }
                }
            }
        }

        all_comments
    }
}

impl Drop for CspiceDAF {
    fn drop(&mut self) {
        unsafe {
            dafcls_c(self.handle);
            // Reset error state after closing
            reset_c();
        }
    }
}

/// File record data from CSPICE
struct FileRecord {
    nd: i32,
    ni: i32,
    internal_name: String,
    fward: i32,
    bward: i32,
    free: i32,
}

/// Unpacked segment summary
struct SegmentSummary {
    dc: Vec<f64>, // Double precision components
    ic: Vec<i32>, // Integer components
}

// ============================================================================
// Validation Tests
// ============================================================================

/// Validate file record fields match between despice and CSPICE.
#[test]
fn validate_file_record_matches_cspice() {
    let _lock = CSPICE_LOCK.lock().unwrap();
    let test_file = test_file_path();

    // Load with despice
    let file = File::open(&test_file).expect("Could not open test file");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");
    let despice_meta = daf.daf_metadata();

    // Load with CSPICE
    let cspice_daf = CspiceDAF::open(&test_file).expect("CSPICE failed to open file");
    let cspice_fr = cspice_daf.read_file_record();

    // Compare ND (number of double precision components)
    assert_eq!(
        despice_meta.nd as i32, cspice_fr.nd,
        "ND mismatch: despice={}, cspice={}",
        despice_meta.nd, cspice_fr.nd
    );

    // Compare NI (number of integer components)
    assert_eq!(
        despice_meta.ni as i32, cspice_fr.ni,
        "NI mismatch: despice={}, cspice={}",
        despice_meta.ni, cspice_fr.ni
    );

    // Compare FWARD (forward pointer to first summary record)
    assert_eq!(
        despice_meta.fward as i32, cspice_fr.fward,
        "FWARD mismatch: despice={}, cspice={}",
        despice_meta.fward, cspice_fr.fward
    );

    // Compare BWARD (backward pointer to last summary record)
    assert_eq!(
        despice_meta.bward as i32, cspice_fr.bward,
        "BWARD mismatch: despice={}, cspice={}",
        despice_meta.bward, cspice_fr.bward
    );

    // Compare FREE (first free address)
    assert_eq!(
        despice_meta.free_address as i32, cspice_fr.free,
        "FREE mismatch: despice={}, cspice={}",
        despice_meta.free_address, cspice_fr.free
    );
}

/// Validate internal name (LOCIFN) matches between despice and CSPICE.
#[test]
fn validate_internal_name_matches_cspice() {
    let _lock = CSPICE_LOCK.lock().unwrap();
    let test_file = test_file_path();

    // Load with despice
    let file = File::open(&test_file).expect("Could not open test file");
    let mut daf = DAFFile::from_file(file).expect("Failed to parse DAF file");
    let despice_header = daf.daf_header().expect("Failed to get header");

    // Load with CSPICE
    let cspice_daf = CspiceDAF::open(&test_file).expect("CSPICE failed to open file");
    let cspice_fr = cspice_daf.read_file_record();

    // Compare internal name (trimmed)
    assert_eq!(
        despice_header.name.trim(),
        cspice_fr.internal_name.trim(),
        "Internal name mismatch: despice='{}', cspice='{}'",
        despice_header.name,
        cspice_fr.internal_name
    );
}

/// Validate segment count matches between despice and CSPICE.
#[test]
fn validate_segment_count_matches_cspice() {
    let _lock = CSPICE_LOCK.lock().unwrap();
    let test_file = test_file_path();

    // Load with despice
    let file = File::open(&test_file).expect("Could not open test file");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");
    let despice_meta = daf.daf_metadata();
    let despice_segments: Vec<_> = daf.collect();
    let despice_count = despice_segments.len();

    // Load with CSPICE
    let cspice_daf = CspiceDAF::open(&test_file).expect("CSPICE failed to open file");
    let cspice_segments = cspice_daf.get_segments(despice_meta.nd as i32, despice_meta.ni as i32);
    let cspice_count = cspice_segments.len();

    assert_eq!(
        despice_count, cspice_count,
        "Segment count mismatch: despice={}, cspice={}",
        despice_count, cspice_count
    );
}

/// Validate segment summary fields match between despice and CSPICE.
#[test]
fn validate_segment_summaries_match_cspice() {
    let _lock = CSPICE_LOCK.lock().unwrap();
    let test_file = test_file_path();

    // Load with despice
    let file = File::open(&test_file).expect("Could not open test file");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");
    let despice_meta = daf.daf_metadata();
    let despice_segments: Vec<_> = daf.filter_map(|s| s.ok()).collect();

    // Load with CSPICE
    let cspice_daf = CspiceDAF::open(&test_file).expect("CSPICE failed to open file");
    let cspice_segments = cspice_daf.get_segments(despice_meta.nd as i32, despice_meta.ni as i32);

    assert_eq!(
        despice_segments.len(),
        cspice_segments.len(),
        "Segment count mismatch before comparison"
    );

    for (i, (despice_seg, cspice_sum)) in
        despice_segments.iter().zip(cspice_segments.iter()).enumerate()
    {
        let spk = match despice_seg {
            DAFSegment::SPK(s) => s,
            _ => panic!("Segment {} is not SPK type", i),
        };

        // Compare double precision components (epochs)
        // dc[0] = initial epoch, dc[1] = final epoch
        assert!(
            (spk.initial_epoch - cspice_sum.dc[0]).abs() < 1e-10,
            "Segment {}: initial_epoch mismatch: despice={}, cspice={}",
            i,
            spk.initial_epoch,
            cspice_sum.dc[0]
        );
        assert!(
            (spk.final_epoch - cspice_sum.dc[1]).abs() < 1e-10,
            "Segment {}: final_epoch mismatch: despice={}, cspice={}",
            i,
            spk.final_epoch,
            cspice_sum.dc[1]
        );

        // Compare integer components
        // ic[0] = target_id, ic[1] = center_id, ic[2] = frame_id,
        // ic[3] = spk_type, ic[4] = data_start, ic[5] = data_end
        assert_eq!(
            spk.target_code, cspice_sum.ic[0],
            "Segment {}: target_code mismatch: despice={}, cspice={}",
            i, spk.target_code, cspice_sum.ic[0]
        );
        assert_eq!(
            spk.center_code, cspice_sum.ic[1],
            "Segment {}: center_code mismatch: despice={}, cspice={}",
            i, spk.center_code, cspice_sum.ic[1]
        );
        assert_eq!(
            spk.frame_code, cspice_sum.ic[2],
            "Segment {}: frame_code mismatch: despice={}, cspice={}",
            i, spk.frame_code, cspice_sum.ic[2]
        );
        assert_eq!(
            spk.spk_type, cspice_sum.ic[3],
            "Segment {}: spk_type mismatch: despice={}, cspice={}",
            i, spk.spk_type, cspice_sum.ic[3]
        );
        assert_eq!(
            spk.data_start as i32, cspice_sum.ic[4],
            "Segment {}: data_start mismatch: despice={}, cspice={}",
            i, spk.data_start, cspice_sum.ic[4]
        );
        assert_eq!(
            spk.data_end as i32, cspice_sum.ic[5],
            "Segment {}: data_end mismatch: despice={}, cspice={}",
            i, spk.data_end, cspice_sum.ic[5]
        );
    }
}

/// Validate segment data arrays match between despice and CSPICE.
#[test]
fn validate_segment_data_matches_cspice() {
    let _lock = CSPICE_LOCK.lock().unwrap();
    let test_file = test_file_path();

    // Load with despice
    let file = File::open(&test_file).expect("Could not open test file");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");
    let despice_segments: Vec<_> = daf.filter_map(|s| s.ok()).collect();

    // Load with CSPICE
    let cspice_daf = CspiceDAF::open(&test_file).expect("CSPICE failed to open file");

    for (i, despice_seg) in despice_segments.iter().enumerate() {
        let spk = match despice_seg {
            DAFSegment::SPK(s) => s,
            _ => panic!("Segment {} is not SPK type", i),
        };

        // Read data from CSPICE
        let cspice_data = cspice_daf.read_data(spk.data_start as i32, spk.data_end as i32);

        assert_eq!(
            spk.data.len(),
            cspice_data.len(),
            "Segment {}: data length mismatch: despice={}, cspice={}",
            i,
            spk.data.len(),
            cspice_data.len()
        );

        // Compare data values
        for (j, (d, c)) in spk.data.iter().zip(cspice_data.iter()).enumerate() {
            assert!(
                (d - c).abs() < 1e-15,
                "Segment {} data[{}] mismatch: despice={}, cspice={}",
                i,
                j,
                d,
                c
            );
        }
    }
}

/// Validate comments match between despice and CSPICE.
#[test]
fn validate_comments_match_cspice() {
    let _lock = CSPICE_LOCK.lock().unwrap();
    let test_file = test_file_path();

    // Load with despice
    let file = File::open(&test_file).expect("Could not open test file");
    let mut daf = DAFFile::from_file(file).expect("Failed to parse DAF file");
    let despice_comment = daf.comment().expect("Failed to get comment");

    // Load with CSPICE
    let cspice_daf = CspiceDAF::open(&test_file).expect("CSPICE failed to open file");
    let cspice_comment = cspice_daf.extract_comments();

    // Normalize whitespace for comparison
    let despice_normalized: String = despice_comment
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let cspice_normalized: String = cspice_comment
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    assert_eq!(
        despice_normalized, cspice_normalized,
        "Comment mismatch:\ndespice: '{}'\ncspice: '{}'",
        despice_comment, cspice_comment
    );
}
