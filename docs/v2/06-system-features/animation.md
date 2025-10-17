# Animation System

**Prerequisites:** [Event Loop](../01-fundamentals/event-loop.md), [Color System](../05-visual-design/color-system.md)

## Terminal Constraints

Animations in terminals have specific limitations:
- Character grid (no sub-cell positioning)
- ~60fps max practical
- **But:** Smooth color interpolation works great!

## Frame Timing (Dynamic Mode Switching)

V2 uses **two rendering modes** that switch automatically:

### 1. Event-Driven (Default - 0% CPU)

- Runtime blocks waiting for events
- No rendering until event arrives
- Perfect for battery life
- App can idle for minutes at 0% CPU

### 2. Frame-Driven (Only When Animating)

- Runtime renders at 60fps
- Switches automatically when animations active
- Back to event-driven when animations complete
- ~1-2% CPU during animation

```rust
impl Runtime {
    async fn run(&mut self) {
        loop {
            let animating = self.has_active_animations();

            if animating {
                // FRAME-DRIVEN: 60fps
                tokio::select! {
                    Some(event) = self.events.recv() => { /* ... */ }
                    _ = tokio::time::sleep(Duration::from_millis(16)) => { /* ... */ }
                }
                self.render();
            } else {
                // EVENT-DRIVEN: Block until event (0% CPU)
                let event = self.events.recv().await;
                self.handle_event(event);
                self.render();
            }
        }
    }
}
```

**Automatic switching** - no user intervention needed.

## Toast System (Global)

**Framework-managed** - apps just call methods, runtime handles rendering and animation.

### Toast API

```rust
// Simple usage
ctx.toast.info("Loading...");
ctx.toast.success("Done!");
ctx.toast.warning("Check this");
ctx.toast.error("Failed!");

// Custom duration
ctx.toast.info("Quick message").duration(Duration::from_secs(1));

// With action button
ctx.toast.info("File saved").action("Undo", Self::handle_undo);
```

### Toast Animation States

```rust
enum ToastState {
    Entering,   // Sliding in from right
    Visible,    // Fully visible
    Exiting,    // Fading out
}
```

Toasts slide in, stay visible, then fade out with color interpolation.

### System-Managed Layer

Runtime automatically adds toast layer - apps never manage it:

```rust
impl Runtime {
    fn render(&mut self) {
        let mut all_layers = vec![];
        all_layers.push(Layer::dock_top(3, self.render_header()));
        all_layers.extend(self.active_app.update());

        // System toast layer (not app-managed!)
        if let Some(toast_layer) = self.toast_manager.render() {
            all_layers.push(toast_layer);
        }

        self.renderer.render(&all_layers);
    }
}
```

## Drag & Drop

```rust
struct DragState {
    dragging: Option<DragData>,
    current_pos: (u16, u16),
}

fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
    let mut layers = vec![
        Layer::fill(panel("Items", |ui| {
            ui.list(&mut self.list_state, &self.items)
                .draggable(true)
                .on_drag_start(|idx| { /* ... */ })
                .on_drop(|from_idx, to_idx| {
                    self.items.swap(from_idx, to_idx);
                });
        }))
    ];

    // Dragged item as overlay layer
    if let Some(drag) = &self.drag_state.dragging {
        layers.push(
            Layer::at(self.drag_state.current_pos, drag.item_snapshot.clone())
                .alpha(0.8)  // Semi-transparent
        );
    }

    layers
}
```

## Animation Easing

```rust
enum Easing {
    Linear,
    EaseOut,          // Decelerate (feels natural)
    EaseInOut,        // S-curve
    Spring(f32),      // Bouncy (damping factor)
}

impl Easing {
    fn apply(&self, t: f32) -> f32 {
        match self {
            Easing::Linear => t,
            Easing::EaseOut => 1.0 - (1.0 - t).powi(3),
            // ... other curves
        }
    }
}
```

## Color Animation

OKLCH color space enables smooth color fading:

```rust
// Calculate animation alpha
let age = Instant::now().duration_since(toast.created);
let alpha = (age.as_secs_f32() / 0.3).clamp(0.0, 1.0);

// Fade colors
let fg = toast_color.fade(&theme.bg_base, alpha);
let bg = theme.bg_surface.fade(&theme.bg_base, alpha * 0.9);
```

No hue shifts or weird interpolation - OKLCH guarantees perceptually uniform fading.

## Benefits

✅ **Automatic mode switching** - 0% CPU when idle, smooth animations when needed
✅ **Framework-managed toasts** - Apps just call methods
✅ **Perceptually uniform** - OKLCH color interpolation looks natural
✅ **Low overhead** - ~1-2% CPU during animation
✅ **Battery friendly** - Event-driven default

**See Also:**
- [Event Loop](../01-fundamentals/event-loop.md) - Event-driven rendering
- [Color System](../05-visual-design/color-system.md) - OKLCH color interpolation
- [Layers](../02-building-ui/layers.md) - Overlay positioning for drag & drop

---

**Next:** Learn about [Background Apps](background-apps.md) or explore [Pub/Sub](../03-state-management/pubsub.md).
