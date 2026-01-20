// NEW Theme struct definition (21 colors - Option 1)
// Replace contents of dynamics-cli/src/tui/state/theme.rs with this

use ratatui::style::{Color, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeVariant {
    Mocha, // Dark theme (default)
    Latte, // Light theme
}

impl Default for ThemeVariant {
    fn default() -> Self {
        Self::Mocha
    }
}

#[derive(Debug, Clone)]
pub struct Theme {
    // Accent colors (8)
    pub accent_primary: Color, // Focus, selection, primary highlight (was: lavender)
    pub accent_secondary: Color, // Info, links, secondary actions (was: blue)
    pub accent_tertiary: Color, // Special emphasis, modal headers (was: mauve)
    pub accent_error: Color,   // Errors, failures, destructive actions (was: red)
    pub accent_warning: Color, // Warnings, cautions, pending (was: yellow)
    pub accent_success: Color, // Success, completion, validation (was: green)
    pub accent_info: Color,    // Informational messages (was: teal)
    pub accent_muted: Color,   // Labels, keys, subtle highlights (was: peach)

    // Text hierarchy (3)
    pub text_primary: Color,   // Main content (was: text)
    pub text_secondary: Color, // Less important content (was: subtext1)
    pub text_tertiary: Color,  // Labels, hints (was: subtext0)

    // UI structure (6)
    pub border_primary: Color,   // Main borders, separators (was: overlay1)
    pub border_secondary: Color, // Subtle borders, disabled state (was: overlay0)
    pub border_tertiary: Color,  // Very subtle, scrollbar track (was: overlay2)
    pub bg_base: Color,          // Main background (was: base)
    pub bg_surface: Color,       // Elevated surfaces, selection bg (was: surface0)
    pub bg_elevated: Color,      // Modals, floating elements (was: surface1)

    // Flexible palette (4)
    pub palette_1: Color, // User customization (was: rosewater)
    pub palette_2: Color, // User customization (was: flamingo)
    pub palette_3: Color, // User customization (was: pink)
    pub palette_4: Color, // User customization (was: sky)
}

impl Theme {
    pub fn new(variant: ThemeVariant) -> Self {
        match variant {
            ThemeVariant::Mocha => Self::mocha(),
            ThemeVariant::Latte => Self::latte(),
        }
    }

    pub fn mocha() -> Self {
        Self {
            // Accent colors (from Catppuccin Mocha)
            accent_primary: Color::Rgb(0xb4, 0xbe, 0xfe), // lavender
            accent_secondary: Color::Rgb(0x89, 0xb4, 0xfa), // blue
            accent_tertiary: Color::Rgb(0xcb, 0xa6, 0xf7), // mauve
            accent_error: Color::Rgb(0xf3, 0x8b, 0xa8),   // red
            accent_warning: Color::Rgb(0xf9, 0xe2, 0xaf), // yellow
            accent_success: Color::Rgb(0xa6, 0xe3, 0xa1), // green
            accent_info: Color::Rgb(0x94, 0xe2, 0xd5),    // teal
            accent_muted: Color::Rgb(0xfa, 0xb3, 0x87),   // peach

            // Text
            text_primary: Color::Rgb(0xcd, 0xd6, 0xf4), // text
            text_secondary: Color::Rgb(0xba, 0xc2, 0xde), // subtext1
            text_tertiary: Color::Rgb(0xa6, 0xad, 0xc8), // subtext0

            // UI structure
            border_primary: Color::Rgb(0x7f, 0x84, 0x9c), // overlay1
            border_secondary: Color::Rgb(0x6c, 0x70, 0x86), // overlay0
            border_tertiary: Color::Rgb(0x93, 0x99, 0xb2), // overlay2
            bg_base: Color::Rgb(0x1e, 0x1e, 0x2e),        // base
            bg_surface: Color::Rgb(0x31, 0x32, 0x44),     // surface0
            bg_elevated: Color::Rgb(0x45, 0x47, 0x5a),    // surface1

            // Flexible palette
            palette_1: Color::Rgb(0xf5, 0xe0, 0xdc), // rosewater
            palette_2: Color::Rgb(0xf2, 0xcd, 0xcd), // flamingo
            palette_3: Color::Rgb(0xf5, 0xc2, 0xe7), // pink
            palette_4: Color::Rgb(0x89, 0xdc, 0xeb), // sky
        }
    }

    pub fn latte() -> Self {
        Self {
            // Accent colors (from Catppuccin Latte)
            accent_primary: Color::Rgb(0x72, 0x87, 0xfd), // lavender
            accent_secondary: Color::Rgb(0x1e, 0x66, 0xf5), // blue
            accent_tertiary: Color::Rgb(0x88, 0x39, 0xef), // mauve
            accent_error: Color::Rgb(0xd2, 0x0f, 0x39),   // red
            accent_warning: Color::Rgb(0xdf, 0x8e, 0x1d), // yellow
            accent_success: Color::Rgb(0x40, 0xa0, 0x2b), // green
            accent_info: Color::Rgb(0x17, 0x92, 0x99),    // teal
            accent_muted: Color::Rgb(0xfe, 0x64, 0x0b),   // peach

            // Text
            text_primary: Color::Rgb(0x4c, 0x4f, 0x69), // text
            text_secondary: Color::Rgb(0x5c, 0x5f, 0x77), // subtext1
            text_tertiary: Color::Rgb(0x6c, 0x6f, 0x85), // subtext0

            // UI structure
            border_primary: Color::Rgb(0x8c, 0x8f, 0xa1), // overlay1
            border_secondary: Color::Rgb(0x9c, 0xa0, 0xb0), // overlay0
            border_tertiary: Color::Rgb(0x7c, 0x7f, 0x93), // overlay2
            bg_base: Color::Rgb(0xef, 0xf1, 0xf5),        // base
            bg_surface: Color::Rgb(0xcc, 0xd0, 0xda),     // surface0
            bg_elevated: Color::Rgb(0xbc, 0xc0, 0xcc),    // surface1

            // Flexible palette
            palette_1: Color::Rgb(0xdc, 0x8a, 0x78), // rosewater
            palette_2: Color::Rgb(0xdd, 0x78, 0x78), // flamingo
            palette_3: Color::Rgb(0xea, 0x76, 0xcb), // pink
            palette_4: Color::Rgb(0x04, 0xa5, 0xe5), // sky
        }
    }

    // Helper methods following semantic naming
    pub fn error_style(&self) -> Style {
        Style::default().fg(self.accent_error)
    }

    pub fn warning_style(&self) -> Style {
        Style::default().fg(self.accent_warning)
    }

    pub fn success_style(&self) -> Style {
        Style::default().fg(self.accent_success)
    }

    pub fn info_style(&self) -> Style {
        Style::default().fg(self.accent_info)
    }

    pub fn link_style(&self) -> Style {
        Style::default().fg(self.accent_secondary)
    }

    pub fn cursor_style(&self) -> Style {
        Style::default().bg(self.palette_1).fg(self.bg_base)
    }

    /// Get all color fields as (name, color) pairs for iteration
    pub fn colors(&self) -> Vec<(&'static str, Color)> {
        vec![
            // Accents
            ("accent_primary", self.accent_primary),
            ("accent_secondary", self.accent_secondary),
            ("accent_tertiary", self.accent_tertiary),
            ("accent_error", self.accent_error),
            ("accent_warning", self.accent_warning),
            ("accent_success", self.accent_success),
            ("accent_info", self.accent_info),
            ("accent_muted", self.accent_muted),
            // Text
            ("text_primary", self.text_primary),
            ("text_secondary", self.text_secondary),
            ("text_tertiary", self.text_tertiary),
            // Borders
            ("border_primary", self.border_primary),
            ("border_secondary", self.border_secondary),
            ("border_tertiary", self.border_tertiary),
            // Backgrounds
            ("bg_base", self.bg_base),
            ("bg_surface", self.bg_surface),
            ("bg_elevated", self.bg_elevated),
            // Palette
            ("palette_1", self.palette_1),
            ("palette_2", self.palette_2),
            ("palette_3", self.palette_3),
            ("palette_4", self.palette_4),
        ]
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::new(ThemeVariant::default())
    }
}

/// Color names in order (for iteration and registration)
pub const COLOR_NAMES: &[&str] = &[
    "accent_primary",
    "accent_secondary",
    "accent_tertiary",
    "accent_error",
    "accent_warning",
    "accent_success",
    "accent_info",
    "accent_muted",
    "text_primary",
    "text_secondary",
    "text_tertiary",
    "border_primary",
    "border_secondary",
    "border_tertiary",
    "bg_base",
    "bg_surface",
    "bg_elevated",
    "palette_1",
    "palette_2",
    "palette_3",
    "palette_4",
];
