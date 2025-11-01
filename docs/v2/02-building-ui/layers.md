# Layer System (Simple Stack)

**Prerequisites:** [App & Context API](../01-fundamentals/app-and-context.md)

## Overview

V2 replaces hardcoded layer types (GlobalUI, AppModal, etc.) with a simple stack. **No enum, no hardcoded types - just a stack with metadata.**

```rust
struct Layer {
    id: String,                // Full hierarchical path (composed by runtime)
    name: Option<String>,      // Display name for help/debug (defaults to id)
    element: Element,
    area: LayerArea,
    dim_below: bool,
    blocks_input: bool,
}

enum LayerArea {
    Fill,                      // Use all available space
    Centered(u16, u16),        // Width, height
    Rect(Rect),                // Explicit position
    Anchor(Anchor, u16, u16),  // TopLeft, BottomRight, etc.
    DockTop(u16),              // Reserve N lines at top
    DockBottom(u16),
    DockLeft(u16),
    DockRight(u16),
}
```

### Layer Positioning

Use `LayerArea` variants to position layers on screen:

```rust
fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
    vec![
        Layer::fill(self.main_ui()),
        Layer::centered(60, 20, panel("Modal", |ui| { /* ... */ })),
        Layer::dock_top(3, panel("Header", |ui| { /* ... */ })),
    ]
}
```

**Positioning options:**
- **Fill** - Use all available space (typical for main UI)
- **Centered(w, h)** - Fixed size, centered on screen (modals)
- **Rect(rect)** - Explicit position (tooltips, context menus)
- **Anchor(anchor, w, h)** - Position relative to screen edge
- **Dock** - Reserve space at screen edge (headers, footers)

## Hierarchical ID System

Each layer must have an ID. The ID is **hierarchically composed** from context:

**Runtime** → **App** → **Layer** = **Full Path**

```rust
// Runtime provides: ["apps", "migration"]
// Layer declares: "delete_modal"
// Composed result: "apps.migration.delete_modal"
```

### ID Requirements

Each layer must provide a single segment ID (no dots). Layer IDs follow the same validation rules as App IDs - see [App ID Requirements](../01-fundamentals/app-and-context.md#app-id-requirements).

### Display Names

Optionally provide a human-readable name for help menus and debug overlays:

```rust
Layer::centered(50, 15, modal)
    .id("delete_modal")
    .name("Delete Confirmation")  // Shows in help menu
```

**If omitted**, the name defaults to the ID segment.

## Multi-Layer Example

Apps return `Vec<Layer>` from `update()`:

```rust
impl App for MyApp {
    fn id() -> &'static str {
        "myapp"  // App provides its segment
    }

    fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
        // ctx.id_stack = ["apps", "myapp"]

        let mut layers = vec![
            // Base UI → "apps.myapp.main"
            Layer::fill(self.main_ui())
                .id("main"),
        ];

        // Confirmation modal (if showing)
        if self.show_confirm {
            layers.push(
                Layer::centered(50, 15, panel("Confirm?", |ui| {
                    ui.text("Are you sure?");
                    ui.button("Yes").on_click(Self::handle_confirm);
                    ui.button("No").on_click(Self::handle_cancel);
                }))
                .id("confirm_modal")  // → "apps.myapp.confirm_modal"
                .name("Confirmation")
                .dim_below(true)
                .blocks_input(true)
            );
        }

        // Tooltip (always on top, doesn't block input)
        if let Some(tooltip) = &self.tooltip {
            layers.push(
                Layer::at(self.mouse_pos, text(tooltip))
                    .id("tooltip")  // → "apps.myapp.tooltip"
            );
        }

        layers
    }
}
```

**Layer composition:**
- **Mandatory IDs** - Each layer declares its segment
- **Hierarchical paths** - Runtime composes full paths
- **Stacking order** - Later layers render on top
- **dim_below** - Dims all layers below this one
- **blocks_input** - Prevents input to layers below
- **Flexible positioning** - Fill, centered, anchored, docked

## System Layers = Same Stack

Instead of hardcoded `GlobalUI`, the runtime provides header/footer via system layers in the **same stack**:

```rust
// Runtime manages the complete layer stack
fn render(&mut self) {
    let mut all_layers = vec![];

    // System header (unless app opts out)
    // Context: ["system"]
    if !app.layout_mode().is_fullscreen() {
        self.ctx.id_stack = vec!["system".into()];
        all_layers.push(
            Layer::dock_top(3, self.render_header())
                .id("header")  // → "system.header"
        );
    }

    // App layers
    // Context: ["apps", "{app_id}"]
    self.ctx.id_stack = vec!["apps".into(), app.id().into()];
    all_layers.extend(self.active_app.update(&mut self.ctx));
    // → "apps.myapp.main", "apps.myapp.confirm_modal", etc.

    // System help modal (if F1 pressed)
    // Context: ["system"]
    if self.showing_help {
        self.ctx.id_stack = vec!["system".into()];
        all_layers.push(
            Layer::centered(80, 30, self.render_help())
                .id("help")  // → "system.help"
                .name("Help Menu")
                .dim_below(true)
                .blocks_input(true)
        );
    }

    self.renderer.render(&all_layers);
}
```

**Key insights:**
- System and app layers share the same stack
- Namespace separation via `"system.*"` vs `"apps.*"` prefixes
- Runtime controls context (`id_stack`) for each layer source
- Apps never know they're under `"apps"` - runtime decides

Apps can opt out of system layers:

```rust
impl App for FullscreenVideoPlayer {
    fn layout_mode() -> LayoutMode {
        LayoutMode::Fullscreen  // No header/footer
    }
}
```

## Widget Dimensions (No More Hacks!)

**V1 Problem:** Scrollable widgets need viewport dimensions, but we don't know until render. Solution was "20" fallback + `on_render` callback - 1-frame delay hack!

**V2 Solution:** Immediate mode - widgets get dimensions during render. No viewport params, no callbacks, no hardcoded fallbacks.

**See Also:**
- [Modals](modals.md) - Modal patterns using layers
- [Focus System](../04-user-interaction/focus.md) - Layer-scoped focus
- [Layout](layout.md) - Layout within layers
- [Plugin System](../06-system-features/plugin-system.md) - System plugins that inject layers

---

**Next:** Learn about [Layout](layout.md) or explore [Modals](modals.md).
