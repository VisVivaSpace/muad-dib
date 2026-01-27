# Changelog

## [0.3.0] - 2026-01-27

### Breaking Changes

- **PointingRecord angular velocity**: Replaced `av_x: Option<f64>`, `av_y: Option<f64>`,
  `av_z: Option<f64>` with `angular_velocity: Option<[f64; 3]>`. Eliminates invalid
  partial-vector states.
- **Segment struct NAIF IDs**: `target_code`, `center_code`, `frame_code`, and
  `instrument_code` fields on `SPKSegment`, `CKSegment`, `BPCKSegment` changed from
  `i32` to `NaifId`. Use `.0` to access the raw i32 value.
- **`lsk_data()` returns `Result`**: `LeapSecondExt::lsk_data()` now returns
  `Result<LeapSecondData>` instead of `Option<LeapSecondData>`, propagating parse
  errors instead of silently skipping corrupted leap second entries.

### Added

- Date validation in `calendar_to_tdb` — rejects invalid month/day/hour/minute/second
  ranges with `TimeParseError`.
- Named constants for DAF file record and summary byte offsets.
- `DafReader` struct consolidating binary read operations with endian handling.

### Fixed

- `read_string` off-by-one: reads `maxlen` bytes instead of `maxlen - 1`.

### Improved

- PCK variable names normalized to uppercase at parse time (matches CSPICE behavior).
- `SegmentRow` constructors deduplicated via shared base initialization in Parquet
  and Arrow format modules.
- Debug assertion on `get_i32` pool conversion for out-of-range f64 values.

## [0.2.0]

- Remove interpolation/computation code; refocus as I/O-only crate.
- Align DELTET computation with CSPICE's `deltet_` for exact time conversion.
- Tighten CSPICE time test tolerances from 1e-6 to 1e-9.
