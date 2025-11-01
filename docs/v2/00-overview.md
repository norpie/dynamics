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
8. **System Feature Sprawl** - Adding features (help, launcher) requires runtime changes

**See Also:**
- [V1 vs V2 Comparison](08-reference/v1-vs-v2-comparison.md) - Detailed comparison table
- [Migration Guide](08-reference/migration-guide.md) - Step-by-step conversion
- [Component Patterns](04-user-interaction/component-patterns.md) - Replacing message passing
- [Multi-View Routing](03-state-management/routing.md) - Replacing separate apps
- [Layer System](02-building-ui/layers.md) - Flexible layer composition
- [Keybinds](04-user-interaction/keybinds.md) - Configurable keybind system
- [Plugin System](06-system-features/plugin-system.md) - Extensible system features

---

## Documentation Index

### 01. Fundamentals
Core concepts and architecture:
- [App & Context API](01-fundamentals/app-and-context.md) - App trait, Context API
- [Lifecycle](01-fundamentals/lifecycle.md) - App lifecycle, hooks, navigation behavior
- [Event Loop](01-fundamentals/event-loop.md) - Event-driven rendering model
- **Elements** *(TODO)* - Element tree concepts

### 02. Building UI
Declarative UI construction:
- [Layout](02-building-ui/layout.md) - Layout primitives, constraints, macros
- [Layers](02-building-ui/layers.md) - Layer system, positioning, stacking
- [Components](02-building-ui/components.md) - Component system, state composition
- [Modals](02-building-ui/modals.md) - Modal patterns, builders, dismissal

### 03. State Management
Managing app state and async operations:
- [Resource Pattern & Error Handling](03-state-management/resource-pattern.md) - Async state, progress tracking, error recovery
- [Routing](03-state-management/routing.md) - Multi-view apps

### 04. User Interaction
Input handling and navigation:
- [Keybinds](04-user-interaction/keybinds.md) - Declarative keybinds, configuration
- [Focus](04-user-interaction/focus.md) - Focus system, layer-scoped focus
- [Mouse](04-user-interaction/mouse.md) - Mouse support, hit testing
- **Navigation** *(TODO)* - Widget navigation patterns
- [Component Patterns](04-user-interaction/component-patterns.md) - Interaction callbacks

### 05. Visual Design
Theming and visual effects:
- [Color System](05-visual-design/color-system.md) - OKLCH color space
- [Theme System](05-visual-design/theme-system.md) - Themes, persistence, runtime switching
- [Animation](05-visual-design/animation.md) - Frame timing, toasts, drag & drop

### 06. System Features
Framework-provided features:
- [Plugin System](06-system-features/plugin-system.md) - Runtime extensions with system-level access
- [App Launcher](06-system-features/app-launcher.md) - Ctrl+Space launcher
- [Help System](06-system-features/help-system.md) - F1 context-aware help
- [Background Apps](06-system-features/background-apps.md) - Background execution

### 07. Advanced Topics
Advanced patterns and systems:
- [Events & Queues](07-advanced/events-and-queues.md) - Event broadcast, work queues
- [Background Work](07-advanced/background-work.md) - Background tasks, invalidation
- [Navigable State](07-advanced/navigable-state.md) - Unified 2D navigation

### 08. Reference
Reference materials:
- **Migration Guide** *(TODO)* - Converting V1 apps to V2
- **V1 vs V2 Comparison** *(TODO)* - Side-by-side comparison
- **Glossary** *(TODO)* - Term index
- [Open Questions](08-reference/open-questions.md) - Future considerations

---

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
