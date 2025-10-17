# Mouse Support

**Prerequisites:** [Focus System](focus.md)

## Hit Testing (1-Frame Delay)

**Problem:** During `update()`, we're building UI but don't know element positions yet (layout happens after).

**Solution:** Use **previous frame's rectangles** for hover detection.

**1-frame delay (16ms @ 60fps)** - imperceptible to users!

## Inline Event Handling

Widgets return events inline - no message passing:

```rust
fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
    vec![Layer::fill(panel("Controls", |ui| {
        // Button handles click internally
        if ui.button("Save").clicked() {
            self.save();
        }

        // List returns events
        if let Some(Event::Activated(idx)) = ui.events().list() {
            self.open_file(idx);
        }
    }))]
}
```

## Automatic Hover Styles

**No app code needed** - widgets handle hover styling internally using `is_over()` check.

## Automatic Scroll Wheel

Widgets handle scroll wheel automatically when focused:

```rust
// Apps don't need to handle scroll - it just works!
ui.list(&mut self.list_state, &self.items);
```

## Double-Click vs Single-Click

Widgets differentiate automatically (500ms threshold):

```rust
if let Some(Event::Selected(idx)) = ui.events().list() {
    self.selected = idx;  // Single-click selects
}

if let Some(Event::Activated(idx)) = ui.events().list() {
    self.open_file(idx);  // Double-click opens
}
```

## Right-Click / Context Menus

Widgets emit right-click events:

```rust
if let Some(Event::RightClick(idx)) = ui.events().list() {
    self.show_context_menu = true;
    self.context_menu_pos = ctx.mouse.position();
}

// Context menu as overlay layer
if self.show_context_menu {
    layers.push(Layer::at(self.context_menu_pos, panel("", |ui| {
        if ui.button("Open").clicked() { /* ... */ }
        if ui.button("Delete").clicked() { /* ... */ }
    })));
}
```

## MouseState API

```rust
impl Context {
    pub fn mouse(&self) -> &MouseState;
}

impl MouseState {
    // Position
    fn position(&self) -> (u16, u16);
    fn is_over(&self, id: &str) -> bool;

    // Buttons
    fn clicked(&self) -> bool;              // Left button just pressed
    fn right_clicked(&self) -> bool;        // Right button just pressed
    fn is_dragging(&self) -> bool;          // Left button held + moved
    fn double_clicked(&self) -> bool;       // Double-click detected

    // Scroll
    fn scroll_delta(&self) -> i16;          // Positive = up, negative = down
}
```

## Focus Integration

Clicking an element focuses it (configurable via FocusMode). See [Focus System](focus.md) for details.

## Terminal Mouse Capture

Enable via crossterm:
```rust
crossterm::execute!(stdout, crossterm::event::EnableMouseCapture)?;
```

**Key insight:** Widgets handle most mouse behavior internally. Apps only handle semantic events (file selected, button clicked) - not raw mouse events.

**See Also:**
- [Focus System](focus.md) - Focus modes and click-to-focus
- [Component Patterns](component-patterns.md) - Event handling patterns
- [Modals](../02-building-ui/modals.md) - Context menu patterns

---

**Next:** Explore [Navigation](navigation.md) or [Component Patterns](component-patterns.md).
