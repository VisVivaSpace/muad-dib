# API Improvements: Rust Idioms and Conventions

## Summary

Improve the muad-dib library API to better follow Rust conventions and idioms.

## Tasks

### Phase 1: Clean Up Prelude
- [x] Move clap imports from `src/prelude.rs` to `src/main.rs` only
- [x] Remove other external crate re-exports (keep only crate types + Result alias)
- [x] Verify compilation

### Phase 2: Implement Standard Traits for State
- [x] Implement `std::ops::Add` for `State`
- [x] Implement `std::ops::Sub` for `State`
- [x] Implement `std::ops::Neg` for `State`
- [x] Update tests to use operators instead of methods
- [x] Removed old methods (no external usage found)

### Phase 3: Add Display for Newtypes
- [x] Implement `Display` for `NaifId`
- [x] Implement `Display` for `DafAddress`
- [x] Implement `Display` for `EpochTDB`
- [x] Add tests for Display implementations

### Phase 4: Add #[must_use] Attributes
- [x] Add `#[must_use]` to `SpiceKernelBuilder::build()`
- [x] Add `#[must_use]` to `SpiceKernel::load()` and `load_many()`
- [x] Verify no new warnings

### Phase 5: Complete Newtype Migration
- [x] Update `Error::NoCoverage` to use `NaifId` for body field
- [x] Keep `epoch` as `f64` (used for both TDB seconds and SCLK ticks)
- [x] Verify compilation and tests pass

## Files NOT to Modify
- `src/kernel/spk_types.rs` - Keep existing structure
- `src/brief/*` - Keep existing implementation
- `src/inspector/*` - TUI-specific code

## Review

### Changes Made

**1. `src/prelude.rs`** - Cleaned up external dependencies
- Removed `clap` re-exports (Arg, Command, value_parser)
- Kept standard library re-exports needed by crate internals
- Kept `Error` type and `Result<T>` alias

**2. `src/main.rs`** - Updated imports
- Added direct `clap` imports
- Added local `Result<T>` alias using library `Error` type

**3. `src/spice/interpolate/mod.rs`** - Added standard traits for State
- Implemented `Add<State>` and `Add<&State>` for State
- Implemented `Sub<State>` and `Sub<&State>` for State
- Implemented `Neg` for State
- Removed old `add()` and `negate()` methods
- Updated tests to use operator syntax

**4. `src/spice/spk.rs`** - Updated to use operators
- Changed `state.negate()` to `-state`
- Changed `state.add(&other)` to `state + other`
- Updated tests

**5. `src/types.rs`** - Added Display implementations
- `DafAddress`: Displays as numeric value
- `EpochTDB`: Displays as "{value} TDB"
- `NaifId`: Displays as numeric value
- Added tests for all Display implementations

**6. `src/kernel/builder.rs`** - Added #[must_use]
- Added `#[must_use]` to `build()` method

**7. `src/kernel/mod.rs`** - Added #[must_use]
- Added `#[must_use]` to `load()` and `load_many()` methods

**8. `src/error.rs`** - Updated NoCoverage variant
- Changed `body: i32` to `body: NaifId`
- Kept `epoch: f64` (semantic mismatch if using EpochTDB for SCLK)
- Updated error message format

### Test Results

All 159 library tests pass.

### User Testing

Run the following to verify:

```bash
cargo build                    # All binaries compile
cargo test --lib              # All library tests pass
cargo run --bin brief -- de430.bsp  # Brief tool works
cargo run --bin despice -- --help   # CLI help works
```

---

# Space Mission Design: State Context Implementation

Add reference frame and body context to `State` and `Pointing` structs for type-safe relativity tracking.

## Implementation Steps

- [x] **1. Add fields to State struct** (`src/spice/interpolate/mod.rs:29-39`)
  - Add `target: NaifId`, `center: NaifId`, `frame: i32` fields
  - Update `State::new()` constructor to accept metadata
  - Add `State::new_raw()` for internal use (position/velocity only without metadata)
  - Update `State::from_position()` to accept metadata
  - Update `State::default()` to include sensible defaults
  - DO NOT MODIFY: The position/velocity fields, distance(), speed() methods

- [x] **2. Update State arithmetic with validation** (`src/spice/interpolate/mod.rs:73-172`)
  - Add impl: validate `self.frame == other.frame` AND `self.target == other.center`, panic on mismatch
    - Result: `State { target: other.target, center: self.center, frame: self.frame, ... }`
  - Sub impl: validate `self.frame == other.frame` AND `self.center == other.center`, panic on mismatch
    - Result: `State { target: self.target, center: other.target, frame: self.frame, ... }`
  - Neg impl: swap target/center, negate position/velocity
  - DO NOT MODIFY: The actual position/velocity math

- [x] **3. Add frame field to Pointing struct** (`src/spice/interpolate/mod.rs:183-193`)
  - Add `frame: i32` field
  - Update `Pointing::new()` constructor
  - Add `Pointing::new_raw()` for internal use (quaternion/angular_velocity only)
  - Update `Pointing::from_quaternion()` to accept frame
  - DO NOT MODIFY: quaternion math, normalize(), is_normalized()

- [x] **4. Update chebyshev/hermite/lagrange/twobody interpolators**
  - These return State with position/velocity only - keep as-is returning State::new_raw()
  - The context will be added at the SpkSegmentViewInterpolate level

- [x] **5. Update SpkSegmentViewInterpolate::state_at()** (`src/spice/spk.rs:43-58`)
  - After computing raw state, populate target/center/frame from segment metadata
  - Use `SpkSegmentView` to access segment's target(), center(), frame()

- [x] **6. Update SpkInterpolateExt::state_of()** (`src/spice/spk.rs:91-126`)
  - Verify returned states have correct metadata after chaining
  - Chain traversal should produce correct target/center values

- [x] **7. Update CK interpolation for Pointing frame** (`src/spice/ck.rs`)
  - Update `evaluate_type1()` and `evaluate_type3()` to use new_raw()
  - Update `CkSegmentViewInterpolate::pointing_at()` to populate frame

- [x] **8. Fix compilation errors**
  - Update all tests that construct State directly
  - Update all tests that construct Pointing directly
  - Ensure all interpolation code compiles

- [x] **9. Add arithmetic validation tests** (`src/spice/interpolate/mod.rs`)
  - Test panic on frame mismatch in Add
  - Test panic on chain invalidity in Add (target != center)
  - Test panic on frame mismatch in Sub
  - Test panic on center mismatch in Sub
  - Test Neg correctly swaps target/center

## Files Modified

| File | Changes |
|------|---------|
| `src/types.rs` | Added `Serialize`, `Deserialize` derives to `NaifId` |
| `src/spice/interpolate/mod.rs` | State + Pointing struct fields, arithmetic validation, updated tests |
| `src/spice/interpolate/chebyshev.rs` | Use `State::new_raw()` |
| `src/spice/interpolate/hermite.rs` | Use `State::new_raw()` |
| `src/spice/interpolate/lagrange.rs` | Use `State::new_raw()` |
| `src/spice/interpolate/twobody.rs` | Use `State::new_raw()` |
| `src/spice/spk.rs` | `state_at()` populates metadata from segment, updated tests |
| `src/spice/ck.rs` | `pointing_at()` populates frame, use `Pointing::new_raw()` |

## Review

### Summary

Implemented type-safe relativity tracking for `State` and `Pointing` structs:

1. **State struct** now carries full context: `target`, `center`, `frame` alongside `position` and `velocity`
2. **State arithmetic** validates frame/center compatibility and panics on mismatches
3. **Pointing struct** now includes `frame` field for reference frame context
4. **Interpolation pipeline** uses internal `new_raw()` constructors, then populates context at the segment view level

### Breaking Changes

- `State::new()` now requires 5 arguments: `(target, center, frame, position, velocity)`
- `State::from_position()` now requires 4 arguments: `(target, center, frame, position)`
- `Pointing::new()` now requires 3 arguments: `(frame, quaternion, angular_velocity)`
- `Pointing::from_quaternion()` now requires 2 arguments: `(frame, quaternion)`
- State arithmetic will panic if frame/center constraints are violated

### Test Results

All 171 tests pass, including 4 new `#[should_panic]` tests for arithmetic validation.

### User Testing

```bash
cargo build              # Verify compilation
cargo test               # Verify all 171 tests pass
cargo clippy             # Pre-existing warnings only
```

To test manual usage:
```rust
let kernel = SpiceKernel::load("de430.bsp")?;
let state = kernel.state_of(NaifId::EARTH, EpochTDB(0.0), NaifId::SSB)?;

// State now includes full context
println!("Target: {}", state.target);   // 399 (Earth)
println!("Center: {}", state.center);   // 0 (SSB)
println!("Frame: {}", state.frame);     // 1 (J2000)
println!("Position: {:?}", state.position);
```

---

# SPK Type 9/13 Window Selection Fix

## Summary

Implemented CSPICE-compatible window selection algorithm for SPK Types 8/9 (Lagrange) and 13 (Hermite).

## Status: COMPLETE ✅

All CSPICE validation tests pass:
- 13 SPK tests
- 10 CK tests
- 12 coordinate tests
- 6 validation tests

## Root Cause Analysis

The initial 1.4 million km difference was NOT a parsing error, but a **reference frame mismatch**:

- test.bsp segments use Frame 17 (ECLIPJ2000)
- Tests were querying CSPICE in "J2000" (Frame 1)
- J2000 vs ECLIPJ2000 differ by ~23.4° rotation around X-axis
- This caused Y and Z components to differ significantly while X matched

## Fixes Applied

### 1. Reference Frame Fix (`tests/cspice_common.rs`, `tests/cspice_spk_tests.rs`)

- Added `frame_name(frame_code: i32)` helper function to convert NAIF frame codes to CSPICE frame strings
- Updated all `cspice_spkgeo()` calls to use `frame_name(spk.frame_code)` instead of hardcoded "J2000"

### 2. Window Selection Algorithm (`src/spice/interpolate/lagrange.rs`, `src/spice/interpolate/hermite.rs`)

Implemented CSPICE-compatible algorithm from spkr09.c:

- **ODD window size**: Center around the NEAREST epoch
- **EVEN window size**: Use the LOWER bracketing epoch

```rust
let first = if wndsiz % 2 == 1 {
    // ODD: center around NEAREST epoch
    let near = if (epoch - data.states[lower].epoch).abs()
        <= (data.states[high].epoch - epoch).abs()
    { lower } else { high };
    // ... boundary clamping
} else {
    // EVEN: use LOWER bracket
    // ... boundary clamping
};
```

### 3. Endpoint Handling (`tests/cspice_spk_tests.rs`)

- Query 1 microsecond before exact endpoints to avoid floating-point precision issues
- At large epoch values (~1.3e9), `epoch - 1e-9 == epoch` due to float precision

### 4. Coordinate Tests (`tests/cspice_coord_tests.rs`)

- Changed `COORD_TOLERANCE` from 1e-12 to 1e-10 (appropriate for values ~5000 km)
- Added `assert_angle_close()` for longitude comparisons with ±2π wraparound handling

### 5. Validation Tests (`tests/cspice_validation_tests.rs`)

- Fixed file path to use "test_data/test.bsp"

## Files Modified

| File | Change |
|------|--------|
| `src/spice/interpolate/lagrange.rs` | Window selection algorithm for Type 8/9 |
| `src/spice/interpolate/hermite.rs` | Window selection algorithm for Type 13 |
| `tests/cspice_common.rs` | Added `frame_name()` helper |
| `tests/cspice_spk_tests.rs` | Frame handling, endpoint epsilon |
| `tests/cspice_coord_tests.rs` | Tolerance and angle wraparound |
| `tests/cspice_validation_tests.rs` | File path fix |
| `tests/spk_debug.rs` | Diagnostic test file (new) |

## Verification

```bash
CSPICE_LIB=/Users/nstrange/cspice/lib cargo test --features cspice,test-data \
  --test cspice_spk_tests --test cspice_coord_tests --test cspice_ck_tests \
  --test cspice_validation_tests -- --test-threads=1

# Result: 41 tests pass (13 SPK + 12 coord + 10 CK + 6 validation)
```

## Known Issues (Pre-existing, Unrelated)

- Pool tests (`cspice_pool_tests.rs`) fail on case sensitivity - CSPICE `gdpool_c` doesn't find lowercase variable names. This is a separate issue not related to the window selection fix.
