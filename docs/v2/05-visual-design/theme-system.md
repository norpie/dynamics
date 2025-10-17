# Theme System

**Prerequisites:** [Color System](color-system.md)

## Overview

V2 separates visual customization into **two orthogonal concerns:**

- **Theme** = Color palette (what things look like)
- **StyleConfig** = Visual behavior (how things are rendered)

These can be mixed independently - swap themes while keeping style, or vice versa.

## Theme Structure (~26 Semantic Colors)

Colors named by **purpose**, not hierarchy:

```rust
pub struct Theme {
    // === INTERACTION STATES ===
    pub focus: Color,         // Focused element border/highlight
    pub selection: Color,     // Selected item background
    pub selection_fg: Color,  // Selected item text
    pub hover: Color,         // Hovered element
    pub active: Color,        // Pressed/active state
    pub disabled: Color,      // Disabled elements

    // === SEMANTIC FEEDBACK ===
    pub error: Color,         // Error messages
    pub warning: Color,       // Warnings
    pub success: Color,       // Success confirmations
    pub info: Color,          // Informational messages

    // === CONTENT ===
    pub text: Color,          // Primary readable text
    pub text_dim: Color,      // Secondary text
    pub text_muted: Color,    // Hints, placeholders
    pub text_disabled: Color, // Disabled state text
    pub link: Color,          // Clickable links
    pub link_hover: Color,    // Hovered link

    // === STRUCTURE ===
    pub border: Color,        // Primary borders
    pub border_dim: Color,    // Secondary borders
    pub separator: Color,     // Explicit dividers
    pub bg_base: Color,       // Main background
    pub bg_surface: Color,    // Elevated surface (lists, inputs)
    pub bg_elevated: Color,   // Modals, floating panels
    pub bg_panel: Color,      // Panel backgrounds

    // === ACCENTS ===
    pub accent_1: Color,      // Generic highlight 1
    pub accent_2: Color,      // Generic highlight 2
    pub accent_3: Color,      // Generic highlight 3
}
```

**Default variants:**
- **Mocha** - Dark theme (Catppuccin-inspired)
- **Latte** - Light theme (Catppuccin-inspired)

Users can create custom themes via settings app.

## StyleConfig (Visual Behavior)

**Non-color customization:**

```rust
pub struct StyleConfig {
    pub borders: BorderStyle,
    pub list_selection: SelectionStyle,
    pub tree_selection: SelectionStyle,
    pub focus_indicator: FocusStyle,
    pub tree_expansion: TreeExpansionStyle,
    pub cursor_shape: CursorShape,
    pub scrollbar_style: ScrollbarStyle,
    pub spinner_style: SpinnerStyle,
    pub panel_padding: u16,
    pub default_spacing: u16,
    pub animations_enabled: bool,
}
```

### Border Styles

```rust
pub enum BorderStyle {
    Default,      // ─│┌┐└┘├┤┬┴ (box drawing)
    Rounded,      // ─│╭╮╰╯├┤┬┴ (rounded corners)
    Double,       // ═║╔╗╚╝╠╣╦╩ (double lines)
    Thick,        // ━┃┏┓┗┛┣┫┳┻ (heavy lines)
    Ascii,        // -|++++++ (ASCII fallback)
    Custom { /* ... */ },
}
```

### Selection Indicators

```rust
pub enum SelectionStyle {
    Highlight,                    // Background color only
    Prefix { char: String },      // "▶ " or "• " prefix
    Both { char: String },        // Prefix + background
    Border,                       // Border around item
    Underline,                    // Underline selected
}
```

### Focus Indicators

```rust
pub enum FocusStyle {
    Border,                       // Colored border
    BorderThick,                  // Thicker/double border
    Background,                   // Background color change
    Underline,                    // Underline only
    Prefix { char: String },      // Prefix indicator
}
```

### Tree Expansion

```rust
pub enum TreeExpansionStyle {
    Arrows,        // ▶ / ▼
    PlusMinus,     // + / -
    Chevrons,      // » / ˅
    Custom { collapsed: String, expanded: String },
}
```

### Cursor Shapes

```rust
pub enum CursorShape {
    Block,         // █
    Underline,     // _
    Bar,           // |
}
```

### Scrollbar Styles

```rust
pub struct ScrollbarStyle {
    pub position: ScrollbarPosition,  // Right/Left
    pub track_char: char,
    pub thumb_char: char,
    pub visible: ScrollbarVisibility,  // Always/WhenScrolling/Never
}
```

### Spinner Styles

```rust
pub enum SpinnerStyle {
    Dots,          // ⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏
    Line,          // -\|/
    Arrow,         // ←↖↑↗→↘↓↙
    Custom { frames: Vec<String> },
}
```

## Usage Examples

**Colors from theme:**

```rust
// Direct color access
let style = Style::default()
    .fg(theme.text)
    .bg(theme.selection);

// Color manipulation
let dimmed = theme.focus.dim(0.7);
let faded = theme.error.fade(&theme.bg_base, 0.3);

// Helper methods
theme.error_style();    // Style with error color
theme.success_style();  // Style with success color
```

**Visual behavior from style config:**

```rust
// Selection rendering
match ctx.style.list_selection {
    SelectionStyle::Prefix { char } => {
        text = format!("{} {}", char, text);
        style = style.bg(theme.selection);
    }
    SelectionStyle::Highlight => {
        style = style.bg(theme.selection);
    }
    // ...
}
```

## Persistence

**Stored in SQLite database:**
- `theme.active` = "mocha" / "latte" / custom name
- `theme.{name}.{color}` = OKLCH values (L, C, H)
- `style.borders` = "default" / "rounded" / etc.
- `style.list_selection` = "highlight" / "prefix:▶ " / etc.

**Loading:**
```rust
RuntimeConfig::load_from_options().await  // From database
Theme::default()                          // Mocha defaults
StyleConfig::default()                    // Default visual behavior
```

## Runtime Switching

**Settings app provides:**
- Theme selector dropdown
- Live color preview (grid of all colors)
- Per-color editor with OKLCH sliders
- Style config editor (dropdowns/toggles)
- Create/delete/edit themes
- Import/export themes

**Changes apply immediately** - no restart required.

## Migration from V1

**Renamed colors:**
```
accent_primary    → focus
accent_secondary  → link
accent_tertiary   → accent_1
text_primary      → text
text_secondary    → text_dim
text_tertiary     → text_muted
border_primary    → border
border_secondary  → border_dim
palette_1-4       → accent_1-3 (reduced from 4 to 3)
```

**New colors:**
- `selection` / `selection_fg` - Separate from focus
- `hover` - Explicit hover state
- `active` - Pressed/active state
- `disabled` / `text_disabled` - Disabled states
- `link_hover` - Hovered link state
- `bg_panel` - Panel-specific background

**New concept:**
- `StyleConfig` - All non-color customization

## Benefits

✅ **Semantic naming** - Colors named by purpose
✅ **Orthogonal concerns** - Theme and style independent
✅ **OKLCH manipulation** - Perceptually uniform color adjustments
✅ **User customizable** - Full editor with live preview
✅ **Hot reload** - Changes apply immediately
✅ **Persistent** - Stored in database

**See Also:**
- [Color System](color-system.md) - OKLCH color manipulation
- [Focus System](../04-user-interaction/focus.md) - Focus indicators
- [Components](../02-building-ui/components.md) - Component styling

---

**Next:** Explore [Color System](color-system.md) for OKLCH details.
