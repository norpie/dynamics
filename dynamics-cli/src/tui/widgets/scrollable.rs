/// Manages scrolling state for Scrollable elements
#[derive(Debug, Clone)]
pub struct ScrollableState {
    scroll_offset: usize,
    viewport_height: Option<usize>, // Last known viewport height from renderer
    content_height: Option<usize>,  // Last known content height from renderer
    horizontal_scroll_offset: usize,
    viewport_width: Option<usize>, // Last known viewport width from renderer
    content_width: Option<usize>,  // Last known content width from renderer
}

impl Default for ScrollableState {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollableState {
    /// Create a new ScrollableState
    pub fn new() -> Self {
        Self {
            scroll_offset: 0,
            viewport_height: None,
            content_height: None,
            horizontal_scroll_offset: 0,
            viewport_width: None,
            content_width: None,
        }
    }

    /// Set the viewport height (called by renderer with actual area height)
    pub fn set_viewport_height(&mut self, height: usize) {
        self.viewport_height = Some(height);
    }

    /// Set the viewport width (called by renderer with actual area width)
    pub fn set_viewport_width(&mut self, width: usize) {
        self.viewport_width = Some(width);
    }

    /// Get current scroll offset
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Get current horizontal scroll offset
    pub fn horizontal_scroll_offset(&self) -> usize {
        self.horizontal_scroll_offset
    }

    /// Get viewport height
    pub fn viewport_height(&self) -> Option<usize> {
        self.viewport_height
    }

    /// Get viewport width
    pub fn viewport_width(&self) -> Option<usize> {
        self.viewport_width
    }

    /// Get content height
    pub fn content_height(&self) -> Option<usize> {
        self.content_height
    }

    /// Get content width
    pub fn content_width(&self) -> Option<usize> {
        self.content_width
    }

    /// Handle keyboard navigation
    pub fn handle_key(
        &mut self,
        key: crossterm::event::KeyCode,
        content_height: usize,
        visible_height: usize,
    ) {
        if content_height == 0 {
            return;
        }

        // Use stored viewport_height if available, otherwise use provided value
        let height = self.viewport_height.unwrap_or(visible_height);
        let max_scroll = content_height.saturating_sub(height);

        match key {
            crossterm::event::KeyCode::Up => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
            crossterm::event::KeyCode::Down => {
                self.scroll_offset = (self.scroll_offset + 1).min(max_scroll);
            }
            crossterm::event::KeyCode::PageUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(height);
            }
            crossterm::event::KeyCode::PageDown => {
                self.scroll_offset = (self.scroll_offset + height).min(max_scroll);
            }
            crossterm::event::KeyCode::Home => {
                self.scroll_offset = 0;
            }
            crossterm::event::KeyCode::End => {
                self.scroll_offset = max_scroll;
            }
            crossterm::event::KeyCode::Left => {
                self.horizontal_scroll_offset = self.horizontal_scroll_offset.saturating_sub(1);
            }
            crossterm::event::KeyCode::Right => {
                if let (Some(content_width), Some(viewport_width)) =
                    (self.content_width, self.viewport_width)
                {
                    let max_h_scroll = content_width.saturating_sub(viewport_width);
                    self.horizontal_scroll_offset =
                        (self.horizontal_scroll_offset + 1).min(max_h_scroll);
                }
            }
            _ => {}
        }
    }

    /// Update scroll offset to stay within bounds
    /// Called during rendering with actual dimensions
    pub fn update_scroll(&mut self, visible_height: usize, content_height: usize) {
        self.content_height = Some(content_height);

        if content_height == 0 {
            self.scroll_offset = 0;
            return;
        }

        // Don't scroll if all content fits on screen
        if content_height <= visible_height {
            self.scroll_offset = 0;
            return;
        }

        // Clamp scroll to valid range
        let max_offset = content_height.saturating_sub(visible_height);
        self.scroll_offset = self.scroll_offset.min(max_offset);
    }

    /// Update horizontal scroll offset to stay within bounds
    /// Called during rendering with actual dimensions
    pub fn update_horizontal_scroll(&mut self, visible_width: usize, content_width: usize) {
        self.content_width = Some(content_width);

        if content_width == 0 {
            self.horizontal_scroll_offset = 0;
            return;
        }

        // Don't scroll if all content fits on screen
        if content_width <= visible_width {
            self.horizontal_scroll_offset = 0;
            return;
        }

        // Clamp scroll to valid range
        let max_offset = content_width.saturating_sub(visible_width);
        self.horizontal_scroll_offset = self.horizontal_scroll_offset.min(max_offset);
    }
}
