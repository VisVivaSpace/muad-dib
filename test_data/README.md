# Test Data Files

This directory contains SPICE kernel files used for testing muad-dib.

## Current Test Files

### SPK Files (Ephemeris)

| File | Type | Description |
|------|------|-------------|
| `test.bsp` | Type 9 | Lagrange interpolation test file |
| `gmat-hermite.bsp` | Type 13 | GMAT-generated Hermite interpolation |
| `gmat-lagrange.bsp` | Type 9 | GMAT-generated Lagrange interpolation |
| `gmat-hermite-big-endian.bsp` | Type 13 | Big-endian version of gmat-hermite.bsp |
| `variable-seg-size-hermite.bsp` | Type 12 | Variable step Hermite (partial support) |
| `rename-test.bsp` | Various | Multiple segment test file |

### CK Files (Pointing)

| File | Type | Description |
|------|------|-------------|
| `test.bc` | Type 3 | C-kernel test file with quaternion data |

### BPC Files (Binary PCK - Planetary Orientation)

| File | Type | Description | Source |
|------|------|-------------|--------|
| `earth_latest_high_prec.bpc` | Type 2 | Earth high-precision orientation | NAIF generic_kernels/pck/ |
| `earth_2025_250826_2125_predict.bpc` | Type 2 | Earth orientation prediction | NAIF generic_kernels/pck/ |
| `earth_longterm_000101_251211_250915.bpc` | Type 2 | Earth long-term orientation | NAIF generic_kernels/pck/ |
| `moon_pa_de440_200625.bpc` | Type 2 | Moon principal axes (DE440) | NAIF generic_kernels/pck/ |

### Text Kernels

| File | Description |
|------|-------------|
| `test.tpc` | Text PCK with planetary constants |
| `naif0012.tls` | Leap seconds kernel (required for CSPICE time conversion) |

### HDF5 Files

| File | Description |
|------|-------------|
| `test_pck.hdf5` | Converted PCK data for HDF5 roundtrip tests |

## Additional Files Needed for Full Coverage

The following kernel types are not yet covered by tests:

| Type | Description | NAIF Source |
|------|-------------|-------------|
| SPK Type 2 | Chebyshev (position only) | `generic_kernels/spk/planets/de440.bsp` |
| SPK Type 3 | Chebyshev (position + velocity) | `generic_kernels/spk/` |
| SPK Type 5 | Two-body discrete states | Mission kernels |
| SPK Type 8 | Lagrange (equal time steps) | Mission kernels |
| CK Type 2 | CMAT rotation matrices | Mission kernels |
| CK Type 4 | CMAT with angular rates | Mission kernels |

## Download Instructions

NAIF kernels can be downloaded from:
- Generic kernels: https://naif.jpl.nasa.gov/pub/naif/generic_kernels/
- Mission kernels: https://naif.jpl.nasa.gov/pub/naif/

Example:
```bash
# Download leap seconds kernel
curl -O https://naif.jpl.nasa.gov/pub/naif/generic_kernels/lsk/naif0012.tls

# Download planetary ephemeris (large file - 120MB)
curl -O https://naif.jpl.nasa.gov/pub/naif/generic_kernels/spk/planets/de440.bsp
```

## Notes

- Large files (>100MB) should use Git LFS if committed to the repository
- All test files are gated behind the `test-data` feature flag
- CSPICE validation tests also require `--features cspice`
