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
Layer 0 (Base App):     focused_index = Some(2)  // "Submit" button
Layer 1 (Modal):        focused_index = Some(0)  // "Yes" button (active)
```

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

### Imperative (Rare Cases)

Programmatic focus by ID - applied after UI construction (same render cycle):

```rust
if let Some(Event::FileSelected(path)) = ui.events().file_browser() {
    self.handle_file_selected(path);
    ctx.focus.request("continue-button");  // Focus continue button
}

ui.button("Continue").id("continue-button");
```

**Focus requests are applied immediately** (same render cycle, after UI construction).

**Optional IDs** - only needed for programmatic focus.

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
    pub fn request(&mut self, id: &str);        // Focus by ID
    pub fn request_first(&mut self);             // Focus first
    pub fn request_last(&mut self);              // Focus last
    pub fn blur(&mut self);                      // Clear focus
    pub fn has_focus(&self) -> bool;             // Check if focused
}
```

## Implementation Details

**Key insight:** Focus list IS the render list - precomputed automatically during UI construction.

Each layer maintains:
- `focused_index: Option<usize>`
- `focusables: Vec<FocusableInfo>` (built during render)
- `id_to_index: HashMap<String, usize>` (for ID lookups)

**See Also:**
- [Layers](../02-building-ui/layers.md) - Layer system
- [Mouse](mouse.md) - Mouse focus integration
- [Navigation](navigation.md) - Tab/Shift-Tab navigation
- [Keybinds](keybinds.md) - Keybind routing priority

---

**Next:** Learn about [Mouse Support](mouse.md) or [Navigation](navigation.md).
