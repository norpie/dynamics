# Multi-View Routing

**Prerequisites:** [App & Context API](../01-fundamentals/app-and-context.md)

## The Problem

V1 forced separate apps when they should share state. Example: Deadlines has FileSelect → Mapping → Inspection as 3 separate apps, but they all need access to the same file data and mappings.

## The Solution: Router

V2 provides `ctx.router` to navigate between views within a single app:

```rust
struct DeadlinesApp {
    // Shared state across all views
    file: Option<PathBuf>,
    mappings: HashMap<String, String>,
    parsed_data: Resource<Vec<Deadline>>,

    // View-specific state
    file_browser: FileBrowserState,
    mapping_list: ListState,
    inspection_tree: TreeState,
}

impl App for DeadlinesApp {
    fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
        let ui = match ctx.router.current() {
            VIEW_FILE_SELECT => self.view_file_select(ctx),
            VIEW_MAPPING => self.view_mapping(ctx),
            VIEW_INSPECTION => self.view_inspection(ctx),
            _ => empty(),
        };
        vec![Layer::fill(ui)]
    }

    fn view_file_select(&mut self, ctx: &mut Context) -> Element {
        // ... handle file selection
        ctx.router.navigate(VIEW_MAPPING);  // Navigate to next view
    }
}
```

**Key benefits:**
- **Shared state** - All views access same app state
- **Type safety** - View methods have access to `&mut self`
- **Direct mutation** - No message passing between views
- **Simple navigation** - `ctx.router.navigate(VIEW_ID)`

**See Also:**
- [Resource Pattern](resource-pattern.md) - Async data loading across views
- [Component Patterns](../04-user-interaction/component-patterns.md) - View interaction patterns
- [Lifecycle](../01-fundamentals/lifecycle.md) - View lifecycle hooks

---

**Next:** Learn about [Pub/Sub](pubsub.md) for cross-app communication.
