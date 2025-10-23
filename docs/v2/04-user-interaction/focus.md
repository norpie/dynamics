# Focus System

**Prerequisites:** [App & Context API](../01-fundamentals/app-and-context.md), [Layers](../02-building-ui/layers.md)

## Automatic Focus Order (Zero Boilerplate)

Focus order follows **render order** - no explicit registration needed:

```rust
fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
    vec![Layer::fill(panel("Form", |ui| {
        ui.text_input(&mut self.name);     // Focus index 0
        ui.text_input(&mut self.email);    // Focus index 1
        ui.button("Cancel");               // Focus index 2
        ui.button("Submit");               // Focus index 3
    }))]
}
```

**Tab/Shift-Tab** cycles through indices: 0 → 1 → 2 → 3 → 0.

## Layer-Scoped Focus (Auto-Restoration)

Each layer maintains independent focus state. When modal closes, underlying layer's focus is **automatically restored**:

```rust
// Focus state tracked by layer ID
"apps.migration.main":         focused_index = Some(2)  // "Submit" button
"apps.migration.delete_modal": focused_index = Some(0)  // "Yes" button (active)
```

**When modal layer is removed:**
1. Runtime detects `"apps.migration.delete_modal"` gone
2. Focus history popped: `["apps.migration.main", "apps.migration.delete_modal"]` → `["apps.migration.main"]`
3. Focus restored to `"apps.migration.main"` at index 2

No manual tracking needed!

## Programmatic Focus

### Declarative (Common Case)

Focus based on app state - evaluated during widget construction:

```rust
ui.text_input(&mut self.name)
    .auto_focus(self.name_invalid);  // Focus if validation failed

ui.button("Submit")
    .auto_focus(self.just_loaded);  // Focus on first render
```

### Imperative (Cross-Widget Focus)

Programmatic focus by ID - applied after render (same frame):

#### Relative Focus (Common) 🌟

Focus within current context - runtime composes full path:

```rust
// ctx.id_stack = ["apps", "migration", "environment"]

fn handle_file_selected(&mut self, ctx: &mut Context, path: PathBuf) {
    self.selected_file = Some(path);

    // Request focus on "continue-button" widget
    ctx.focus.request("continue-button");
    // Runtime composes: "apps.migration.environment.continue-button"
}

// Later in UI
ui.button("Continue").id("continue-button");
```

**Key points:**
- Widget IDs are **relative** to current context
- Runtime automatically composes full path
- Most common case (95% of focus requests)

#### Absolute Focus (Rare)

Focus across boundaries using full path:

```rust
// Rare: Focus a widget in a different layer/app
ctx.focus.request_absolute("apps.migration.comparison.submit");
```

**Only needed for:**
- App launcher focusing into target app
- System focusing into apps
- Cross-app navigation
- Debug/testing tools

### Focus Request Timing

Focus requests are applied **after render** completes (same frame):

```
1. Event handling     → ctx.focus.request() queued
2. update() returns   → Vec<Layer>
3. Render layers      → Build focusable lists per layer
4. Apply requests     → Validate against focusable lists ⬅ HERE
5. Draw to terminal   → Final frame with focus applied
```

**Validation**: If widget ID not found, warning logged and request ignored.

```rust
// Example warning:
WARN: Focus request failed: widget 'continue-button' not found in layer 'apps.migration.environment'
```

## User Navigation Takes Precedence

Auto-focus doesn't fight user navigation. If user presses Tab/Shift-Tab or clicks, auto-focus hints are suppressed for that frame.

## Progressive Unfocus (Esc Behavior)

**Esc key behavior:**
1. If something focused → blur it
2. If multiple layers → close top layer (focus auto-restored to layer below)
3. Otherwise → quit app

## Focus Modes (User Configurable)

```toml
# ~/.config/dynamics/config.toml
[ui]
focus_mode = "HoverWhenUnfocused"
```

**Modes:**
- **Click** - Focus only on click (default)
- **Hover** - Focus follows mouse
- **HoverWhenUnfocused** - Hover only when nothing focused

## Focus Context API

```rust
impl Context {
    pub fn focus(&mut self) -> &mut FocusManager;
}

impl FocusManager {
    // Relative focus (common case)
    pub fn request(&mut self, id: &str);
    // Composes full path using ctx.id_stack
    // Example: "button" → "apps.migration.environment.button"

    // Absolute focus (rare case)
    pub fn request_absolute(&mut self, full_path: &str);
    // Uses exact path provided
    // Example: "apps.migration.comparison.submit"

    // Convenience methods
    pub fn request_first(&mut self);             // Focus first widget in active layer
    pub fn request_last(&mut self);              // Focus last widget in active layer
    pub fn blur(&mut self);                      // Clear focus
    pub fn has_focus(&self) -> bool;             // Check if any widget focused
}
```

## Implementation Details

**Key insight:** Focus list IS the render list - precomputed automatically during UI construction.

### Runtime State

```rust
struct Runtime {
    // Per-layer focus state (keyed by layer ID)
    layer_focus: HashMap<String, LayerFocusState>,

    // Focus history stack (layer IDs)
    focus_history: Vec<String>,

    // LRU eviction (max 100 layers)
    // Time-based cleanup (30 seconds)
}

struct LayerFocusState {
    focused_index: Option<usize>,
    focusables: Vec<FocusableInfo>,        // Built during render
    id_to_index: HashMap<String, usize>,   // For ID lookups
}
```

### Focus Restoration Algorithm

```rust
fn restore_focus_after_layer_removed(&mut self, layers: &[Layer]) {
    // Walk focus history backwards to find still-existing layer
    while let Some(layer_id) = self.focus_history.pop() {
        if layers.iter().any(|l| l.id() == layer_id) {
            // Found valid layer - restore focus
            self.active_layer = layer_id;
            return;
        }
        // Layer gone, try next in history
    }
    // History exhausted - no layer focused
}
```

### Memory Management

- **LRU eviction**: Keep last 100 layer focus states
- **Time-based**: Remove states not seen for 30+ seconds
- **Prevents leaks**: Apps with dynamic layer IDs won't leak memory

**See Also:**
- [Layers](../02-building-ui/layers.md) - Layer system
- [Mouse](mouse.md) - Mouse focus integration
- [Navigation](navigation.md) - Tab/Shift-Tab navigation
- [Keybinds](keybinds.md) - Keybind routing priority
- [Plugin System](../06-system-features/plugin-system.md) - Plugins can request focus via PluginContext

---

**Next:** Learn about [Mouse Support](mouse.md) or [Navigation](navigation.md).
