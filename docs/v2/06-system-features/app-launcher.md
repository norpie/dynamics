# App Launcher (Ctrl+Space)

**Prerequisites:** [Lifecycle](../01-fundamentals/lifecycle.md)

## Overview

**Ctrl+Space** brings up a modal showing all registered apps with search and status indicators.

V2 simplifies the launcher significantly from V1 - since apps are now true background-able entities (rather than every view being its own app), we can show ALL registered apps in a single searchable list with their current status.

## Core Concept

- **No categories** - One flat list sorted by recency/alphabetically
- **Status indicators** - Show whether each app is running, backgrounded, or not started
- **Search** - Simple substring match on name/description

## App List Structure

```rust
pub struct AppLauncherEntry {
    pub app_id: String,      // e.g., "migration_environments"
    pub name: String,
    pub description: String,
    pub status: AppStatus,
}

pub enum AppStatus {
    Running,      // Currently active (shown to user)
    Background,   // Running but not visible
    NotStarted,   // Never created or was destroyed
}
```

## Sorting Logic

**Apps are sorted by:**
1. **Status priority:** Running → Background → NotStarted
2. **Within status groups:**
   - Running/Background: Sort by recency (most recently active first)
   - NotStarted: Sort alphabetically by name

**Example ordering:**
```
[Running]
  • Entity Comparison           (active now)

[Background - Recent]
  • Migration Environments      (used 2 min ago)
  • Operation Queue             (used 5 min ago)

[Background - Older]
  • Deadlines                   (used 1 hour ago)

[Not Started]
  • Environment Selector
  • Settings
  • Updates
```

## Search/Filtering

**Simple substring matching** - case-insensitive search across name + description:

```rust
fn filter_entries(entries: &[AppLauncherEntry], query: &str) -> Vec<AppLauncherEntry> {
    if query.is_empty() {
        return entries.to_vec();
    }

    let query_lower = query.to_lowercase();
    entries.iter()
        .filter(|entry| {
            entry.name.to_lowercase().contains(&query_lower) ||
            entry.description.to_lowercase().contains(&query_lower)
        })
        .cloned()
        .collect()
}
```

**No fuzzy matching** - keep it simple.

## UI Layout

**Modal overlay** (centered, ~60x20):

```
╭─ App Launcher ────────────────────────────────────────────╮
│ Search: migr_                                             │
│                                                            │
│ ▶ Migration Environments                        [Running] │
│   Manage Dynamics 365 migrations                          │
│                                                            │
│ • Operation Queue                             [Background] │
│   Manage and execute API operation batches                │
│                                                            │
│ • Migration Comparison                        [Background] │
│   Compare entity metadata between environments            │
│                                                            │
│                                                            │
│ [↑↓ Navigate | Enter Select | Esc Cancel]                 │
╰────────────────────────────────────────────────────────────╯
```

**Status indicators:**
- `▶` - Currently running (active)
- `•` - Background (not visible but running)
- ` ` (no icon) - Not started

**Status badge colors:**
- `[Running]` - Accent primary (blue/lavender)
- `[Background]` - Text dim (gray)
- `[Not Started]` - Text muted (very dim gray)

## Implementation

### State

```rust
struct AppLauncherState {
    search_query: String,
    text_input_state: TextInputState,
    list_state: ListState,
    all_entries: Vec<AppLauncherEntry>,       // Full list (pre-sorted)
    filtered_entries: Vec<AppLauncherEntry>,  // After search filter
}
```

### View

```rust
fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
    vec![
        Layer::fill(self.main_ui()),

        Layer::centered(60, 20, panel("App Launcher", |ui| {
            // Search input
            ui.text_input(&mut self.search_query)
                .placeholder("Search apps...")
                .on_change(Self::on_search_changed);

            ui.spacer(1);

            // App list
            ui.list(&self.filtered_entries)
                .on_activate(Self::on_select);

            ui.spacer(1);

            // Footer hint
            ui.text("[↑↓ Navigate | Enter Select | Esc Cancel]")
                .style(ctx.theme.text_dim)
                .align(Align::Center);
        }))
        .dim_below(true),
    ]
}
```

### Discovery Pattern: Inventory Crate

**Apps auto-register using the `inventory` crate** - no manual registration:

```rust
use inventory;

pub struct AppRegistration {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub constructor: fn(&AppContext) -> Box<dyn App>,
    pub lifecycle: Lifecycle,
}

inventory::collect!(AppRegistration);
```

**Each app file has ONE inventory submission:**

```rust
// At the bottom of migration_environment_app.rs
inventory::submit! {
    AppRegistration {
        id: "migration_environments",
        name: "Migration Environments",
        description: "Manage Dynamics 365 migration environments",
        constructor: |ctx| Box::new(MigrationEnvironmentApp::new(ctx)),
        lifecycle: Lifecycle::Background,
    }
}
```

**Runtime auto-discovers all apps at compile time:**

```rust
impl Runtime {
    pub fn new() -> Self {
        let registrations: HashMap<String, &'static AppRegistration> =
            inventory::iter::<AppRegistration>()
                .map(|reg| (reg.id.to_string(), reg))
                .collect();

        Self {
            registrations,
            active_app: "app_launcher".to_string(),
            background_apps: HashMap::new(),
            // ...
        }
    }

    pub fn navigate_to(&mut self, app_id: &str) {
        let reg = self.registrations.get(app_id).expect("Unknown app ID");
        let app = (reg.constructor)(&self.app_context);
        self.active_app = app_id.to_string();
    }
}
```

**Adding a new app:**
1. Create app file implementing `App` trait
2. Add `inventory::submit!` at the bottom
3. Done! App appears in launcher automatically

## Keybinds

**While launcher is open:**
- **Up/Down** - Navigate list
- **Enter** - Launch/switch to selected app
- **Esc** - Close launcher
- **Type** - Filter by search query
- **Backspace** - Remove character from search

## Benefits

✅ **Simple flat list** - No category confusion
✅ **Status awareness** - Know what's running/backgrounded
✅ **Recency sorting** - Recent apps at top
✅ **Fast search** - Substring matching
✅ **Keyboard-friendly** - Type to filter, Enter to launch

**See Also:**
- [Lifecycle](../01-fundamentals/lifecycle.md) - Background apps
- [Routing](../03-state-management/routing.md) - Navigation between apps
- [Modals](../02-building-ui/modals.md) - Modal overlay pattern

---

**Next:** Learn about [Background Apps](background-apps.md) or explore [Settings](settings.md).
