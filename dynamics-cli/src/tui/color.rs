//! Color conversion utilities for theme customization
//!
//! Provides conversion between RGB, HSL, and hex color formats.

use anyhow::{Context, Result};
use ratatui::style::Color;

/// HSL color representation
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HSL {
    /// Hue: 0.0-360.0 degrees
    pub h: f32,
    /// Saturation: 0.0-100.0 percent
    pub s: f32,
    /// Lightness: 0.0-100.0 percent
    pub l: f32,
}

impl HSL {
    /// Create a new HSL color
    pub fn new(h: f32, s: f32, l: f32) -> Self {
        Self {
            h: h.clamp(0.0, 360.0),
            s: s.clamp(0.0, 100.0),
            l: l.clamp(0.0, 100.0),
        }
    }

    /// Convert to RGB
    pub fn to_rgb(&self) -> (u8, u8, u8) {
        hsl_to_rgb(*self)
    }
}

/// Convert HSL to RGB
///
/// Based on: https://en.wikipedia.org/wiki/HSL_and_HSV#HSL_to_RGB
pub fn hsl_to_rgb(hsl: HSL) -> (u8, u8, u8) {
    let h = hsl.h / 360.0; // Normalize to 0-1
    let s = hsl.s / 100.0; // Normalize to 0-1
    let l = hsl.l / 100.0; // Normalize to 0-1

    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r_prime, g_prime, b_prime) = match (h * 6.0) as u8 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        5 => (c, 0.0, x),
        _ => (c, 0.0, x), // Fallback for h=360
    };

    let r = ((r_prime + m) * 255.0).round() as u8;
    let g = ((g_prime + m) * 255.0).round() as u8;
    let b = ((b_prime + m) * 255.0).round() as u8;

    (r, g, b)
}

/// Convert RGB to HSL
///
/// Based on: https://en.wikipedia.org/wiki/HSL_and_HSV#From_RGB
pub fn rgb_to_hsl(r: u8, g: u8, b: u8) -> HSL {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    // Lightness
    let l = (max + min) / 2.0;

    // Saturation
    let s = if delta == 0.0 {
        0.0
    } else {
        delta / (1.0 - (2.0 * l - 1.0).abs())
    };

    // Hue
    let h = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };

    // Normalize hue to 0-360
    let h = if h < 0.0 { h + 360.0 } else { h };

    HSL {
        h,
        s: s * 100.0,
        l: l * 100.0,
    }
}

/// Parse hex color string to RGB Color
///
/// Accepts formats: "#RRGGBB" or "RRGGBB"
pub fn hex_to_color(hex: &str) -> Result<Color> {
    let hex = hex.trim_start_matches('#');

    if hex.len() != 6 {
        anyhow::bail!(
            "Invalid hex color format: expected 6 characters, got {}",
            hex.len()
        );
    }

    let r = u8::from_str_radix(&hex[0..2], 16).context("Failed to parse red component")?;
    let g = u8::from_str_radix(&hex[2..4], 16).context("Failed to parse green component")?;
    let b = u8::from_str_radix(&hex[4..6], 16).context("Failed to parse blue component")?;

    Ok(Color::Rgb(r, g, b))
}

/// Convert RGB Color to hex string
///
/// Returns format: "#RRGGBB"
pub fn color_to_hex(color: Color) -> String {
    match color {
        Color::Rgb(r, g, b) => format!("#{:02x}{:02x}{:02x}", r, g, b),
        _ => "#000000".to_string(), // Fallback for non-RGB colors
    }
}

/// Extract RGB components from Color
pub fn color_to_rgb(color: Color) -> Option<(u8, u8, u8)> {
    match color {
        Color::Rgb(r, g, b) => Some((r, g, b)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hsl_to_rgb_red() {
        let hsl = HSL::new(0.0, 100.0, 50.0);
        let (r, g, b) = hsl_to_rgb(hsl);
        assert_eq!((r, g, b), (255, 0, 0));
    }

    #[test]
    fn test_hsl_to_rgb_green() {
        let hsl = HSL::new(120.0, 100.0, 50.0);
        let (r, g, b) = hsl_to_rgb(hsl);
        assert_eq!((r, g, b), (0, 255, 0));
    }

    #[test]
    fn test_hsl_to_rgb_blue() {
        let hsl = HSL::new(240.0, 100.0, 50.0);
        let (r, g, b) = hsl_to_rgb(hsl);
        assert_eq!((r, g, b), (0, 0, 255));
    }

    #[test]
    fn test_hsl_to_rgb_gray() {
        let hsl = HSL::new(0.0, 0.0, 50.0);
        let (r, g, b) = hsl_to_rgb(hsl);
        assert_eq!((r, g, b), (128, 128, 128));
    }

    #[test]
    fn test_rgb_to_hsl_red() {
        let hsl = rgb_to_hsl(255, 0, 0);
        assert!((hsl.h - 0.0).abs() < 0.1);
        assert!((hsl.s - 100.0).abs() < 0.1);
        assert!((hsl.l - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_rgb_to_hsl_green() {
        let hsl = rgb_to_hsl(0, 255, 0);
        assert!((hsl.h - 120.0).abs() < 0.1);
        assert!((hsl.s - 100.0).abs() < 0.1);
        assert!((hsl.l - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_rgb_to_hsl_blue() {
        let hsl = rgb_to_hsl(0, 0, 255);
        assert!((hsl.h - 240.0).abs() < 0.1);
        assert!((hsl.s - 100.0).abs() < 0.1);
        assert!((hsl.l - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_rgb_to_hsl_gray() {
        let hsl = rgb_to_hsl(128, 128, 128);
        assert!((hsl.s - 0.0).abs() < 0.1);
        assert!((hsl.l - 50.2).abs() < 0.5); // Allow small rounding error
    }

    #[test]
    fn test_hex_to_color() {
        let color = hex_to_color("#b4befe").unwrap();
        assert_eq!(color, Color::Rgb(0xb4, 0xbe, 0xfe));
    }

    #[test]
    fn test_hex_to_color_without_hash() {
        let color = hex_to_color("b4befe").unwrap();
        assert_eq!(color, Color::Rgb(0xb4, 0xbe, 0xfe));
    }

    #[test]
    fn test_color_to_hex() {
        let hex = color_to_hex(Color::Rgb(0xb4, 0xbe, 0xfe));
        assert_eq!(hex, "#b4befe");
    }

    #[test]
    fn test_roundtrip_hsl_rgb() {
        let original = (180, 190, 254);
        let hsl = rgb_to_hsl(original.0, original.1, original.2);
        let (r, g, b) = hsl_to_rgb(hsl);

        // Allow small rounding errors
        assert!((r as i16 - original.0 as i16).abs() <= 1);
        assert!((g as i16 - original.1 as i16).abs() <= 1);
        assert!((b as i16 - original.2 as i16).abs() <= 1);
    }

    #[test]
    fn test_roundtrip_hex() {
        let original = "#b4befe";
        let color = hex_to_color(original).unwrap();
        let hex = color_to_hex(color);
        assert_eq!(hex, original);
    }
}
