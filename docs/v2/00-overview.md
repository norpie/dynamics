# TUI Framework V2 - Overview

**Status**: Design phase - Not ready for implementation

## Core Philosophy

### Immediate Mode + Structured Concurrency

V2 adopts an immediate-mode approach where apps have direct control:

- Apps get `&mut self` during render - direct mutation, no message passing
- Async is first-class via Resource/tasks
- Event-driven rendering only (no FPS loop)
- Zero boilerplate for common patterns

**See Also:**
- [App & Context API](01-fundamentals/app-and-context.md) - Core interfaces
- [Resource Pattern](03-state-management/resource-pattern.md) - Async state management
- [Event-Driven Rendering](01-fundamentals/event-loop.md) - Rendering model

### What We're Fixing from V1

V2 addresses major pain points from the V1 architecture:

1. **Message Explosion** - Every interaction needs a Msg enum variant
2. **Command Ceremony** - Side effects require wrapping in Command::Perform
3. **Navigation/Focus Boilerplate** - Manual event routing, on_render callbacks
4. **Multi-View Hacks** - Separate apps when they should share state
5. **Hardcoded Layers** - GlobalUI, AppModal, etc. baked into framework
6. **Viewport Dimension Hacks** - "20" fallback + on_render callback for real size
7. **Keybind Hell** - Hardcoded keys, no user configuration

**See Also:**
- [V1 vs V2 Comparison](08-reference/v1-vs-v2-comparison.md) - Detailed comparison table
- [Migration Guide](08-reference/migration-guide.md) - Step-by-step conversion
- [Component Patterns](04-user-interaction/component-patterns.md) - Replacing message passing
- [Multi-View Routing](03-state-management/routing.md) - Replacing separate apps
- [Layer System](02-building-ui/layers.md) - Flexible layer composition
- [Keybinds](04-user-interaction/keybinds.md) - Configurable keybind system

## Non-Goals

*(Content to be added from v2.md L6912)*

## Next Steps

*(Content to be added from v2.md L6921)*

---

**Next:** Start with [App & Context API](01-fundamentals/app-and-context.md) to understand the core interfaces.
