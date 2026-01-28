# Test Suite

This directory contains validation tests for muad-dib DAF parsing and interpolation.

## Test Modes

### 1. Default Mode (no features)

Safe for crates.io - runs tests that don't require external data files.

```bash
cargo test
```

Runs ~177 tests including:
- Unit tests for all modules
- Error handling tests
- Doc tests

### 2. Test Data Mode

Requires `test_data/` directory with SPICE kernel files (use Git LFS).

```bash
cargo test --features test-data
```

Runs ~206 tests including everything from default mode plus:
- **integration_tests.rs** - DAF file parsing validation
- **anise_validation_tests.rs** - Cross-validation with anise crate
- **format_tests.rs** - Round-trip format conversion tests
- **round_trip_tests.rs** - SPK → HDF5 → SPK preservation tests

### 3. CSPICE Validation Mode

Requires CSPICE library AND test data files.

```bash
export CSPICE_LIB=/path/to/cspice/lib
cargo test --features cspice,test-data -- --test-threads=1
```

Adds CSPICE validation tests:
- **cspice_spk_tests.rs** - SPK ephemeris interpolation vs CSPICE
- **cspice_ck_tests.rs** - CK pointing interpolation vs CSPICE
- **cspice_coord_tests.rs** - Coordinate transformations vs CSPICE
- **cspice_time_tests.rs** - Time conversions (ET/TDB) vs CSPICE
- **cspice_pool_tests.rs** - Kernel pool operations vs CSPICE
- **cspice_validation_tests.rs** - General validation vs CSPICE
- **cspice_common.rs** - Shared test utilities (mutex, FFI wrappers)

**Important:** CSPICE tests MUST use `--test-threads=1` (CSPICE is not thread-safe).

## Quick Reference

| Command | Tests | Requirements |
|---------|-------|--------------|
| `cargo test` | ~177 | None |
| `cargo test --features test-data` | ~206 | test_data/ (Git LFS) |
| `cargo test --features cspice,test-data -- --test-threads=1` | ~220+ | test_data/ + CSPICE |

## Test Data Files

Located in `test_data/` (tracked via Git LFS):

### SPK Files (Spacecraft/Planetary Ephemeris)
| File | Description |
|------|-------------|
| `test.bsp` | Primary test SPK (Type 9 Lagrange) |
| `gmat-hermite.bsp` | GMAT-generated Hermite interpolation |
| `gmat-hermite-big-endian.bsp` | Big-endian Hermite SPK |
| `gmat-lagrange.bsp` | GMAT-generated Lagrange interpolation |
| `variable-seg-size-hermite.bsp` | Variable segment size Hermite |
| `rename-test.bsp` | Segment renaming test file |

### BPC Files (Binary Planetary Constants)
| File | Description |
|------|-------------|
| `earth_latest_high_prec.bpc` | High-precision Earth orientation |
| `earth_longterm_000101_251211_250915.bpc` | Long-term Earth orientation |
| `earth_2025_250826_2125_predict.bpc` | Predicted Earth orientation |
| `moon_pa_de440_200625.bpc` | Moon principal axes (DE440) |

### Other Files
| File | Description |
|------|-------------|
| `test.bc` | CK pointing data |
| `test.tpc` | Text PCK planetary constants |
| `test_pck.hdf5` | Pre-converted PCK in HDF5 |

## CSPICE Setup

### 1. Download CSPICE

Download from NAIF: https://naif.jpl.nasa.gov/naif/toolkit_C.html

Select your platform (e.g., PC/Linux/GCC, MacIntel, Mac/M1):
```bash
# Example for macOS ARM
curl -O https://naif.jpl.nasa.gov/pub/naif/toolkit/C/MacM1_OSX_clang_64bit/packages/cspice.tar.Z
tar xzf cspice.tar.Z
```

### 2. Build CSPICE

```bash
cd cspice
./makeall.csh    # Unix/macOS
# or
call makeall.bat  # Windows
```

### 3. Set Environment Variable

```bash
export CSPICE_LIB=/path/to/cspice/lib
```

Add to your shell profile for persistence.

## Tolerance Reference

Comparison tolerances used in CSPICE validation tests:

| Quantity | Tolerance | Precision | File |
|----------|-----------|-----------|------|
| Position (km) | 1e-9 | ~1 micrometer | cspice_spk_tests.rs |
| Velocity (km/s) | 1e-12 | ~1 nm/s | cspice_spk_tests.rs |
| Quaternion | 1e-8 | ~0.00001 degrees | cspice_ck_tests.rs |
| ET/TDB (s) | 1e-9 | ~1 nanosecond | cspice_time_tests.rs |

These tolerances account for minor differences in interpolation algorithms between implementations.

## Architecture Notes

### Test Isolation

The library has **no runtime dependency** on CSPICE or anise:

- `anise` is a dev-dependency only
- `cspice` tests are behind `#![cfg(feature = "cspice")]`
- Neither appears in `src/`, binaries, or examples

This means users can use muad-dib without installing CSPICE.

### CSPICE Mutex

Tests use `CSPICE_LOCK` from `cspice_common.rs`:

```rust
pub static CSPICE_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn my_test() {
    let _lock = CSPICE_LOCK.lock().unwrap();
    // ... test code ...
}
```

This ensures kernel loading/unloading doesn't conflict between tests.

### CspiceKernels RAII Helper

Tests use `CspiceKernels` for automatic kernel management:

```rust
let mut kernels = CspiceKernels::new();
kernels.load(&lsk_path());  // Loaded via furnsh_c
kernels.load(&spk_path());
// Kernels automatically unloaded via unload_c when dropped
```

## CSPICE Validation Coverage

This section documents which SPK/CK types and functionality have been validated against CSPICE.

### SPK Types

| Type | Name | Status | Test File | Notes |
|------|------|--------|-----------|-------|
| **2** | Chebyshev (position only) | ✅ **VALIDATED** | `cspice_spk_tests.rs` | Uses `de440s.bsp` |
| **9** | Lagrange (unequal time) | ✅ **VALIDATED** | `cspice_spk_tests.rs` | Uses `test.bsp` |
| **13** | Hermite (unequal time) | ✅ **VALIDATED** | `cspice_spk_tests.rs` | Uses `gmat-hermite.bsp` |
| 3 | Chebyshev (pos+vel) | ⚠️ Needs test data | — | Implemented, not validated |
| 5 | Two-body propagation | ⚠️ Needs test data | — | Implemented, not validated |
| 8 | Lagrange (equal time) | ⚠️ Needs test data | — | Implemented, not validated |

### CK Types

| Type | Name | Status | Test File | Notes |
|------|------|--------|-----------|-------|
| **1** | Discrete pointing | ✅ **VALIDATED** | `cspice_ck_tests.rs` | Quaternion interpolation |
| **3** | Linear/SLERP | ✅ **VALIDATED** | `cspice_ck_tests.rs` | Spherical linear interp |

**Note:** Angular velocity (`ckgpav_c`) is not currently validated. Only quaternion orientation is tested.

### Other Functionality

| Feature | Status | Test File | Notes |
|---------|--------|-----------|-------|
| Time parsing (str2et) | ✅ **VALIDATED** | `cspice_time_tests.rs` | UTC, ISO, calendar formats |
| UTC↔TDB conversion | ✅ **VALIDATED** | `cspice_time_tests.rs` | Leap second handling |
| Coordinate transforms | ✅ **VALIDATED** | `cspice_coord_tests.rs` | Cartesian↔Spherical↔Cylindrical |
| Kernel pool (gdpool) | ✅ **VALIDATED** | `cspice_pool_tests.rs` | Text kernel variables |
| DAF parsing | ✅ **VALIDATED** | `cspice_validation_tests.rs` | Segment iteration |

### Adding New Validation Tests

To add CSPICE validation for a new SPK/CK type:

1. Obtain test data file (BSP/BC) containing that segment type
2. Add file to `test_data/` (track with Git LFS)
3. Create test in appropriate `cspice_*_tests.rs` file
4. Use `CspiceKernels` helper for kernel management
5. Compare against CSPICE using established tolerances

See `docs/SPK_CK_TYPE_SUPPORT.md` for full type support documentation.
