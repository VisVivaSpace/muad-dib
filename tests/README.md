# Test Suite

This directory contains validation tests for muad-dib DAF parsing and interpolation.

## Test Categories

### Standard Tests (no external dependencies)

Run with `cargo test`:

- **anise_validation_tests.rs** - Pure-Rust structural validation using anise
  - File record validation (ND, NI, endian, FWARD)
  - Segment count and metadata matching
  - Data array size validation

### CSPICE Validation Tests (feature-gated)

Require CSPICE library installed. Run with:
```bash
export CSPICE_LIB=/path/to/cspice/lib
cargo test --features cspice -- --test-threads=1
```

Test files:
- **cspice_spk_tests.rs** - SPK ephemeris interpolation
- **cspice_ck_tests.rs** - CK pointing interpolation
- **cspice_coord_tests.rs** - Coordinate transformations
- **cspice_time_tests.rs** - Time conversions (ET/TDB)
- **cspice_pool_tests.rs** - Kernel pool operations
- **cspice_validation_tests.rs** - General validation
- **cspice_common.rs** - Shared test utilities (mutex, FFI wrappers)

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

## Test Data Requirements

CSPICE tests require kernel files in the project root:

| File | Type | Purpose |
|------|------|---------|
| `test.bsp` | SPK | Ephemeris data for position/velocity tests |
| `test.bc` | CK | Pointing data for orientation tests |
| `naif0012.tls` | LSK | Leap seconds for time conversions |

## Running Tests

### Single-threaded requirement

CSPICE is NOT thread-safe. Tests use a global mutex (`CSPICE_LOCK`) but must run single-threaded:

```bash
# CSPICE tests MUST use --test-threads=1
cargo test --features cspice -- --test-threads=1

# Run specific test file
cargo test --features cspice --test cspice_spk_tests -- --test-threads=1

# Run specific test
cargo test --features cspice validate_spk_position_midpoint -- --test-threads=1
```

### Verbose output

```bash
cargo test --features cspice -- --test-threads=1 --nocapture
```

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
lazy_static! {
    pub static ref CSPICE_LOCK: Mutex<()> = Mutex::new(());
}

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
