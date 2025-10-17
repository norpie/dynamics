# V2 Documentation Split Progress

## Directory Structure

```
docs/v2/
├── [ ] README.md
├── [ ] 00-overview.md
│
├── 01-fundamentals/
│   ├── [ ] app-and-context.md
│   ├── [ ] lifecycle.md
│   ├── [ ] event-loop.md
│   └── [ ] elements.md
│
├── 02-building-ui/
│   ├── [ ] layout.md
│   ├── [ ] layers.md
│   ├── [ ] components.md
│   └── [ ] modals.md
│
├── 03-state-management/
│   ├── [ ] resource-pattern.md
│   ├── [ ] error-recovery.md
│   ├── [ ] pubsub.md
│   └── [ ] routing.md
│
├── 04-user-interaction/
│   ├── [ ] keybinds.md
│   ├── [ ] focus.md
│   ├── [ ] mouse.md
│   ├── [ ] navigation.md
│   └── [ ] component-patterns.md
│
├── 05-visual-design/
│   ├── [ ] color-system.md
│   ├── [ ] theming.md
│   └── [ ] animation.md
│
├── 06-system-features/
│   ├── [ ] app-launcher.md
│   ├── [ ] help-system.md
│   └── [ ] background-apps.md
│
├── 07-advanced/
│   ├── [ ] events-and-queues.md
│   ├── [ ] background-work.md
│   ├── [ ] navigable-state.md
│   └── [ ] containers-alignment.md
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

- [ ] Core Philosophy (L5)
  - [ ] Immediate Mode + Structured Concurrency (L7)
  - [ ] What We're Fixing from V1 (L14)
- [ ] Architecture (L26)
  - [ ] App Trait (L28)
  - [ ] Context API (L51)
  - [ ] Example: Simple App (L69)
- [ ] Multi-View Apps (L115)
- [ ] Pub/Sub (Auto-Managed) (L189)
- [ ] Resource Pattern (Auto-Managed Async) (L228)
- [ ] Keybinds (First-Class) (L260)
  - [ ] Declarative Definition (L262)
  - [ ] User Configuration (L275)
  - [ ] Automatic Widget Navigation (L290)
  - [ ] Button Keybind Integration (L318)
- [ ] Layer System (Simple Stack) (L329)
  - [ ] Multi-Layer Example (L353)
  - [ ] Global UI = Just Another Layer (L386)
- [ ] Widget Dimensions (No More Hacks!) (L430)
- [ ] Focus System (L453)
  - [ ] Automatic Focus Order (Zero Boilerplate) (L455)
  - [ ] Layer-Scoped Focus (Auto-Restoration) (L474)
  - [ ] Programmatic Focus (L504)
    - [ ] 1. Declarative (Common Case) (L508)
    - [ ] 2. Imperative (Rare Cases) (L528)
  - [ ] User Navigation Takes Precedence (L554)
  - [ ] Progressive Unfocus (Esc Behavior) (L600)
  - [ ] Focus Modes (User Configurable) (L626)
  - [ ] Focus Context API (L669)
  - [ ] Implementation Details (L699)
- [ ] Mouse Support (L740)
  - [ ] Hit Testing (1-Frame Delay) (L742)
  - [ ] Inline Event Handling (L776)
  - [ ] Automatic Hover Styles (L814)
  - [ ] Automatic Scroll Wheel (L836)
  - [ ] Double-Click vs Single-Click (L865)
  - [ ] Right-Click / Context Menus (L905)
  - [ ] MouseState API (L978)
  - [ ] Focus Integration (L1029)
  - [ ] Terminal Mouse Capture (L1056)
- [ ] Event-Driven Rendering (L1086)
- [ ] Background Apps (L1099)
- [ ] Component System (L1129)
  - [ ] Terminology (L1131)
  - [ ] NavigableState: Unified 2D Navigation (L1168)
  - [ ] Component State Composition (L1382)
  - [ ] Shared Styling Helpers (L1513)
  - [ ] Benefits (L1535)
- [ ] Lifecycle & App Management (L1545)
  - [ ] Simplified Lifecycle Model (L1547)
  - [ ] Lifecycle Policy (L1559)
  - [ ] When update() Is Called (L1589)
  - [ ] Runtime Navigation Behavior (L1609)
  - [ ] Global Quit Coordination (L1641)
  - [ ] Memory Pressure Management (L1755)
  - [ ] Hook Details (L1785)
    - [ ] can_quit() - Veto Quit Attempts (L1787)
    - [ ] quit_requested() - Handle Veto (L1803)
    - [ ] on_background() - Moved to Background (L1817)
    - [ ] on_foreground() - Returned to Foreground (L1832)
    - [ ] on_destroy() - About to Be Destroyed (L1847)
  - [ ] Why Hooks Are Sync (L1864)
  - [ ] Migration from V1 (L1895)
  - [ ] Benefits (L1929)
- [ ] Modal System (L1940)
  - [ ] Modals Are Just Layers (L1942)
  - [ ] Hybrid Approach: Pattern + Optional Helpers (L2001)
  - [ ] Raw Layers (Maximum Flexibility) (L2021)
  - [ ] Builder Helpers (Convenience) (L2058)
  - [ ] Built-in Modal Helpers (L2160)
    - [ ] ConfirmationModal (L2164)
    - [ ] ErrorModal (L2176)
    - [ ] LoadingModal (L2186)
    - [ ] HelpModal (L2201)
  - [ ] Modal Dismissal (Esc Behavior) (L2211)
  - [ ] Context Menu Pattern (L2271)
  - [ ] Benefits (L2356)
- [ ] Color System (OKLCH) (L2367)
- [ ] Animation System (L2438)
  - [ ] Frame Timing (Dynamic Mode Switching) (L2445)
  - [ ] Toast System (Global) (L2501)
  - [ ] Drag & Drop (L2592)
  - [ ] Animation Easing (L2633)
- [ ] Background Work + Invalidation (L2665)
- [ ] Component Interaction Patterns (L2734)
  - [ ] V1 Problems (L2738)
  - [ ] V2 Solution: Callbacks + Internal State (L2804)
  - [ ] Callback Patterns by Component Type (L2848)
    - [ ] Simple Components (Button, Link) (L2852)
    - [ ] Text Input (L2883)
    - [ ] Complex Components (List, Tree, Table) (L2911)
  - [ ] Component State Management (L2977)
  - [ ] Component State: Automatic vs Semantic (L3029)
    - [ ] Automatic State (Component-Managed, Hidden) (L3033)
    - [ ] Semantic State (App-Managed, Exposed) (L3051)
  - [ ] Multiple Components of Same Type (L3131)
  - [ ] Keybind Integration (L3159)
  - [ ] Benefits (L3198)
  - [ ] Migration from V1 (L3209)
- [ ] Layout System (L3318)
  - [ ] Layout Primitives (L3320)
  - [ ] Constraint System (L3353)
  - [ ] Auto-Constraints (Smart Defaults) (L3377)
  - [ ] Nesting Behavior (L3399)
  - [ ] Alignment System (L3419)
    - [ ] 1. Manual Fill Spacers (L3423)
    - [ ] 2. Parent-Level Alignment (L3442)
    - [ ] 3. Child-Level Alignment (Override) (L3478)
  - [ ] Container Features (L3492)
    - [ ] Panel (L3494)
    - [ ] Container (L3511)
  - [ ] Layer Positioning (L3524)
  - [ ] Macros: Binned (L3568)
  - [ ] Theme System (L3602)
    - [ ] Color System (OKLCH) (L3611)
    - [ ] Theme Structure (~26 Semantic Colors) (L3661)
    - [ ] StyleConfig (Visual Behavior) (L3711)
    - [ ] Usage Examples (L3845)
    - [ ] Persistence (L3913)
    - [ ] Runtime Switching (L3928)
    - [ ] File Structure (L3940)
    - [ ] Migration from V1 (L3950)
  - [ ] Keybind System (L3988)
    - [ ] Three Binding Categories (L3992)
    - [ ] Navigation Actions (L4010)
    - [ ] Alias System (L4051)
    - [ ] Keybind Registration (L4083)
    - [ ] Runtime Key Handling (L4214)
    - [ ] Component Navigation Handling (L4282)
    - [ ] Keybind Presets (L4354)
    - [ ] Conditional Keybinds (L4386)
    - [ ] Conflict Detection (L4421)
    - [ ] Settings UI (L4454)
    - [ ] Benefits (L4490)
  - [ ] App Launcher (Ctrl+Space) (L4502)
    - [ ] Core Concept (L4506)
    - [ ] App List Structure (L4514)
    - [ ] Sorting Logic (L4532)
    - [ ] Search/Filtering (L4596)
    - [ ] UI Layout (L4619)
    - [ ] Implementation (L4655)
    - [ ] Discovery Pattern: Inventory Crate (L4790)
    - [ ] Keybinds (L4894)
    - [ ] Benefits (L4905)
    - [ ] Migration from V1 (L4915)
  - [ ] Context-Aware Help (F1) (L4942)
    - [ ] Core Concept (L4946)
    - [ ] Help Entry Structure (L4955)
    - [ ] Help Generation (L4977)
    - [ ] Component Nav Action Reporting (L5127)
    - [ ] Help Modal Rendering (L5237)
    - [ ] Example Output (L5331)
    - [ ] Runtime Help Toggle (L5408)
    - [ ] Benefits (L5439)
    - [ ] No Custom Help Content (L5449)
- [ ] Container Features (L5460)
- [ ] Alignment & Positioning (L5466)
- [ ] Constraints System (L5471)
- [ ] Events & Queue System (L5477)
  - [ ] Event Broadcast System (L5486)
    - [ ] Publishing Events (L5490)
    - [ ] Subscribing to Events (L5502)
    - [ ] Persistent Subscriptions (L5530)
    - [ ] Event System Characteristics (L5545)
  - [ ] Work Queue System (L5553)
    - [ ] Queue Registration (L5557)
    - [ ] Sending Work to Queues (L5576)
    - [ ] Processing Queue Items (L5599)
    - [ ] WorkQueue API (L5621)
    - [ ] Queue Persistence (L5654)
    - [ ] Queue System Characteristics (L5674)
  - [ ] Type Safety Architecture (L5682)
    - [ ] Implementation Pattern (L5686)
  - [ ] Usage Guidelines (L5743)
  - [ ] Examples (L5762)
    - [ ] Example 1: Operation Queue (L5764)
    - [ ] Example 2: Migration Events (L5818)
    - [ ] Example 3: Theme Changes (L5851)
- [ ] Resource Progress Tracking (L5885)
  - [ ] Progress Enum (L5889)
  - [ ] Updated Resource Enum (L5934)
  - [ ] Helper Methods (L5947)
  - [ ] Usage Examples (L6072)
  - [ ] Updating Progress from Async Tasks (L6186)
  - [ ] UI Rendering (L6237)
  - [ ] Migration from V1 (L6298)
- [ ] Resource Error Recovery (L6336)
  - [ ] Error Types (L6340)
  - [ ] ResourceError Constructors (L6406)
  - [ ] Automatic From Implementations (L6516)
  - [ ] Resource Helper Methods (L6587)
  - [ ] Usage Examples (L6647)
  - [ ] UI Rendering with Retry (L6764)
  - [ ] Error Kind Guidelines (L6855)
  - [ ] Migration from V1 (L6867)
- [ ] Open Questions / TODO (L6906)
- [ ] Non-Goals (L6912)
- [ ] Next Steps (L6921)

## Notes

- **Minimize Examples**: Use small code snippets only, avoid full implementations
- **Cross-Link Heavily**: Use relative links `[text](../path/to/doc.md#anchor)`
- **Design Phase**: Content will change frequently, keep it flexible
- **Prerequisites Section**: Each file should list what to read first
- **See Also Section**: Each file should list related docs
- **Contradictions**: When found during processing, ASK which version is correct
