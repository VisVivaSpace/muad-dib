# Installation Guide

This document covers installation options for `muad-dib`.

## System Requirements

**HDF5 Library**: muad-dib requires the HDF5 C library installed on your system.

### macOS (Homebrew)
```bash
brew install hdf5
```

### Ubuntu/Debian
```bash
sudo apt-get install libhdf5-dev
```

### Fedora/RHEL
```bash
sudo dnf install hdf5-devel
```

### Windows
Download HDF5 from [The HDF Group](https://www.hdfgroup.org/downloads/hdf5/) and set `HDF5_DIR` environment variable.

---

## Installation Methods

### 1. Cargo Install (Recommended for Rust Users)

If you have Rust installed:

```bash
cargo install muad-dib
```

This downloads and compiles from crates.io. Requires:
- Rust toolchain (rustc, cargo)
- HDF5 development libraries (see above)
- C compiler (for HDF5 bindings)

### 2. Pre-built Binaries (GitHub Releases)

Download pre-built binaries from [GitHub Releases](https://github.com/yourusername/despice/releases):

| Platform | Architecture | File |
|----------|--------------|------|
| macOS | Apple Silicon (M1/M2/M3) | `despice-aarch64-apple-darwin.tar.gz` |
| macOS | Intel | `despice-x86_64-apple-darwin.tar.gz` |
| Linux | x86_64 | `despice-x86_64-unknown-linux-gnu.tar.gz` |
| Windows | x86_64 | `despice-x86_64-pc-windows-msvc.zip` |

Extract and add to your PATH:

```bash
# macOS/Linux
tar -xzf despice-*.tar.gz
sudo mv despice /usr/local/bin/

# Or add to your PATH
export PATH="$PATH:/path/to/extracted/directory"
```

**Note**: Pre-built binaries are dynamically linked against HDF5. You still need HDF5 installed on your system.

### 3. Homebrew Tap (macOS/Linux)

```bash
# Add the tap
brew tap yourusername/despice

# Install
brew install despice
```

The formula automatically installs the HDF5 dependency.

#### Creating Your Own Homebrew Tap

To publish despice via Homebrew:

1. Create a GitHub repo named `homebrew-despice`
2. Add a formula file `Formula/despice.rb`:

```ruby
class Despice < Formula
  desc "Convert NAIF SPICE DAF files to HDF5 format"
  homepage "https://github.com/yourusername/despice"
  url "https://github.com/yourusername/despice/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "YOUR_SHA256_HERE"
  license "MIT"

  depends_on "rust" => :build
  depends_on "hdf5"

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    system "#{bin}/despice", "--version"
  end
end
```

### 4. Build from Source

```bash
# Clone the repository
git clone https://github.com/yourusername/despice.git
cd despice

# Build release binary
cargo build --release

# Binary is at target/release/despice
./target/release/despice --help
```

### 5. Docker

For CI/CD pipelines or isolated environments:

```dockerfile
FROM rust:1.75-slim

RUN apt-get update && apt-get install -y libhdf5-dev && rm -rf /var/lib/apt/lists/*
RUN cargo install muad-dib

ENTRYPOINT ["despice"]
```

Usage:
```bash
docker build -t despice .
docker run -v $(pwd):/data despice /data/input.bsp -o /data/output.hdf5
```

### 6. Nix/NixOS

Add to your `flake.nix`:

```nix
{
  inputs.despice.url = "github:yourusername/despice";

  # In your outputs
  environment.systemPackages = [ inputs.despice.packages.${system}.default ];
}
```

Or use directly:
```bash
nix run github:yourusername/despice -- input.bsp -o output.hdf5
```

---

## Verifying Installation

```bash
# Check version
despice --version

# Show help
despice --help

# Test conversion
despice test.bsp -o test.hdf5
```

---

## Troubleshooting

### HDF5 Not Found

If you see errors about HDF5 not being found:

```bash
# Set HDF5_DIR to your HDF5 installation
export HDF5_DIR=/usr/local/opt/hdf5  # macOS Homebrew
export HDF5_DIR=/usr                  # Linux system install

# Then rebuild
cargo build --release
```

### macOS: Library Not Loaded

If you get "Library not loaded" errors on macOS:

```bash
# Check HDF5 is installed
brew list hdf5

# Reinstall if needed
brew reinstall hdf5
```

### Linux: Missing libhdf5.so

```bash
# Install development package
sudo apt-get install libhdf5-dev  # Debian/Ubuntu
sudo dnf install hdf5-devel       # Fedora/RHEL
```

---

## Platform-Specific Notes

### Apple Silicon (M1/M2/M3)

Native ARM64 builds work out of the box with Homebrew HDF5:

```bash
brew install hdf5
cargo build --release
```

### Windows Subsystem for Linux (WSL)

Follow the Linux instructions within WSL:

```bash
sudo apt-get install libhdf5-dev
cargo install muad-dib
```
