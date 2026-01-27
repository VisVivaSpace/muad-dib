//! Common CSPICE FFI bindings and helper utilities for validation tests.
//!
//! This module provides:
//! - FFI bindings for CSPICE functions
//! - RAII wrapper for kernel loading/unloading
//! - Helper functions for test data paths
//!
//! CSPICE is linked as a static library during test builds only.
//! Set CSPICE_LIB environment variable to point to your CSPICE lib directory.

#![cfg(feature = "cspice")]

use std::ffi::{c_char, c_double, c_int, CStr, CString};
use std::sync::Mutex;

// Mutex to ensure CSPICE calls are serialized across tests (CSPICE uses global state)
pub static CSPICE_LOCK: Mutex<()> = Mutex::new(());

// ============================================================================
// CSPICE FFI Bindings
// ============================================================================

#[link(name = "cspice")]
extern "C" {
    // Error handling
    pub fn reset_c();
    pub fn failed_c() -> c_int;

    // Kernel management
    pub fn furnsh_c(file: *const c_char);
    pub fn unload_c(file: *const c_char);
    pub fn kclear_c();

    // Time functions
    pub fn str2et_c(str: *const c_char, et: *mut c_double);
    pub fn utc2et_c(utcstr: *const c_char, et: *mut c_double);
    pub fn et2utc_c(
        et: c_double,
        format: *const c_char,
        prec: c_int,
        lenout: c_int,
        utcstr: *mut c_char,
    );

    // Coordinate conversions
    pub fn reclat_c(
        rectan: *const c_double,
        radius: *mut c_double,
        lon: *mut c_double,
        lat: *mut c_double,
    );
    pub fn latrec_c(radius: c_double, lon: c_double, lat: c_double, rectan: *mut c_double);
    pub fn recsph_c(
        rectan: *const c_double,
        r: *mut c_double,
        colat: *mut c_double,
        lon: *mut c_double,
    );
    pub fn sphrec_c(r: c_double, colat: c_double, lon: c_double, rectan: *mut c_double);
    pub fn reccyl_c(
        rectan: *const c_double,
        r: *mut c_double,
        lon: *mut c_double,
        z: *mut c_double,
    );
    pub fn cylrec_c(r: c_double, lon: c_double, z: c_double, rectan: *mut c_double);

    // Kernel pool
    pub fn gdpool_c(
        name: *const c_char,
        start: c_int,
        room: c_int,
        n: *mut c_int,
        values: *mut c_double,
        found: *mut c_int,
    );
    pub fn gipool_c(
        name: *const c_char,
        start: c_int,
        room: c_int,
        n: *mut c_int,
        ivals: *mut c_int,
        found: *mut c_int,
    );
    pub fn dtpool_c(name: *const c_char, found: *mut c_int, n: *mut c_int, type_: *mut c_char);

    // SPK
    pub fn spkgeo_c(
        targ: c_int,
        et: c_double,
        ref_: *const c_char,
        obs: c_int,
        state: *mut c_double,
        lt: *mut c_double,
    );

    // CK
    pub fn ckgp_c(
        inst: c_int,
        sclkdp: c_double,
        tol: c_double,
        ref_: *const c_char,
        cmat: *mut c_double,
        clkout: *mut c_double,
        found: *mut c_int,
    );
    pub fn m2q_c(r: *const c_double, q: *mut c_double);
}

// ============================================================================
// Safe Rust Wrappers
// ============================================================================

/// RAII wrapper for CSPICE kernel management.
///
/// Automatically loads kernels on construction and unloads them on drop.
/// Uses CSPICE's furnsh/unload functions.
pub struct CspiceKernels {
    files: Vec<CString>,
}

impl CspiceKernels {
    /// Create a new empty kernel manager.
    pub fn new() -> Self {
        unsafe {
            reset_c();
            kclear_c();
            reset_c();
        }
        CspiceKernels { files: Vec::new() }
    }

    /// Load a kernel file into CSPICE.
    pub fn load(&mut self, path: &str) {
        let c_path = CString::new(path).expect("Invalid kernel path");
        unsafe {
            reset_c();
            furnsh_c(c_path.as_ptr());
            if failed_c() != 0 {
                reset_c();
                panic!("CSPICE failed to load kernel: {}", path);
            }
        }
        self.files.push(c_path);
    }

    /// Load multiple kernel files.
    pub fn load_all(&mut self, paths: &[&str]) {
        for path in paths {
            self.load(path);
        }
    }
}

impl Default for CspiceKernels {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for CspiceKernels {
    fn drop(&mut self) {
        unsafe {
            // Unload all kernels in reverse order
            for c_path in self.files.iter().rev() {
                unload_c(c_path.as_ptr());
            }
            // Clear any error state
            reset_c();
            // Clear all kernel pool variables
            kclear_c();
            reset_c();
        }
    }
}

// ============================================================================
// Safe Wrappers for Individual CSPICE Functions
// ============================================================================

/// Convert time string to ET using str2et_c.
pub fn cspice_str2et(time_str: &str) -> f64 {
    let c_str = CString::new(time_str).expect("Invalid time string");
    let mut et: f64 = 0.0;
    unsafe {
        reset_c();
        str2et_c(c_str.as_ptr(), &mut et);
        if failed_c() != 0 {
            reset_c();
            panic!("CSPICE str2et_c failed for: {}", time_str);
        }
    }
    et
}

/// Convert UTC string to ET using utc2et_c.
pub fn cspice_utc2et(utc_str: &str) -> f64 {
    let c_str = CString::new(utc_str).expect("Invalid UTC string");
    let mut et: f64 = 0.0;
    unsafe {
        reset_c();
        utc2et_c(c_str.as_ptr(), &mut et);
        if failed_c() != 0 {
            reset_c();
            panic!("CSPICE utc2et_c failed for: {}", utc_str);
        }
    }
    et
}

/// Convert ET to UTC string using et2utc_c.
pub fn cspice_et2utc(et: f64, format: &str, precision: i32) -> String {
    let c_format = CString::new(format).expect("Invalid format string");
    let mut buffer = vec![0i8; 64];
    unsafe {
        reset_c();
        et2utc_c(et, c_format.as_ptr(), precision, 64, buffer.as_mut_ptr());
        if failed_c() != 0 {
            reset_c();
            panic!("CSPICE et2utc_c failed for et={}", et);
        }
        CStr::from_ptr(buffer.as_ptr())
            .to_string_lossy()
            .into_owned()
    }
}

/// Convert rectangular to latitudinal coordinates using reclat_c.
pub fn cspice_reclat(rectan: &[f64; 3]) -> (f64, f64, f64) {
    let mut radius: f64 = 0.0;
    let mut lon: f64 = 0.0;
    let mut lat: f64 = 0.0;
    unsafe {
        reclat_c(rectan.as_ptr(), &mut radius, &mut lon, &mut lat);
    }
    (radius, lon, lat)
}

/// Convert latitudinal to rectangular coordinates using latrec_c.
pub fn cspice_latrec(radius: f64, lon: f64, lat: f64) -> [f64; 3] {
    let mut rectan = [0.0f64; 3];
    unsafe {
        latrec_c(radius, lon, lat, rectan.as_mut_ptr());
    }
    rectan
}

/// Convert rectangular to spherical coordinates using recsph_c.
pub fn cspice_recsph(rectan: &[f64; 3]) -> (f64, f64, f64) {
    let mut r: f64 = 0.0;
    let mut colat: f64 = 0.0;
    let mut lon: f64 = 0.0;
    unsafe {
        recsph_c(rectan.as_ptr(), &mut r, &mut colat, &mut lon);
    }
    (r, colat, lon)
}

/// Convert spherical to rectangular coordinates using sphrec_c.
pub fn cspice_sphrec(r: f64, colat: f64, lon: f64) -> [f64; 3] {
    let mut rectan = [0.0f64; 3];
    unsafe {
        sphrec_c(r, colat, lon, rectan.as_mut_ptr());
    }
    rectan
}

/// Convert rectangular to cylindrical coordinates using reccyl_c.
pub fn cspice_reccyl(rectan: &[f64; 3]) -> (f64, f64, f64) {
    let mut r: f64 = 0.0;
    let mut lon: f64 = 0.0;
    let mut z: f64 = 0.0;
    unsafe {
        reccyl_c(rectan.as_ptr(), &mut r, &mut lon, &mut z);
    }
    (r, lon, z)
}

/// Convert cylindrical to rectangular coordinates using cylrec_c.
pub fn cspice_cylrec(r: f64, lon: f64, z: f64) -> [f64; 3] {
    let mut rectan = [0.0f64; 3];
    unsafe {
        cylrec_c(r, lon, z, rectan.as_mut_ptr());
    }
    rectan
}

/// Get double values from kernel pool using gdpool_c.
pub fn cspice_gdpool(name: &str) -> Option<Vec<f64>> {
    let c_name = CString::new(name).expect("Invalid variable name");
    let mut n: c_int = 0;
    let mut found: c_int = 0;
    let mut values = vec![0.0f64; 100]; // Max 100 values

    unsafe {
        reset_c();
        gdpool_c(
            c_name.as_ptr(),
            0,
            100,
            &mut n,
            values.as_mut_ptr(),
            &mut found,
        );
        reset_c();
    }

    if found != 0 {
        values.truncate(n as usize);
        Some(values)
    } else {
        None
    }
}

/// Get integer values from kernel pool using gipool_c.
pub fn cspice_gipool(name: &str) -> Option<Vec<i32>> {
    let c_name = CString::new(name).expect("Invalid variable name");
    let mut n: c_int = 0;
    let mut found: c_int = 0;
    let mut values = vec![0i32; 100]; // Max 100 values

    unsafe {
        reset_c();
        gipool_c(
            c_name.as_ptr(),
            0,
            100,
            &mut n,
            values.as_mut_ptr(),
            &mut found,
        );
        reset_c();
    }

    if found != 0 {
        values.truncate(n as usize);
        Some(values)
    } else {
        None
    }
}

/// Check if variable exists and get its count using dtpool_c.
pub fn cspice_dtpool(name: &str) -> Option<(usize, char)> {
    let c_name = CString::new(name).expect("Invalid variable name");
    let mut found: c_int = 0;
    let mut n: c_int = 0;
    let mut type_char: c_char = 0;

    unsafe {
        reset_c();
        dtpool_c(c_name.as_ptr(), &mut found, &mut n, &mut type_char);
        reset_c();
    }

    if found != 0 {
        Some((n as usize, type_char as u8 as char))
    } else {
        None
    }
}

/// Get SPK state using spkgeo_c.
pub fn cspice_spkgeo(target: i32, et: f64, frame: &str, observer: i32) -> ([f64; 6], f64) {
    let c_frame = CString::new(frame).expect("Invalid frame name");
    let mut state = [0.0f64; 6];
    let mut lt: f64 = 0.0;

    unsafe {
        reset_c();
        spkgeo_c(
            target,
            et,
            c_frame.as_ptr(),
            observer,
            state.as_mut_ptr(),
            &mut lt,
        );
        if failed_c() != 0 {
            reset_c();
            panic!(
                "CSPICE spkgeo_c failed for target={}, et={}, frame={}, observer={}",
                target, et, frame, observer
            );
        }
    }

    (state, lt)
}

/// Get CK pointing using ckgp_c, returns quaternion.
pub fn cspice_ckgp(inst: i32, sclk: f64, tol: f64, frame: &str) -> Option<([f64; 4], f64)> {
    let c_frame = CString::new(frame).expect("Invalid frame name");
    let mut cmat = [0.0f64; 9]; // 3x3 rotation matrix
    let mut clkout: f64 = 0.0;
    let mut found: c_int = 0;

    unsafe {
        reset_c();
        ckgp_c(
            inst,
            sclk,
            tol,
            c_frame.as_ptr(),
            cmat.as_mut_ptr(),
            &mut clkout,
            &mut found,
        );
        if failed_c() != 0 {
            reset_c();
            return None;
        }
    }

    if found == 0 {
        return None;
    }

    // Convert rotation matrix to quaternion using m2q_c
    let mut quat = [0.0f64; 4];
    unsafe {
        m2q_c(cmat.as_ptr(), quat.as_mut_ptr());
    }

    Some((quat, clkout))
}

// ============================================================================
// Test File Path Helpers
// ============================================================================

/// Get absolute path to a test file in the repo root.
pub fn test_file_path(filename: &str) -> String {
    std::env::current_dir()
        .expect("Could not get current directory")
        .join(filename)
        .to_string_lossy()
        .into_owned()
}

/// Get path to naif0012.tls
pub fn lsk_path() -> String {
    test_file_path("test_data/naif0012.tls")
}

/// Get path to test.bsp
pub fn spk_path() -> String {
    test_file_path("test_data/test.bsp")
}

/// Get path to test.bc
pub fn ck_path() -> String {
    test_file_path("test_data/test.bc")
}

/// Get path to test.tpc
pub fn tpc_path() -> String {
    test_file_path("test_data/test.tpc")
}

/// Get path to gmat-hermite.bsp (Type 13)
pub fn hermite_spk_path() -> String {
    test_file_path("test_data/gmat-hermite.bsp")
}

/// Get path to de440s.bsp (Type 2 Chebyshev - JPL DE440 planetary ephemeris)
pub fn de440s_spk_path() -> String {
    test_file_path("test_data/de440s.bsp")
}

// ============================================================================
// Frame Helpers
// ============================================================================

/// Convert NAIF frame code to CSPICE frame name string.
pub fn frame_name(frame_code: i32) -> &'static str {
    match frame_code {
        1 => "J2000",
        17 => "ECLIPJ2000",
        _ => "J2000", // fallback to J2000 for unknown frames
    }
}

// ============================================================================
// Assertion Helpers
// ============================================================================

/// Assert two f64 values are close within tolerance.
pub fn assert_close(a: f64, b: f64, tolerance: f64, msg: &str) {
    let diff = (a - b).abs();
    assert!(
        diff < tolerance,
        "{}: {} != {} (diff={}, tolerance={})",
        msg,
        a,
        b,
        diff,
        tolerance
    );
}

/// Assert two 3D vectors are close within tolerance.
pub fn assert_vector_close(a: &[f64; 3], b: &[f64; 3], tolerance: f64, msg: &str) {
    for i in 0..3 {
        assert_close(a[i], b[i], tolerance, &format!("{}[{}]", msg, i));
    }
}

/// Assert two quaternions are close (accounting for q == -q equivalence).
pub fn assert_quaternion_close(a: &[f64; 4], b: &[f64; 4], tolerance: f64, msg: &str) {
    // Quaternions q and -q represent the same rotation
    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let sign = if dot >= 0.0 { 1.0 } else { -1.0 };

    for i in 0..4 {
        let diff = (a[i] - sign * b[i]).abs();
        assert!(
            diff < tolerance,
            "{}[{}]: {} != {} (diff={}, tolerance={})",
            msg,
            i,
            a[i],
            sign * b[i],
            diff,
            tolerance
        );
    }
}
