# Options System

**Prerequisites:** [App & Context API](../01-fundamentals/app-and-context.md)

## Overview

The Options system provides type-safe, database-backed configuration with automatic validation and UI generation.

**Key Features:**
- Type-safe storage with validation (Bool, Int, UInt, Float, String, Enum, Array)
- SQLite-backed persistence
- Registry as single source of truth for defaults
- Auto-generated settings UI
- Bulk operations for performance
- Namespace organization

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

        // Try DB first
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

## Type System

```rust
pub enum OptionType {
    Bool,
    Int { min: Option<i64>, max: Option<i64> },
    UInt { min: Option<u64>, max: Option<u64> },
    Float { min: Option<f64>, max: Option<f64> },
    String { max_length: Option<usize> },
    Enum { variants: Vec<String> },

    // Arrays with element constraints
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
    Array(Vec<OptionValue>),
}
```

**Array storage uses JSON in SQLite:**
```sql
INSERT INTO options (key, value) VALUES
    ('keybind.global.save', '["Ctrl+s", "s"]'),  -- Primary + alias
    ('api.endpoints', '["https://api1.com", "https://api2.com"]');
```

---

## Basic Usage

### Registering Options

```rust
pub fn register_app_options(registry: &OptionsRegistry) -> Result<()> {
    registry.register(
        OptionDefBuilder::new("api.retry", "enabled")
            .display_name("Enable Retry")
            .description("Automatically retry failed API requests")
            .bool_type(true)  // default
            .build()?
    )?;

    registry.register(
        OptionDefBuilder::new("api.retry", "max_attempts")
            .display_name("Max Attempts")
            .uint_type(3, Some(1), Some(10))  // default, min, max
            .build()?
    )?;

    Ok(())
}
```

### Reading Options

```rust
// Single value
let enabled = config.options.get("api.retry.enabled").await?.as_bool()?;

// Array
let endpoints = config.options.get("api.endpoints").await?.as_array()?;
```

### Setting Options

```rust
// Single value
options.set("api.retry.enabled", OptionValue::Bool(false)).await?;

// Array
options.set("api.endpoints", OptionValue::Array(vec![
    OptionValue::String("https://api1.com".into()),
    OptionValue::String("https://api2.com".into()),
])).await?;
```

---

## Bulk Operations

For performance, use bulk operations instead of individual queries:

```rust
// Get multiple in one query
let keys = &["api.retry.enabled", "api.retry.max_attempts", "api.retry.base_delay_ms"];
let values = options.get_many(keys).await?;

let enabled = values["api.retry.enabled"].as_bool()?;
let max_attempts = values["api.retry.max_attempts"].as_uint()?;

// Get by prefix (one query)
let all_retry_opts = options.get_by_prefix("api.retry.").await?;

// Set multiple in one transaction
options.set_many(&[
    ("api.retry.enabled", OptionValue::Bool(true)),
    ("api.retry.max_attempts", OptionValue::UInt(5)),
]).await?;
```

**Performance:**
- Load 6-field struct: 6 queries → 1 query (`get_many`)
- Load namespace: 20 queries → 1 query (`get_by_prefix`)
- Load RuntimeConfig: ~50 queries → ~5 queries (prefix per namespace)

---

## V2 Derive Macro

Group related options into structs with automatic registration and loading:

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

**Generated code:**
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
        // ... other fields
        Ok(())
    }

    // Load from options (single bulk query, no Result - registry guarantees defaults)
    pub async fn load(options: &Options) -> Self {
        let keys = &["api.retry.enabled", "api.retry.max_attempts", "api.retry.base_delay_ms"];
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
}
```

**Usage:**
```rust
// Registration (at startup)
RetryOptions::register(&registry)?;

// Loading (single query)
let retry_opts = RetryOptions::load(&options).await;
```

---

## Validation

Types enforce constraints automatically:

```rust
// Range validation
registry.register(
    OptionDefBuilder::new("api", "timeout_ms")
        .uint_type(5000, Some(100), Some(30000))  // default, min, max
        .build()?
)?;

// Will fail validation
options.set("api.timeout_ms", OptionValue::UInt(50)).await?;  // < 100
options.set("api.timeout_ms", OptionValue::UInt(40000)).await?;  // > 30000

// Enum validation
registry.register(
    OptionDefBuilder::new("tui", "focus_mode")
        .enum_type(vec!["click", "hover", "hover_when_unfocused"], "hover")
        .build()?
)?;

// Will fail validation
options.set("tui.focus_mode", OptionValue::String("invalid".into())).await?;
```

---

## Error Recovery

Options handles corrupted database values gracefully:

```rust
impl Options {
    pub async fn get(&self, key: &str) -> Result<OptionValue> {
        let def = self.registry.get(key)?;

        if let Some(raw) = self.get_raw(key).await? {
            match self.parse_value(&raw, &def.ty) {
                Ok(value) => Ok(value),
                Err(e) => {
                    // Log error, fall back to registry default
                    log::error!("Invalid value for '{}': {}. Using default.", key, e);
                    Ok(def.default.clone())
                }
            }
        } else {
            Ok(def.default.clone())
        }
    }
}
```

**Corrupted data never crashes the app** - always falls back to registry defaults.

---

## Common Patterns

### Loading RuntimeConfig

```rust
impl RuntimeConfig {
    pub async fn load_from_options(options: &Options) -> Result<Self> {
        // Load focus mode (registry provides default)
        let focus_mode_str = options.get("tui.focus_mode").await?.as_string()?;
        let focus_mode = match focus_mode_str.as_str() {
            "click" => FocusMode::Click,
            "hover" => FocusMode::Hover,
            "hover_when_unfocused" => FocusMode::HoverWhenUnfocused,
            _ => anyhow::bail!("Invalid focus mode '{}' - database corrupted?", focus_mode_str),
        };

        // Load theme (registry provides default)
        let theme_name = options.get("theme.active").await?.as_string()?;
        let theme = load_theme(&theme_name).await?;

        Ok(Self { theme, focus_mode })
    }
}
```

### Namespace Utilities

```rust
// List all options in a namespace (for settings UI)
let keybind_defs = registry.list_namespace("keybind");

// Delete entire namespace
options.delete_by_prefix("api.retry.").await?;

// Reset to defaults (delete DB values)
for opt in registry.list_namespace("api.retry") {
    options.delete(&opt.key).await?;
}
```

---

## See Also

- **[Keybinds](../04-user-interaction/keybinds.md)** - Keybind system built on Options (arrays, aliases, layer-scoped)
- **[Settings UI](../06-system-features/settings-ui.md)** - Auto-generated settings UI from Options registry
- **[Help System](../06-system-features/help-system.md)** - Help menu integration with keybinds

---

**Next:** Learn about [Keybinds](../04-user-interaction/keybinds.md) or explore [Resource Pattern](resource-pattern.md).
