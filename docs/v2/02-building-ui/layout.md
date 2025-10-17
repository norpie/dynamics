# Layout System

**Prerequisites:** [Layers](layers.md)

## Layout Primitives

Four core layout primitives:

```rust
// Horizontal layout
ui.row(|ui| {
    ui.button("A");
    ui.button("B");
});

// Vertical layout
ui.col(|ui| {
    ui.text("Header");
    ui.button("Action");
});

// Panel with border and title
ui.panel("Settings", |ui| {
    ui.text("Content");
});

// Container with padding only (no border)
ui.container(|ui| {
    ui.text("Padded content");
});
```

**Omissions:**
- ❌ **stack** - Use layer system for overlays
- ❌ **grid** - Compose using row/col
- ❌ **scroll container** - Use Scrollable component

## Constraint System

Four constraint types for sizing:

```rust
pub enum Constraint {
    Length(u16),      // Exactly N chars/lines
    Percentage(u16),  // N% of parent space (0-100)
    Fill(u16),        // Proportional weight
    Min(u16),         // At least N chars/lines
}
```

**Usage:**
```rust
ui.row(|ui| {
    ui.button("Fixed").width(Length(20));
    ui.button("25%").width(Percentage(25));
    ui.button("Fill 2x").width(Fill(2));
    ui.button("Fill 1x").width(Fill(1));  // Half as much as Fill(2)
    ui.button("Min 10").width(Min(10));
});
```

## Auto-Constraints (Smart Defaults)

**Elements auto-size to content by default:**

```rust
// Auto-sizes to label + padding
ui.button("Save");

// Override to fill available space
ui.button("Save").width(Fill(1));

// Override to exact size
ui.button("Save").width(Length(30));
```

**Default behavior per element:**
- **Button** - `Length(label.len() + padding)`
- **Text** - `Length(text.len())`
- **TextInput** - `Fill(1)` (wants to expand)
- **List** - `Fill(1)` for both width and height
- **Panel** - Auto-size to content + 2 lines for border

## Nesting Behavior

**Parent is truth** - child cannot exceed parent's allocated space:

```rust
ui.col(|ui| {
    ui.row(|ui| {
        ui.button("Wide").width(Length(100));  // Requests 100 chars
    }).width(Length(50));  // Row only has 50 chars available

    // Button gets 50 chars, not 100 (parent clips)
});
```

**Constraint inheritance:**
- Children inherit parent's available space as maximum
- Children can request less than parent's space
- Children **cannot** exceed parent's space
- Parent does NOT inherit child's constraints upward

## Alignment System

Alignment is **separate from constraints** - two orthogonal concerns.

### 1. Manual Fill Spacers

Explicit flexible space for manual control:

```rust
ui.row(|ui| {
    ui.button("Left");
    ui.fill();  // Flexible spacer
    ui.button("Right");
});

// Center an element
ui.row(|ui| {
    ui.fill();
    ui.button("Center");
    ui.fill();
});
```

### 2. Parent-Level Alignment

Align all children as a group:

```rust
ui.row()
    .justify(Justify::Center)       // Main axis (horizontal)
    .align(Align::Center)           // Cross axis (vertical)
    .children(|ui| {
        ui.button("A");
        ui.button("B");
    });
```

**Justify options** (main axis):
```rust
enum Justify {
    Start,         // Pack to start
    End,           // Pack to end
    Center,        // Pack to center
    SpaceBetween,  // First at start, last at end, equal gaps
    SpaceAround,   // Equal space around each element
    SpaceEvenly,   // Equal space between all (including edges)
}
```

**Align options** (cross axis):
```rust
enum Align {
    Start,    // Align to top (row) or left (col)
    End,      // Align to bottom (row) or right (col)
    Center,   // Center
    Stretch,  // Fill cross axis
}
```

### 3. Child-Level Alignment (Override)

Individual child overrides parent alignment:

```rust
ui.row()
    .align(Align::Center)  // Default: center all
    .children(|ui| {
        ui.button("Top").align_self(AlignSelf::Start);
        ui.button("Center");  // Uses parent default
        ui.button("Bottom").align_self(AlignSelf::End);
    });
```

## Container Features

### Panel

Border + title + content:

```rust
ui.panel("Settings", |ui| {
    ui.text("Content");
})
.width(Length(40))
.height(Length(20));

// Or auto-size to content + borders
ui.panel("Auto", |ui| {
    ui.text("Sized to fit");
});  // Height = content + 2 (borders)
```

### Container

Padding only, no border:

```rust
ui.container(|ui| {
    ui.text("Padded content");
})
.padding(1)      // All sides
.padding_x(2)    // Left/right
.padding_y(1);   // Top/bottom
```

## Layer Positioning

Layers use `LayerArea` for positioning (separate from internal layout):

```rust
enum LayerArea {
    Fill,                      // Use all available space
    Centered(u16, u16),        // Width, height - centered
    Rect(Rect),                // Explicit position
    Anchor(Anchor, u16, u16),  // Anchor point + size
    DockTop(u16),              // Reserve N lines at top
    DockBottom(u16),           // Reserve N lines at bottom
    DockLeft(u16),             // Reserve N columns at left
    DockRight(u16),            // Reserve N columns at right
}
```

**Usage:**
```rust
fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
    vec![
        Layer::fill(self.main_ui()),
        Layer::centered(60, 20, panel("Modal", |ui| { /* ... */ })),
        Layer::dock_top(3, panel("Header", |ui| { /* ... */ })),
    ]
}
```

## No Layout Macros

**Decision:** No layout macros in V2.

Closure-based API is clean enough in immediate mode:

```rust
// Closure style (V2)
ui.col(|ui| {
    ui.text("Header");
    ui.row(|ui| {
        ui.button("A");
        ui.button("B");
    });
});
```

**Reasoning:**
- Closures already provide clean nesting
- No Msg enum = no need to reduce boilerplate
- Macros add complexity for minimal benefit
- IDE autocomplete works better with methods

## Benefits

✅ **Smart defaults** - Elements auto-size to content
✅ **Flexible constraints** - Length, Percentage, Fill, Min
✅ **Orthogonal alignment** - Constraints separate from alignment
✅ **Manual control** - Fill spacers for explicit positioning
✅ **Clean API** - Closures instead of macros

**See Also:**
- [Layers](layers.md) - Layer positioning system
- [Components](components.md) - Component composition
- [Modals](modals.md) - Modal positioning patterns

---

**Next:** Explore [Components](components.md) or learn about [Modals](modals.md).
