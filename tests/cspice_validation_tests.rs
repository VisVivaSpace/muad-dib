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

/// Get the absolute path to a test file
fn test_file_path(filename: &str) -> String {
    std::env::current_dir()
        .expect("Could not get current directory")
        .join(filename)
        .to_string_lossy()
        .into_owned()
}

/// Get the absolute path to the default test BSP file
fn default_test_file_path() -> String {
    test_file_path("test_data/test.bsp")
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
                dafus_c(sum.as_ptr(), nd, ni, dc.as_mut_ptr(), ic.as_mut_ptr());

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
    let test_file = default_test_file_path();

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
    let test_file = default_test_file_path();

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
    let test_file = default_test_file_path();

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
    let test_file = default_test_file_path();

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

    for (i, (despice_seg, cspice_sum)) in despice_segments
        .iter()
        .zip(cspice_segments.iter())
        .enumerate()
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
    let test_file = default_test_file_path();

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
    let test_file = default_test_file_path();

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

// ============================================================================
// DE440s (SPK Type 2) Validation Tests
// ============================================================================

/// Validate de440s.bsp file record fields match between despice and CSPICE.
#[test]
fn validate_de440s_file_record() {
    let _lock = CSPICE_LOCK.lock().unwrap();
    let path = test_file_path("test_data/de440s.bsp");

    let file = File::open(&path).expect("Could not open de440s.bsp");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");
    let meta = daf.daf_metadata();

    let cspice_daf = CspiceDAF::open(&path).expect("CSPICE failed to open de440s.bsp");
    let fr = cspice_daf.read_file_record();

    assert_eq!(meta.nd as i32, fr.nd, "ND mismatch");
    assert_eq!(meta.ni as i32, fr.ni, "NI mismatch");
    assert_eq!(meta.fward as i32, fr.fward, "FWARD mismatch");
    assert_eq!(meta.bward as i32, fr.bward, "BWARD mismatch");
    assert_eq!(meta.free_address as i32, fr.free, "FREE mismatch");
}

/// Validate de440s.bsp segment summaries match between despice and CSPICE.
#[test]
fn validate_de440s_segment_summaries() {
    let _lock = CSPICE_LOCK.lock().unwrap();
    let path = test_file_path("test_data/de440s.bsp");

    let file = File::open(&path).expect("Could not open de440s.bsp");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");
    let meta = daf.daf_metadata();
    let despice_segments: Vec<_> = daf.filter_map(|s| s.ok()).collect();

    let cspice_daf = CspiceDAF::open(&path).expect("CSPICE failed to open de440s.bsp");
    let cspice_segments = cspice_daf.get_segments(meta.nd as i32, meta.ni as i32);

    assert_eq!(
        despice_segments.len(),
        cspice_segments.len(),
        "de440s segment count mismatch"
    );

    for (i, (despice_seg, cspice_sum)) in despice_segments
        .iter()
        .zip(cspice_segments.iter())
        .enumerate()
    {
        let spk = match despice_seg {
            DAFSegment::SPK(s) => s,
            _ => panic!("de440s segment {} is not SPK", i),
        };

        assert!(
            (spk.initial_epoch - cspice_sum.dc[0]).abs() < 1e-10,
            "de440s segment {}: initial_epoch mismatch",
            i
        );
        assert!(
            (spk.final_epoch - cspice_sum.dc[1]).abs() < 1e-10,
            "de440s segment {}: final_epoch mismatch",
            i
        );
        assert_eq!(
            spk.target_code, cspice_sum.ic[0],
            "de440s segment {}: target_code",
            i
        );
        assert_eq!(
            spk.center_code, cspice_sum.ic[1],
            "de440s segment {}: center_code",
            i
        );
        assert_eq!(
            spk.frame_code, cspice_sum.ic[2],
            "de440s segment {}: frame_code",
            i
        );
        assert_eq!(
            spk.spk_type, cspice_sum.ic[3],
            "de440s segment {}: spk_type",
            i
        );
        assert_eq!(
            spk.data_start as i32, cspice_sum.ic[4],
            "de440s segment {}: data_start",
            i
        );
        assert_eq!(
            spk.data_end as i32, cspice_sum.ic[5],
            "de440s segment {}: data_end",
            i
        );
    }
}

/// Validate de440s.bsp segment data arrays match between despice and CSPICE.
/// Spot-checks first and last 100 values per segment to keep test fast.
#[test]
fn validate_de440s_segment_data() {
    let _lock = CSPICE_LOCK.lock().unwrap();
    let path = test_file_path("test_data/de440s.bsp");

    let file = File::open(&path).expect("Could not open de440s.bsp");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");
    let despice_segments: Vec<_> = daf.filter_map(|s| s.ok()).collect();

    let cspice_daf = CspiceDAF::open(&path).expect("CSPICE failed to open de440s.bsp");

    for (i, despice_seg) in despice_segments.iter().enumerate() {
        let spk = match despice_seg {
            DAFSegment::SPK(s) => s,
            _ => panic!("de440s segment {} is not SPK", i),
        };

        let cspice_data = cspice_daf.read_data(spk.data_start as i32, spk.data_end as i32);

        assert_eq!(
            spk.data.len(),
            cspice_data.len(),
            "de440s segment {}: data length mismatch",
            i
        );

        // Spot-check first 100 and last 100 values
        let check_count = 100.min(spk.data.len());
        for j in 0..check_count {
            assert!(
                (spk.data[j] - cspice_data[j]).abs() < 1e-15,
                "de440s segment {} data[{}] mismatch: despice={}, cspice={}",
                i,
                j,
                spk.data[j],
                cspice_data[j]
            );
        }
        let start = spk.data.len().saturating_sub(check_count);
        for j in start..spk.data.len() {
            assert!(
                (spk.data[j] - cspice_data[j]).abs() < 1e-15,
                "de440s segment {} data[{}] (tail) mismatch: despice={}, cspice={}",
                i,
                j,
                spk.data[j],
                cspice_data[j]
            );
        }
    }
}

// ============================================================================
// CK Validation Tests
// ============================================================================

/// Validate test.bc CK file record matches between despice and CSPICE.
#[test]
fn validate_ck_file_record() {
    let _lock = CSPICE_LOCK.lock().unwrap();
    let path = test_file_path("test_data/test.bc");

    let file = File::open(&path).expect("Could not open test.bc");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");
    let meta = daf.daf_metadata();

    let cspice_daf = CspiceDAF::open(&path).expect("CSPICE failed to open test.bc");
    let fr = cspice_daf.read_file_record();

    assert_eq!(meta.nd as i32, fr.nd, "CK ND mismatch");
    assert_eq!(meta.ni as i32, fr.ni, "CK NI mismatch");
    assert_eq!(meta.fward as i32, fr.fward, "CK FWARD mismatch");
    assert_eq!(meta.bward as i32, fr.bward, "CK BWARD mismatch");
    assert_eq!(meta.free_address as i32, fr.free, "CK FREE mismatch");
}

/// Validate test.bc CK segment summaries match between despice and CSPICE.
#[test]
fn validate_ck_segment_summaries() {
    let _lock = CSPICE_LOCK.lock().unwrap();
    let path = test_file_path("test_data/test.bc");

    let file = File::open(&path).expect("Could not open test.bc");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");
    let meta = daf.daf_metadata();
    let despice_segments: Vec<_> = daf.filter_map(|s| s.ok()).collect();

    let cspice_daf = CspiceDAF::open(&path).expect("CSPICE failed to open test.bc");
    let cspice_segments = cspice_daf.get_segments(meta.nd as i32, meta.ni as i32);

    assert_eq!(
        despice_segments.len(),
        cspice_segments.len(),
        "CK segment count mismatch"
    );

    for (i, (despice_seg, cspice_sum)) in despice_segments
        .iter()
        .zip(cspice_segments.iter())
        .enumerate()
    {
        let ck = match despice_seg {
            DAFSegment::CK(c) => c,
            _ => panic!("test.bc segment {} is not CK", i),
        };

        // CK summary: dc[0]=initial_sclk, dc[1]=final_sclk
        assert!(
            (ck.initial_sclk - cspice_sum.dc[0]).abs() < 1e-10,
            "CK segment {}: initial_sclk mismatch",
            i
        );
        assert!(
            (ck.final_sclk - cspice_sum.dc[1]).abs() < 1e-10,
            "CK segment {}: final_sclk mismatch",
            i
        );

        // CK summary: ic[0]=instrument, ic[1]=frame, ic[2]=ck_type, ic[3]=rates, ic[4]=start, ic[5]=end
        assert_eq!(
            ck.instrument_code, cspice_sum.ic[0],
            "CK segment {}: instrument_code",
            i
        );
        assert_eq!(
            ck.frame_code, cspice_sum.ic[1],
            "CK segment {}: frame_code",
            i
        );
        assert_eq!(ck.ck_type, cspice_sum.ic[2], "CK segment {}: ck_type", i);
        assert_eq!(
            ck.rates as i32, cspice_sum.ic[3],
            "CK segment {}: rates flag",
            i
        );
        assert_eq!(
            ck.data_start as i32, cspice_sum.ic[4],
            "CK segment {}: data_start",
            i
        );
        assert_eq!(
            ck.data_end as i32, cspice_sum.ic[5],
            "CK segment {}: data_end",
            i
        );
    }
}

/// Validate test.bc CK data arrays match between despice and CSPICE.
#[test]
fn validate_ck_segment_data() {
    let _lock = CSPICE_LOCK.lock().unwrap();
    let path = test_file_path("test_data/test.bc");

    let file = File::open(&path).expect("Could not open test.bc");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");
    let despice_segments: Vec<_> = daf.filter_map(|s| s.ok()).collect();

    let cspice_daf = CspiceDAF::open(&path).expect("CSPICE failed to open test.bc");

    for (i, despice_seg) in despice_segments.iter().enumerate() {
        let ck = match despice_seg {
            DAFSegment::CK(c) => c,
            _ => panic!("test.bc segment {} is not CK", i),
        };

        let cspice_data = cspice_daf.read_data(ck.data_start as i32, ck.data_end as i32);

        assert_eq!(
            ck.data.len(),
            cspice_data.len(),
            "CK segment {}: data length mismatch",
            i
        );

        for (j, (d, c)) in ck.data.iter().zip(cspice_data.iter()).enumerate() {
            assert!(
                (d - c).abs() < 1e-15,
                "CK segment {} data[{}] mismatch: despice={}, cspice={}",
                i,
                j,
                d,
                c
            );
        }
    }
}

// ============================================================================
// BPC Validation Tests
// ============================================================================

/// Validate earth_latest_high_prec.bpc file record matches between despice and CSPICE.
#[test]
fn validate_bpc_file_record() {
    let _lock = CSPICE_LOCK.lock().unwrap();
    let path = test_file_path("test_data/earth_latest_high_prec.bpc");

    let file = File::open(&path).expect("Could not open BPC file");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");
    let meta = daf.daf_metadata();

    let cspice_daf = CspiceDAF::open(&path).expect("CSPICE failed to open BPC file");
    let fr = cspice_daf.read_file_record();

    assert_eq!(meta.nd as i32, fr.nd, "BPC ND mismatch");
    assert_eq!(meta.ni as i32, fr.ni, "BPC NI mismatch");
    assert_eq!(meta.fward as i32, fr.fward, "BPC FWARD mismatch");
    assert_eq!(meta.bward as i32, fr.bward, "BPC BWARD mismatch");
    assert_eq!(meta.free_address as i32, fr.free, "BPC FREE mismatch");
}

/// Validate earth_latest_high_prec.bpc segment summaries match between despice and CSPICE.
#[test]
fn validate_bpc_segment_summaries() {
    let _lock = CSPICE_LOCK.lock().unwrap();
    let path = test_file_path("test_data/earth_latest_high_prec.bpc");

    let file = File::open(&path).expect("Could not open BPC file");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");
    let meta = daf.daf_metadata();
    let despice_segments: Vec<_> = daf.filter_map(|s| s.ok()).collect();

    let cspice_daf = CspiceDAF::open(&path).expect("CSPICE failed to open BPC file");
    let cspice_segments = cspice_daf.get_segments(meta.nd as i32, meta.ni as i32);

    assert_eq!(
        despice_segments.len(),
        cspice_segments.len(),
        "BPC segment count mismatch"
    );

    for (i, (despice_seg, cspice_sum)) in despice_segments
        .iter()
        .zip(cspice_segments.iter())
        .enumerate()
    {
        let bpck = match despice_seg {
            DAFSegment::BPCK(b) => b,
            _ => panic!("BPC segment {} is not BPCK", i),
        };

        // BPC summary: dc[0]=initial_epoch, dc[1]=final_epoch
        assert!(
            (bpck.initial_epoch - cspice_sum.dc[0]).abs() < 1e-10,
            "BPC segment {}: initial_epoch mismatch",
            i
        );
        assert!(
            (bpck.final_epoch - cspice_sum.dc[1]).abs() < 1e-10,
            "BPC segment {}: final_epoch mismatch",
            i
        );

        // BPC summary: ic[0]=frame_id, ic[1]=base_frame, ic[2]=bpck_type, ic[3]=start, ic[4]=end
        assert_eq!(
            bpck.frame_id, cspice_sum.ic[0],
            "BPC segment {}: frame_id",
            i
        );
        assert_eq!(
            bpck.base_frame, cspice_sum.ic[1],
            "BPC segment {}: base_frame",
            i
        );
        assert_eq!(
            bpck.bpck_type, cspice_sum.ic[2],
            "BPC segment {}: bpck_type",
            i
        );
        assert_eq!(
            bpck.data_start as i32, cspice_sum.ic[3],
            "BPC segment {}: data_start",
            i
        );
        assert_eq!(
            bpck.data_end as i32, cspice_sum.ic[4],
            "BPC segment {}: data_end",
            i
        );
    }
}

// ============================================================================
// Hermite SPK (Type 13) Validation Tests
// ============================================================================

/// Validate gmat-hermite.bsp data arrays match between despice and CSPICE.
#[test]
fn validate_hermite_spk_data() {
    let _lock = CSPICE_LOCK.lock().unwrap();
    let path = test_file_path("test_data/gmat-hermite.bsp");

    let file = File::open(&path).expect("Could not open gmat-hermite.bsp");
    let daf = DAFFile::from_file(file).expect("Failed to parse DAF file");
    let despice_segments: Vec<_> = daf.filter_map(|s| s.ok()).collect();

    let cspice_daf = CspiceDAF::open(&path).expect("CSPICE failed to open gmat-hermite.bsp");

    for (i, despice_seg) in despice_segments.iter().enumerate() {
        let spk = match despice_seg {
            DAFSegment::SPK(s) => s,
            _ => panic!("gmat-hermite segment {} is not SPK", i),
        };

        assert_eq!(spk.spk_type, 13, "Expected SPK Type 13 for gmat-hermite");

        let cspice_data = cspice_daf.read_data(spk.data_start as i32, spk.data_end as i32);

        assert_eq!(
            spk.data.len(),
            cspice_data.len(),
            "Hermite segment {}: data length mismatch",
            i
        );

        for (j, (d, c)) in spk.data.iter().zip(cspice_data.iter()).enumerate() {
            assert!(
                (d - c).abs() < 1e-15,
                "Hermite segment {} data[{}] mismatch: despice={}, cspice={}",
                i,
                j,
                d,
                c
            );
        }
    }
}
