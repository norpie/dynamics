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

**See Also:**
- [Theme System](theme-system.md) - Color palette organization
- [Animation](../06-system-features/animation.md) - Color interpolation for animations

---

**Next:** Explore [Theme System](theme-system.md) for semantic color organization.
