//! Formatting helpers for Excel export

use rust_xlsxwriter::*;

pub fn create_header_format() -> Format {
    Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0x4472C4))
        .set_font_color(Color::White)
}

pub fn create_title_format() -> Format {
    Format::new().set_bold().set_font_size(16)
}

pub fn create_exact_match_format() -> Format {
    Format::new().set_background_color(Color::RGB(0x90EE90)) // Light Green
}

pub fn create_manual_mapping_format() -> Format {
    Format::new().set_background_color(Color::RGB(0x87CEEB)) // Sky Blue
}

pub fn create_prefix_match_format() -> Format {
    Format::new().set_background_color(Color::RGB(0xFFE4B5)) // Moccasin/Light Orange
}

pub fn create_type_mismatch_format() -> Format {
    Format::new().set_background_color(Color::RGB(0xFFD700)) // Gold/Yellow
}

pub fn create_unmapped_format() -> Format {
    Format::new().set_background_color(Color::RGB(0xFFB6C1)) // Light Pink
}

pub fn create_required_format() -> Format {
    Format::new().set_background_color(Color::RGB(0xFFA07A)) // Light Salmon
}

pub fn create_relationship_format() -> Format {
    Format::new().set_background_color(Color::RGB(0xDDA0DD)) // Plum/Light Purple
}

pub fn create_custom_format() -> Format {
    Format::new()
        .set_background_color(Color::RGB(0x20B2AA)) // Light Sea Green
        .set_font_color(Color::White)
}

pub fn create_values_match_format() -> Format {
    Format::new().set_background_color(Color::RGB(0x90EE90)) // Light Green
}

pub fn create_values_differ_format() -> Format {
    Format::new().set_background_color(Color::RGB(0xFFB6C1)) // Light Pink
}

pub fn create_missing_data_format() -> Format {
    Format::new().set_background_color(Color::RGB(0xD3D3D3)) // Light Gray
}

pub fn create_example_value_format() -> Format {
    Format::new().set_background_color(Color::RGB(0xAFEEEE)) // Pale Turquoise/Cyan
}
