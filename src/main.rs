use clap::{value_parser, Arg, Command};
use std::fs::File;
use std::path::PathBuf;

use muad_dib::{
    formats::{available_formats, get_format},
    hdf5_output::{write_pck_sources, DAFSource},
    kernel::{DAFSourceExt, SpiceKernel},
    text_pck::PCKSource,
    DAFFile, DAFSegment,
};

fn main() {
    let input_files = Arg::new("input")
        .value_name("FILE(S)")
        .value_parser(value_parser!(PathBuf))
        .required(true)
        .num_args(1..);

    let output_file = Arg::new("output")
        .value_parser(value_parser!(PathBuf))
        .long("output")
        .short('o')
        .help("Output file (default: <first-input>.<format>)");

    let format_arg = Arg::new("format")
        .long("format")
        .short('f')
        .value_parser(available_formats())
        .default_value("hdf5")
        .help("Output format: hdf5, msgpack, bson");

    let info_flag = Arg::new("info")
        .long("info")
        .short('i')
        .action(clap::ArgAction::SetTrue)
        .help("Show kernel info without writing output");

    let app = Command::new("despice")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Convert NAIF SPICE DAF files (SPK, CK, BPCK) and text PCK files to various formats")
        .arg(input_files)
        .arg(output_file)
        .arg(format_arg)
        .arg(info_flag);

    let matches = app.get_matches();

    let input_paths: Vec<_> = matches
        .get_many::<PathBuf>("input")
        .expect("Must specify input file(s).")
        .collect();

    // Check for --info flag
    let info_mode = matches.get_flag("info");
    if info_mode {
        show_kernel_info(&input_paths);
        return;
    }

    // Get format
    let format_name = matches.get_one::<String>("format").unwrap();
    let format = match get_format(format_name) {
        Some(f) => f,
        None => {
            eprintln!("Error: unknown format '{}'", format_name);
            eprintln!("Available formats: {}", available_formats().join(", "));
            std::process::exit(1);
        }
    };

    // Determine output file path
    let output_path = match matches.get_one::<PathBuf>("output") {
        Some(p) => p.clone(),
        None => {
            // Default: first input file with format extension
            let first = input_paths[0];
            first.with_extension(format.extension())
        }
    };

    // Parse all input files - separate DAF and PCK sources
    let mut daf_sources: Vec<DAFSource> = Vec::new();
    let mut pck_sources: Vec<PCKSource> = Vec::new();
    let mut total_segments = 0;
    let mut total_pck_vars = 0;

    for infile in &input_paths {
        // Check file extension to determine parser
        let ext = infile
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            // Text kernel files - parse to PCKSource (not DAFSource)
            // Includes PCK, LSK, SCLK, and FK files
            "tpc" | "pck" | "tls" | "tsc" | "tf" => match PCKSource::from_path(infile) {
                Ok(pck) => {
                    let var_count = pck.variables().len();
                    total_pck_vars += var_count;
                    let kind = match ext.as_str() {
                        "tls" => "LSK",
                        "tsc" => "SCLK",
                        "tf" => "FK",
                        _ => "PCK",
                    };
                    println!("  {} ({}): {} variables", infile.display(), kind, var_count);
                    pck_sources.push(pck);
                }
                Err(why) => {
                    eprintln!("Error: couldn't parse {}: {}", infile.display(), why);
                    std::process::exit(1);
                }
            },
            // Binary DAF files (SPK, CK, BPCK)
            _ => {
                let file = match File::open(infile) {
                    Ok(f) => f,
                    Err(why) => {
                        eprintln!("Error: couldn't open {}: {}", infile.display(), why);
                        std::process::exit(1);
                    }
                };

                match DAFFile::from_file(file) {
                    Err(why) => {
                        eprintln!("Error: couldn't parse {}: {}", infile.display(), why);
                        std::process::exit(1);
                    }
                    Ok(mut daf) => {
                        let header = match daf.daf_header() {
                            Ok(h) => h,
                            Err(why) => {
                                eprintln!(
                                    "Error: couldn't read header from {}: {}",
                                    infile.display(),
                                    why
                                );
                                std::process::exit(1);
                            }
                        };

                        // Capture metadata before consuming the iterator
                        let metadata = daf.daf_metadata();

                        let segments: Vec<DAFSegment> = daf.filter_map(|r| r.ok()).collect();
                        let seg_count = segments.len();
                        total_segments += seg_count;

                        println!(
                            "  {} ({} {}): {} segments",
                            infile.display(),
                            header.kind,
                            header.name,
                            seg_count
                        );

                        daf_sources.push(DAFSource {
                            filename: infile.display().to_string(),
                            header,
                            metadata,
                            segments,
                        });
                    }
                }
            }
        };
    }

    // Check if we have PCK sources with non-HDF5 format
    if !pck_sources.is_empty() && format.name() != "hdf5" {
        eprintln!(
            "Warning: PCK files can only be written to HDF5 format. {} PCK file(s) will be skipped.",
            pck_sources.len()
        );
        pck_sources.clear();
    }

    // Write DAF sources to output format
    if !daf_sources.is_empty() {
        if let Err(why) = format.write(&output_path, &daf_sources) {
            eprintln!(
                "Error: couldn't write {} file {}: {}",
                format.name(),
                output_path.display(),
                why
            );
            std::process::exit(1);
        }
    }

    // Write PCK sources to HDF5 (append to existing file if DAF sources were written)
    if !pck_sources.is_empty() {
        // Open the HDF5 file that was created above (or create new if no DAF sources)
        let h5_file = if daf_sources.is_empty() {
            match hdf5::File::create(&output_path) {
                Ok(f) => f,
                Err(why) => {
                    eprintln!(
                        "Error: couldn't create HDF5 file {}: {}",
                        output_path.display(),
                        why
                    );
                    std::process::exit(1);
                }
            }
        } else {
            match hdf5::File::open_rw(&output_path) {
                Ok(f) => f,
                Err(why) => {
                    eprintln!(
                        "Error: couldn't open HDF5 file {}: {}",
                        output_path.display(),
                        why
                    );
                    std::process::exit(1);
                }
            }
        };

        if let Err(why) = write_pck_sources(&h5_file, &pck_sources) {
            eprintln!(
                "Error: couldn't write PCK sources to {}: {}",
                output_path.display(),
                why
            );
            std::process::exit(1);
        }
    }

    // Print summary
    let daf_count = daf_sources.len();
    let pck_count = pck_sources.len();

    if daf_count > 0 || pck_count > 0 {
        println!(
            "\nWrote {} ({} DAF file(s) with {} segments, {} PCK file(s) with {} variables)",
            output_path.display(),
            daf_count,
            total_segments,
            pck_count,
            total_pck_vars
        );
    } else {
        eprintln!("Error: no valid input files found");
        std::process::exit(1);
    }
}

/// Show kernel info without writing output.
fn show_kernel_info(input_paths: &[&PathBuf]) {
    use muad_dib::brief::names::format_id;

    let paths: Vec<_> = input_paths.iter().map(|p| p.as_path()).collect();

    // Load all files into a SpiceKernel
    let kernel = match SpiceKernel::load_many(&paths) {
        Ok(k) => k,
        Err(why) => {
            eprintln!("Error loading files: {}", why);
            std::process::exit(1);
        }
    };

    println!("Kernel Summary");
    println!("==============\n");

    // Show DAF sources
    for source in kernel.daf_sources() {
        let (spk, ck, bpck) = source.segment_counts();
        println!("File: {}", source.filename);
        println!("  Type: {}", source.header.kind);
        println!("  Name: {}", source.header.name);
        println!("  Segments: {} SPK, {} CK, {} BPCK", spk, ck, bpck);
        println!();
    }

    // Show PCK sources
    for source in kernel.pck_sources() {
        println!("File: {}", source.filename);
        println!("  Type: Text PCK");
        println!("  Variables: {}", source.variables().len());
        println!("  Body IDs: {:?}", source.body_ids());
        println!();
    }

    // SPK coverage summary
    let spk_bodies = kernel.spk_bodies();
    if !spk_bodies.is_empty() {
        println!("SPK Bodies ({})", spk_bodies.len());
        println!("-----------");
        for body in spk_bodies {
            let name = format_id(body.0, false);
            if let Some(intervals) = kernel.spk_coverage(body) {
                let total: f64 = intervals.iter().map(|i| i.end - i.start).sum();
                let days = total / 86400.0;
                println!(
                    "  {}: {:.1} days coverage ({} interval(s))",
                    name,
                    days,
                    intervals.len()
                );
            }
        }
        println!();
    }

    // CK coverage summary
    let ck_instruments = kernel.ck_instruments();
    if !ck_instruments.is_empty() {
        println!("CK Instruments ({})", ck_instruments.len());
        println!("---------------");
        for inst in ck_instruments {
            if let Some(intervals) = kernel.ck_coverage(inst) {
                let total: f64 = intervals.iter().map(|i| i.end - i.start).sum();
                println!(
                    "  {}: {} SCLK ticks coverage ({} interval(s))",
                    inst.0,
                    total as i64,
                    intervals.len()
                );
            }
        }
        println!();
    }

    // BPCK frame summary
    let bpck_frames = kernel.bpck_frames();
    if !bpck_frames.is_empty() {
        println!("BPCK Frames ({})", bpck_frames.len());
        println!("------------");
        for frame in bpck_frames {
            if let Some(intervals) = kernel.bpck_coverage(frame) {
                let total: f64 = intervals.iter().map(|i| i.end - i.start).sum();
                let days = total / 86400.0;
                println!(
                    "  {}: {:.1} days coverage ({} interval(s))",
                    frame.0,
                    days,
                    intervals.len()
                );
            }
        }
        println!();
    }

    // PCK variable summary
    let pck_body_ids = kernel.pck_body_ids();
    if !pck_body_ids.is_empty() {
        println!("PCK Bodies ({})", pck_body_ids.len());
        println!("-----------");
        for body_id in &pck_body_ids[..pck_body_ids.len().min(10)] {
            let vars = kernel.pck_variables_for_body(*body_id);
            let name = format_id(*body_id, false);
            println!("  {}: {} variables", name, vars.len());
        }
        if pck_body_ids.len() > 10 {
            println!("  ... and {} more bodies", pck_body_ids.len() - 10);
        }
        println!();
    }

    // Total summary
    println!(
        "Total: {} DAF file(s), {} PCK file(s), {} segment(s)",
        kernel.daf_sources().len(),
        kernel.pck_sources().len(),
        kernel.segment_count()
    );
}
