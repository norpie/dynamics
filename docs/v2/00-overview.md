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

V2 deliberately excludes certain features to maintain focus and simplicity:

- **V1 Compatibility** - Clean slate, rewrite apps from scratch
- **Web/GUI Support** - TUI only, don't over-abstract
- **Time-Travel Debug** - Nice to have but not priority

**Rationale:**
- **V1 Compatibility:** A clean break allows removing architectural debt without compromise
- **Multi-platform:** Abstracting for web/GUI would complicate the API unnecessarily
- **Debug features:** Focus on getting the core right first, tooling can come later

**See Also:**
- [Open Questions](08-reference/open-questions.md) - Future considerations

## Next Steps

Implementation roadmap for V2:

1. **Continue brainstorming edge cases** - Validate design against real-world scenarios
2. **Prototype core abstractions** - Build Context, Layer, Resource types
3. **Build 1-2 example apps** - Validate API ergonomics with real apps
4. **Iterate on ergonomics** - Refine based on prototype feedback
5. **Implementation plan** - Separate from design docs, focus on execution

**Current Phase:** Design & documentation (this document set)

**See Also:**
- [Migration Guide](08-reference/migration-guide.md) - Converting V1 apps to V2
- [Open Questions](08-reference/open-questions.md) - Unresolved design items

---

**Next:** Start with [App & Context API](01-fundamentals/app-and-context.md) to understand the core interfaces.
