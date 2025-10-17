# Container Features, Alignment & Constraints

**Prerequisites:**
- [Layout](../02-building-ui/layout.md) - Basic layout primitives

This reference covers the advanced container features, alignment system, and constraint types used throughout the framework.

---

## Container Features

V2 provides three container types for specialized layout needs:

### Panel

Title bar with borders and auto-sizing:

```rust
Panel::new()
    .title("Entity List")
    .width(Length(40))      // Fixed width
    .height(Min(10))        // Minimum height
    .child(list_element)
    .build()
```

**Features:**
- Automatic border drawing
- Title rendering in top border
- Auto-sizing: adds +2 lines for borders
- Optional explicit width/height

### Container

Configurable padding around child elements:

```rust
Container::new()
    .padding(Padding::all(2))       // 2 units all sides
    .padding(Padding::horizontal(3)) // 3 units left/right
    .padding(Padding::vertical(1))   // 1 unit top/bottom
    .child(content)
    .build()
```

**Padding options:**
- `Padding::all(n)` - Same padding all sides
- `Padding::horizontal(n)` - Left/right padding
- `Padding::vertical(n)` - Top/bottom padding
- `Padding::new(top, right, bottom, left)` - Individual sides

### Stack

Multiple layers with alignment (see [Layers](../02-building-ui/layers.md)):

```rust
Stack::new()
    .layer(background_element, LayerAlignment::Fill)
    .layer(overlay_element, LayerAlignment::Center)
    .dim_below(true)  // Dim lower layers
    .build()
```

**Features:**
- Multiple visual layers in single element
- Per-layer alignment
- Optional dimming of lower layers
- See [Layer System](../02-building-ui/layers.md) for app-level layers

---

## Alignment & Positioning

V2 provides 9 alignment options for positioning elements within containers:

```rust
pub enum LayerAlignment {
    Center,          // Center both axes
    TopLeft,         // Top-left corner
    TopCenter,       // Top edge, centered horizontally
    TopRight,        // Top-right corner
    BottomLeft,      // Bottom-left corner
    BottomCenter,    // Bottom edge, centered horizontally
    BottomRight,     // Bottom-right corner
    LeftCenter,      // Left edge, centered vertically
    RightCenter,     // Right edge, centered vertically
    Fill,            // Expand to fill entire area
}
```

**Usage contexts:**
- **Stack layers** - Position elements within stack
- **Modal positioning** - Center dialogs, position context menus
- **Container children** - Align content within padded containers

**Alignment strategies:**
Three ways to align elements (see [Layout](../02-building-ui/layout.md#alignment-system)):
1. **Manual Fill Spacers** - `col![spacer!(), content, spacer!()]`
2. **Parent-Level Alignment** - `.align(LayerAlignment::Center)`
3. **Child-Level Alignment** - Override parent alignment

**Dim below:**
When stacking visual layers, the `dim_below` option reduces opacity of lower layers to emphasize focused content:

```rust
Stack::new()
    .layer(app_content, LayerAlignment::Fill)
    .layer(modal, LayerAlignment::Center)
    .dim_below(true)  // Dims app_content when modal is shown
    .build()
```

---

## Constraints System

Layout constraints control how elements request space during layout.

### Constraint Types

```rust
pub enum LayoutConstraint {
    /// Fixed size in characters/lines
    Length(u16),

    /// Minimum size (expands if space available)
    Min(u16),

    /// Proportional share of available space
    Fill(u16),  // Weight (higher = more space)
}
```

**Examples:**
```rust
// Fixed size - exactly 20 columns wide
Length(20)

// Minimum size - at least 5 lines, expands if possible
Min(5)

// Proportional - takes 2x space compared to Fill(1)
Fill(2)
```

### Auto-Constraints

Every element type has a sensible default constraint via `default_constraint()`:

```rust
// Element type      Default constraint
Button              Length(1 + label.len())
TextInput           Length(20)
List                Fill(1)
Panel               Fill(1)
Text                Min(1)
```

**Auto-constraint behavior:**
When you don't specify constraints explicitly, the layout system calls `default_constraint()` on each child element (see [Layout](../02-building-ui/layout.md#auto-constraints)).

### Macro Shorthand

For cleaner code, import constraint constructors with `use_constraints!()`:

```rust
use_constraints!();  // Imports Fill, Length, Min

col![
    panel => Fill(1),      // Instead of LayoutConstraint::Fill(1)
    button => Length(3),   // Instead of LayoutConstraint::Length(3)
    footer => Min(2),      // Instead of LayoutConstraint::Min(2)
]
```

**Without macro:**
```rust
col![
    panel => LayoutConstraint::Fill(1),
    button => LayoutConstraint::Length(3),
]
```

---

## See Also

- [Layout](../02-building-ui/layout.md) - Layout primitives and macros
- [Layers](../02-building-ui/layers.md) - App-level layer system
- [Modals](../02-building-ui/modals.md) - Modal positioning and alignment
