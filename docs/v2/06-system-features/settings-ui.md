# Settings UI

**Prerequisites:** [Options System](../03-state-management/options.md), [Keybinds](../04-user-interaction/keybinds.md)

## Overview

The Settings UI is **auto-generated** from the Options registry - no manual menu construction needed.

**Key features:**
- Auto-discovery from OptionsRegistry
- Namespace-based organization
- Type-specific editors (text input, sliders, dropdowns, arrays)
- Keybind conflict detection
- Preset system (Vim mode, etc.)
- Resource pattern for async loading

---

## Auto-Discovery

Settings UI iterates the registry and groups options by namespace:

```rust
struct SettingsApp {
    options_values: Resource<HashMap<String, OptionValue>>,
    selected_namespace: Option<String>,
}

fn init(ctx: &AppContext) -> Self {
    Self {
        options_values: Resource::NotAsked,
        selected_namespace: None,
    }
}

fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
    // Load all options on first render
    if matches!(self.options_values, Resource::NotAsked) {
        self.options_values.load(ctx, async {
            ctx.options.get_by_prefix("").await  // Load ALL options
        });
    }

    // Render based on resource state
    match &self.options_values {
        Resource::Loading(_) => self.render_loading(ctx),
        Resource::Success(values) => self.render_settings(ctx, values),
        Resource::Failure { error, .. } => self.render_error(ctx, error),
        Resource::NotAsked => self.render_loading(ctx),
    }
}

fn render_settings(&mut self, ctx: &mut Context, values: &HashMap<String, OptionValue>) -> Vec<Layer> {
    let registry = ctx.config.options.registry();

    // Group options by namespace
    let namespaces = self.group_by_namespace(registry);

    vec![Layer::fill(panel("Settings", |ui| {
        // Namespace selector
        ui.list(&mut self.namespace_list, &namespaces, |ns, ui| {
            ui.text(ns);
        });

        // Options for selected namespace
        if let Some(ns) = &self.selected_namespace {
            for opt_def in registry.list_namespace(ns) {
                self.render_option(ui, &opt_def, values);
            }
        }
    }))]
}
```

---

## Namespace Organization

Options are grouped by their first segment:

```rust
fn group_by_namespace(&self, registry: &OptionsRegistry) -> Vec<String> {
    let mut namespaces = HashSet::new();

    for opt_def in registry.list_all() {
        // Extract first segment: "api.retry.enabled" → "api"
        if let Some(ns) = opt_def.key.split('.').next() {
            namespaces.insert(ns.to_string());
        }
    }

    let mut sorted: Vec<_> = namespaces.into_iter().collect();
    sorted.sort();
    sorted
}
```

**Example namespaces:**
- `api` - API configuration (retry, timeout, endpoints)
- `tui` - UI configuration (focus mode, theme)
- `keybind` - All keybinds (auto-organized by app)

---

## Type-Specific Editors

Each OptionType gets a specialized editor:

```rust
fn render_option(
    &mut self,
    ui: &mut Ui,
    opt_def: &OptionDefinition,
    values: &HashMap<String, OptionValue>
) {
    ui.label(&opt_def.display_name);
    if !opt_def.description.is_empty() {
        ui.text_secondary(&opt_def.description);
    }

    // Get current value (from cache or registry default)
    let current = values.get(&opt_def.key)
        .unwrap_or(&opt_def.default);

    match &opt_def.ty {
        OptionType::Bool => self.render_bool_editor(ui, opt_def, current),
        OptionType::Int { min, max } => self.render_int_editor(ui, opt_def, current, *min, *max),
        OptionType::UInt { min, max } => self.render_uint_editor(ui, opt_def, current, *min, *max),
        OptionType::String { max_length } => self.render_string_editor(ui, opt_def, current, *max_length),
        OptionType::Enum { variants } => self.render_enum_editor(ui, opt_def, current, variants),
        OptionType::Array { .. } => self.render_array_editor(ui, opt_def, current),
    }
}
```

### Bool Editor

```rust
fn render_bool_editor(&mut self, ui: &mut Ui, opt_def: &OptionDefinition, current: &OptionValue) {
    let value = current.as_bool().unwrap_or(false);

    if ui.checkbox(&mut value.clone()).changed() {
        self.queue_update(opt_def.key.clone(), OptionValue::Bool(!value));
    }
}
```

### Enum Editor

```rust
fn render_enum_editor(
    &mut self,
    ui: &mut Ui,
    opt_def: &OptionDefinition,
    current: &OptionValue,
    variants: &[String]
) {
    let current_str = current.as_string().unwrap_or_default();

    ui.select(&mut self.enum_select_state, variants, |variant, ui| {
        ui.text(variant);

        if variant == &current_str {
            ui.icon("✓");
        }
    }).on_select(|selected| {
        self.queue_update(opt_def.key.clone(), OptionValue::String(selected.clone()));
    });
}
```

### Range Editor (Int/UInt)

```rust
fn render_uint_editor(
    &mut self,
    ui: &mut Ui,
    opt_def: &OptionDefinition,
    current: &OptionValue,
    min: Option<u64>,
    max: Option<u64>
) {
    let value = current.as_uint().unwrap_or(0);

    ui.slider(&mut value.clone())
        .range(min.unwrap_or(0)..=max.unwrap_or(u64::MAX))
        .on_change(|new_value| {
            self.queue_update(opt_def.key.clone(), OptionValue::UInt(new_value));
        });
}
```

---

## Array Editor (Keybind Aliases)

Arrays get special treatment for adding/removing elements:

```rust
fn render_array_editor(&mut self, ui: &mut Ui, opt_def: &OptionDefinition, current: &OptionValue) {
    let arr = current.as_array().unwrap_or_default();

    // Render each element
    for (i, elem) in arr.iter().enumerate() {
        ui.row(|ui| {
            // Value display
            if let OptionValue::String(s) = elem {
                ui.badge(s, if i == 0 { Style::primary() } else { Style::secondary() });

                let label = if i == 0 { "(primary)" } else { &format!("(alias {})", i) };
                ui.text(label);
            }

            // Remove button (if > min_length)
            if arr.len() > self.get_min_length(&opt_def.ty) {
                if ui.button("Remove").clicked() {
                    self.remove_array_element(&opt_def.key, i);
                }
            }
        });
    }

    // Add button
    if arr.len() < self.get_max_length(&opt_def.ty).unwrap_or(usize::MAX) {
        if ui.button("+ Add").clicked() {
            self.start_array_capture(&opt_def.key);
        }
    }
}

fn get_min_length(&self, ty: &OptionType) -> usize {
    match ty {
        OptionType::Array { min_length, .. } => min_length.unwrap_or(0),
        _ => 0,
    }
}

fn get_max_length(&self, ty: &OptionType) -> Option<usize> {
    match ty {
        OptionType::Array { max_length, .. } => *max_length,
        _ => None,
    }
}
```

---

## Keybind Capture Mode

For keybind arrays, enter "capture mode" to record a key press:

```rust
struct CaptureMode {
    option_key: String,
    captured_key: Option<KeyBinding>,
}

fn start_array_capture(&mut self, option_key: &str) {
    self.capture_mode = Some(CaptureMode {
        option_key: option_key.to_string(),
        captured_key: None,
    });
}

fn handle_key_capture(&mut self, ctx: &mut Context, event: KeyEvent) {
    if let Some(capture) = &mut self.capture_mode {
        let binding = KeyBinding::from(event);

        // Check for conflicts
        if let Some(conflict) = self.find_conflict(&binding) {
            // Show warning modal
            self.show_conflict_warning = Some(conflict);
        } else {
            // Add to array
            self.add_array_element(&capture.option_key, binding);
            self.capture_mode = None;
        }
    }
}

fn add_array_element(&mut self, key: &str, binding: KeyBinding) {
    // Get current array
    let current = self.options_values.as_success()
        .and_then(|values| values.get(key))
        .cloned()
        .unwrap_or(OptionValue::Array(vec![]));

    if let OptionValue::Array(mut arr) = current {
        // Add new binding
        arr.push(OptionValue::String(binding.to_string()));

        // Queue update
        self.queue_update(key.to_string(), OptionValue::Array(arr));
    }
}
```

---

## Conflict Detection

Warn users about keybind conflicts:

```rust
fn find_conflict(&self, binding: &KeyBinding) -> Option<String> {
    let registry = self.ctx.config.options.registry();

    // Check all keybind options
    for opt_def in registry.list_namespace("keybind") {
        if let Some(values) = self.options_values.as_success() {
            if let Some(value) = values.get(&opt_def.key) {
                if let Ok(arr) = value.as_array() {
                    for elem in arr {
                        if let Ok(kb_str) = elem.as_string() {
                            if let Ok(kb) = KeyBinding::from_str(&kb_str) {
                                if kb == *binding {
                                    return Some(opt_def.display_name.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

fn render_conflict_warning(&mut self, ctx: &mut Context, conflict: &str) -> Vec<Layer> {
    // ... modal showing conflict
    Layer::modal(panel("Conflict Warning", |ui| {
        ui.text(&format!("Key already bound to: {}", conflict));
        ui.text("Do you want to replace it?");

        ui.button("Replace").on_click(|app| {
            // Remove from conflicting option, add to current
        });

        ui.button("Cancel").on_click(|app| {
            app.show_conflict_warning = None;
        });
    }))
}
```

---

## Preset System

Apply preset configurations (e.g., Vim mode):

```rust
async fn apply_vim_preset(options: &Options) -> Result<()> {
    // Collect all navigation keybind updates
    let updates = vec![
        ("keybind.global.nav.up", OptionValue::Array(vec![
            OptionValue::String("Up".into()),
            OptionValue::String("k".into()),
        ])),
        ("keybind.global.nav.down", OptionValue::Array(vec![
            OptionValue::String("Down".into()),
            OptionValue::String("j".into()),
        ])),
        ("keybind.global.nav.left", OptionValue::Array(vec![
            OptionValue::String("Left".into()),
            OptionValue::String("h".into()),
        ])),
        ("keybind.global.nav.right", OptionValue::Array(vec![
            OptionValue::String("Right".into()),
            OptionValue::String("l".into()),
        ])),
    ];

    // Single transaction for all updates
    options.set_many(&updates).await?;

    Ok(())
}

// In UI
fn render_presets(&mut self, ui: &mut Ui) {
    ui.section("Presets", |ui| {
        if ui.button("Enable Vim Navigation").clicked() {
            self.apply_preset_task.load(ctx, async {
                apply_vim_preset(&ctx.options).await
            });
        }

        if ui.button("Reset All Keybinds").clicked() {
            self.show_reset_confirm = true;
        }
    });
}
```

---

## Batched Updates

Don't spam the database - batch updates and apply on save/debounce:

```rust
struct SettingsApp {
    options_values: Resource<HashMap<String, OptionValue>>,
    pending_updates: HashMap<String, OptionValue>,
    save_task: Resource<()>,
}

fn queue_update(&mut self, key: String, value: OptionValue) {
    // Update local cache immediately (optimistic UI)
    if let Resource::Success(values) = &mut self.options_values {
        values.insert(key.clone(), value.clone());
    }

    // Queue for database write
    self.pending_updates.insert(key, value);
}

fn save_changes(&mut self, ctx: &mut Context) {
    let updates: Vec<_> = self.pending_updates.drain().collect();

    self.save_task.load(ctx, async move {
        ctx.options.set_many(&updates).await
    });
}

// Auto-save on navigation away
fn on_background(&mut self, ctx: &mut Context) {
    if !self.pending_updates.is_empty() {
        self.save_changes(ctx);
    }
}
```

---

## Keybind-Specific Organization

Keybinds get special hierarchical organization:

```rust
fn render_keybind_settings(&mut self, ui: &mut Ui, values: &HashMap<String, OptionValue>) {
    let registry = self.ctx.config.options.registry();
    let keybind_opts = registry.list_namespace("keybind");

    // Group by app/layer
    let mut by_app: HashMap<String, Vec<OptionDefinition>> = HashMap::new();

    for opt in keybind_opts {
        // "keybind.queue.refresh" → app="queue"
        // "keybind.queue.main.process" → app="queue", layer="main"
        let parts: Vec<&str> = opt.key.split('.').collect();

        if parts.len() >= 2 {
            let app = parts[1];
            by_app.entry(app.to_string())
                .or_insert_with(Vec::new)
                .push(opt);
        }
    }

    // Render global first
    if let Some(global_opts) = by_app.remove("global") {
        ui.section("Global Keybinds", |ui| {
            for opt in global_opts {
                self.render_keybind_option(ui, &opt, values);
            }
        });
    }

    // Render each app
    for (app_id, opts) in by_app {
        ui.section(&format!("{} Keybinds", app_id), |ui| {
            // Separate global and layer-scoped
            let mut global = Vec::new();
            let mut by_layer: HashMap<String, Vec<OptionDefinition>> = HashMap::new();

            for opt in opts {
                let parts: Vec<&str> = opt.key.split('.').collect();
                if parts.len() == 3 {
                    // keybind.app.action (global)
                    global.push(opt);
                } else if parts.len() == 4 {
                    // keybind.app.layer.action
                    by_layer.entry(parts[2].to_string())
                        .or_insert_with(Vec::new)
                        .push(opt);
                }
            }

            // Render global
            for opt in global {
                self.render_keybind_option(ui, &opt, values);
            }

            // Render layer-scoped
            for (layer_id, layer_opts) in by_layer {
                ui.subsection(&format!("Layer: {}", layer_id), |ui| {
                    for opt in layer_opts {
                        self.render_keybind_option(ui, &opt, values);
                    }
                });
            }
        });
    }
}

fn render_keybind_option(
    &mut self,
    ui: &mut Ui,
    opt_def: &OptionDefinition,
    values: &HashMap<String, OptionValue>
) {
    ui.label(&opt_def.display_name);

    // Render array of keybinds
    self.render_array_editor(ui, opt_def, values.get(&opt_def.key).unwrap_or(&opt_def.default));
}
```

---

## See Also

- **[Options System](../03-state-management/options.md)** - Storage backend
- **[Keybinds](../04-user-interaction/keybinds.md)** - Keybind system
- **[Resource Pattern](../03-state-management/resource-pattern.md)** - Async loading pattern

---

**Next:** Learn about [Help System](help-system.md) or explore [App Launcher](app-launcher.md).
