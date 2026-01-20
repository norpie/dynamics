use super::events::FileBrowserEvent;
use super::list::ListState;
use crate::tui::{Element, Theme};
use anyhow::Result;
use ratatui::prelude::Stylize;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use std::fs;
use std::path::{Path, PathBuf};

/// Represents a file or directory entry in the file browser
#[derive(Debug, Clone)]
pub struct FileBrowserEntry {
    pub name: String,
    pub is_dir: bool,
    pub path: PathBuf,
}

impl FileBrowserEntry {
    /// Render this entry as an Element with proper styling
    pub fn to_element<Msg>(&self, is_selected: bool) -> Element<Msg> {
        let theme = &crate::global_runtime_config().theme;
        let (fg_color, bg_style) = if is_selected {
            (
                theme.accent_primary,
                Some(Style::default().bg(theme.bg_surface)),
            )
        } else {
            (theme.text_primary, None)
        };

        let display_name = if self.is_dir {
            format!("{}/", self.name)
        } else {
            self.name.clone()
        };

        let color = if self.is_dir {
            theme.accent_secondary
        } else {
            fg_color
        };

        let mut builder = Element::styled_text(Line::from(vec![Span::styled(
            format!("  {}", display_name),
            Style::default().fg(color),
        )]));

        if let Some(bg) = bg_style {
            builder = builder.background(bg);
        }

        builder.build()
    }
}

/// Action returned by the file browser event handler
#[derive(Debug, Clone)]
pub enum FileBrowserAction {
    FileSelected(PathBuf),
    DirectoryChanged(PathBuf),
    DirectoryEntered(PathBuf),
}

/// Manages state for file browser widget
#[derive(Debug, Clone)]
pub struct FileBrowserState {
    current_path: PathBuf,
    entries: Vec<FileBrowserEntry>,
    list_state: ListState,
    filter: Option<fn(&FileBrowserEntry) -> bool>,
}

impl FileBrowserState {
    /// Create a new FileBrowserState with the given initial path
    pub fn new(initial_path: PathBuf) -> Self {
        let mut state = Self {
            current_path: initial_path.clone(),
            entries: Vec::new(),
            list_state: ListState::with_selection(),
            filter: None,
        };

        // Try to read initial directory, fallback to current dir on error
        if let Err(_) = state.refresh() {
            if let Ok(cwd) = std::env::current_dir() {
                state.current_path = cwd;
                let _ = state.refresh();
            }
        }

        state
    }

    /// Get current directory path
    pub fn current_path(&self) -> &Path {
        &self.current_path
    }

    /// Get current entries
    pub fn entries(&self) -> &[FileBrowserEntry] {
        &self.entries
    }

    /// Get currently selected entry
    pub fn selected_entry(&self) -> Option<&FileBrowserEntry> {
        self.list_state
            .selected()
            .and_then(|idx| self.entries.get(idx))
    }

    /// Get selected index
    pub fn selected_index(&self) -> Option<usize> {
        self.list_state.selected()
    }

    /// Get reference to list state for rendering
    pub fn list_state(&self) -> &ListState {
        &self.list_state
    }

    /// Get mutable reference to list state
    pub fn list_state_mut(&mut self) -> &mut ListState {
        &mut self.list_state
    }

    /// Set a filter function to show only certain entries
    pub fn set_filter(&mut self, filter: fn(&FileBrowserEntry) -> bool) {
        self.filter = Some(filter);
    }

    /// Clear the filter
    pub fn clear_filter(&mut self) {
        self.filter = None;
    }

    /// Select first entry matching the predicate
    pub fn select_first_matching(&mut self, predicate: impl Fn(&FileBrowserEntry) -> bool) {
        if let Some(idx) = self.entries.iter().position(|e| predicate(e)) {
            let item_count = self.entries.len();
            self.list_state.select_and_scroll(Some(idx), item_count);
        }
    }

    /// Navigate selection up
    pub fn navigate_up(&mut self) {
        self.list_state
            .handle_key(crossterm::event::KeyCode::Up, self.entries.len(), 20);
    }

    /// Navigate selection down
    pub fn navigate_down(&mut self) {
        self.list_state
            .handle_key(crossterm::event::KeyCode::Down, self.entries.len(), 20);
    }

    /// Handle navigation key (for PageUp/PageDown/Home/End)
    pub fn handle_navigation_key(&mut self, key: crossterm::event::KeyCode) {
        self.list_state.handle_key(key, self.entries.len(), 20);
    }

    /// Enter a directory by name
    pub fn enter_directory(&mut self, name: &str) -> Result<()> {
        if name == ".." {
            self.go_to_parent()
        } else {
            let new_path = self.current_path.join(name);
            self.set_path(new_path)
        }
    }

    /// Go to parent directory
    pub fn go_to_parent(&mut self) -> Result<()> {
        if let Some(parent) = self.current_path.parent() {
            self.set_path(parent.to_path_buf())
        } else {
            Ok(())
        }
    }

    /// Set current path and refresh entries
    pub fn set_path(&mut self, path: PathBuf) -> Result<()> {
        self.current_path = path;
        self.refresh()
    }

    /// Refresh directory entries from filesystem
    pub fn refresh(&mut self) -> Result<()> {
        self.entries = read_directory(&self.current_path, self.filter)?;

        // Reset selection to first item
        let item_count = self.entries.len();
        if !self.entries.is_empty() {
            self.list_state.select_and_scroll(Some(0), item_count);
        } else {
            self.list_state.select_and_scroll(None, item_count);
        }

        Ok(())
    }

    /// Handle file browser event and return action
    pub fn handle_event(&mut self, event: FileBrowserEvent) -> Option<FileBrowserAction> {
        use crossterm::event::KeyCode;

        match event {
            FileBrowserEvent::Navigate(key) => {
                self.list_state.handle_key(key, self.entries.len(), 20);
                None
            }
            FileBrowserEvent::Activate => {
                if let Some(entry) = self.selected_entry() {
                    let entry = entry.clone(); // Clone to avoid borrow issues
                    if entry.is_dir {
                        if let Ok(()) = self.enter_directory(&entry.name) {
                            Some(FileBrowserAction::DirectoryEntered(
                                self.current_path.clone(),
                            ))
                        } else {
                            None
                        }
                    } else {
                        Some(FileBrowserAction::FileSelected(entry.path))
                    }
                } else {
                    None
                }
            }
            FileBrowserEvent::GoUp => {
                if let Ok(()) = self.go_to_parent() {
                    Some(FileBrowserAction::DirectoryChanged(
                        self.current_path.clone(),
                    ))
                } else {
                    None
                }
            }
            FileBrowserEvent::Refresh => {
                let _ = self.refresh();
                Some(FileBrowserAction::DirectoryChanged(
                    self.current_path.clone(),
                ))
            }
        }
    }
}

/// Read directory entries and sort them (directories first, then files)
fn read_directory(
    path: &Path,
    filter: Option<fn(&FileBrowserEntry) -> bool>,
) -> Result<Vec<FileBrowserEntry>> {
    let mut entries = Vec::new();

    // Add parent directory entry if not at root
    if path.parent().is_some() {
        entries.push(FileBrowserEntry {
            name: "..".to_string(),
            is_dir: true,
            path: path.parent().unwrap().to_path_buf(),
        });
    }

    // Read directory entries
    let dir_entries = fs::read_dir(path)?;
    let mut dirs = Vec::new();
    let mut files = Vec::new();

    for entry_result in dir_entries {
        let entry = entry_result?;
        let file_name = entry.file_name().to_string_lossy().to_string();
        let is_dir = entry.file_type()?.is_dir();
        let path = entry.path();

        // Skip hidden files/directories (except "..")
        if file_name.starts_with('.') && file_name != ".." {
            continue;
        }

        let browser_entry = FileBrowserEntry {
            name: file_name,
            is_dir,
            path,
        };

        // Apply filter if set
        if let Some(f) = filter {
            if !f(&browser_entry) {
                continue;
            }
        }

        if is_dir {
            dirs.push(browser_entry);
        } else {
            files.push(browser_entry);
        }
    }

    // Sort directories and files separately
    dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    // Add sorted entries (directories first, then files)
    entries.extend(dirs);
    entries.extend(files);

    Ok(entries)
}
