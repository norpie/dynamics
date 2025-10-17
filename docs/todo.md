# V2 Documentation Split Progress

## Directory Structure

```
docs/v2/
├── [ ] README.md
├── [x] 00-overview.md
│
├── 01-fundamentals/
│   ├── [x] app-and-context.md
│   ├── [x] lifecycle.md
│   ├── [x] event-loop.md
│   └── [ ] elements.md
│
├── 02-building-ui/
│   ├── [x] layout.md
│   ├── [x] layers.md
│   ├── [x] components.md
│   └── [x] modals.md
│
├── 03-state-management/
│   ├── [x] resource-pattern.md
│   ├── [x] error-recovery.md
│   ├── [x] pubsub.md
│   └── [x] routing.md
│
├── 04-user-interaction/
│   ├── [x] keybinds.md (TODO: alternative system noted)
│   ├── [x] focus.md
│   ├── [x] mouse.md
│   ├── [ ] navigation.md
│   └── [x] component-patterns.md
│
├── 05-visual-design/
│   ├── [x] color-system.md
│   ├── [x] theme-system.md
│   └── [x] animation.md
│
├── 06-system-features/
│   ├── [x] app-launcher.md
│   ├── [x] help-system.md
│   └── [x] background-apps.md
│
├── 07-advanced/
│   ├── [x] events-and-queues.md
│   ├── [x] background-work.md
│   ├── [x] navigable-state.md
│   └── [x] containers-alignment.md
│
└── 08-reference/
    ├── [ ] migration-guide.md
    ├── [ ] v1-vs-v2-comparison.md
    ├── [ ] glossary.md
    └── [ ] open-questions.md
```

## Content Mapping: v2.md → docs/v2/

### README.md
- Custom content (navigation hub, learning paths)

### 00-overview.md
- Core Philosophy
- What We're Fixing from V1
- Non-Goals
- Next Steps

### 01-fundamentals/app-and-context.md
- Architecture → App Trait
- Architecture → Context API
- Architecture → Example: Simple App

### 01-fundamentals/lifecycle.md
- Lifecycle & App Management (entire section)
  - Simplified Lifecycle Model
  - Lifecycle Policy
  - When update() Is Called
  - Runtime Navigation Behavior
  - Global Quit Coordination
  - Memory Pressure Management
  - Hook Details
  - Why Hooks Are Sync
  - Benefits

### 01-fundamentals/event-loop.md
- Event-Driven Rendering

### 01-fundamentals/elements.md
- Basic element tree concepts (extract from various examples)

### 02-building-ui/layout.md
- Layout System (entire section)
  - Layout Primitives
  - Constraint System
  - Auto-Constraints
  - Nesting Behavior
  - Alignment System
  - Container Features (Panel, Container)
  - Macros: Binned (note about removal)

### 02-building-ui/layers.md
- Layer System (Simple Stack)
  - Layer struct definition
  - LayerArea enum
  - Multi-Layer Example
  - Global UI = Just Another Layer
  - Layer Positioning

### 02-building-ui/components.md
- Component System (entire section)
  - Terminology
  - Component State Composition
  - Shared Styling Helpers
  - Benefits

### 02-building-ui/modals.md
- Modal System (entire section)
  - Modals Are Just Layers
  - Hybrid Approach
  - Raw Layers
  - Builder Helpers
  - Built-in Modal Helpers
  - Modal Dismissal
  - Context Menu Pattern

### 03-state-management/resource-pattern.md
- Resource Pattern (Auto-Managed Async)
- Resource Progress Tracking (entire section)

### 03-state-management/error-recovery.md
- Resource Error Recovery (entire section)
  - Error Types
  - ResourceError Constructors
  - Automatic From Implementations
  - Resource Helper Methods
  - Usage Examples
  - UI Rendering with Retry
  - Error Kind Guidelines

### 03-state-management/pubsub.md
- Pub/Sub (Auto-Managed)

### 03-state-management/routing.md
- Multi-View Apps

### 04-user-interaction/keybinds.md
- Keybinds (First-Class)
  - Declarative Definition
  - User Configuration
  - Automatic Widget Navigation
  - Button Keybind Integration
- Layout System → Keybind System (nested section)
  - Three Binding Categories
  - Navigation Actions
  - Alias System
  - Keybind Registration
  - Runtime Key Handling
  - Component Navigation Handling
  - Keybind Presets
  - Conditional Keybinds
  - Conflict Detection
  - Settings UI

### 04-user-interaction/focus.md
- Focus System (entire section)
  - Automatic Focus Order
  - Layer-Scoped Focus
  - Programmatic Focus
  - User Navigation Takes Precedence
  - Progressive Unfocus
  - Focus Modes
  - Focus Context API
  - Implementation Details
  - Focus Integration (from Mouse Support)

### 04-user-interaction/mouse.md
- Mouse Support (entire section)
  - Hit Testing
  - Inline Event Handling
  - Automatic Hover Styles
  - Automatic Scroll Wheel
  - Double-Click vs Single-Click
  - Right-Click / Context Menus
  - MouseState API
  - Terminal Mouse Capture

### 04-user-interaction/navigation.md
- Keybinds → Automatic Widget Navigation (concepts)
- Focus System → Automatic Focus Order (tab/shift-tab)

### 04-user-interaction/component-patterns.md
- Component Interaction Patterns (entire section)
  - V1 Problems
  - V2 Solution
  - Callback Patterns by Component Type
  - Component State Management
  - Component State: Automatic vs Semantic
  - Multiple Components of Same Type
  - Keybind Integration
  - Migration from V1

### 05-visual-design/color-system.md
- Color System (OKLCH) (standalone section)
- Layout System → Theme System → Color System (nested)

### 05-visual-design/theming.md
- Layout System → Theme System (nested section)
  - Theme Structure
  - StyleConfig
  - Usage Examples
  - Persistence
  - Runtime Switching
  - File Structure

### 05-visual-design/animation.md
- Animation System (entire section)
  - Frame Timing
  - Toast System
  - Drag & Drop
  - Animation Easing

### 06-system-features/app-launcher.md
- Layout System → App Launcher (nested section)
  - Core Concept
  - App List Structure
  - Sorting Logic
  - Search/Filtering
  - UI Layout
  - Implementation
  - Discovery Pattern: Inventory Crate
  - Keybinds
  - Benefits

### 06-system-features/help-system.md
- Layout System → Context-Aware Help (nested section)
  - Core Concept
  - Help Entry Structure
  - Help Generation
  - Component Nav Action Reporting
  - Help Modal Rendering
  - Example Output
  - Runtime Help Toggle
  - No Custom Help Content

### 06-system-features/background-apps.md
- Background Apps

### 07-advanced/events-and-queues.md
- Events & Queue System (entire section under Container Features)
  - Event Broadcast System
  - Work Queue System
  - Type Safety Architecture
  - Usage Guidelines
  - Examples

### 07-advanced/background-work.md
- Background Work + Invalidation

### 07-advanced/navigable-state.md
- Component System → NavigableState: Unified 2D Navigation

### 07-advanced/containers-alignment.md
- Container Features (top-level section)
- Alignment & Positioning
- Constraints System (reference section)

### 08-reference/migration-guide.md
- Migration from V1 (extract all "Migration from V1" subsections)
- Step-by-step conversion guide

### 08-reference/v1-vs-v2-comparison.md
- What We're Fixing from V1 (expanded into comparison table)

### 08-reference/glossary.md
- Custom content (term index with cross-links)

### 08-reference/open-questions.md
- Open Questions / TODO

## Heading-by-Heading Processing Order

Process sections in order. When contradictions are found, **ASK** which version is correct.

- [x] Core Philosophy (L5) → 00-overview.md
  - [x] Immediate Mode + Structured Concurrency (L7)
  - [x] What We're Fixing from V1 (L14)
- [x] Architecture (L26) → 01-fundamentals/app-and-context.md
  - [x] App Trait (L28)
  - [x] Context API (L51)
  - [x] Example: Simple App (L69)
- [x] Multi-View Apps (L115) → 03-state-management/routing.md
- [x] Pub/Sub (Auto-Managed) (L189) → 03-state-management/pubsub.md
- [x] Resource Pattern (Auto-Managed Async) (L228) → 03-state-management/resource-pattern.md
- [x] Keybinds (First-Class) (L260) → 04-user-interaction/keybinds.md
  - [x] Declarative Definition (L262)
  - [x] User Configuration (L275)
  - [x] Automatic Widget Navigation (L290)
  - [x] Button Keybind Integration (L318)
- [x] Layer System (Simple Stack) (L329) → 02-building-ui/layers.md
  - [x] Multi-Layer Example (L353)
  - [x] Global UI = Just Another Layer (L386)
- [x] Widget Dimensions (No More Hacks!) (L430) → 02-building-ui/layers.md
- [x] Focus System (L453) → 04-user-interaction/focus.md
  - [x] Automatic Focus Order (Zero Boilerplate) (L455)
  - [x] Layer-Scoped Focus (Auto-Restoration) (L474)
  - [x] Programmatic Focus (L504)
    - [x] 1. Declarative (Common Case) (L508)
    - [x] 2. Imperative (Rare Cases) (L528)
  - [x] User Navigation Takes Precedence (L554)
  - [x] Progressive Unfocus (Esc Behavior) (L600)
  - [x] Focus Modes (User Configurable) (L626)
  - [x] Focus Context API (L669)
  - [x] Implementation Details (L699)
- [x] Mouse Support (L740) → 04-user-interaction/mouse.md
  - [x] Hit Testing (1-Frame Delay) (L742)
  - [x] Inline Event Handling (L776)
  - [x] Automatic Hover Styles (L814)
  - [x] Automatic Scroll Wheel (L836)
  - [x] Double-Click vs Single-Click (L865)
  - [x] Right-Click / Context Menus (L905)
  - [x] MouseState API (L978)
  - [x] Focus Integration (L1029)
  - [x] Terminal Mouse Capture (L1056)
- [x] Event-Driven Rendering (L1086) → 01-fundamentals/event-loop.md
- [x] Background Apps (L1099) → 06-system-features/background-apps.md
- [x] Component System (L1129) → 02-building-ui/components.md
  - [x] Terminology (L1131)
  - [x] NavigableState: Unified 2D Navigation (L1168) → 07-advanced/navigable-state.md
  - [x] Component State Composition (L1382)
  - [x] Shared Styling Helpers (L1513)
  - [x] Benefits (L1535)
- [x] Lifecycle & App Management (L1545) → 01-fundamentals/lifecycle.md
  - [x] Simplified Lifecycle Model (L1547)
  - [x] Lifecycle Policy (L1559)
  - [x] When update() Is Called (L1589)
  - [x] Runtime Navigation Behavior (L1609)
  - [x] Global Quit Coordination (L1641)
  - [x] Memory Pressure Management (L1755)
  - [x] Hook Details (L1785)
    - [x] can_quit() - Veto Quit Attempts (L1787)
    - [x] quit_requested() - Handle Veto (L1803)
    - [x] on_background() - Moved to Background (L1817)
    - [x] on_foreground() - Returned to Foreground (L1832)
    - [x] on_destroy() - About to Be Destroyed (L1847)
  - [x] Why Hooks Are Sync (L1864)
  - [x] Migration from V1 (L1895)
  - [x] Benefits (L1929)
- [x] Modal System (L1940) → 02-building-ui/modals.md
  - [x] Modals Are Just Layers (L1942)
  - [x] Hybrid Approach: Pattern + Optional Helpers (L2001)
  - [x] Raw Layers (Maximum Flexibility) (L2021)
  - [x] Builder Helpers (Convenience) (L2058)
  - [x] Built-in Modal Helpers (L2160)
    - [x] ConfirmationModal (L2164)
    - [x] ErrorModal (L2176)
    - [x] LoadingModal (L2186)
    - [x] HelpModal (L2201)
  - [x] Modal Dismissal (Esc Behavior) (L2211)
  - [x] Context Menu Pattern (L2271)
  - [x] Benefits (L2356)
- [x] Color System (OKLCH) (L2367) → 05-visual-design/color-system.md
- [x] Animation System (L2438) → 06-system-features/animation.md
  - [x] Frame Timing (Dynamic Mode Switching) (L2445)
  - [x] Toast System (Global) (L2501)
  - [x] Drag & Drop (L2592)
  - [x] Animation Easing (L2633)
- [x] Background Work + Invalidation (L2665) → 03-state-management/background-work.md
- [x] Component Interaction Patterns (L2734) → 04-user-interaction/component-patterns.md
  - [x] V1 Problems (L2738)
  - [x] V2 Solution: Callbacks + Internal State (L2804)
  - [x] Callback Patterns by Component Type (L2848)
    - [x] Simple Components (Button, Link) (L2852)
    - [x] Text Input (L2883)
    - [x] Complex Components (List, Tree, Table) (L2911)
  - [x] Component State Management (L2977)
  - [x] Component State: Automatic vs Semantic (L3029)
    - [x] Automatic State (Component-Managed, Hidden) (L3033)
    - [x] Semantic State (App-Managed, Exposed) (L3051)
  - [x] Multiple Components of Same Type (L3131)
  - [x] Keybind Integration (L3159)
  - [x] Benefits (L3198)
  - [x] Migration from V1 (L3209)
- [x] Layout System (L3318) → 02-building-ui/layout.md
  - [x] Layout Primitives (L3320)
  - [x] Constraint System (L3353)
  - [x] Auto-Constraints (Smart Defaults) (L3377)
  - [x] Nesting Behavior (L3399)
  - [x] Alignment System (L3419)
    - [x] 1. Manual Fill Spacers (L3423)
    - [x] 2. Parent-Level Alignment (L3442)
    - [x] 3. Child-Level Alignment (Override) (L3478)
  - [x] Container Features (L3492)
    - [x] Panel (L3494)
    - [x] Container (L3511)
  - [x] Layer Positioning (L3524)
  - [x] Macros: Binned (L3568)
  - [x] Theme System (L3602) → 05-visual-design/theme-system.md
    - [x] Color System (OKLCH) (L3611)
    - [x] Theme Structure (~26 Semantic Colors) (L3661)
    - [x] StyleConfig (Visual Behavior) (L3711)
    - [x] Usage Examples (L3845)
    - [x] Persistence (L3913)
    - [x] Runtime Switching (L3928)
    - [x] File Structure (L3940)
    - [x] Migration from V1 (L3950)
  - [~] Keybind System (L3988) → TODO note added to keybinds.md
    - [~] Three Binding Categories (L3992)
    - [~] Navigation Actions (L4010)
    - [~] Alias System (L4051)
    - [~] Keybind Registration (L4083)
    - [~] Runtime Key Handling (L4214)
    - [~] Component Navigation Handling (L4282)
    - [~] Keybind Presets (L4354)
    - [~] Conditional Keybinds (L4386)
    - [~] Conflict Detection (L4421)
    - [~] Settings UI (L4454)
    - [~] Benefits (L4490)
  - [x] App Launcher (Ctrl+Space) (L4502) → 06-system-features/app-launcher.md
    - [x] Core Concept (L4506)
    - [x] App List Structure (L4514)
    - [x] Sorting Logic (L4532)
    - [x] Search/Filtering (L4596)
    - [x] UI Layout (L4619)
    - [x] Implementation (L4655)
    - [x] Discovery Pattern: Inventory Crate (L4790)
    - [x] Keybinds (L4894)
    - [x] Benefits (L4905)
    - [x] Migration from V1 (L4915)
  - [x] Context-Aware Help (F1) (L4942) → 06-system-features/help-system.md
    - [x] Core Concept (L4946)
    - [x] Help Entry Structure (L4955)
    - [x] Help Generation (L4977)
    - [x] Component Nav Action Reporting (L5127)
    - [x] Help Modal Rendering (L5237)
    - [x] Example Output (L5331)
    - [x] Runtime Help Toggle (L5408)
    - [x] Benefits (L5439)
    - [x] No Custom Help Content (L5449)
- [x] Container Features (L5460) → 07-advanced/containers-alignment.md
- [x] Alignment & Positioning (L5466) → 07-advanced/containers-alignment.md
- [x] Constraints System (L5471) → 07-advanced/containers-alignment.md
- [x] Events & Queue System (L5477) → 07-advanced/events-and-queues.md
  - [x] Event Broadcast System (L5486)
    - [x] Publishing Events (L5490)
    - [x] Subscribing to Events (L5502)
    - [x] Persistent Subscriptions (L5530)
    - [x] Event System Characteristics (L5545)
  - [x] Work Queue System (L5553)
    - [x] Queue Registration (L5557)
    - [x] Sending Work to Queues (L5576)
    - [x] Processing Queue Items (L5599)
    - [x] WorkQueue API (L5621)
    - [x] Queue Persistence (L5654)
    - [x] Queue System Characteristics (L5674)
  - [x] Type Safety Architecture (L5682)
    - [x] Implementation Pattern (L5686)
  - [x] Usage Guidelines (L5743)
  - [x] Examples (L5762)
    - [x] Example 1: Operation Queue (L5764)
    - [x] Example 2: Migration Events (L5818)
    - [x] Example 3: Theme Changes (L5851)
- [x] Resource Progress Tracking (L5885) → 03-state-management/resource-pattern.md
  - [x] Progress Enum (L5889)
  - [x] Updated Resource Enum (L5934)
  - [x] Helper Methods (L5947)
  - [x] Usage Examples (L6072)
  - [x] Updating Progress from Async Tasks (L6186)
  - [x] UI Rendering (L6237)
  - [x] Migration from V1 (L6298)
- [x] Resource Error Recovery (L6336) → 03-state-management/error-recovery.md
  - [x] Error Types (L6340)
  - [x] ResourceError Constructors (L6406)
  - [x] Automatic From Implementations (L6516)
  - [x] Resource Helper Methods (L6587)
  - [x] Usage Examples (L6647)
  - [x] UI Rendering with Retry (L6764)
  - [x] Error Kind Guidelines (L6855)
  - [x] Migration from V1 (L6867)
- [ ] Open Questions / TODO (L6906)
- [ ] Non-Goals (L6912)
- [ ] Next Steps (L6921)

## Processing Workflow

**Chunk-by-chunk approach:**

1. **Read sections** from v2.md in order (using offset/limit for large file)
2. **Process heading-by-heading** - not file-by-file (catches contradictions early)
3. **Create/update docs** with minimized examples and heavy cross-linking
4. **Check for contradictions** - if found, ask user which version is correct
5. **Update todo.md** - mark sections complete with destination files
6. **Report progress** - show sections completed, files created, line numbers processed
7. **Continue** - take next chunk and repeat

**Progress tracking:**
- Lines processed: ~6,903 / 6,927 total (~99.7%)
- Files created: 26/29 (~90%)
- Subsections complete: 133/136+ (~98%)

**When contradictions found:**
- Document alternative approach in target file
- Add TODO note for decision
- Example: keybinds.md has alternative system documented (v2.md L3988-4500)

**Batch size:** Process multiple sections per batch, report after each batch completes

## Notes

- **Minimize Examples**: Use small code snippets only, avoid full implementations
- **Cross-Link Heavily**: Use relative links `[text](../path/to/doc.md#anchor)`
- **Design Phase**: Content will change frequently, keep it flexible
- **Prerequisites Section**: Each file should list what to read first
- **See Also Section**: Each file should list related docs
- **Contradictions**: When found during processing, ASK which version is correct
