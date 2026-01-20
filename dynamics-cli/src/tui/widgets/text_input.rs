use crossterm::event::KeyCode;

/// Manages text input cursor and scrolling state
#[derive(Debug, Clone)]
pub struct TextInputState {
    cursor_pos: usize,    // Character index (0 = before first char)
    scroll_offset: usize, // For horizontal scrolling when text > width
}

impl Default for TextInputState {
    fn default() -> Self {
        Self::new()
    }
}

impl TextInputState {
    /// Create a new TextInputState with cursor at start
    pub fn new() -> Self {
        Self {
            cursor_pos: 0,
            scroll_offset: 0,
        }
    }

    /// Get current cursor position
    pub fn cursor_pos(&self) -> usize {
        self.cursor_pos
    }

    /// Get current scroll offset
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Set cursor position to the end of the given text
    pub fn set_cursor_to_end(&mut self, text: &str) {
        self.cursor_pos = text.chars().count();
    }

    /// Handle a key press and update text value
    /// Returns Some(new_value) if text changed, None if only cursor moved
    pub fn handle_key(
        &mut self,
        key: KeyCode,
        current_value: &str,
        max_length: Option<usize>,
    ) -> Option<String> {
        let char_count = current_value.chars().count();

        match key {
            KeyCode::Char(c) => {
                // Insert character at cursor if under max length
                if let Some(max) = max_length {
                    if char_count >= max {
                        return None;
                    }
                }

                let mut chars: Vec<char> = current_value.chars().collect();
                chars.insert(self.cursor_pos, c);
                self.cursor_pos += 1;

                Some(chars.into_iter().collect())
            }
            KeyCode::Backspace => {
                // Delete character before cursor
                if self.cursor_pos > 0 {
                    let mut chars: Vec<char> = current_value.chars().collect();
                    chars.remove(self.cursor_pos - 1);
                    self.cursor_pos -= 1;
                    Some(chars.into_iter().collect())
                } else {
                    None
                }
            }
            KeyCode::Delete => {
                // Delete character at cursor position
                if self.cursor_pos < char_count {
                    let mut chars: Vec<char> = current_value.chars().collect();
                    chars.remove(self.cursor_pos);
                    Some(chars.into_iter().collect())
                } else {
                    None
                }
            }
            KeyCode::Left => {
                // Move cursor left
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                }
                None
            }
            KeyCode::Right => {
                // Move cursor right
                if self.cursor_pos < char_count {
                    self.cursor_pos += 1;
                }
                None
            }
            KeyCode::Home => {
                // Jump to start
                self.cursor_pos = 0;
                None
            }
            KeyCode::End => {
                // Jump to end
                self.cursor_pos = char_count;
                None
            }
            _ => None,
        }
    }

    /// Update scroll offset to keep cursor visible
    /// Called during rendering
    pub fn update_scroll(&mut self, visible_width: usize, text: &str) {
        let char_count = text.chars().count();

        // Ensure cursor is visible within the visible window
        if self.cursor_pos < self.scroll_offset {
            // Cursor moved left of visible area
            self.scroll_offset = self.cursor_pos;
        } else if self.cursor_pos >= self.scroll_offset + visible_width {
            // Cursor moved right of visible area
            self.scroll_offset = self.cursor_pos.saturating_sub(visible_width - 1);
        }

        // Clamp scroll offset
        let max_offset = char_count.saturating_sub(visible_width);
        self.scroll_offset = self.scroll_offset.min(max_offset);
    }
}

/*
 * ============================================================================
 * PHASE 2 / V2 FEATURES (not yet implemented)
 * ============================================================================
 *
 * The following features are deferred to a future version:
 *
 * 1. TEXT SELECTION
 *    - Shift+Left/Right to select characters
 *    - Ctrl+A to select all
 *    - Visual highlight of selected text
 *    - Delete selection on typing
 *
 * 2. CLIPBOARD OPERATIONS
 *    - Ctrl+C to copy
 *    - Ctrl+X to cut
 *    - Ctrl+V to paste
 *    - Integration with system clipboard
 *
 * 3. WORD NAVIGATION
 *    - Ctrl+Left/Right to jump by word
 *    - Ctrl+Backspace/Delete to delete word
 *
 * 4. UNDO/REDO
 *    - Ctrl+Z to undo
 *    - Ctrl+Y or Ctrl+Shift+Z to redo
 *    - History stack of text changes
 *
 * 5. MOUSE INTERACTIONS
 *    - Click to position cursor
 *    - Double-click to select word
 *    - Triple-click to select all
 *    - Drag to select text
 *
 * 6. PASSWORD MODE
 *    - Display ••• instead of actual characters
 *    - Secure input masking
 *
 * 7. CURSOR STYLING (theming)
 *    - Block cursor █
 *    - Underscore cursor _
 *    - Vertical line cursor │ (current default)
 *    - Configurable via theme or RuntimeConfig
 *
 * 8. CURSOR BLINKING
 *    - Animated blinking cursor
 *    - Requires timer subscription
 *    - Configurable on/off
 *
 * 9. INPUT VALIDATION (UI feedback)
 *    - Visual indication of invalid input (red border)
 *    - Built-in validators (email, number, etc.)
 *    - Custom validation functions
 *
 * 10. AUTOCOMPLETE/SUGGESTIONS
 *     - Dropdown of suggestions while typing
 *     - Tab to complete
 *     - Arrow keys to navigate suggestions
 *
 * ============================================================================
 */
