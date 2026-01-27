//! Inspector - Interactive TUI for SPICE kernel file inspection.
//!
//! Usage: inspector <file.bsp> [file2.bc] ...

use clap::{value_parser, Arg, Command};
use crossterm::{
    event::Event,
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use muad_dib::inspector::{event, ui, App};
use ratatui::prelude::*;
use std::io::{self, stdout};
use std::path::PathBuf;
use std::time::Duration;

fn main() -> io::Result<()> {
    let matches = Command::new("inspector")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Interactive TUI for inspecting SPICE kernel files")
        .arg(
            Arg::new("files")
                .value_name("FILE")
                .value_parser(value_parser!(PathBuf))
                .num_args(0..)
                .help("Input files (SPK, CK, BPCK, HDF5, Parquet, Arrow, etc.)"),
        )
        .get_matches();

    // Initialize app
    let mut app = App::new();

    // Load any files specified on command line
    if let Some(files) = matches.get_many::<PathBuf>("files") {
        for path in files {
            if let Err(e) = app.load_file(path) {
                eprintln!("Warning: Could not load {}: {}", path.display(), e);
            }
        }
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Main loop
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        // Draw
        terminal.draw(|frame| ui::render(frame, app))?;

        // Handle events
        if let Some(evt) = event::poll_event(Duration::from_millis(100))? {
            match evt {
                Event::Key(key) => event::handle_key_event(app, key),
                Event::Resize(_, _) => {
                    // Terminal resize is handled automatically by ratatui
                }
                _ => {}
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
