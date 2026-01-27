//! Respice - Convert serialized files back to NAIF SPICE DAF format.

use clap::{value_parser, Arg, Command};
use muad_dib::error::Error;
use muad_dib::formats::read_sources;
use muad_dib::hdf5_input::read_pck_sources;
use muad_dib::pck_writer::write_text_pck;
use muad_dib::spk_writer::write_spk;
use muad_dib::text_pck::PCKSource;
use std::path::{Path, PathBuf};

/// Read PCK sources from HDF5 file (only HDF5 supports PCK storage).
fn read_pck_from_file(path: &Path) -> Result<Vec<PCKSource>, Error> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "hdf5" | "h5" => read_pck_sources(path),
        _ => Ok(Vec::new()), // PCK only supported in HDF5
    }
}

fn main() {
    let input_file = Arg::new("input")
        .value_name("FILE")
        .value_parser(value_parser!(PathBuf))
        .required(true)
        .help("Input file (hdf5, parquet, arrow, msgpack, or bson)");

    let output_dir = Arg::new("output")
        .value_parser(value_parser!(PathBuf))
        .long("output")
        .short('o')
        .help("Output directory (default: current directory)");

    let app = Command::new("respice")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Convert serialized files back to NAIF SPICE format (SPK, CK, PCK)")
        .arg(input_file)
        .arg(output_dir);

    let matches = app.get_matches();

    let input_path = matches
        .get_one::<PathBuf>("input")
        .expect("Must specify input file.");

    let output_dir = matches
        .get_one::<PathBuf>("output")
        .cloned()
        .unwrap_or_else(|| PathBuf::from("."));

    // Read DAF sources from input file (auto-detect format)
    println!("Reading {}...", input_path.display());
    let daf_sources = match read_sources(input_path) {
        Ok(s) => s,
        Err(why) => {
            eprintln!("Error reading file: {}", why);
            std::process::exit(1);
        }
    };

    // Read PCK sources from input file (HDF5 only)
    let pck_sources = match read_pck_from_file(input_path) {
        Ok(s) => s,
        Err(why) => {
            eprintln!("Error reading PCK sources: {}", why);
            std::process::exit(1);
        }
    };

    println!(
        "Found {} DAF source(s) and {} PCK source(s)",
        daf_sources.len(),
        pck_sources.len()
    );

    // Create output directory if it doesn't exist
    if !output_dir.exists() {
        if let Err(why) = std::fs::create_dir_all(&output_dir) {
            eprintln!("Error creating output directory: {}", why);
            std::process::exit(1);
        }
    }

    // Separate DAF sources by type
    let spk_sources: Vec<_> = daf_sources.iter().filter(|s| s.header.kind == "SPK").collect();
    let ck_sources: Vec<_> = daf_sources.iter().filter(|s| s.header.kind == "CK").collect();
    let bpck_sources: Vec<_> = daf_sources.iter().filter(|s| s.header.kind == "BPCK").collect();

    let mut written_count = 0;

    // Write SPK files
    for source in &spk_sources {
        let original_name = PathBuf::from(&source.filename);
        let file_name = original_name
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("output_{}.bsp", written_count));

        let output_path = output_dir.join(&file_name);

        println!(
            "  Writing {} ({} segments)...",
            output_path.display(),
            source.segments.len()
        );

        if let Err(why) = write_spk(&output_path, source) {
            eprintln!("Error writing {}: {}", output_path.display(), why);
            std::process::exit(1);
        }

        written_count += 1;
    }

    // Write PCK files (from PCKSource, preserving block structure)
    for pck_source in &pck_sources {
        let original_name = PathBuf::from(&pck_source.filename);
        let file_stem = original_name
            .file_stem()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("output_{}", written_count));

        let output_path = output_dir.join(format!("{}.tpc", file_stem));

        let var_count = pck_source.variables().len();
        println!(
            "  Writing {} ({} variables)...",
            output_path.display(),
            var_count
        );

        if let Err(why) = write_text_pck(&output_path, pck_source) {
            eprintln!("Error writing {}: {}", output_path.display(), why);
            std::process::exit(1);
        }

        written_count += 1;
    }

    // Report skipped sources
    for source in &ck_sources {
        println!(
            "  Skipping {} (type: CK, not yet supported for round-trip)",
            source.filename
        );
    }

    for source in &bpck_sources {
        println!(
            "  Skipping {} (type: BPCK, not yet supported for round-trip)",
            source.filename
        );
    }

    println!(
        "\nWrote {} file(s) to {} ({} SPK, {} PCK)",
        written_count,
        output_dir.display(),
        spk_sources.len(),
        pck_sources.len()
    );
}
