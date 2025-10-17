# Layer System (Simple Stack)

**Prerequisites:** [App & Context API](../01-fundamentals/app-and-context.md)

## Overview

V2 replaces hardcoded layer types (GlobalUI, AppModal, etc.) with a simple stack. **No enum, no hardcoded types - just a stack with metadata.**

```rust
struct Layer {
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

## Multi-Layer Example

Apps return `Vec<Layer>` from `update()`:

```rust
fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
    let mut layers = vec![
        // Base UI
        Layer::fill(self.main_ui()),
    ];

    // Confirmation modal (if showing)
    if self.show_confirm {
        layers.push(
            Layer::centered(50, 15, panel("Confirm?", |ui| {
                ui.text("Are you sure?");
                ui.button("Yes").on_click(Self::handle_confirm);
                ui.button("No").on_click(Self::handle_cancel);
            }))
            .dim_below(true)
            .blocks_input(true)
        );
    }

    // Tooltip (always on top, doesn't block input)
    if let Some(tooltip) = &self.tooltip {
        layers.push(
            Layer::at(self.mouse_pos, text(tooltip))
        );
    }

    layers
}
```

**Layer composition:**
- **Stacking order** - Later layers render on top
- **dim_below** - Dims all layers below this one
- **blocks_input** - Prevents input to layers below
- **Flexible positioning** - Fill, centered, anchored, docked

## Global UI = Just Another Layer

Instead of hardcoded `GlobalUI`, the runtime provides header/footer via system layers:

```rust
// Runtime automatically prepends/appends system layers
fn render(&mut self) {
    let mut all_layers = vec![];

    // System header (unless app opts out)
    if !app.layout_mode().is_fullscreen() {
        all_layers.push(Layer::dock_top(3, self.render_header()));
    }

    // Get app's layers
    all_layers.extend(self.active_app.update());

    // System help modal (if F1 pressed)
    if self.showing_help {
        all_layers.push(
            Layer::centered(80, 30, self.render_help())
                .dim_below(true)
                .blocks_input(true)
        );
    }

    self.renderer.render(&all_layers);
}
```

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

---

**Next:** Learn about [Layout](layout.md) or explore [Modals](modals.md).
