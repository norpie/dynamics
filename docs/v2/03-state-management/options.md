# Options System V2

**Prerequisites:** [App & Context API](../01-fundamentals/app-and-context.md)

## Overview

The V2 options system builds on V1's foundation while addressing key ergonomics and performance issues.

**V1 Strengths (Preserved):**
- Type-safe storage with validation
- Database-backed persistence (SQLite)
- Namespace organization
- Self-describing metadata for UI generation
- Registry as single source of truth for defaults

**V2 Improvements:**
- **Array support** - Multiple values per option (keybind aliases)
- **Single-definition keybinds** - No more duplication between registration and subscriptions
- **Layer-scoped keybinds** - Replace per-frame conditional subscriptions
- **Bulk operations** - Fetch/set multiple options in one query
- **Prefix queries** - Load entire namespaces efficiently
- **Auto-registration** - Derive macro + inventory (future)
- **No hardcoded defaults** - Registry is the only source of truth

---

## Core Principle: Registry as Source of Truth

The `OptionsRegistry` holds the **only** definition of defaults. Loading code never hardcodes fallback values.

```rust
// ✅ Correct - registry provides default
let enabled = options.get("api.retry.enabled").await?.as_bool()?;

// ❌ NEVER do this - duplicates the default
let enabled = options.get("api.retry.enabled").await?
    .as_bool()
    .unwrap_or(true);  // This "true" should only exist in registry!
```

**How it works:**
```rust
impl Options {
    pub async fn get(&self, key: &str) -> Result<OptionValue> {
        let def = self.registry.get(key)
            .ok_or_else(|| anyhow::anyhow!("Option '{}' not registered", key))?;

        // Try DB
        if let Some(raw) = self.get_raw(key).await? {
            self.parse_value(&raw, &def.ty)
        } else {
            // Not in DB? Return registry default
            Ok(def.default.clone())  // ← Single source of truth
        }
    }
}
```

---

## Array Support

### Core Types

```rust
pub enum OptionType {
    Bool,
    Int { min: Option<i64>, max: Option<i64> },
    UInt { min: Option<u64>, max: Option<u64> },
    Float { min: Option<f64>, max: Option<f64> },
    String { max_length: Option<usize> },
    Enum { variants: Vec<String> },

    // NEW: Array type with element constraints
    Array {
        element_type: Box<OptionType>,
        min_length: Option<usize>,
        max_length: Option<usize>,
    },
}

pub enum OptionValue {
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    String(String),

    // NEW: Array value
    Array(Vec<OptionValue>),
}

impl OptionValue {
    pub fn as_array(&self) -> Result<Vec<OptionValue>> {
        match self {
            OptionValue::Array(v) => Ok(v.clone()),
            _ => anyhow::bail!("Expected Array, got {:?}", self),
        }
    }
}
```

### Validation

```rust
impl OptionType {
    pub fn validate(&self, value: &OptionValue) -> Result<()> {
        match (self, value) {
            (OptionType::Array { element_type, min_length, max_length }, OptionValue::Array(arr)) => {
                // Check length constraints
                if let Some(min) = min_length {
                    if arr.len() < *min {
                        anyhow::bail!("Array length {} below minimum {}", arr.len(), min);
                    }
                }
                if let Some(max) = max_length {
                    if arr.len() > *max {
                        anyhow::bail!("Array length {} exceeds maximum {}", arr.len(), max);
                    }
                }

                // Validate each element against element_type
                for (i, elem) in arr.iter().enumerate() {
                    element_type.validate(elem)
                        .with_context(|| format!("Invalid element at index {}", i))?;
                }

                Ok(())
            }
            // ... other type validations
            _ => { /* ... */ }
        }
    }
}
```

### Serialization (JSON in SQLite)

```rust
impl Options {
    fn serialize_value(&self, value: &OptionValue) -> Result<String> {
        match value {
            OptionValue::Array(arr) => {
                serde_json::to_string(arr)
                    .context("Failed to serialize array to JSON")
            }
            OptionValue::Bool(v) => Ok(v.to_string()),
            OptionValue::String(v) => Ok(v.clone()),
            // ...
        }
    }

    fn parse_value(&self, raw: &str, ty: &OptionType) -> Result<OptionValue> {
        match ty {
            OptionType::Array { element_type, .. } => {
                let json_arr: Vec<serde_json::Value> = serde_json::from_str(raw)?;

                let mut values = Vec::new();
                for json_val in json_arr {
                    let elem_str = match json_val {
                        serde_json::Value::String(s) => s,
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        _ => anyhow::bail!("Unsupported JSON type"),
                    };

                    values.push(self.parse_value(&elem_str, element_type)?);
                }

                Ok(OptionValue::Array(values))
            }
            // ... other parsers
        }
    }
}
```

### Builder API

```rust
impl OptionDefBuilder {
    /// Define array type with element constraints
    pub fn array_type(
        mut self,
        element_type: OptionType,
        default: Vec<OptionValue>,
        min_length: Option<usize>,
        max_length: Option<usize>
    ) -> Self {
        self.ty = Some(OptionType::Array {
            element_type: Box::new(element_type),
            min_length,
            max_length,
        });
        self.default = Some(OptionValue::Array(default));
        self
    }

    /// Convenience for keybind arrays (primary + aliases)
    pub fn keybinds_type(mut self, defaults: Vec<KeyBinding>) -> Self {
        let default_values: Vec<OptionValue> = defaults.iter()
            .map(|kb| OptionValue::String(kb.to_string()))
            .collect();

        self.array_type(
            OptionType::String { max_length: Some(32) },
            default_values,
            Some(1),  // Must have at least 1 keybind
            None,     // No upper limit on aliases
        )
    }
}
```

### Storage Format

```sql
-- SQLite storage (JSON arrays)
INSERT INTO options (key, value) VALUES
    ('keybind.entity_comparison.save', '["Ctrl+s"]'),              -- Primary only
    ('keybind.entity_comparison.refresh', '["F5", "r", "Ctrl+r"]'), -- Primary + aliases
    ('keybind.global.help', '["F1"]');
```

---

## Keybind System

### The Problem (V1)

**Duplication everywhere:**
```rust
// 1. Define in registrations/keybinds.rs
registry.register(
    OptionDefBuilder::new("keybind", "entity_comparison.refresh")
        .display_name("Refresh Metadata")
        .keybind_type(KeyCode::F(5))
        .build()?
)?;

// 2. Use in subscriptions() - DUPLICATE!
fn subscriptions(state: &State) -> Vec<Subscription<Msg>> {
    vec![
        Subscription::keyboard(
            config.get_keybind("entity_comparison.refresh"),  // String literal again!
            "Refresh metadata",  // Description duplicated!
            Msg::Refresh
        ),
    ]
}
```

**Conditional binds require per-frame regeneration:**
```rust
fn subscriptions(state: &State) -> Vec<Subscription<Msg>> {
    let mut subs = vec![/* base binds */];

    // Different binds based on state
    if state.show_modal {
        subs.push(Subscription::keyboard('y', "Yes", Msg::Yes));
        subs.push(Subscription::keyboard('n', "No", Msg::No));
    }

    subs  // Rebuilt every frame!
}
```

### The Solution (V2)

**Single definition, layer-scoped dispatch with callbacks:**

```rust
use indexmap::IndexMap;

pub struct KeybindMap<A> {
    global_binds: Vec<KeybindDef<A>>,
    layer_binds: IndexMap<String, Vec<KeybindDef<A>>>,
    _phantom: std::marker::PhantomData<A>,
}

pub struct KeybindDef<A> {
    action: String,
    description: String,
    default_key: KeyBinding,
    callback: fn(&mut A, &mut Context),
}

impl<A> KeybindMap<A> {
    pub fn new() -> Self {
        Self {
            global_binds: Vec::new(),
            layer_binds: IndexMap::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Add global keybind (active on all layers)
    pub fn bind(
        mut self,
        action: &str,
        desc: &str,
        key: impl Into<KeyBinding>,
        callback: fn(&mut A, &mut Context)
    ) -> Self {
        self.global_binds.push(KeybindDef {
            action: action.to_string(),
            description: desc.to_string(),
            default_key: key.into(),
            callback,
        });
        self
    }

    /// Start layer scope - subsequent bind_layer() calls apply to this layer
    /// Uses IndexMap to maintain insertion order for .iter().last()
    pub fn layer(mut self, layer_id: &str) -> Self {
        self.layer_binds.entry(layer_id.to_string()).or_insert_with(Vec::new);
        self
    }

    /// Add keybind to most recently declared layer
    pub fn bind_layer(
        mut self,
        action: &str,
        desc: &str,
        key: impl Into<KeyBinding>,
        callback: fn(&mut A, &mut Context)
    ) -> Self {
        if let Some((layer_id, _)) = self.layer_binds.iter().last() {
            let layer_id = layer_id.clone();
            self.layer_binds.get_mut(&layer_id).unwrap().push(KeybindDef {
                action: action.to_string(),
                description: desc.to_string(),
                default_key: key.into(),
                callback,
            });
        }
        self
    }
}
```

### App Usage

```rust
impl App for EntityComparisonApp {
    fn id() -> &'static str { "entity_comparison" }

    // Single definition point - called once at registration
    fn keybinds() -> KeybindMap<Self> {
        KeybindMap::new()
            // Global binds (active on all layers) - use method references
            .bind("refresh", "Refresh metadata", KeyCode::F(5), Self::refresh_metadata)
            .bind("back", "Back to list", KeyCode::Char('b'), Self::back)
            .bind("export", "Export to Excel", KeyCode::F(10), Self::export_to_excel)

            // Confirmation modal binds (only when layer rendered) - use closures
            .layer("confirm_delete")
            .bind_layer("yes", "Confirm deletion", KeyCode::Char('y'), |app, _ctx| {
                app.confirm_delete = true;
                app.show_confirm = false;
            })
            .bind_layer("no", "Cancel", KeyCode::Char('n'), |app, _ctx| {
                app.show_confirm = false;
            })

            // Manual mappings modal binds - mix of methods and closures
            .layer("manual_mappings")
            .bind_layer("create", "Create mapping", KeyCode::Char('m'), Self::create_manual_mapping)
            .bind_layer("delete", "Delete mapping", KeyCode::Char('d'), Self::delete_manual_mapping)
            .bind_layer("close", "Close modal", KeyCode::Esc, |app, _ctx| {
                app.close_current_modal();
            })
    }

    // Layers determine which keybinds are active
    fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
        let mut layers = vec![Layer::fill(main_ui).id("main")];

        // When this renders, "confirm_delete" layer binds activate
        if self.show_confirm {
            layers.push(Layer::modal(confirm_ui).id("confirm_delete"));
        }

        // When this renders, "manual_mappings" layer binds activate
        if self.show_manual_mappings {
            layers.push(Layer::modal(mappings_ui).id("manual_mappings"));
        }

        layers
    }
}

// Helper methods for keybind callbacks
impl EntityComparisonApp {
    fn refresh_metadata(&mut self, ctx: &mut Context) {
        // ...
    }

    fn back(&mut self, ctx: &mut Context) {
        ctx.navigate(AppId::ComparisonSelect);
    }

    fn export_to_excel(&mut self, ctx: &mut Context) {
        // ...
    }

    fn create_manual_mapping(&mut self, ctx: &mut Context) {
        // ...
    }

    fn delete_manual_mapping(&mut self, ctx: &mut Context) {
        // ...
    }

    fn close_current_modal(&mut self) {
        // ...
    }
}
```

**Benefits:**
1. ✅ Single definition (no duplication)
2. ✅ Direct callback invocation (no string matching)
3. ✅ Layer-scoped (not full conditional, but good enough)
4. ✅ Static (not rebuilt per frame)
5. ✅ Auto-registers to Options
6. ✅ Supports aliases via Options array storage
7. ✅ Type-safe (callbacks are `fn(&mut App, &mut Context)`)

---

## Performance: Bulk Operations

### Fetch Multiple in One Query

```rust
impl Options {
    /// Get multiple option values in a single query
    pub async fn get_many(&self, keys: &[&str]) -> Result<HashMap<String, OptionValue>> {
        if keys.is_empty() {
            return Ok(HashMap::new());
        }

        // Build SQL: SELECT key, value FROM options WHERE key IN (?, ?, ?)
        let placeholders = keys.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let query = format!(
            "SELECT key, value FROM options WHERE key IN ({})",
            placeholders
        );

        let mut query_builder = sqlx::query(&query);
        for key in keys {
            query_builder = query_builder.bind(key);
        }

        let rows = query_builder.fetch_all(&self.pool).await?;

        let mut results = HashMap::new();
        for row in rows {
            let key: String = row.try_get("key")?;
            let raw_value: String = row.try_get("value")?;

            if let Some(def) = self.registry.get(&key) {
                match self.parse_value(&raw_value, &def.ty) {
                    Ok(value) => { results.insert(key, value); }
                    Err(e) => {
                        log::warn!("Parse failed for '{}': {}. Using default.", key, e);
                        results.insert(key, def.default.clone());  // Registry default
                    }
                }
            }
        }

        // Fill missing keys with registry defaults
        for key in keys {
            if !results.contains_key(*key) {
                if let Some(def) = self.registry.get(key) {
                    results.insert(key.to_string(), def.default.clone());
                }
            }
        }

        Ok(results)
    }

    /// Set multiple option values in a single transaction
    pub async fn set_many(&self, values: &[(&str, OptionValue)]) -> Result<()> {
        if values.is_empty() {
            return Ok(());
        }

        // Validate all first
        for (key, value) in values {
            let def = self.registry.get(key)
                .ok_or_else(|| anyhow::anyhow!("Option '{}' not registered", key))?;
            def.ty.validate(value)?;
        }

        // Transaction
        let mut tx = self.pool.begin().await?;

        for (key, value) in values {
            let raw = self.serialize_value(value)?;
            sqlx::query(
                "INSERT INTO options (key, value, updated_at) VALUES (?, ?, CURRENT_TIMESTAMP)
                 ON CONFLICT(key) DO UPDATE SET value = ?, updated_at = CURRENT_TIMESTAMP"
            )
            .bind(key)
            .bind(&raw)
            .bind(&raw)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}
```

### Prefix-Based Queries

```rust
impl Options {
    /// Get all options matching key prefix (e.g., "api.retry.")
    pub async fn get_by_prefix(&self, prefix: &str) -> Result<HashMap<String, OptionValue>> {
        let pattern = format!("{}%", prefix);
        let rows = sqlx::query("SELECT key, value FROM options WHERE key LIKE ?")
            .bind(&pattern)
            .fetch_all(&self.pool)
            .await?;

        let mut results = HashMap::new();
        for row in rows {
            let key: String = row.try_get("key")?;
            let raw: String = row.try_get("value")?;

            if let Some(def) = self.registry.get(&key) {
                if let Ok(value) = self.parse_value(&raw, &def.ty) {
                    results.insert(key, value);
                }
            }
        }

        // Add registry defaults for registered keys not in DB
        for def in self.registry.list_prefix(prefix) {
            if !results.contains_key(&def.key) {
                results.insert(def.key.clone(), def.default.clone());
            }
        }

        Ok(results)
    }

    /// Convenience: Load entire namespace
    pub async fn get_namespace(&self, namespace: &str) -> Result<HashMap<String, OptionValue>> {
        self.get_by_prefix(&format!("{}.", namespace)).await
    }

    /// Delete all options matching prefix
    pub async fn delete_by_prefix(&self, prefix: &str) -> Result<u64> {
        let pattern = format!("{}%", prefix);
        let result = sqlx::query("DELETE FROM options WHERE key LIKE ?")
            .bind(&pattern)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }
}
```

### Specialized Getters

```rust
impl Options {
    /// Get all keybinds for an app (optimized for RuntimeConfig loading)
    pub async fn get_app_keybinds(&self, app_id: &str) -> Result<HashMap<String, Vec<KeyBinding>>> {
        let prefix = format!("keybind.{}.", app_id);
        let values = self.get_by_prefix(&prefix).await?;

        let mut keybinds = HashMap::new();
        for (key, value) in values {
            // Parse array of keybind strings
            let bindings = match value.as_array() {
                Ok(array) => {
                    let mut parsed_bindings = Vec::new();
                    for val in array {
                        match val.as_string() {
                            Ok(kb_str) => {
                                match KeyBinding::from_str(&kb_str) {
                                    Ok(kb) => parsed_bindings.push(kb),
                                    Err(e) => {
                                        log::error!("Invalid keybind '{}' for '{}': {}", kb_str, key, e);
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("Invalid array element type for '{}': {}", key, e);
                            }
                        }
                    }

                    if parsed_bindings.is_empty() {
                        // All elements invalid, fall back to registry default
                        log::error!("All keybinds invalid for '{}', using registry default", key);
                        self.get_registry_default_keybind(&key)?
                    } else {
                        parsed_bindings
                    }
                }
                Err(e) => {
                    // Not an array, fall back to registry default
                    log::error!("Invalid type for keybind '{}': {}. Using registry default", key, e);
                    self.get_registry_default_keybind(&key)?
                }
            };

            if !bindings.is_empty() {
                // Remove "keybind.app_id." prefix
                let action_key = key.strip_prefix(&prefix).unwrap_or(&key);
                keybinds.insert(action_key.to_string(), bindings);
            }
        }

        Ok(keybinds)
    }

    /// Get registry default keybind for a key, parsed into Vec<KeyBinding>
    fn get_registry_default_keybind(&self, key: &str) -> Result<Vec<KeyBinding>> {
        let def = self.registry.get(key)
            .ok_or_else(|| anyhow::anyhow!("Keybind '{}' not in registry", key))?;

        match &def.default {
            OptionValue::Array(arr) => {
                let mut bindings = Vec::new();
                for val in arr {
                    if let OptionValue::String(kb_str) = val {
                        if let Ok(kb) = KeyBinding::from_str(kb_str) {
                            bindings.push(kb);
                        }
                    }
                }
                Ok(bindings)
            }
            _ => Ok(Vec::new())  // Registry default malformed, return empty
        }
    }
}
```

---

## Registration System

### Startup Flow

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // 1. Create global options registry
    let registry = Arc::new(OptionsRegistry::new());
    init_options_registry(registry.clone());

    // 2. Register system options and global keybinds
    register_system_options(&registry)?;
    register_global_keybinds(&registry)?;

    // 3. Auto-discover V2 options (future: inventory)
    register_discovered_options(&registry)?;

    // 4. Load database
    let config = Config::load().await?;

    // 5. Create runtime
    let mut runtime = MultiAppRuntime::new(config);

    // 6. Register apps (auto-registers their keybinds)
    runtime.register_app::<EntityComparisonApp>()?;
    runtime.register_app::<QueueApp>()?;
    // ...

    // 7. Load runtime config (now all options registered)
    let runtime_config = RuntimeConfig::load_from_options().await?;
    init_runtime_config(runtime_config);

    // 8. Run
    runtime.run().await
}
```

### System Options Registration

```rust
pub fn register_system_options(registry: &OptionsRegistry) -> Result<()> {
    registry.register(
        OptionDefBuilder::new("tui", "focus_mode")
            .display_name("Focus Mode")
            .description("How interactive elements gain keyboard focus")
            .enum_type(vec!["click", "hover", "hover_when_unfocused"], "hover")
            .build()?
    )?;

    registry.register(
        OptionDefBuilder::new("keys", "tab.debouncing")
            .display_name("Tab Debouncing (ms)")
            .description("Debounce duration for tab key")
            .uint_type(150, Some(0), Some(1000))
            .build()?
    )?;

    registry.register(
        OptionDefBuilder::new("theme", "active")
            .display_name("Active Theme")
            .description("Currently active color theme")
            .string_type("mocha", Some(64))
            .build()?
    )?;

    Ok(())
}

pub fn register_global_keybinds(registry: &OptionsRegistry) -> Result<()> {
    // Runtime-level keybinds
    registry.register(
        OptionDefBuilder::new("keybind", "global.help")
            .display_name("Help Menu")
            .description("Toggle context-aware help menu")
            .keybinds_type(vec![KeyBinding::new(KeyCode::F(1))])
            .build()?
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "global.launcher")
            .display_name("App Launcher")
            .description("Open app launcher")
            .keybinds_type(vec![KeyBinding::ctrl(KeyCode::Char('a'))])
            .build()?
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "global.escape")
            .display_name("Escape")
            .description("Progressive unfocus, close layer, or quit")
            .keybinds_type(vec![KeyBinding::new(KeyCode::Esc)])
            .build()?
    )?;

    // Navigation keybinds
    registry.register(
        OptionDefBuilder::new("keybind", "nav.up")
            .display_name("Navigate Up")
            .keybinds_type(vec![KeyBinding::new(KeyCode::Up)])
            .build()?
    )?;

    // ... more nav binds

    Ok(())
}
```

### App Keybind Registration

```rust
impl MultiAppRuntime {
    pub fn register_app<A: App + 'static>(&mut self) -> Result<()> {
        let app_id = A::id();
        let keybinds = A::keybinds();

        // Register global binds
        for bind_def in &keybinds.global_binds {
            let option_key = format!("{}.{}", app_id, bind_def.action);

            self.registry.register(
                OptionDefBuilder::new("keybind", &option_key)
                    .display_name(&bind_def.description)
                    .description(&bind_def.description)
                    .keybinds_type(vec![bind_def.default_key])
                    .build()?
            )?;
        }

        // Register layer-scoped binds
        for (layer_id, layer_binds) in &keybinds.layer_binds {
            for bind_def in layer_binds {
                let option_key = format!("{}.{}.{}", app_id, layer_id, bind_def.action);

                self.registry.register(
                    OptionDefBuilder::new("keybind", &option_key)
                        .display_name(&format!("{} ({})", bind_def.description, layer_id))
                        .description(&bind_def.description)
                        .keybinds_type(vec![bind_def.default_key])
                        .build()?
                )?;
            }
        }

        // Store app factory
        self.apps.insert(app_id.into(), Box::new(AppFactoryImpl::<A>::new()));

        Ok(())
    }
}
```

### Complete Registration Timeline

```
Startup
  |
  ├─> OptionsRegistry::new()
  |
  ├─> register_system_options()
  |    ├─> tui.focus_mode = Enum["click", "hover", "hover_when_unfocused"] (default: "hover")
  |    ├─> keys.tab.debouncing = UInt{min=0, max=1000} (default: 150)
  |    └─> theme.active = String{max=64} (default: "mocha")
  |
  ├─> register_global_keybinds()
  |    ├─> keybind.global.help = Array<String> (default: ["F1"])
  |    ├─> keybind.global.launcher = Array<String> (default: ["Ctrl+a"])
  |    ├─> keybind.global.escape = Array<String> (default: ["Esc"])
  |    └─> keybind.nav.* = ...
  |
  ├─> register_discovered_options() [future: inventory]
  |    └─> ... (V2 derive macro structs)
  |
  ├─> Config::load() [DB connection]
  |
  ├─> MultiAppRuntime::new()
  |
  ├─> register_app<EntityComparisonApp>()
  |    ├─> keybind.entity_comparison.refresh = ["F5"]
  |    ├─> keybind.entity_comparison.back = ["b"]
  |    ├─> keybind.entity_comparison.confirm_delete.yes = ["y"]
  |    ├─> keybind.entity_comparison.confirm_delete.no = ["n"]
  |    └─> ...
  |
  ├─> register_app<QueueApp>()
  |    └─> ...
  |
  ├─> RuntimeConfig::load_from_options()
  |    (All options now registered, safe to load)
  |
  └─> runtime.run()
```

---

## Runtime Dispatch

### Key Event Handling

```rust
impl Runtime {
    fn handle_key_event(&mut self, event: KeyEvent) -> Result<()> {
        let binding = KeyBinding::from(event);

        // 1. Check runtime global keybinds first (F1, Ctrl+A, Esc)
        if self.handle_runtime_keybind(&binding)? {
            return Ok(());
        }

        // 2. Get active layer IDs from last render
        let active_layers = self.get_active_layer_ids();

        // 3. Check layers in reverse order (top to bottom)
        for layer_id in active_layers.iter().rev() {
            if let Some(callback) = self.find_keybind_for_layer(&binding, layer_id) {
                // Call callback directly - no string matching!
                callback(&mut self.current_app, &mut self.ctx);
                return Ok(());
            }
        }

        // 4. Check app global binds
        if let Some(callback) = self.find_global_keybind(&binding) {
            // Call callback directly
            callback(&mut self.current_app, &mut self.ctx);
            return Ok(());
        }

        Ok(())
    }

    fn find_keybind_for_layer<A>(
        &self,
        binding: &KeyBinding,
        layer_id: &str
    ) -> Option<fn(&mut A, &mut Context)> {
        let app_id = self.current_app.id();
        let keybinds = self.current_app.get_keybinds();
        let layer_binds = keybinds.layer_binds.get(layer_id)?;

        for bind_def in layer_binds {
            let action_key = format!("{}.{}.{}", app_id, layer_id, bind_def.action);

            // Check if binding matches any alias (from RuntimeConfig)
            if self.config.matches_keybind(&action_key, binding) {
                return Some(bind_def.callback);
            }
        }
        None
    }

    fn find_global_keybind<A>(
        &self,
        binding: &KeyBinding
    ) -> Option<fn(&mut A, &mut Context)> {
        let app_id = self.current_app.id();
        let keybinds = self.current_app.get_keybinds();

        for bind_def in &keybinds.global_binds {
            let action_key = format!("{}.{}", app_id, bind_def.action);

            // Check if binding matches any alias (from RuntimeConfig)
            if self.config.matches_keybind(&action_key, binding) {
                return Some(bind_def.callback);
            }
        }
        None
    }
}
```

### RuntimeConfig Loading

```rust
impl RuntimeConfig {
    pub async fn load_from_options() -> Result<Self> {
        let config = crate::global_config();

        // Load focus mode (registry provides default)
        let focus_mode_str = config.options.get("tui.focus_mode").await?.as_string()?;
        let focus_mode = match focus_mode_str.as_str() {
            "click" => FocusMode::Click,
            "hover" => FocusMode::Hover,
            "hover_when_unfocused" => FocusMode::HoverWhenUnfocused,
            _ => anyhow::bail!("Invalid focus mode '{}' - database corrupted?", focus_mode_str),
        };

        // Load theme (registry provides default)
        let theme_name = config.options.get("theme.active").await?.as_string()?;
        let theme = load_theme_optimized(&config.options, &theme_name).await?;

        // Load tab debouncing (registry provides default)
        let tab_debouncing_ms = config.options.get("keys.tab.debouncing").await?.as_uint()?;

        // Load all keybinds (bulk fetch per app)
        let registry = config.options.registry();
        let apps = list_apps_from_registry(&registry);

        let mut keybinds = HashMap::new();
        for app in apps {
            let app_keybinds = config.options.get_app_keybinds(&app).await?;
            for (action, bindings) in app_keybinds {
                let full_key = format!("{}.{}", app, action);
                keybinds.insert(full_key, bindings);
            }
        }

        Ok(Self {
            theme,
            focus_mode,
            keybinds,
            tab_debouncing_ms,
        })
    }

    /// Check if any alias matches for this action
    pub fn matches_keybind(&self, action_key: &str, binding: &KeyBinding) -> bool {
        self.keybinds.get(action_key)
            .map(|bindings| bindings.contains(binding))
            .unwrap_or(false)
    }
}
```

---

## Help Menu Integration

```rust
struct HelpSection {
    title: String,
    binds: Vec<(KeyBinding, String)>,
}

impl Runtime {
    fn get_help_sections(&self) -> Vec<HelpSection> {
        let mut sections = vec![];

        // 1. Global runtime keybinds
        let mut global_binds = Vec::new();
        if let Some(kb) = self.get_keybind_primary("global.help") {
            global_binds.push((kb, "Help menu".to_string()));
        }
        if let Some(kb) = self.get_keybind_primary("global.launcher") {
            global_binds.push((kb, "App launcher".to_string()));
        }
        if let Some(kb) = self.get_keybind_primary("global.escape") {
            global_binds.push((kb, "Escape / Back".to_string()));
        }

        sections.push(HelpSection {
            title: "Global".to_string(),
            binds: global_binds,
        });

        // 2. Current app global binds
        let keybinds = self.current_app.get_keybinds();
        let active_layers = self.get_active_layer_ids();

        let app_id = self.current_app.id();
        let mut global_binds = Vec::new();
        for bind_def in &keybinds.global_binds {
            let action_key = format!("{}.{}", app_id, bind_def.action);

            // Get all bindings (primary + aliases)
            if let Some(bindings) = self.config.keybinds.get(&action_key) {
                for (i, binding) in bindings.iter().enumerate() {
                    let desc = if i == 0 {
                        bind_def.description.clone()
                    } else {
                        format!("{} (alias)", bind_def.description)
                    };
                    global_binds.push((*binding, desc));
                }
            }
        }

        if !global_binds.is_empty() {
            sections.push(HelpSection {
                title: format!("{} - Global", self.current_app.get_title()),
                binds: global_binds,
            });
        }

        // 3. Active layer binds
        for layer_id in &active_layers {
            if let Some(layer_binds) = keybinds.layer_binds.get(layer_id) {
                let mut binds = Vec::new();

                for bind_def in layer_binds {
                    let action_key = format!("{}.{}.{}", app_id, layer_id, bind_def.action);

                    if let Some(bindings) = self.config.keybinds.get(&action_key) {
                        for (i, binding) in bindings.iter().enumerate() {
                            let desc = if i == 0 {
                                bind_def.description.clone()
                            } else {
                                format!("{} (alias)", bind_def.description)
                            };
                            binds.push((*binding, desc));
                        }
                    }
                }

                sections.push(HelpSection {
                    title: format!("{} - {}", self.current_app.get_title(), layer_id),
                    binds,
                });
            }
        }

        sections
    }

    fn get_keybind_primary(&self, action: &str) -> Option<KeyBinding> {
        self.config.keybinds.get(action)
            .and_then(|bindings| bindings.first())
            .copied()
    }
}
```

---

## Settings UI

### Auto-Discovery

```rust
fn render_keybind_settings(ui: &mut Ui, registry: &OptionsRegistry, options: &Options) {
    let keybind_opts = registry.list_namespace("keybind");

    // Group by context (global, app, etc)
    let mut by_context: HashMap<String, Vec<OptionDefinition>> = HashMap::new();
    for opt in keybind_opts {
        let parts: Vec<&str> = opt.key.split('.').collect();
        if parts.len() >= 2 {
            by_context.entry(parts[1].to_string())
                .or_insert_with(Vec::new)
                .push(opt);
        }
    }

    // Render global first
    if let Some(global_opts) = by_context.remove("global") {
        ui.section("Global Keybinds", |ui| {
            for opt in global_opts {
                render_keybind_option(ui, &opt, options);
            }
        });
    }

    // Render each app
    for (app_id, opts) in by_context {
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
                render_keybind_option(ui, &opt, options);
            }

            // Render layer-scoped
            for (layer_id, layer_opts) in by_layer {
                ui.subsection(&format!("Layer: {}", layer_id), |ui| {
                    for opt in layer_opts {
                        render_keybind_option(ui, &opt, options);
                    }
                });
            }
        });
    }
}
```

### Array Editor Widget

```rust
// Settings app state
struct SettingsAppState {
    options_values: Resource<HashMap<String, OptionValue>>,
    editing_key: Option<String>,
    capture_mode: Option<CaptureMode>,
}

fn init() -> (Self, Command<Msg>) {
    let load_cmd = Command::perform(async {
        config.options.get_by_prefix("").await
    }, Msg::OptionsLoaded);

    (State {
        options_values: Resource::Loading,
        editing_key: None,
        capture_mode: None,
    }, load_cmd)
}

fn render_keybind_option(
    ui: &mut Ui,
    opt_def: &OptionDefinition,
    values: &HashMap<String, OptionValue>
) {
    ui.label(&opt_def.display_name);

    // Read from pre-loaded cache (no await)
    let current = values.get(&opt_def.key)
        .unwrap_or(&opt_def.default);

    if let OptionValue::Array(bindings) = current {
        // Show each keybind
        for (i, binding_val) in bindings.iter().enumerate() {
            if let OptionValue::String(kb_str) = binding_val {
                ui.row(|ui| {
                    // Badge
                    let style = if i == 0 { Style::primary() } else { Style::secondary() };
                    ui.badge(kb_str, style);

                    // Label
                    let label = if i == 0 { "(primary)" } else { &format!("(alias {})", i) };
                    ui.text(label);

                    // Remove button (if > min_length)
                    if bindings.len() > get_min_length(&opt_def.ty) {
                        ui.button("Remove", || Msg::RemoveKeybind(opt_def.key.clone(), i));
                    }
                });
            }
        }

        // Add alias button
        if bindings.len() < get_max_length(&opt_def.ty).unwrap_or(usize::MAX) {
            ui.button("+ Add Alias", || Msg::AddKeybindAlias(opt_def.key.clone()));
        }
    }
}

// Handle adding alias
fn handle_add_alias(state: &mut State, option_key: String) {
    state.capture_mode = Some(CaptureMode {
        option_key,
        capturing: true,
        captured_key: None,
    });
}

// When key captured
async fn confirm_alias(options: &Options, option_key: &str, new_binding: KeyBinding) -> Result<()> {
    let current = options.get(option_key).await?;

    if let OptionValue::Array(mut bindings) = current {
        let new_str = new_binding.to_string();

        // Check for duplicates
        if !bindings.iter().any(|v| matches!(v, OptionValue::String(s) if s == &new_str)) {
            bindings.push(OptionValue::String(new_str));
            options.set(option_key, OptionValue::Array(bindings)).await?;
        }
    }

    Ok(())
}

fn get_min_length(ty: &OptionType) -> usize {
    match ty {
        OptionType::Array { min_length, .. } => min_length.unwrap_or(0),
        _ => 0,
    }
}

fn get_max_length(ty: &OptionType) -> Option<usize> {
    match ty {
        OptionType::Array { max_length, .. } => *max_length,
        _ => None,
    }
}
```

---

## V2 Derive Macro (Future)

### Declaration

```rust
#[derive(Options)]
#[options(namespace = "api.retry")]
struct RetryOptions {
    /// Automatically retry failed API requests
    #[option(default = true)]
    enabled: bool,

    /// Maximum number of retry attempts (1-10)
    #[option(default = 3, min = 1, max = 10)]
    max_attempts: u64,

    /// Initial delay in milliseconds (100-10000)
    #[option(default = 1000, min = 100, max = 10000)]
    base_delay_ms: u64,
}
```

### Generated Code

```rust
impl RetryOptions {
    // Auto-register to Options system
    pub fn register(registry: &OptionsRegistry) -> Result<()> {
        registry.register(
            OptionDefBuilder::new("api.retry", "enabled")
                .display_name("Enabled")
                .description("Automatically retry failed API requests")
                .bool_type(true)
                .build()?
        )?;

        registry.register(
            OptionDefBuilder::new("api.retry", "max_attempts")
                .display_name("Max Attempts")
                .description("Maximum number of retry attempts (1-10)")
                .uint_type(3, Some(1), Some(10))
                .build()?
        )?;

        registry.register(
            OptionDefBuilder::new("api.retry", "base_delay_ms")
                .display_name("Base Delay Ms")
                .description("Initial delay in milliseconds (100-10000)")
                .uint_type(1000, Some(100), Some(10000))
                .build()?
        )?;

        Ok(())
    }

    // Load from options (single bulk query)
    // No Result - registry guarantees defaults exist, types match
    pub async fn load(options: &Options) -> Self {
        let keys = &[
            "api.retry.enabled",
            "api.retry.max_attempts",
            "api.retry.base_delay_ms",
        ];

        let values = options.get_many(keys).await
            .expect("Registry guarantees these keys exist");

        Self {
            enabled: values["api.retry.enabled"]
                .as_bool()
                .expect("Type mismatch - macro generated wrong type"),
            max_attempts: values["api.retry.max_attempts"]
                .as_uint()
                .expect("Type mismatch - macro generated wrong type"),
            base_delay_ms: values["api.retry.base_delay_ms"]
                .as_uint()
                .expect("Type mismatch - macro generated wrong type"),
        }
    }

    // Compile-time checked keys
    pub mod keys {
        pub const ENABLED: &str = "api.retry.enabled";
        pub const MAX_ATTEMPTS: &str = "api.retry.max_attempts";
        pub const BASE_DELAY_MS: &str = "api.retry.base_delay_ms";
    }
}

// Auto-submit to inventory for discovery
inventory::submit! {
    OptionGroupRegistration {
        namespace: "api.retry",
        register: RetryOptions::register,
    }
}
```

---

## Performance Summary

| Operation | V1 (Individual) | V2 (Optimized) |
|-----------|----------------|----------------|
| Load 6-field struct | 6 queries | 1 query (get_many) |
| Load namespace (20 opts) | 20 queries | 1 query (get_by_prefix) |
| Load all keybinds (30 actions) | 30 queries | ~3-5 queries (per app prefix) |
| Load RuntimeConfig | ~50+ queries | ~5-10 queries |
| Reset namespace | N deletes + N inserts | 1 delete + 1 bulk insert |

---

## Migration from V1

### Step 1: Convert Keybinds

**Before (V1):**
```rust
// registrations/keybinds.rs
registry.register(
    OptionDefBuilder::new("keybind", "entity_comparison.refresh")
        .display_name("Refresh Metadata")
        .keybind_type(KeyCode::F(5))
        .build()?
)?;

// app.rs
fn subscriptions(state: &State) -> Vec<Subscription<Msg>> {
    vec![
        Subscription::keyboard(
            config.get_keybind("entity_comparison.refresh"),
            "Refresh metadata",
            Msg::Refresh
        ),
    ]
}
```

**After (V2):**
```rust
// app.rs - single definition with callback
impl App for EntityComparisonApp {
    fn keybinds() -> KeybindMap<Self> {
        KeybindMap::new()
            .bind("refresh", "Refresh metadata", KeyCode::F(5), Self::refresh_metadata)
    }
}

impl EntityComparisonApp {
    fn refresh_metadata(&mut self, ctx: &mut Context) {
        // Implementation
    }
}
```

### Step 2: Remove Manual Registration Files

Delete:
- `config/options/registrations/keybinds.rs`
- References in `config/options/registrations/mod.rs`

Keybinds now auto-register during `runtime.register_app()`.

### Step 3: Convert Options to Bulk Loading

**Before:**
```rust
let enabled = config.options.get_bool("api.retry.enabled").await?;
let max_attempts = config.options.get_uint("api.retry.max_attempts").await?;
let base_delay = config.options.get_uint("api.retry.base_delay_ms").await?;
// ... 3 queries
```

**After:**
```rust
let keys = &["api.retry.enabled", "api.retry.max_attempts", "api.retry.base_delay_ms"];
let values = config.options.get_many(keys).await
    .expect("Registry guarantees defaults");  // 1 query

let enabled = values["api.retry.enabled"].as_bool()
    .expect("Type guaranteed by registry");
let max_attempts = values["api.retry.max_attempts"].as_uint()
    .expect("Type guaranteed by registry");
let base_delay = values["api.retry.base_delay_ms"].as_uint()
    .expect("Type guaranteed by registry");
```

Or even better, group into a struct with V2 derive macro which uses bulk loading automatically.

---

## Summary

**V2 Options System:**
1. ✅ Array support for keybind aliases
2. ✅ Single-definition keybinds with direct callbacks (no duplication, no string dispatch)
3. ✅ Layer-scoped dispatch (replaces per-frame subscriptions)
4. ✅ Registry as single source of truth (no hardcoded defaults)
5. ✅ Bulk operations (get_many, set_many, get_by_prefix)
6. ✅ Auto-registration during app registration
7. ✅ Type-safe callbacks (`fn(&mut App, &mut Context)`)
8. ✅ Help menu shows all aliases
9. ✅ Settings UI auto-discovers and edits aliases
10. ✅ Massive performance improvements

**Key Principle:** The registry defines defaults exactly once. Loading code trusts the registry and never hardcodes fallback values.

---

**Next:** Explore [Resource Pattern](resource-pattern.md) for async state management, or [Keybinds Deep Dive](../04-user-interaction/keybinds.md) for runtime dispatch details.
