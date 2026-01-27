# muad-dib

*Free the spice.*

A Rust library and CLI for liberating NASA SPICE data from legacy binary formats into modern, accessible formats like HDF5, Parquet, and Arrow.

## What is SPICE?

[SPICE](https://naif.jpl.nasa.gov/naif/) is NASA's system for spacecraft navigation and observation geometry, maintained by the Navigation and Ancillary Information Facility (NAIF) at JPL. It provides the critical data needed to answer questions like:

- **Where is my spacecraft?** (SPK - ephemeris data)
- **Which way is it pointing?** (CK - orientation data)
- **What are the properties of this planet?** (PCK - planetary constants)

SPICE kernels are the standard format for distributing this data across the space industry. Every major mission—from Voyager to Mars rovers to James Webb—uses SPICE for geometry calculations.

## Why muad-dib?

SPICE binary data is trapped in DAF (Double precision Array File), a Fortran-era binary format designed in the 1980s. While robust, these files are:

- **Opaque**: Requires specialized CSPICE libraries to read
- **Monolithic**: Difficult to query or transform with modern tools
- **Inaccessible**: No native support in Python/Pandas, Julia, R, or data science ecosystems

muad-dib extracts the valuable spice data within and converts it to formats that work everywhere. The spice must flow—into your data pipelines, your notebooks, your experiments.

## Features

- Parse NAIF DAF binary files with automatic endian detection
- Multiple output formats: HDF5, Parquet, Arrow IPC, MessagePack, BSON
- Round-trip support: convert back to SPK/CK/PCK with `respice`
- Preserves all segment metadata for exact reconstruction
- Kernel pool access, time parsing, and leap second conversions
- Text kernel support: LSK (leap seconds), SCLK, FK (frames)
- Interpolation/computation available in the [`understated`](https://github.com/VisVivaSpace/understated) crate

## Supported Formats

| Format | Extension | Description |
|--------|-----------|-------------|
| HDF5 | `.hdf5` | Hierarchical Data Format, scientific computing standard |
| Parquet | `.parquet` | Columnar storage, optimized for analytics |
| Arrow | `.arrow` | Arrow IPC (Feather v2), fast in-memory format |
| MessagePack | `.msgpack` | Compact binary serialization |
| BSON | `.bson` | Binary JSON, MongoDB compatible |

## Installation

See [docs/INSTALLATION.md](docs/INSTALLATION.md) for detailed instructions.

```bash
# From source
cargo install --path .

# Or build locally
cargo build --release
```

## Quick Start

### Convert SPK to HDF5

```bash
despice ephemeris.bsp -o ephemeris.hdf5
```

### Convert to other formats

```bash
despice input.bsp --format parquet -o output.parquet
despice input.bsp --format arrow -o output.arrow
```

### Convert back to SPICE format

```bash
respice output.hdf5 -o restored/
```

## Library API

### Basic DAF Parsing

```rust
use muad_dib::{DAFFile, DAFSegment};
use std::fs::File;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open("ephemeris.bsp")?;
    let daf = DAFFile::from_file(file)?;

    for segment in daf {
        match segment? {
            DAFSegment::SPK(spk) => {
                println!("Target: {}, Epochs: {} to {}",
                    spk.target_code, spk.initial_epoch, spk.final_epoch);
            }
            _ => {}
        }
    }
    Ok(())
}
```

### Kernel Loading

```rust
use muad_dib::kernel::SpiceKernel;
use muad_dib::types::NaifId;

let kernel = SpiceKernel::load("de440.bsp")?;

// List available bodies
let bodies = kernel.spk_bodies();
println!("Bodies: {:?}", bodies);

// Iterate segments
for segment in kernel.spk_segments() {
    println!("Target: {}, Type: {}", segment.target_code, segment.spk_type);
}
```

> **Note:** Interpolation and state computation have moved to the [`understated`](https://github.com/VisVivaSpace/understated) crate.

See `examples/` for complete working programs demonstrating each API.

## Examples

The `examples/` directory contains runnable demonstrations:

```bash
# Time string parsing and conversion
cargo run --example time_conversion

# Access kernel pool variables from text PCK
cargo run --example kernel_pool -- pck00010.tpc
```

Download sample kernels from NAIF:
- Ephemeris: https://naif.jpl.nasa.gov/pub/naif/generic_kernels/spk/planets/
- PCK: https://naif.jpl.nasa.gov/pub/naif/generic_kernels/pck/

## License

MIT License - see [LICENSE.txt](LICENSE.txt)

---

*"He who controls the spice controls the universe." We just want to read it in Parquet.*
