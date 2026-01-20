//! Color picker widget state and logic

use crate::tui::color::{HSL, hsl_to_rgb, rgb_to_hsl};
use crossterm::event::KeyCode;
use ratatui::style::Color;

/// Color picker display mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorPickerMode {
    /// HSL mode: Hue, Saturation, Lightness
    HSL,
    /// RGB mode: Red, Green, Blue
    RGB,
}

impl ColorPickerMode {
    /// Toggle between HSL and RGB
    pub fn toggle(&self) -> Self {
        match self {
            Self::HSL => Self::RGB,
            Self::RGB => Self::HSL,
        }
    }
}

/// Which channel/input field is currently focused
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    /// First channel (H in HSL, R in RGB)
    Primary,
    /// Second channel (S in HSL, G in RGB)
    Secondary,
    /// Third channel (L in HSL, B in RGB)
    Tertiary,
    /// Hex input field
    Hex,
}

impl Channel {
    /// Move to next channel
    pub fn next(&self) -> Self {
        match self {
            Self::Primary => Self::Secondary,
            Self::Secondary => Self::Tertiary,
            Self::Tertiary => Self::Hex,
            Self::Hex => Self::Primary,
        }
    }

    /// Move to previous channel
    pub fn prev(&self) -> Self {
        match self {
            Self::Primary => Self::Hex,
            Self::Secondary => Self::Primary,
            Self::Tertiary => Self::Secondary,
            Self::Hex => Self::Tertiary,
        }
    }
}

/// Color picker widget state
#[derive(Debug, Clone)]
pub struct ColorPickerState {
    /// Display mode (HSL or RGB)
    mode: ColorPickerMode,

    /// HSL values (canonical storage)
    hsl: HSL,

    /// Currently focused channel
    focused_channel: Channel,

    /// Hex input buffer (for manual hex entry)
    hex_input: String,

    /// Whether hex input is currently being edited
    hex_editing: bool,
}

impl ColorPickerState {
    /// Create a new color picker from a Color
    pub fn from_color(color: Color, mode: ColorPickerMode) -> Self {
        let (r, g, b) = match color {
            Color::Rgb(r, g, b) => (r, g, b),
            _ => (0, 0, 0), // Fallback for non-RGB colors
        };

        let hsl = rgb_to_hsl(r, g, b);

        Self {
            mode,
            hsl,
            focused_channel: Channel::Primary,
            hex_input: format!("{:02x}{:02x}{:02x}", r, g, b),
            hex_editing: false,
        }
    }

    /// Get current display mode
    pub fn mode(&self) -> ColorPickerMode {
        self.mode
    }

    /// Get HSL values
    pub fn hsl(&self) -> HSL {
        self.hsl
    }

    /// Get RGB values
    pub fn rgb(&self) -> (u8, u8, u8) {
        hsl_to_rgb(self.hsl)
    }

    /// Get current Color
    pub fn color(&self) -> Color {
        let (r, g, b) = self.rgb();
        Color::Rgb(r, g, b)
    }

    /// Get hex string (without # prefix)
    pub fn hex(&self) -> String {
        if self.hex_editing {
            self.hex_input.clone()
        } else {
            let (r, g, b) = self.rgb();
            format!("{:02x}{:02x}{:02x}", r, g, b)
        }
    }

    /// Get currently focused channel
    pub fn focused_channel(&self) -> Channel {
        self.focused_channel
    }

    /// Is hex input being edited?
    pub fn is_hex_editing(&self) -> bool {
        self.hex_editing
    }

    /// Toggle display mode
    pub fn toggle_mode(&mut self) {
        self.mode = self.mode.toggle();
    }

    /// Handle keyboard input
    ///
    /// Returns true if the value changed
    pub fn handle_key(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Tab => {
                self.focused_channel = self.focused_channel.next();
                if self.focused_channel == Channel::Hex {
                    self.start_hex_edit();
                } else if self.hex_editing {
                    self.finish_hex_edit();
                }
                false
            }
            KeyCode::BackTab => {
                self.focused_channel = self.focused_channel.prev();
                if self.focused_channel == Channel::Hex {
                    self.start_hex_edit();
                } else if self.hex_editing {
                    self.finish_hex_edit();
                }
                false
            }
            KeyCode::Char('m') | KeyCode::Char('M') if !self.hex_editing => {
                self.toggle_mode();
                false
            }
            _ if self.hex_editing => self.handle_hex_input(key),
            KeyCode::Up | KeyCode::Right => {
                self.adjust_channel(1);
                true
            }
            KeyCode::Down | KeyCode::Left => {
                self.adjust_channel(-1);
                true
            }
            _ => false,
        }
    }

    /// Adjust the currently focused channel by delta
    fn adjust_channel(&mut self, delta: i32) {
        match self.mode {
            ColorPickerMode::HSL => match self.focused_channel {
                Channel::Primary => {
                    let new_h = (self.hsl.h + delta as f32).rem_euclid(360.0);
                    self.hsl.h = new_h;
                }
                Channel::Secondary => {
                    self.hsl.s = (self.hsl.s + delta as f32).clamp(0.0, 100.0);
                }
                Channel::Tertiary => {
                    self.hsl.l = (self.hsl.l + delta as f32).clamp(0.0, 100.0);
                }
                Channel::Hex => {}
            },
            ColorPickerMode::RGB => {
                let (mut r, mut g, mut b) = self.rgb();
                match self.focused_channel {
                    Channel::Primary => {
                        r = (r as i32 + delta).clamp(0, 255) as u8;
                    }
                    Channel::Secondary => {
                        g = (g as i32 + delta).clamp(0, 255) as u8;
                    }
                    Channel::Tertiary => {
                        b = (b as i32 + delta).clamp(0, 255) as u8;
                    }
                    Channel::Hex => {}
                }
                self.hsl = rgb_to_hsl(r, g, b);
            }
        }
    }

    /// Start editing hex input
    fn start_hex_edit(&mut self) {
        self.hex_editing = true;
        let (r, g, b) = self.rgb();
        self.hex_input = format!("{:02x}{:02x}{:02x}", r, g, b);
    }

    /// Finish editing hex input and apply if valid
    fn finish_hex_edit(&mut self) {
        self.hex_editing = false;

        // Try to parse hex input
        if self.hex_input.len() == 6 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&self.hex_input[0..2], 16),
                u8::from_str_radix(&self.hex_input[2..4], 16),
                u8::from_str_radix(&self.hex_input[4..6], 16),
            ) {
                self.hsl = rgb_to_hsl(r, g, b);
            }
        }
    }

    /// Handle hex input editing
    ///
    /// Returns true if value changed
    fn handle_hex_input(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Char(c) if c.is_ascii_hexdigit() && self.hex_input.len() < 6 => {
                self.hex_input.push(c.to_ascii_lowercase());
                true
            }
            KeyCode::Backspace if !self.hex_input.is_empty() => {
                self.hex_input.pop();
                true
            }
            KeyCode::Enter => {
                self.finish_hex_edit();
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_color() {
        let color = Color::Rgb(180, 190, 254);
        let state = ColorPickerState::from_color(color, ColorPickerMode::HSL);

        let (r, g, b) = state.rgb();
        assert_eq!((r, g, b), (180, 190, 254));
    }

    #[test]
    fn test_toggle_mode() {
        let mut state = ColorPickerState::from_color(Color::Rgb(255, 0, 0), ColorPickerMode::HSL);
        assert_eq!(state.mode(), ColorPickerMode::HSL);

        state.toggle_mode();
        assert_eq!(state.mode(), ColorPickerMode::RGB);

        state.toggle_mode();
        assert_eq!(state.mode(), ColorPickerMode::HSL);
    }

    #[test]
    fn test_channel_navigation() {
        let mut state = ColorPickerState::from_color(Color::Rgb(255, 0, 0), ColorPickerMode::HSL);

        assert_eq!(state.focused_channel(), Channel::Primary);

        state.handle_key(KeyCode::Tab);
        assert_eq!(state.focused_channel(), Channel::Secondary);

        state.handle_key(KeyCode::Tab);
        assert_eq!(state.focused_channel(), Channel::Tertiary);

        state.handle_key(KeyCode::Tab);
        assert_eq!(state.focused_channel(), Channel::Hex);

        state.handle_key(KeyCode::Tab);
        assert_eq!(state.focused_channel(), Channel::Primary);
    }

    #[test]
    fn test_adjust_hsl() {
        let mut state = ColorPickerState::from_color(
            Color::Rgb(255, 0, 0), // Pure red: H=0, S=100, L=50
            ColorPickerMode::HSL,
        );

        // Adjust hue
        state.focused_channel = Channel::Primary;
        state.adjust_channel(10);
        assert!((state.hsl().h - 10.0).abs() < 1.0);

        // Adjust saturation
        state.focused_channel = Channel::Secondary;
        state.adjust_channel(-10);
        assert!((state.hsl().s - 90.0).abs() < 1.0);
    }
}
