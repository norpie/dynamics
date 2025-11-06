# Color System (OKLCH)

**Prerequisites:** [Theme System](theme-system.md)

## Why OKLCH?

V2 uses **OKLCH color space** internally for all color manipulation:

- **Perceptually uniform** - 50% dimming looks like 50% to human eye (HSL doesn't)
- **Consistent saturation** - Red and blue at same chroma look equally vibrant
- **Better gradients** - No weird hue shifts when interpolating
- **Easy manipulation** - Brightness, saturation, hue are independent

## Color Type

```rust
#[derive(Clone, Copy)]
struct Color {
    l: f32,  // Lightness: 0.0 - 1.0
    c: f32,  // Chroma: 0.0 - 0.4 (practical max)
    h: f32,  // Hue: 0.0 - 360.0
}
```

## Color Manipulation

**Brightness adjustment:**
```rust
fn dim(&self, factor: f32) -> Self;

// Example
let dimmed = color.dim(0.5);  // 50% darker
```

**Fade toward background:**
```rust
fn fade(&self, background: &Color, alpha: f32) -> Self;

// Example
let faded = accent.fade(&theme.bg_base, 0.3);  // 30% opacity
```

**Saturation adjustment:**
```rust
fn with_chroma(&self, c: f32) -> Self;

// Example
let desaturated = color.with_chroma(0.1);
```

## Rendering

**Convert to terminal RGB:**
```rust
fn to_rgb(&self) -> RatatuiColor {
    let (r, g, b) = oklch_to_rgb(self.l, self.c, self.h);
    RatatuiColor::Rgb(r, g, b)
}
```

Conversion only happens at render time - all manipulation uses OKLCH internally.

## Theme Integration

Theme colors are defined in OKLCH:

```rust
struct Theme {
    bg_base: Color,      // L=0.2, C=0.02, H=240
    text_primary: Color, // L=0.9, C=0.02, H=240
    accent: Color,       // L=0.7, C=0.15, H=200
    // ...
}
```

Easy to generate variations:
```rust
let overlay = theme.bg_base.dim(0.5);    // Darker overlay
let hover = theme.accent.dim(1.2);       // Brighter hover
```

## Benefits

✅ **Perceptually accurate** - Color adjustments match human perception
✅ **Predictable** - Same chroma = same vibrancy across hues
✅ **Simple API** - dim/fade/with_chroma cover most use cases
✅ **No color shifts** - Gradients don't shift hue unexpectedly

## Implementation Details

### Conversion and Caching

**Conversion timing:** OKLCH → RGB conversion happens at **theme load time**, not render time.

```rust
impl Theme {
    pub fn load(config: &ThemeConfig) -> Self {
        let mut cache = ColorCache::new();

        // Convert all semantic colors once at load time
        Self {
            bg_base: cache.convert("bg_base", config.bg_base),
            text_primary: cache.convert("text_primary", config.text_primary),
            accent: cache.convert("accent", config.accent),
            // ...
        }
    }
}
```

**Cache keys:** Colors are cached using semantic palette names (`bg_base`, `text_primary`, etc.).

**Dynamic adjustments:** When calling `dim()`, `fade()`, or `with_chroma()`, the framework:
1. Performs OKLCH manipulation (cheap)
2. Checks cache for resulting color
3. Converts and caches if new

This ensures frequent color operations (hover states, dimming) are fast after first use.

### Terminal Compatibility

**Target environment:** True-color (24-bit RGB) terminals, specifically Windows Terminal and modern terminal emulators.

**Fallback strategy:** Best-effort rendering on limited terminals:
- 256-color terminals: Map to nearest color (ratatui handles this)
- 16-color terminals: Framework doesn't actively support, but won't crash

**Justification:** Primary user base uses Windows Terminal with true-color support. Adding complex fallback logic for legacy terminals provides minimal value.

### Color Picker

**User configuration:** Themes are fully customizable via TUI color picker.

**Implementation:** Adapt V1's theme/color picker implementation:
- Visual color selection
- Live preview
- Hex code input support
- OKLCH value display (optional, for advanced users)

**See [Theme System](theme-system.md) for theme customization details.**

### Accessibility

**Framework position:** No built-in accessibility tooling (contrast checking, colorblind simulation, etc.).

**Rationale:**
- Themes are fully user-customizable
- Users can create accessible themes that work for their needs
- Framework provides functional, aesthetically pleasing default theme
- Adding accessibility validation would be scope creep

**User responsibility:** If accessibility is required, users can:
- Customize theme colors to meet their needs
- Use external tools to validate contrast ratios
- Create and share accessibility-focused theme presets

**See Also:**
- [Theme System](theme-system.md) - Color palette organization
- [Animation](../06-system-features/animation.md) - Color interpolation for animations

---

**Next:** Explore [Theme System](theme-system.md) for semantic color organization.
