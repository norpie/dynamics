# Component Interaction Patterns

**Prerequisites:** [App & Context API](../01-fundamentals/app-and-context.md), [Components](../02-building-ui/components.md)

## V1 Problems

### Problem 1: Message Explosion

Every component interaction needs 3-5 Msg enum variants:

```rust
enum Msg {
    SourceTreeNavigate(KeyCode),
    SourceTreeSelect(String),
    SourceTreeToggle(String),
    SourceTreeViewportHeight(usize),
    SourceTreeClicked(String),
    // Repeat for EVERY component...
}
```

**Result:** Msg enum bloat, ceremony, hard to maintain.

### Problem 2: on_render Callbacks Hack

Scrollable components need viewport dimensions, but dimensions aren't known until render:

```rust
// App uses hardcoded fallback
state.list_state.set_viewport_height(20);  // GUESS!

// Widget requests real dimensions via callback
Element::List {
    on_render: Some(|actual_height| Msg::SetViewportHeight(actual_height)),
    // ...
}
```

**Result:** 1-frame delay, hardcoded fallbacks, boilerplate.

### Problem 3: State Management Boilerplate

Apps must manually track component state:

```rust
struct MyApp {
    name_value: String,
    name_cursor: usize,
    name_scroll: usize,
    name_selection: Option<(usize, usize)>,
    // Repeat for every text input...
}
```

**Result:** Apps manage low-level details they shouldn't care about.

## V2 Solution: Callbacks + Internal State

**Three key improvements:**

1. **Components manage internal state** (cursor, scroll, focus)
2. **Callbacks replace message passing** (direct method calls)
3. **Dimensions known during construction** (no on_render hacks)

```rust
struct MyApp {
    name: String,           // Just the data
    items: Vec<Item>,       // Just the data
    selected: Option<usize>, // App-level selection
}

fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
    vec![
        Layer::fill(panel("Form", |ui| {
            ui.text_input(&mut self.name)
                .placeholder("Enter name")
                .on_change(Self::handle_name_change)
                .on_submit(Self::handle_submit);

            ui.list(&self.items)
                .on_select(Self::handle_select)
                .on_activate(Self::handle_activate);
        }))
    ]
}
```

**No messages, no state boilerplate, no dimension hacks.**

## Callback Patterns by Component Type

### Simple Components (Button, Link)

Single-action components with one callback:

```rust
ui.button("Save").on_click(Self::handle_save);
ui.button("Cancel").on_click(Self::handle_cancel);

fn handle_save(&mut self, ctx: &mut Context) {
    self.save();
}
```

### Text Input

Multiple callbacks for different events:

```rust
ui.text_input(&mut self.name)
    .placeholder("Enter name")
    .on_change(Self::handle_name_change)  // Each keystroke
    .on_submit(Self::handle_submit);       // Enter key

fn handle_name_change(&mut self, ctx: &mut Context) {
    // self.name already updated by component
}
```

### Complex Components (List, Tree, Table)

Multiple callbacks for rich interactions:

```rust
ui.list(&self.files)
    .on_select(Self::handle_select)        // Arrow keys or click
    .on_activate(Self::handle_activate)    // Enter or double-click
    .on_right_click(Self::handle_context); // Right-click

fn handle_select(&mut self, ctx: &mut Context, index: usize) {
    // Single-click - update preview
    self.selected_file = Some(index);
}

fn handle_activate(&mut self, ctx: &mut Context, index: usize) {
    // Double-click or Enter - open file
    self.open_file(index);
}
```

## Callback Signatures

**Consistent pattern across components:**

```rust
// Button
fn on_click(&mut self, ctx: &mut Context);

// Text Input
fn on_change(&mut self, ctx: &mut Context);
fn on_submit(&mut self, ctx: &mut Context);

// List
fn on_select(&mut self, ctx: &mut Context, index: usize);
fn on_activate(&mut self, ctx: &mut Context, index: usize);
fn on_right_click(&mut self, ctx: &mut Context, index: usize);

// Tree
fn on_select(&mut self, ctx: &mut Context, node_id: String);
fn on_toggle(&mut self, ctx: &mut Context, node_id: String);
fn on_activate(&mut self, ctx: &mut Context, node_id: String);

// Table
fn on_select(&mut self, ctx: &mut Context, row: usize, col: usize);
fn on_activate(&mut self, ctx: &mut Context, row: usize, col: usize);
```

**Pattern consistency:**
- **on_select** = navigation (arrow keys, single-click)
- **on_activate** = action (Enter, double-click)
- **on_right_click** = context menu
- **on_toggle** = expand/collapse (Tree-specific)

## Component State: Automatic vs Semantic

### Automatic State (Component-Managed, Hidden)

Low-level UI state apps shouldn't see:
- **Text cursor position**
- **Scroll offset**
- **Hover state**
- **Focus ring position**

**Components manage this entirely.**

```rust
ui.text_input(&mut self.name)
    .on_submit(Self::handle_submit);

// Component handles cursor, scroll, selection internally
```

### Semantic State (App-Managed, Exposed)

High-level state apps need to query/control:
- **Tree expansion** - Which nodes expanded/collapsed
- **Selection** - Which item selected
- **Sort order** - How table sorted
- **Column widths** - Table column sizes

**Apps manage via state objects:**

```rust
struct MyApp {
    tree_state: TreeState,      // App owns, can query/mutate
    table_state: TableState,    // App owns, can query/mutate
}

fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
    // App can imperatively control semantic state
    if self.should_expand_path {
        self.tree_state.expand_path(&["root", "folder", "subfolder"]);
    }

    // App can query state
    if self.tree_state.is_expanded("node-123") {
        // ...
    }

    vec![
        Layer::fill(panel("Files", |ui| {
            ui.tree(&self.nodes, &mut self.tree_state)
                .on_select(Self::handle_select);

            // Use selection to drive other UI
            if let Some(selected) = self.tree_state.selected() {
                ui.text(format!("Selected: {}", selected));
            }
        }))
    ]
}
```

**State object APIs:**

```rust
// TreeState
impl TreeState {
    pub fn expand_path(&mut self, path: &[&str]);
    pub fn is_expanded(&self, node_id: &str) -> bool;
    pub fn selected(&self) -> Option<String>;
}

// ListState
impl ListState {
    pub fn selected(&self) -> Option<usize>;
    pub fn select(&mut self, index: usize);
}
```

## Automatic Navigation

**Components handle all navigation internally** - apps never see arrow keys:

```rust
fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
    vec![
        Layer::fill(panel("Items", |ui| {
            ui.list(&self.items)
                .on_activate(Self::handle_activate);

            // Arrow keys, scroll wheel, PageUp/Down all work automatically!
        }))
    ]
}
```

**Automatic behaviors (no app code):**
- Arrow keys update selection & scroll
- Scroll wheel updates scroll offset
- PageUp/PageDown navigate by page
- Home/End jump to start/end
- Tab/Shift-Tab move focus
- Escape blurs focus
- Mouse click selects
- Double-click activates

**Callbacks only for semantic actions:**
- Open file (activate)
- Show context menu (right-click)
- Custom business logic (select with side effects)

## Keybind Integration

**Keybind priority system:**

1. **Focused component** gets first chance at key
2. **Component refuses** → App keybinds checked
3. **App doesn't handle** → Global keybinds

```rust
impl Runtime {
    fn handle_key(&mut self, key: KeyCode) {
        // 1. Give focused component first chance
        if let Some(focused) = self.focus.focused_component() {
            if focused.handle_key(key) {
                return;  // Consumed
            }
        }

        // 2. Check app keybinds
        if let Some(action) = self.active_app.keybinds().get(key) {
            action.call(&mut self.active_app);
            return;
        }

        // 3. Check global keybinds
        // ...
    }
}
```

**Components only consume navigation keys** - letter keys, Ctrl+keys pass through to app.

## Benefits

✅ **No on_render callbacks** - Dimensions known immediately
✅ **No message explosion** - Callbacks instead of Msg variants
✅ **No state boilerplate** - Components manage cursor/scroll
✅ **Automatic navigation** - Arrow keys, scroll wheel handled
✅ **Consistent callbacks** - Same pattern across components
✅ **Type-safe** - Compiler catches mismatched signatures
✅ **~90% less code** compared to V1

**See Also:**
- [Components](../02-building-ui/components.md) - Component state composition
- [Keybinds](keybinds.md) - Keybind system details
- [Focus System](focus.md) - Focus management

---

**Next:** Learn about [Navigation](navigation.md) or explore [Mouse Support](mouse.md).
