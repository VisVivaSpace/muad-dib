# Usage Guide

## CLI Tools

### despice

Convert NAIF SPICE DAF files to modern data formats.

```
despice [OPTIONS] <INPUT>...

ARGUMENTS:
  <INPUT>...  One or more SPK/CK/BPCK files to convert

OPTIONS:
  -f, --format <FORMAT>  Output format [default: hdf5]
                         Choices: hdf5, parquet, arrow, msgpack, bson
  -o, --output <FILE>    Output file path
  -h, --help             Print help
  -V, --version          Print version
```

**Examples:**

```bash
# Convert single file to HDF5 (default)
despice mission.bsp

# Convert to Parquet with explicit output path
despice mission.bsp --format parquet -o mission.parquet

# Convert multiple files to Arrow
despice de430.bsp de431.bsp --format arrow -o ephemeris.arrow
```

### respice

Convert serialized files back to NAIF SPICE SPK format.

```
respice [OPTIONS] <INPUT>

ARGUMENTS:
  <INPUT>  Input file (hdf5, parquet, arrow, msgpack, or bson)

OPTIONS:
  -o, --output <DIR>  Output directory [default: current directory]
  -h, --help          Print help
  -V, --version       Print version
```

**Examples:**

```bash
# Restore SPK from HDF5
respice mission.hdf5 -o restored/

# Restore from Parquet
respice mission.parquet -o restored/
```

## Rust Library

### Getting Started

```rust
use muad_dib::kernel::SpiceKernel;
use muad_dib::types::NaifId;

// Load kernel and inspect segments
let kernel = SpiceKernel::load("de440.bsp")?;
let bodies = kernel.spk_bodies();
let segments = kernel.spk_segments();
```

> **Note:** Interpolation and computation have moved to the [`understated`](https://github.com/VisVivaSpace/understated) crate.

### Example Programs

The `examples/` directory contains complete working programs:

| Example | Run Command | Description |
|---------|-------------|-------------|
| `time_conversion` | `cargo run --example time_conversion` | Parse and format time strings |
| `kernel_pool` | `cargo run --example kernel_pool -- <file.tpc>` | Read text kernel variables |

### Key Types

| Type | Module | Description |
|------|--------|-------------|
| `SpiceKernel` | `kernel` | Loaded kernel with segments |
| `EpochTDB` | `spice` / `types` | TDB epoch (seconds past J2000) |
| `NaifId` | `types` | NAIF body/frame identifier |

### Traits

- `KernelPoolExt` - Adds `get_f64()`, `pool_has()`, etc. for kernel pool access
- `LeapSecondExt` - Adds `lsk_data()` (returns `Result<LeapSecondData>`) for leap second data extraction

## Format Comparison

| Format | Extension | Size | Read Speed | Write Speed | Best Use Case |
|--------|-----------|------|------------|-------------|---------------|
| HDF5 | `.hdf5` | Baseline | Fast | Fast | Scientific tools (h5py, MATLAB) |
| Parquet | `.parquet` | ~1.1x | Fast | Medium | Analytics (Spark, DuckDB, pandas) |
| Arrow | `.arrow` | ~1.1x | Very Fast | Fast | In-memory processing, Python/pyarrow |
| MessagePack | `.msgpack` | ~1.0x | Fast | Very Fast | Compact storage, streaming |
| BSON | `.bson` | ~1.5x | Medium | Medium | MongoDB integration |

## Round-Trip Workflow

The typical workflow for archiving and restoring SPK files:

```bash
# 1. Convert SPK to portable format
despice mission.bsp --format parquet -o archive/mission.parquet

# 2. Verify the conversion
# (use Python, DuckDB, or other tools to inspect)

# 3. Restore when needed
respice archive/mission.parquet -o restored/

# 4. Verify restoration
diff mission.bsp restored/mission.bsp
```

## Python Integration

### Using pyarrow (Arrow format)

```python
import pyarrow as pa
import pyarrow.ipc as ipc

# Read Arrow file
with pa.memory_map('mission.arrow', 'r') as source:
    reader = ipc.open_file(source)
    table = reader.read_all()

# Access segment data
for batch in table.to_batches():
    df = batch.to_pandas()
    print(df[['segment_name', 'target_code', 'initial_epoch']])
```

### Using pandas with Parquet

```python
import pandas as pd

# Read Parquet file
df = pd.read_parquet('mission.parquet')

# Filter segments
spk_segments = df[df['segment_type'] == 'SPK']
print(spk_segments[['segment_name', 'target_code']])
```

### Using h5py (HDF5 format)

```python
import h5py

with h5py.File('mission.hdf5', 'r') as f:
    # List sources
    for source_name in f.keys():
        source = f[source_name]
        print(f"Source: {source_name}")
        print(f"  Kind: {source.attrs['kind']}")

        # Access segments
        segments = source['segments']
        for seg_name in segments.keys():
            seg = segments[seg_name]
            print(f"  Segment: {seg_name}")
            print(f"    Data shape: {seg['data'].shape}")
```

## Schema Reference

### Flattened Schema (Parquet/Arrow)

Both Parquet and Arrow use a flattened row-per-segment schema:

| Column | Type | Description |
|--------|------|-------------|
| `source_filename` | string | Original SPK filename |
| `source_name` | string | DAF internal name |
| `source_comment` | string | DAF comment block |
| `source_kind` | string | "SPK", "CK", or "BPCK" |
| `segment_type` | string | "SPK", "CK", or "BPCK" |
| `segment_name` | string | Segment descriptor name |
| `initial_epoch` | float64 | Start epoch (SPK only) |
| `final_epoch` | float64 | End epoch (SPK only) |
| `target_code` | int32 | NAIF body ID (SPK only) |
| `center_code` | int32 | NAIF center body ID (SPK only) |
| `frame_code` | int32 | NAIF frame ID |
| `spk_type` | int32 | SPK segment type (SPK only) |
| `data` | list<float64> | Raw segment data array |

Additional columns exist for CK and BPCK segments (e.g., `initial_sclk`, `instrument_code`).

> **Note:** In the Rust API, `target_code`, `center_code`, `frame_code`, and `instrument_code` are `NaifId` newtypes. They serialize transparently as `int32` in Parquet/Arrow output.

### Hierarchical Schema (HDF5)

HDF5 files use a hierarchical structure:

```
/
├── source_0/
│   ├── @kind = "SPK"
│   ├── @name = "DE430"
│   ├── @comment = "..."
│   ├── metadata/
│   │   ├── @nd, @ni, @endian, @fward, @bward, @free_address, @ftpstr
│   └── segments/
│       ├── segment_0/
│       │   ├── @name, @initial_epoch, @final_epoch
│       │   ├── @target_code, @center_code, @frame_code, @spk_type
│       │   └── data [dataset]
│       └── segment_1/
│           └── ...
└── source_1/
    └── ...
```
