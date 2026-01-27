# SPK/CK Type Support

This document describes which NAIF DAF segment types are supported by muad-dib for parsing and interpolation.

## SPK Types (Spacecraft/Planetary Ephemeris)

SPK files contain ephemeris data for spacecraft and planetary bodies. NAIF defines 21+ segment types, of which muad-dib supports the most common ones.

| Type | Name | Parse | Interpolate | CSPICE Validated | Notes |
|------|------|-------|-------------|------------------|-------|
| **2** | Chebyshev (position only) | ✅ | ✅ | ✅ | Used by DE planetary ephemerides |
| **3** | Chebyshev (position+velocity) | ✅ | ✅ | ⚠️ | Needs test data |
| **5** | Two-body propagation | ✅ | ✅ | ⚠️ | Needs test data |
| **8** | Lagrange (equal time steps) | ✅ | ✅ | ⚠️ | Needs test data |
| **9** | Lagrange (unequal time steps) | ✅ | ✅ | ✅ | Variable step interpolation |
| **13** | Hermite (unequal time steps) | ✅ | ✅ | ✅ | Includes velocity data |
| 1 | Modified Difference | ❌ | ❌ | N/A | Legacy format |
| 10 | TLE (Two-Line Elements) | ❌ | ❌ | N/A | Not implemented |
| 12 | Hermite (equal time steps) | ❌ | ❌ | N/A | Not implemented |
| 14 | Chebyshev (unequal time) | ❌ | ❌ | N/A | Not implemented |
| 15 | Precessing conic | ❌ | ❌ | N/A | Not implemented |
| 17 | Equinoctial elements | ❌ | ❌ | N/A | Not implemented |
| 18 | ESOC/ESA Hermite/Lagrange | ❌ | ❌ | N/A | Not implemented |
| 19 | ESOC/ESA piecewise | ❌ | ❌ | N/A | Not implemented |
| 20 | Chebyshev (velocity only) | ❌ | ❌ | N/A | Rare |
| **21** | Extended Modified Diff | ❌ | ❌ | N/A | **Modern DE ephemerides** |

### Legend
- ✅ **Validated** - Tested against CSPICE with sub-micrometer accuracy
- ⚠️ **Needs test data** - Implemented but lacks CSPICE validation data
- ❌ **Not implemented** - Not supported

### Coverage by Use Case

| Use Case | SPK Types Needed | Support Status |
|----------|------------------|----------------|
| DE430/DE440 planetary ephemerides | Type 2 | ✅ Supported |
| DE441+ extended ephemerides | Type 21 | ❌ Not implemented |
| GMAT-generated trajectories | Types 9, 13 | ✅ Supported |
| STK-generated trajectories | Types 9, 13 | ✅ Supported |
| Cassini mission data | Types 2, 9 | ✅ Supported |
| Mars missions | Types 2, 9, 13 | ✅ Supported |

### Type 21 (Extended Modified Difference)

Type 21 is used by newer JPL planetary ephemerides (DE441 and later). It is the most significant gap in current support. Adding Type 21 would require:

1. Implementing the modified difference algorithm
2. Obtaining DE441+ test data
3. CSPICE validation

**Priority**: Medium - needed for latest ephemerides

---

## CK Types (C-Kernel Pointing)

CK files contain spacecraft orientation/pointing data. NAIF defines 6 segment types.

| Type | Name | Parse | Interpolate | CSPICE Validated | Notes |
|------|------|-------|-------------|------------------|-------|
| **1** | Discrete pointing | ✅ | ✅ | ✅ | Explicit quaternions at each time |
| **3** | Linear/SLERP interpolation | ✅ | ✅ | ✅ | Spherical linear interpolation |
| 2 | Constant angular velocity | ❌ | ❌ | N/A | Rarely used |
| 4 | Chebyshev | ❌ | ❌ | N/A | Rarely used |
| 5 | MEX/VEX pointing | ❌ | ❌ | N/A | ESA missions |
| 6 | ESOC pointing | ❌ | ❌ | N/A | ESA missions |

### Coverage by Use Case

| Use Case | CK Types Needed | Support Status |
|----------|-----------------|----------------|
| Cassini mission data | Types 1, 3 | ✅ Supported |
| Mars missions (MRO, MAVEN) | Types 1, 3 | ✅ Supported |
| JWST pointing | Type 3 | ✅ Supported |
| ESA missions (MEX, VEX) | Types 5, 6 | ❌ Not supported |

### Angular Velocity

CK Types 1 and 3 optionally include angular velocity data. muad-dib parses this data but it has **not been validated** against CSPICE `ckgpav_c`. The quaternion interpolation is validated.

---

## BPC Types (Binary Planetary Constants)

BPC files contain binary planetary constants (orientation, shape, etc.).

| Type | Name | Parse | Interpolate | CSPICE Validated | Notes |
|------|------|-------|-------------|------------------|-------|
| **2** | Chebyshev angles | ✅ | ❌ | Parsing only | Earth, Moon orientation |

### BPC Notes

- BPC segments are **parsed** (metadata extracted, data read) but **not interpolated**
- The CLI tools (`despice`/`respice`) only need parsing for format conversion
- Interpolation would require implementing the Chebyshev angle evaluation

---

## Obtaining Test Data

### SPK Types Needing Validation

| Type | Description | Where to Find Test Data |
|------|-------------|-------------------------|
| 3 | Chebyshev pos+vel | Some mission SPKs; GMAT can generate |
| 5 | Two-body propagation | Rare; used for comets/asteroids |
| 8 | Lagrange equal-time | GMAT can generate; older missions |

### NAIF Data Archives

- **NAIF Generic Kernels**: https://naif.jpl.nasa.gov/pub/naif/generic_kernels/
- **Mission-Specific**: https://naif.jpl.nasa.gov/pub/naif/

### Generating Test Data

GMAT (General Mission Analysis Tool) can generate SPK files with specific types:

```matlab
% GMAT script to generate Type 13 Hermite SPK
Create Spacecraft sat;
Create EphemerisFile eph;
eph.Spacecraft = sat;
eph.FileFormat = 'SPK';
eph.OutputFormat = 'HermiteInterpolation'; % Type 13
```

---

## Validation Accuracy

When validated against CSPICE, muad-dib achieves:

| Quantity | Tolerance | Achieved Accuracy |
|----------|-----------|-------------------|
| Position | 1e-9 km | ~1 micrometer |
| Velocity | 1e-12 km/s | ~1 nanometer/second |
| Quaternion | 1e-8 | ~0.00001 degrees |

See `tests/README.md` for detailed tolerance information.
