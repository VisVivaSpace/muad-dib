//! Inspector TUI - Interactive SPICE kernel file viewer.
//!
//! This module provides an interactive terminal user interface for inspecting
//! SPICE kernel files (SPK, CK, BPCK) and converted formats (HDF5, Parquet, etc.).

pub mod event;
pub mod ui;
pub mod widgets;

use crate::brief::{collect_summaries, FileSummary, FileType, ObjectSummary};
use crate::error::Error;
use crate::{DAFFile, DAFHeader, DAFSegment};
use std::fs::File;
use std::path::Path;

/// Active pane in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActivePane {
    #[default]
    Tree,
    Detail,
}

/// Detail section tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DetailSection {
    #[default]
    Overview,
    Segments,
    Comments,
}

impl DetailSection {
    pub fn as_str(&self) -> &'static str {
        match self {
            DetailSection::Overview => "Overview",
            DetailSection::Segments => "Segments",
            DetailSection::Comments => "Comments",
        }
    }

    pub fn all() -> &'static [DetailSection] {
        &[
            DetailSection::Overview,
            DetailSection::Segments,
            DetailSection::Comments,
        ]
    }

    pub fn next(&self) -> Self {
        match self {
            DetailSection::Overview => DetailSection::Segments,
            DetailSection::Segments => DetailSection::Comments,
            DetailSection::Comments => DetailSection::Overview,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            DetailSection::Overview => DetailSection::Comments,
            DetailSection::Segments => DetailSection::Overview,
            DetailSection::Comments => DetailSection::Segments,
        }
    }
}

/// A tree node that can be expanded/collapsed.
#[derive(Debug, Clone)]
pub struct TreeNode {
    /// Display label
    pub label: String,
    /// Unique identifier
    pub id: String,
    /// Whether this node is expanded
    pub expanded: bool,
    /// Child nodes
    pub children: Vec<TreeNode>,
    /// Associated data key (for segment lookup)
    pub data_key: Option<TreeDataKey>,
}

/// Key to look up associated data.
#[derive(Debug, Clone)]
pub struct TreeDataKey {
    pub file_index: usize,
    pub object_id: Option<i32>,
}

/// Loaded file data.
#[derive(Debug)]
pub struct LoadedFile {
    /// File path
    pub path: String,
    /// File summary (from brief module)
    pub summary: FileSummary,
    /// Full header with comments
    pub header: DAFHeader,
    /// All segments from the file
    pub segments: Vec<DAFSegment>,
}

/// Application state.
pub struct App {
    /// Whether the app should quit
    pub should_quit: bool,
    /// Loaded files
    pub files: Vec<LoadedFile>,
    /// Currently active file tab index
    pub active_file: usize,
    /// Currently active pane
    pub active_pane: ActivePane,
    /// Current detail section
    pub detail_section: DetailSection,
    /// Tree nodes for the current file
    pub tree_nodes: Vec<TreeNode>,
    /// Currently selected tree node index (flattened)
    pub tree_selection: usize,
    /// Scroll offset for tree
    pub tree_scroll: usize,
    /// Scroll offset for detail pane
    pub detail_scroll: usize,
    /// Show help overlay
    pub show_help: bool,
    /// Error message to display
    pub error_message: Option<String>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            files: Vec::new(),
            active_file: 0,
            active_pane: ActivePane::Tree,
            detail_section: DetailSection::Overview,
            tree_nodes: Vec::new(),
            tree_selection: 0,
            tree_scroll: 0,
            detail_scroll: 0,
            show_help: false,
            error_message: None,
        }
    }

    /// Load a file into the application.
    pub fn load_file(&mut self, path: &Path) -> Result<(), Error> {
        // Get file summary (handles both DAF and converted formats)
        let summaries = collect_summaries(path)?;
        let summary = summaries
            .into_iter()
            .next()
            .ok_or_else(|| Error::EmptyData {
                context: "No summaries found in file".into(),
            })?;

        // For DAF files, also load the full header and segments
        let (header, segments) = load_daf_details(path)?;

        let loaded = LoadedFile {
            path: path.display().to_string(),
            summary,
            header,
            segments,
        };

        self.files.push(loaded);
        self.active_file = self.files.len() - 1;
        self.rebuild_tree();

        Ok(())
    }

    /// Rebuild tree nodes for the current file.
    pub fn rebuild_tree(&mut self) {
        self.tree_nodes.clear();
        self.tree_selection = 0;
        self.tree_scroll = 0;

        if self.files.is_empty() {
            return;
        }

        let file_idx = self.active_file;
        if file_idx >= self.files.len() {
            return;
        }

        let file = &self.files[file_idx];

        // Create root node for the file
        let mut root = TreeNode {
            label: format!(
                "{} ({})",
                file.summary
                    .filename
                    .rsplit('/')
                    .next()
                    .unwrap_or(&file.summary.filename),
                file.summary.file_type
            ),
            id: format!("file_{}", file_idx),
            expanded: true,
            children: Vec::new(),
            data_key: Some(TreeDataKey {
                file_index: file_idx,
                object_id: None,
            }),
        };

        // Add children for each object
        for obj in &file.summary.objects {
            let label = format_object_label(obj, file.summary.file_type);
            let child = TreeNode {
                label,
                id: format!("obj_{}_{}", file_idx, obj.id),
                expanded: false,
                children: Vec::new(),
                data_key: Some(TreeDataKey {
                    file_index: file_idx,
                    object_id: Some(obj.id),
                }),
            };
            root.children.push(child);
        }

        self.tree_nodes.push(root);
    }

    /// Get flattened list of visible tree nodes for rendering.
    pub fn visible_tree_nodes(&self) -> Vec<(usize, &TreeNode, usize)> {
        let mut result = Vec::new();
        for node in &self.tree_nodes {
            collect_visible_nodes(node, 0, &mut result);
        }
        result
    }

    /// Move tree selection up.
    pub fn tree_up(&mut self) {
        if self.tree_selection > 0 {
            self.tree_selection -= 1;
        }
    }

    /// Move tree selection down.
    pub fn tree_down(&mut self) {
        let visible = self.visible_tree_nodes();
        if self.tree_selection < visible.len().saturating_sub(1) {
            self.tree_selection += 1;
        }
    }

    /// Toggle expand/collapse on current tree node.
    pub fn tree_toggle(&mut self) {
        let visible = self.visible_tree_nodes();
        if let Some((_, node, _)) = visible.get(self.tree_selection) {
            let id = node.id.clone();
            self.toggle_node_by_id(&id);
        }
    }

    fn toggle_node_by_id(&mut self, id: &str) {
        for node in &mut self.tree_nodes {
            if toggle_node_recursive(node, id) {
                return;
            }
        }
    }

    /// Get currently selected tree node's data key.
    pub fn selected_data_key(&self) -> Option<TreeDataKey> {
        let visible = self.visible_tree_nodes();
        visible
            .get(self.tree_selection)
            .and_then(|(_, node, _)| node.data_key.clone())
    }

    /// Scroll detail pane up.
    pub fn detail_up(&mut self) {
        if self.detail_scroll > 0 {
            self.detail_scroll -= 1;
        }
    }

    /// Scroll detail pane down.
    pub fn detail_down(&mut self) {
        self.detail_scroll += 1;
    }

    /// Switch to next file tab.
    pub fn next_file(&mut self) {
        if !self.files.is_empty() {
            self.active_file = (self.active_file + 1) % self.files.len();
            self.rebuild_tree();
        }
    }

    /// Switch to previous file tab.
    pub fn prev_file(&mut self) {
        if !self.files.is_empty() {
            self.active_file = if self.active_file == 0 {
                self.files.len() - 1
            } else {
                self.active_file - 1
            };
            self.rebuild_tree();
        }
    }

    /// Switch to next detail section.
    pub fn next_section(&mut self) {
        self.detail_section = self.detail_section.next();
        self.detail_scroll = 0;
    }

    /// Switch to previous detail section.
    pub fn prev_section(&mut self) {
        self.detail_section = self.detail_section.prev();
        self.detail_scroll = 0;
    }

    /// Get the current file if any.
    pub fn current_file(&self) -> Option<&LoadedFile> {
        self.files.get(self.active_file)
    }
}

fn collect_visible_nodes<'a>(
    node: &'a TreeNode,
    depth: usize,
    result: &mut Vec<(usize, &'a TreeNode, usize)>,
) {
    let index = result.len();
    result.push((index, node, depth));

    if node.expanded {
        for child in &node.children {
            collect_visible_nodes(child, depth + 1, result);
        }
    }
}

fn toggle_node_recursive(node: &mut TreeNode, id: &str) -> bool {
    if node.id == id {
        node.expanded = !node.expanded;
        return true;
    }
    for child in &mut node.children {
        if toggle_node_recursive(child, id) {
            return true;
        }
    }
    false
}

fn format_object_label(obj: &ObjectSummary, file_type: FileType) -> String {
    use crate::brief::names::{body_name, frame_name, spacecraft_name};

    match file_type {
        FileType::SPK => {
            if let Some(name) = body_name(obj.id) {
                format!("{} ({})", name, obj.id)
            } else {
                format!("Body {}", obj.id)
            }
        }
        FileType::CK => {
            // Try to get spacecraft name from instrument code
            let sc_id = obj.id / 1000;
            if let Some(name) = spacecraft_name(sc_id) {
                format!("{} ({})", name, obj.id)
            } else {
                format!("Instrument {}", obj.id)
            }
        }
        FileType::BPCK => {
            if let Some(name) = frame_name(obj.id) {
                format!("{} ({})", name, obj.id)
            } else {
                format!("Frame {}", obj.id)
            }
        }
    }
}

fn load_daf_details(path: &Path) -> Result<(DAFHeader, Vec<DAFSegment>), Error> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "bsp" | "spk" | "bc" | "ck" | "bpc" | "bpck" => {
            let file = File::open(path)?;
            let mut daf = DAFFile::from_file(file)?;
            let header = daf.daf_header()?;

            // Re-open to iterate segments (DAFFile consumes during iteration)
            let file = File::open(path)?;
            let daf = DAFFile::from_file(file)?;
            let segments: Vec<DAFSegment> = daf.filter_map(|r| r.ok()).collect();

            Ok((header, segments))
        }
        // For converted formats, create a placeholder header
        _ => Ok((
            DAFHeader {
                name: path.display().to_string(),
                comment: String::new(),
                kind: "Converted".to_string(),
            },
            Vec::new(),
        )),
    }
}
