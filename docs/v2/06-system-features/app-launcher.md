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

```rust
struct AppLauncherState {
    entries: Vec<AppLauncherEntry>,
    filtered_entries: Vec<AppLauncherEntry>,
    search_query: String,
    selected_index: usize,
    last_active_time: HashMap<String, Instant>,
}

impl AppLauncher {
    fn update_entries(&mut self, runtime: &Runtime) {
        self.entries = runtime.list_apps().map(|app| {
            AppLauncherEntry {
                app_id: app.id(),
                name: app.name(),
                description: app.description(),
                status: if app.is_active() {
                    AppStatus::Running
                } else if app.is_background() {
                    AppStatus::Background
                } else {
                    AppStatus::NotStarted
                },
            }
        }).collect();

        sort_app_entries(&mut self.entries, &self.last_active_time);
        self.update_filtered_entries();
    }

    fn update_filtered_entries(&mut self) {
        self.filtered_entries = filter_entries(&self.entries, &self.search_query);
    }

    fn handle_select(&mut self, ctx: &mut Context) {
        if let Some(entry) = self.filtered_entries.get(self.selected_index) {
            ctx.router.navigate(&entry.app_id);
            self.last_active_time.insert(entry.app_id.clone(), Instant::now());
            ctx.close_launcher();
        }
    }
}
```

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
