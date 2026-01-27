//! UI rendering for the Inspector TUI.

use super::{ActivePane, App, DetailSection, TreeDataKey};
use crate::brief::{FileType, TimeFormat, TimeKind};
use crate::brief::time::format_time;
use crate::DAFSegment;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Tabs, Wrap};

/// Main render function.
pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Main layout: title bar, content, status bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Title bar
            Constraint::Length(1), // File tabs
            Constraint::Min(10),   // Content
            Constraint::Length(1), // Status bar
        ])
        .split(area);

    render_title_bar(frame, chunks[0]);
    render_file_tabs(frame, chunks[1], app);
    render_content(frame, chunks[2], app);
    render_status_bar(frame, chunks[3], app);

    // Help overlay
    if app.show_help {
        render_help_overlay(frame, area);
    }

    // Error message overlay
    if let Some(ref msg) = app.error_message {
        render_error_overlay(frame, area, msg);
    }
}

fn render_title_bar(frame: &mut Frame, area: Rect) {
    let title = Line::from(vec![
        Span::styled(" Inspector ", Style::default().bold()),
        Span::raw("v"),
        Span::raw(env!("CARGO_PKG_VERSION")),
        Span::raw("  "),
        Span::styled("[?] Help  [q] Quit", Style::default().dim()),
    ]);
    frame.render_widget(Paragraph::new(title).style(Style::default().bg(Color::DarkGray)), area);
}

fn render_file_tabs(frame: &mut Frame, area: Rect, app: &App) {
    if app.files.is_empty() {
        let empty = Paragraph::new(" No files loaded. Run: inspector <file.bsp> ...")
            .style(Style::default().dim());
        frame.render_widget(empty, area);
        return;
    }

    let titles: Vec<Line> = app
        .files
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let name = f.path.rsplit('/').next().unwrap_or(&f.path);
            if i == app.active_file {
                Line::from(format!(" [{}] ", name))
            } else {
                Line::from(format!("  {}  ", name))
            }
        })
        .collect();

    let tabs = Tabs::new(titles)
        .select(app.active_file)
        .highlight_style(Style::default().fg(Color::Yellow).bold());

    frame.render_widget(tabs, area);
}

fn render_content(frame: &mut Frame, area: Rect, app: &App) {
    if app.files.is_empty() {
        let welcome = Paragraph::new(
            "\n  Welcome to Inspector!\n\n\
             Usage: inspector <file.bsp> [file2.bc] ...\n\n\
             Supported formats:\n\
               - SPK files (.bsp, .spk)\n\
               - CK files (.bc, .ck)\n\
               - BPCK files (.bpc, .bpck)\n\
               - HDF5 converted files (.hdf5, .h5)\n\
               - Parquet files (.parquet, .pq)\n\
               - Arrow files (.arrow, .feather)\n\
               - MsgPack files (.msgpack, .mp)\n\
               - BSON files (.bson)"
        )
        .block(Block::default().borders(Borders::ALL).title("Inspector"));
        frame.render_widget(welcome, area);
        return;
    }

    // Split into tree pane (left) and detail pane (right)
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);

    render_tree_pane(frame, chunks[0], app);
    render_detail_pane(frame, chunks[1], app);
}

fn render_tree_pane(frame: &mut Frame, area: Rect, app: &App) {
    let is_active = app.active_pane == ActivePane::Tree;
    let border_style = if is_active {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(" Contents ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible = app.visible_tree_nodes();
    let items: Vec<ListItem> = visible
        .iter()
        .map(|(idx, node, depth)| {
            let indent = "  ".repeat(*depth);
            let prefix = if node.children.is_empty() {
                "  "
            } else if node.expanded {
                "▼ "
            } else {
                "▶ "
            };

            let selected = *idx == app.tree_selection;
            let style = if selected && is_active {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else if selected {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };

            ListItem::new(format!("{}{}{}", indent, prefix, node.label)).style(style)
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, inner);
}

fn render_detail_pane(frame: &mut Frame, area: Rect, app: &App) {
    let is_active = app.active_pane == ActivePane::Detail;
    let border_style = if is_active {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    // Outer block with section tabs in title
    let section_tabs: String = DetailSection::all()
        .iter()
        .map(|s| {
            if *s == app.detail_section {
                format!("[{}]", s.as_str())
            } else {
                format!(" {} ", s.as_str())
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(format!(" {} ", section_tabs));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Get selected data
    let data_key = app.selected_data_key();

    match app.detail_section {
        DetailSection::Overview => render_overview(frame, inner, app, data_key.as_ref()),
        DetailSection::Segments => render_segments(frame, inner, app, data_key.as_ref()),
        DetailSection::Comments => render_comments(frame, inner, app),
    }
}

fn render_overview(frame: &mut Frame, area: Rect, app: &App, data_key: Option<&TreeDataKey>) {
    let Some(file) = app.current_file() else {
        return;
    };

    let mut lines = Vec::new();

    // File info
    lines.push(Line::from(vec![
        Span::styled("File: ", Style::default().bold()),
        Span::raw(&file.path),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Type: ", Style::default().bold()),
        Span::raw(format!("{}", file.summary.file_type)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Internal: ", Style::default().bold()),
        Span::raw(&file.summary.internal_name),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Objects: ", Style::default().bold()),
        Span::raw(format!("{}", file.summary.objects.len())),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Segments: ", Style::default().bold()),
        Span::raw(format!("{}", file.segments.len())),
    ]));

    // If an object is selected, show its details
    if let Some(key) = data_key {
        if let Some(obj_id) = key.object_id {
            lines.push(Line::from(""));
            lines.push(Line::styled(
                "Selected Object",
                Style::default().bold().underlined(),
            ));

            if let Some(obj) = file.summary.objects.iter().find(|o| o.id == obj_id) {
                lines.push(Line::from(vec![
                    Span::styled("ID: ", Style::default().bold()),
                    Span::raw(format!("{}", obj.id)),
                ]));

                if let Some(center) = obj.center {
                    let center_label = match file.summary.file_type {
                        FileType::SPK => "Center: ",
                        FileType::BPCK => "Base Frame: ",
                        FileType::CK => "Frame: ",
                    };
                    lines.push(Line::from(vec![
                        Span::styled(center_label, Style::default().bold()),
                        Span::raw(format!("{}", center)),
                    ]));
                }

                lines.push(Line::from(vec![
                    Span::styled("Intervals: ", Style::default().bold()),
                    Span::raw(format!("{}", obj.intervals.len())),
                ]));

                // Show coverage
                if !obj.intervals.is_empty() {
                    let start = obj.intervals.iter().map(|i| i.start).fold(f64::INFINITY, f64::min);
                    let end = obj.intervals.iter().map(|i| i.end).fold(f64::NEG_INFINITY, f64::max);

                    let (start_str, end_str) = if obj.time_kind == TimeKind::SCLK {
                        (format!("{:.3} SCLK", start), format!("{:.3} SCLK", end))
                    } else {
                        (format_tdb(start), format_tdb(end))
                    };

                    lines.push(Line::from(vec![
                        Span::styled("Start: ", Style::default().bold()),
                        Span::raw(start_str),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled("End: ", Style::default().bold()),
                        Span::raw(end_str),
                    ]));
                }
            }
        }
    }

    let para = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((app.detail_scroll as u16, 0));
    frame.render_widget(para, area);
}

fn render_segments(frame: &mut Frame, area: Rect, app: &App, data_key: Option<&TreeDataKey>) {
    let Some(file) = app.current_file() else {
        return;
    };

    // Filter segments by selected object if any
    let segments: Vec<&DAFSegment> = if let Some(key) = data_key {
        if let Some(obj_id) = key.object_id {
            file.segments
                .iter()
                .filter(|seg| match seg {
                    DAFSegment::SPK(s) => s.target_code == obj_id,
                    DAFSegment::CK(s) => s.instrument_code == obj_id,
                    DAFSegment::BPCK(s) => s.frame_id == obj_id,
                })
                .collect()
        } else {
            file.segments.iter().collect()
        }
    } else {
        file.segments.iter().collect()
    };

    if segments.is_empty() {
        let para = Paragraph::new("No segments to display");
        frame.render_widget(para, area);
        return;
    }

    // Build segment table
    let mut lines = Vec::new();

    // Header based on file type
    match file.summary.file_type {
        FileType::SPK => {
            lines.push(Line::styled(
                " #  | Target     | Center | Frame | Type | Start             | End",
                Style::default().bold(),
            ));
            lines.push(Line::raw("────┼────────────┼────────┼───────┼──────┼───────────────────┼───────────────────"));
        }
        FileType::CK => {
            lines.push(Line::styled(
                " #  | Instrument | Frame  | Type | AV | Start SCLK        | End SCLK",
                Style::default().bold(),
            ));
            lines.push(Line::raw("────┼────────────┼────────┼──────┼────┼───────────────────┼───────────────────"));
        }
        FileType::BPCK => {
            lines.push(Line::styled(
                " #  | Frame ID   | Base   | Type | Start             | End",
                Style::default().bold(),
            ));
            lines.push(Line::raw("────┼────────────┼────────┼──────┼───────────────────┼───────────────────"));
        }
    }

    for (i, seg) in segments.iter().skip(app.detail_scroll).take(area.height as usize - 2).enumerate() {
        let row_num = i + app.detail_scroll + 1;
        let line = match seg {
            DAFSegment::SPK(s) => format!(
                "{:3} | {:>10} | {:>6} | {:>5} | {:>4} | {:17} | {:17}",
                row_num,
                s.target_code,
                s.center_code,
                s.frame_code,
                s.spk_type,
                truncate_epoch(&format_tdb(s.initial_epoch), 17),
                truncate_epoch(&format_tdb(s.final_epoch), 17),
            ),
            DAFSegment::CK(s) => format!(
                "{:3} | {:>10} | {:>6} | {:>4} | {:>2} | {:17.3} | {:17.3}",
                row_num,
                s.instrument_code,
                s.frame_code,
                s.ck_type,
                if s.rates { "Y" } else { "N" },
                s.initial_sclk,
                s.final_sclk,
            ),
            DAFSegment::BPCK(s) => format!(
                "{:3} | {:>10} | {:>6} | {:>4} | {:17} | {:17}",
                row_num,
                s.frame_id,
                s.base_frame,
                s.bpck_type,
                truncate_epoch(&format_tdb(s.initial_epoch), 17),
                truncate_epoch(&format_tdb(s.final_epoch), 17),
            ),
        };
        lines.push(Line::raw(line));
    }

    // Show count
    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::styled("Total: ", Style::default().bold()),
        Span::raw(format!("{} segments", segments.len())),
    ]));

    let para = Paragraph::new(lines);
    frame.render_widget(para, area);
}

fn render_comments(frame: &mut Frame, area: Rect, app: &App) {
    let Some(file) = app.current_file() else {
        return;
    };

    if file.header.comment.is_empty() {
        let para = Paragraph::new("No comments in this file");
        frame.render_widget(para, area);
        return;
    }

    let lines: Vec<Line> = file
        .header
        .comment
        .lines()
        .skip(app.detail_scroll)
        .take(area.height as usize)
        .map(Line::raw)
        .collect();

    let _total_lines = file.header.comment.lines().count();

    let para = Paragraph::new(lines);
    frame.render_widget(para, area);
}

fn render_status_bar(frame: &mut Frame, area: Rect, app: &App) {
    let hints = match app.active_pane {
        ActivePane::Tree => {
            "[Tab] Switch pane  [↑↓] Navigate  [←→] Expand/Collapse  [1-3] Section  [?] Help  [q] Quit"
        }
        ActivePane::Detail => {
            "[Tab] Switch pane  [↑↓] Scroll  [←→] Section  [1-3] Section  [?] Help  [q] Quit"
        }
    };

    let status = Paragraph::new(hints).style(Style::default().bg(Color::DarkGray));
    frame.render_widget(status, area);
}

fn render_help_overlay(frame: &mut Frame, area: Rect) {
    let help_text = r#"
  Inspector - SPICE Kernel File Viewer

  Navigation
  ──────────
  Tab        Switch between tree and detail panes
  ↑/↓ or j/k Navigate up/down
  ←/→ or h/l Expand/collapse (tree) or change section (detail)
  [ / ]      Previous/next file tab

  Sections
  ────────
  1          Overview
  2          Segments
  3          Comments

  General
  ───────
  ?          Toggle this help
  q / Esc    Quit

  Press any key to close this help...
"#;

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Help ")
        .style(Style::default().bg(Color::Black));

    // Center the popup
    let popup_area = centered_rect(60, 70, area);
    frame.render_widget(Clear, popup_area);
    frame.render_widget(block.clone(), popup_area);

    let inner = block.inner(popup_area);
    let para = Paragraph::new(help_text);
    frame.render_widget(para, inner);
}

fn render_error_overlay(frame: &mut Frame, area: Rect, message: &str) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Error ")
        .style(Style::default().bg(Color::Red).fg(Color::White));

    let popup_area = centered_rect(50, 20, area);
    frame.render_widget(Clear, popup_area);
    frame.render_widget(block.clone(), popup_area);

    let inner = block.inner(popup_area);
    let para = Paragraph::new(message).wrap(Wrap { trim: true });
    frame.render_widget(para, inner);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn truncate_epoch(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

/// Format TDB seconds as a calendar date string.
fn format_tdb(tdb_seconds: f64) -> String {
    format_time(tdb_seconds, TimeFormat::CalendarET)
}
