//! Event handling for the Inspector TUI.

use super::{ActivePane, App, DetailSection};
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use std::time::Duration;

/// Poll for events with a timeout.
pub fn poll_event(timeout: Duration) -> std::io::Result<Option<Event>> {
    if event::poll(timeout)? {
        Ok(Some(event::read()?))
    } else {
        Ok(None)
    }
}

/// Handle a key event.
pub fn handle_key_event(app: &mut App, key: KeyEvent) {
    // Help overlay takes precedence
    if app.show_help {
        app.show_help = false;
        return;
    }

    // Global keys (work regardless of active pane)
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            app.should_quit = true;
            return;
        }
        KeyCode::Char('?') => {
            app.show_help = true;
            return;
        }
        KeyCode::Tab => {
            // Switch between panes
            app.active_pane = match app.active_pane {
                ActivePane::Tree => ActivePane::Detail,
                ActivePane::Detail => ActivePane::Tree,
            };
            return;
        }
        KeyCode::Char('1') => {
            app.detail_section = DetailSection::Overview;
            app.detail_scroll = 0;
            return;
        }
        KeyCode::Char('2') => {
            app.detail_section = DetailSection::Segments;
            app.detail_scroll = 0;
            return;
        }
        KeyCode::Char('3') => {
            app.detail_section = DetailSection::Comments;
            app.detail_scroll = 0;
            return;
        }
        KeyCode::Char('[') => {
            app.prev_file();
            return;
        }
        KeyCode::Char(']') => {
            app.next_file();
            return;
        }
        _ => {}
    }

    // Pane-specific keys
    match app.active_pane {
        ActivePane::Tree => handle_tree_key(app, key),
        ActivePane::Detail => handle_detail_key(app, key),
    }
}

fn handle_tree_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => app.tree_up(),
        KeyCode::Down | KeyCode::Char('j') => app.tree_down(),
        KeyCode::Left | KeyCode::Char('h') => {
            // Collapse or move to parent (for now just collapse)
            let visible = app.visible_tree_nodes();
            if let Some((_, node, _)) = visible.get(app.tree_selection) {
                if node.expanded && !node.children.is_empty() {
                    app.tree_toggle();
                }
            }
        }
        KeyCode::Right | KeyCode::Char('l') => {
            // Expand
            let visible = app.visible_tree_nodes();
            if let Some((_, node, _)) = visible.get(app.tree_selection) {
                if !node.expanded && !node.children.is_empty() {
                    app.tree_toggle();
                }
            }
        }
        KeyCode::Enter | KeyCode::Char(' ') => app.tree_toggle(),
        _ => {}
    }
}

fn handle_detail_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => app.detail_up(),
        KeyCode::Down | KeyCode::Char('j') => app.detail_down(),
        KeyCode::Left | KeyCode::Char('h') => app.prev_section(),
        KeyCode::Right | KeyCode::Char('l') => app.next_section(),
        KeyCode::PageUp => {
            for _ in 0..10 {
                app.detail_up();
            }
        }
        KeyCode::PageDown => {
            for _ in 0..10 {
                app.detail_down();
            }
        }
        _ => {}
    }
}
