# Options V2 & Keybind Integration

**Status:** Proposal
**Related:** [Keybinds](04-user-interaction/keybinds.md), [Theme System](05-visual-design/theme-system.md)

---

## Overview

Options V2 is a two-tier configuration system:

1. **Low-level SQLite store** - Async persistence with type validation
2. **High-level derive macros** - Type-safe config structs and auto-registration

**Key improvements over V1:**
- Bulk namespace loading (1 query instead of N)
- Prefix enumeration (for keybind aliases)
- Derive macro for config structs (eliminates boilerplate)
- Dynamic namespaces (for themes, user-defined configs)
- Better integration with V2 keybind system

---

## Architecture

### Two-Tier System

```
┌─────────────────────────────────────────────────┐
│  High-Level: Derive Macros                      │
│  - #[derive(Options)] for configs               │
│  - #[derive(Keybinds)] for app keybinds         │
│  - Type-safe loading/saving                     │
└─────────────────┬───────────────────────────────┘
                  │
┌─────────────────▼───────────────────────────────┐
│  Low-Level: Options Store                       │
│  - SQLite persistence                           │
│  - Async get/set with validation                │
│  - Namespace bulk loading                       │
│  - Prefix enumeration                           │
└─────────────────────────────────────────────────┘
```

---

## Tier 1: Low-Level Options Store

### Core API (V1 - Keep As Is)

```rust
pub struct Options {
    pool: SqlitePool,
    registry: Arc<OptionsRegistry>,
}

impl Options {
    // Single value access
    pub async fn get(&self, key: &str) -> Result<OptionValue>;
    pub async fn set(&self, key: &str, value: OptionValue) -> Result<()>;

    // Type-specific getters
    pub async fn get_bool(&self, key: &str) -> Result<bool>;
    pub async fn get_uint(&self, key: &str) -> Result<u64>;
    pub async fn get_string(&self, key: &str) -> Result<String>;
    // etc.
}

pub struct OptionsRegistry {
    options: RwLock<HashMap<String, OptionDefinition>>,
}

impl OptionsRegistry {
    pub fn register(&self, def: OptionDefinition) -> Result<()>;
    pub fn get(&self, key: &str) -> Option<OptionDefinition>;
    pub fn list_namespace(&self, namespace: &str) -> Vec<OptionDefinition>;
}
```

### New Additions (V2)

```rust
impl Options {
    /// Load all options in a namespace with one query
    /// Returns HashMap with defaults + DB overrides
    pub async fn load_namespace(&self, namespace: &str) -> Result<HashMap<String, OptionValue>> {
        let definitions = self.registry.list_namespace(namespace);
        let mut result = HashMap::new();

        // Start with defaults from registry
        for def in &definitions {
            result.insert(def.key.clone(), def.default.clone());
        }

        // Override with DB values in one query
        let rows = sqlx::query!(
            "SELECT key, value, value_type FROM options WHERE key LIKE ?",
            format!("{}%", namespace)
        )
        .fetch_all(&self.pool)
        .await?;

        for row in rows {
            if let Some(def) = self.registry.get(&row.key) {
                if let Ok(value) = self.deserialize_value(&row.value, &row.value_type, &def.ty) {
                    result.insert(row.key, value);
                }
            }
        }

        Ok(result)
    }

    /// List all keys with a given prefix (for alias enumeration)
    pub async fn list_prefix(&self, prefix: &str) -> Result<Vec<String>> {
        let rows = sqlx::query!(
            "SELECT key FROM options WHERE key LIKE ?",
            format!("{}%", prefix)
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.key).collect())
    }

    /// Get value with automatic fallback to registry default
    pub async fn get_or_default(&self, key: &str) -> Result<OptionValue> {
        match self.get(key).await {
            Ok(value) => Ok(value),
            Err(_) => {
                let def = self.registry.get(key)
                    .ok_or_else(|| format!("Option '{}' not registered", key))?;
                Ok(def.default.clone())
            }
        }
    }
}

impl OptionValue {
    /// Parse from string based on expected type with validation
    pub fn parse(s: &str, expected_type: &OptionType) -> Result<Self, String> {
        match expected_type {
            OptionType::UInt { min, max } => {
                let val = s.parse::<u64>()
                    .map_err(|e| format!("Invalid number: {}", e))?;
                if let Some(min) = min {
                    if val < *min { return Err(format!("Must be >= {}", min)); }
                }
                if let Some(max) = max {
                    if val > *max { return Err(format!("Must be <= {}", max)); }
                }
                Ok(OptionValue::UInt(val))
            }
            OptionType::Int { min, max } => {
                let val = s.parse::<i64>()
                    .map_err(|e| format!("Invalid number: {}", e))?;
                if let Some(min) = min {
                    if val < *min { return Err(format!("Must be >= {}", min)); }
                }
                if let Some(max) = max {
                    if val > *max { return Err(format!("Must be <= {}", max)); }
                }
                Ok(OptionValue::Int(val))
            }
            OptionType::Float { min, max } => {
                let val = s.parse::<f64>()
                    .map_err(|e| format!("Invalid number: {}", e))?;
                if let Some(min) = min {
                    if val < *min { return Err(format!("Must be >= {}", min)); }
                }
                if let Some(max) = max {
                    if val > *max { return Err(format!("Must be <= {}", max)); }
                }
                Ok(OptionValue::Float(val))
            }
            OptionType::Bool => {
                match s.to_lowercase().as_str() {
                    "true" | "1" | "yes" | "on" => Ok(OptionValue::Bool(true)),
                    "false" | "0" | "no" | "off" => Ok(OptionValue::Bool(false)),
                    _ => Err(format!("Invalid boolean: {}", s)),
                }
            }
            OptionType::String { max_length } => {
                if let Some(max) = max_length {
                    if s.len() > *max {
                        return Err(format!("Max length is {}", max));
                    }
                }
                Ok(OptionValue::String(s.to_string()))
            }
            OptionType::Enum { variants } => {
                if variants.contains(&s.to_string()) {
                    Ok(OptionValue::String(s.to_string()))
                } else {
                    Err(format!("Must be one of: {}", variants.join(", ")))
                }
            }
        }
    }

    /// Format for display in UI
    pub fn display(&self) -> String {
        match self {
            Self::Bool(v) => v.to_string(),
            Self::Int(v) => v.to_string(),
            Self::UInt(v) => v.to_string(),
            Self::Float(v) => format!("{:.2}", v),
            Self::String(v) => format!("\"{}\"", v),
        }
    }
}
```

---

## Tier 2: Derive Macros

### For Non-Keybind Configuration

**Example: API Configuration**

```rust
use dynamics_options::Options;

#[derive(Options)]
#[options(namespace = "api")]
struct ApiConfig {
    /// Enable automatic retries for failed requests
    #[option(default = true)]
    retry_enabled: bool,

    /// Maximum retry attempts (1-10)
    #[option(default = 3, min = 1, max = 10)]
    max_attempts: u32,

    /// Base delay between retries in milliseconds
    #[option(default = 500)]
    base_delay_ms: u64,

    /// Backoff multiplier for exponential backoff
    #[option(default = 2.0)]
    backoff_multiplier: f64,
}
```

**Generated Code:**

```rust
impl ApiConfig {
    /// Register all options to the registry (called once at startup)
    pub fn register(registry: &OptionsRegistry) -> Result<()> {
        registry.register(
            OptionDefBuilder::new("api", "retry_enabled")
                .display_name("Enable Retries")
                .description("Enable automatic retries for failed requests")
                .bool_type(true)
                .build()?
        )?;

        registry.register(
            OptionDefBuilder::new("api", "max_attempts")
                .display_name("Max Retry Attempts")
                .description("Maximum retry attempts (1-10)")
                .uint_type(3, Some(1), Some(10))
                .build()?
        )?;

        registry.register(
            OptionDefBuilder::new("api", "base_delay_ms")
                .display_name("Base Delay (ms)")
                .description("Base delay between retries in milliseconds")
                .uint_type(500, None, None)
                .build()?
        )?;

        registry.register(
            OptionDefBuilder::new("api", "backoff_multiplier")
                .display_name("Backoff Multiplier")
                .description("Backoff multiplier for exponential backoff")
                .float_type(2.0, None, None)
                .build()?
        )?;

        Ok(())
    }

    /// Load config from Options store with one bulk query
    pub async fn load(options: &Options) -> Result<Self> {
        let values = options.load_namespace("api").await?;

        Ok(Self {
            retry_enabled: values.get("api.retry_enabled")
                .and_then(|v| v.as_bool().ok())
                .unwrap_or(true),
            max_attempts: values.get("api.max_attempts")
                .and_then(|v| v.as_uint().ok())
                .unwrap_or(3) as u32,
            base_delay_ms: values.get("api.base_delay_ms")
                .and_then(|v| v.as_uint().ok())
                .unwrap_or(500),
            backoff_multiplier: values.get("api.backoff_multiplier")
                .and_then(|v| v.as_float().ok())
                .unwrap_or(2.0),
        })
    }

    /// Save all fields to Options store
    pub async fn save(&self, options: &Options) -> Result<()> {
        options.set("api.retry_enabled", OptionValue::Bool(self.retry_enabled)).await?;
        options.set("api.max_attempts", OptionValue::UInt(self.max_attempts as u64)).await?;
        options.set("api.base_delay_ms", OptionValue::UInt(self.base_delay_ms)).await?;
        options.set("api.backoff_multiplier", OptionValue::Float(self.backoff_multiplier)).await?;
        Ok(())
    }
}
```

**Usage:**

```rust
// V1 (before - 8 individual awaits + repeated defaults)
let retry_enabled = config.options.get_bool("api.retry_enabled").await.unwrap_or(true);
let max_attempts = config.options.get_uint("api.retry_attempts").await.unwrap_or(3) as u32;
let base_delay_ms = config.options.get_uint("api.base_delay_ms").await.unwrap_or(500);
let backoff_multiplier = config.options.get_float("api.backoff_multiplier").await.unwrap_or(2.0);

// V2 (after - 1 await, type-safe)
let api_config = ApiConfig::load(&options).await?;
```

---

## Keybind Integration

### Keybind Storage Pattern

**Namespace:** `keybind.{app_id}.{action_id}`

**Primary binding:**
```
keybind.entity_comparison.save = "Ctrl+s"
```

**Aliases:**
```
keybind.entity_comparison.save.alias1 = "s"
keybind.entity_comparison.save.alias2 = "Ctrl+Shift+s"
```

### App Definition

```rust
use dynamics_tui::prelude::*;

pub struct EntityComparisonApp {
    source_items: Vec<Entity>,
    target_items: Vec<Entity>,
    mappings: Vec<(usize, usize)>,
}

impl App for EntityComparisonApp {
    fn new(ctx: &AppContext) -> Self {
        Self {
            source_items: vec![],
            target_items: vec![],
            mappings: vec![],
        }
    }

    fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
        // ... UI building
    }

    /// Define all keybinds (static method)
    fn keybinds() -> KeybindMap<Self> {
        KeybindMap::new()
            .bind("save", "Save mapping changes",
                  KeyBinding::ctrl('s'),
                  Self::handle_save)

            .bind("map", "Map selected entities",
                  KeyBinding::key('m'),
                  Self::handle_map)

            .bind("unmap", "Remove mapping",
                  KeyBinding::key('u'),
                  Self::handle_unmap)

            .bind("quit", "Exit comparison view",
                  KeyBinding::key('q'),
                  Self::handle_quit)
    }
}

impl EntityComparisonApp {
    async fn handle_save(&mut self, ctx: &mut Context) {
        // Save mappings
    }

    async fn handle_map(&mut self, ctx: &mut Context) {
        // Create mapping
    }

    async fn handle_unmap(&mut self, ctx: &mut Context) {
        // Remove mapping
    }

    async fn handle_quit(&mut self, ctx: &mut Context) {
        ctx.quit();
    }
}
```

### Auto-Registration Macro

```rust
#[derive(Keybinds)]
impl App for EntityComparisonApp {
    // ... keybinds() method above
}

// Macro generates:
impl EntityComparisonApp {
    pub fn register_keybind_options(registry: &OptionsRegistry) -> Result<()> {
        registry.register(
            OptionDefBuilder::new("keybind", "entity_comparison.save")
                .display_name("Save mapping changes")
                .keybind_type(KeyBinding::ctrl('s'))
                .build()?
        )?;

        registry.register(
            OptionDefBuilder::new("keybind", "entity_comparison.map")
                .display_name("Map selected entities")
                .keybind_type(KeyBinding::key('m'))
                .build()?
        )?;

        registry.register(
            OptionDefBuilder::new("keybind", "entity_comparison.unmap")
                .display_name("Remove mapping")
                .keybind_type(KeyBinding::key('u'))
                .build()?
        )?;

        registry.register(
            OptionDefBuilder::new("keybind", "entity_comparison.quit")
                .display_name("Exit comparison view")
                .keybind_type(KeyBinding::key('q'))
                .build()?
        )?;

        Ok(())
    }
}
```

### Runtime Integration

```rust
// At app registration (startup)
impl Runtime {
    pub fn register_app<A: App + Keybinds>(&mut self) -> Result<()> {
        // Register keybinds to Options registry (for Settings UI discovery)
        A::register_keybind_options(&self.options_registry)?;

        // Store keybind map for runtime dispatch
        self.keybind_maps.insert(A::app_id(), A::keybinds());

        Ok(())
    }
}

// During key handling
impl Runtime {
    fn handle_key(&mut self, key_event: KeyEvent) {
        // ... navigation checks first ...

        // Check app keybinds
        let app_id = self.current_app_id();
        if let Some(keymap) = self.keybind_maps.get(&app_id) {
            if let Some(action) = self.lookup_keybind_action(app_id, keymap, key_event) {
                // Call handler
                action.handler.call(&mut self.current_app, &mut self.context).await;
            }
        }
    }

    fn lookup_keybind_action(&self, app_id: AppId, keymap: &KeybindMap, key: KeyEvent) -> Option<&KeybindAction> {
        for action in keymap.actions() {
            let base_key = format!("{}.{}", app_id.as_str(), action.id);

            // Check primary binding (from Options DB or default)
            let primary_option_key = format!("keybind.{}", base_key);
            let binding_str = self.config.options.get_string(&primary_option_key)
                .unwrap_or_else(|_| action.default_key.to_string());

            if let Ok(binding) = KeyBinding::from_str(&binding_str) {
                if binding.matches(&key) {
                    return Some(action);
                }
            }

            // Check aliases
            for i in 1..=10 {
                let alias_key = format!("keybind.{}.alias{}", base_key, i);
                if let Ok(alias_str) = self.config.options.get_string(&alias_key) {
                    if let Ok(binding) = KeyBinding::from_str(&alias_str) {
                        if binding.matches(&key) {
                            return Some(action);
                        }
                    }
                }
            }
        }

        None
    }
}
```

### Settings UI

**Discovery:**
```rust
// Settings app queries registry to discover all keybinds
let all_keybind_defs = registry.list_namespace("keybind");

// Group by app
let mut by_app: HashMap<String, Vec<OptionDefinition>> = HashMap::new();
for def in all_keybind_defs {
    // Extract app_id from key: "keybind.entity_comparison.save" -> "entity_comparison"
    let parts: Vec<&str> = def.key.split('.').collect();
    if parts.len() >= 2 {
        let app_id = parts[1];
        by_app.entry(app_id.to_string())
            .or_insert_with(Vec::new)
            .push(def);
    }
}
```

**Display:**
```
╭─ Global Navigation ───────────────────────────╮
│ Navigate Up       ↑, k                        │
│ Navigate Down     ↓, j                        │
├─ Global Actions ──────────────────────────────┤
│ Help Menu         F1                          │
│ App Launcher      Ctrl+Space                  │
├─ Entity Comparison ───────────────────────────┤
│ Save              Ctrl+s, s                   │  ← Primary + alias1
│ Map               m                           │
│ Unmap             u                           │
│ Quit              q                           │
╰───────────────────────────────────────────────╯

[Edit Selected] [Add Alias] [Remove Alias] [Reset to Default]
```

**Adding an alias:**
```rust
// User clicks "Add Alias" for "Save" action
// Settings UI calls:
async fn add_alias(action_key: &str, new_binding: KeyBinding) -> Result<()> {
    let options = global_options();

    // Find next free alias slot
    let existing = options.list_prefix(&format!("keybind.{}", action_key)).await?;
    let next_num = existing.len(); // If primary + 2 aliases exist, this is 3

    // Set the alias
    options.set(
        &format!("keybind.{}.alias{}", action_key, next_num),
        OptionValue::String(new_binding.to_string())
    ).await?;

    Ok(())
}
```

**User never manually types namespace strings - all UI-driven.**

---

## Dynamic Namespaces (Themes)

**Problem:** Theme name is user-defined, can't be in struct name.

**Solution:** Dynamic namespace parameter.

```rust
#[derive(Options)]
#[options(namespace = "theme.{name}", dynamic)]  // {name} substituted at runtime
struct ThemeColors {
    #[option(default = "#89b4fa")]
    accent_primary: String,

    #[option(default = "#f5c2e7")]
    accent_secondary: String,

    // ... 19 more colors
}

// Generated with dynamic namespace support:
impl ThemeColors {
    /// Load theme by name
    pub async fn load(name: &str, options: &Options) -> Result<Self> {
        let namespace = format!("theme.{}", name);
        let values = options.load_namespace(&namespace).await?;

        Ok(Self {
            accent_primary: values.get(&format!("{}.accent_primary", namespace))
                .and_then(|v| v.as_string().ok())
                .unwrap_or("#89b4fa".to_string()),
            accent_secondary: values.get(&format!("{}.accent_secondary", namespace))
                .and_then(|v| v.as_string().ok())
                .unwrap_or("#f5c2e7".to_string()),
            // ...
        })
    }

    /// Save theme under a given name
    pub async fn save(&self, name: &str, options: &Options) -> Result<()> {
        let namespace = format!("theme.{}", name);
        options.set(&format!("{}.accent_primary", namespace), OptionValue::String(self.accent_primary.clone())).await?;
        options.set(&format!("{}.accent_secondary", namespace), OptionValue::String(self.accent_secondary.clone())).await?;
        // ...
        Ok(())
    }
}
```

**Usage:**
```rust
// Load built-in themes
let mocha = ThemeColors::load("mocha", &options).await?;
let latte = ThemeColors::load("latte", &options).await?;

// Load user's custom theme
let my_theme = ThemeColors::load("my_custom_purple", &options).await?;
```

---

## Benefits Over V1

| Issue in V1 | V2 Solution | Impact |
|-------------|-------------|--------|
| Sequential awaits for config loading | `load_namespace()` bulk query | 10-20x fewer DB queries |
| Repeated default values | Derive macro generates from single source | No sync issues |
| Manual type conversion everywhere | `OptionValue::parse()` + derive macro | 75% less boilerplate |
| String-based enum serialization | Serde integration (future) | Type-safe round-trips |
| No alias support | `list_prefix()` + UI helpers | Keybind customization |
| Manual registration code | Derive macro auto-generation | Zero registration boilerplate |
| No theme customization | Dynamic namespaces | User-defined themes |

---

## Implementation Plan

### Phase 1: Low-Level Additions (Week 1)
- [ ] Implement `Options::load_namespace()`
- [ ] Implement `Options::list_prefix()`
- [ ] Implement `Options::get_or_default()`
- [ ] Implement `OptionValue::parse()` and `OptionValue::display()`
- [ ] Add tests for new methods

### Phase 2: Derive Macro for Configs (Week 2)
- [ ] Create `#[derive(Options)]` proc macro in `dynamics-lib-macros`
- [ ] Support basic types: bool, int, uint, float, string
- [ ] Generate `register()`, `load()`, `save()` methods
- [ ] Add validation attributes (min, max, etc.)
- [ ] Test with ApiConfig example

### Phase 3: Keybind Integration (Week 3)
- [ ] Create `#[derive(Keybinds)]` proc macro
- [ ] Parse `keybinds()` method AST
- [ ] Generate `register_keybind_options()` method
- [ ] Update Runtime to call registration at app startup
- [ ] Update Runtime key dispatch to check Options for customizations

### Phase 4: Settings UI (Week 4)
- [ ] Settings app discovers keybinds from registry
- [ ] Display primary + aliases for each action
- [ ] UI for adding/removing aliases
- [ ] UI for editing primary binding
- [ ] Conflict detection warnings

### Phase 5: Dynamic Namespaces (Week 5)
- [ ] Add `dynamic` flag to `#[options]` attribute
- [ ] Support `{name}` placeholder in namespace
- [ ] Generate `load(name: &str)` and `save(name: &str)` signatures
- [ ] Test with ThemeColors example

### Phase 6: Migration (Week 6+)
- [ ] Migrate RuntimeConfig to use derive macro
- [ ] Migrate API config to use derive macro
- [ ] Remove old manual registration code
- [ ] Update documentation

---

## Open Questions

1. **Keybind conflicts:** Should Options system detect conflicts, or is it purely UI concern?
2. **Preset system:** How to implement "vim mode" preset? Bulk transaction API?
3. **Field-level saves:** Should derive macro support saving individual fields, or always save entire struct?
4. **Cache invalidation:** If Options changes in DB, how do loaded config structs get notified?
5. **Color type:** Should Options support OKLCH type natively, or use string serialization?

---

## Related Files

- V1 Implementation: `dynamics-cli/src/config/options/`
- Keybind Spec: `docs/v2/04-user-interaction/keybinds.md`
- Theme Spec: `docs/v2/05-visual-design/theme-system.md`
