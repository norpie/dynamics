# Pre-Implementation Discussion Points

**Status:** Design review - Must be resolved before implementation

This document tracks critical questions, inconsistencies, and missing documentation that must be addressed before V2 implementation begins.

---

## 🔴 CRITICAL BLOCKERS

### 1. The Missing Options System

**Problem:** The keybind system (and theme system) extensively reference an "Options" system that has NO documentation.

```rust
// From keybinds.md - but no Options docs exist!
options.get_string("keybind.my_app.save")
OptionDefBuilder::new("keybind", "my_app.save")
ctx.register_work_queue::<Operation>("queue_name")
```

**Questions:**
- What IS the Options system? SQLite-backed config store?
- Schema design: namespace + key + type + default?
- Type safety: Runtime type checks or compile-time?
- Validation: Who validates values? Registry? Apps?
- Default handling: Registry defines defaults vs runtime fallbacks?
- Migration strategy: How do option schemas evolve across versions?
- Where's the settings UI documentation?

**Impact:** Cannot implement keybinds or themes without this foundation.

**Resolution needed:** Complete Options system specification document.

---

### 2. Component State Ownership Inconsistency

**Problem:** Different components have wildly inconsistent state management patterns:

```rust
// Pattern 1: Direct mutation
ui.text_input(&mut self.name)  // Component mutates app field directly

// Pattern 2: Separate state object
ui.list(&self.items, &mut self.list_state)  // Explicit state

// Pattern 3: Event-based
ui.list(&self.items)
    .on_select(|idx| { self.selected = idx; })
```

**Questions:**
- **Why the inconsistency?** What's the design principle?
- **Multiple instances:** Can two text inputs reference `&mut self.name`? How does cursor position work?
- **Validation:** How do apps intercept/validate mutations?
- **Serialization:** If components hold hidden state (scroll, cursor), can app state be serialized?
- **Field types mentioned:** Docs reference "TextInputField" pattern for "~80% boilerplate reduction" but examples don't show it. Which is real?

**Impact:** Affects every app. Unclear how to structure app state.

**Resolution needed:** Choose ONE consistent pattern with clear rules. Document XxxState vs XxxField vs direct mutation.

---

### 3. Callback Signature & Async Handlers

**Problem:** Callback examples show method references, but practical questions remain:

```rust
ui.button("Save").on_click(Self::handle_save);

fn handle_save(&mut self, ctx: &mut Context) { }  // Sync only?
async fn handle_save_async(&mut self, ctx: &mut Context) { }  // Allowed?
```

**Questions:**
- **Async handlers:** Are they supported? `on_click` needs `fn() -> impl Future`?
- **Closures:** Can I use closures that capture state? `|&mut self| self.count += 1`?
- **Error handling:** What if handler returns `Result`? Does framework handle errors?
- **Lifetime issues:** How do callbacks interact with `&mut self` borrowing?

**Impact:** Determines how apps structure all interaction logic.

**Resolution needed:** Complete callback signature specification. Document sync vs async, closures, error handling.

---

## 🟠 MAJOR ARCHITECTURAL QUESTIONS

### 4. Immediate Mode Philosophy: Testability & Traceability

**Problem:** V2 abandons Elm's pure functions for immediate mode with `&mut self`:

**V1 (pure):**
```rust
fn update(state: &mut State, msg: Msg) -> Command<Msg>  // Pure, testable
fn view(state: &State) -> Element<Msg>                   // Pure, testable
```

**V2 (immediate):**
```rust
fn update(&mut self, ctx: &mut Context) -> Vec<Layer>  // Mutation + I/O
```

**Questions:**
- **Testing:** How do we unit test apps? Mock Context? Fake terminal?
- **Traceability:** In Elm, every state change has an explicit Msg. How do we trace state changes now?
- **Undo/redo:** Elm's pure functions make this trivial. How do we implement it in V2?
- **Debugging:** Without explicit messages, how do we debug what triggered a state change?
- **Time-travel:** Acknowledged as non-goal, but are we okay losing this capability permanently?

**Impact:** Affects entire framework philosophy and developer experience.

**Resolution needed:** Document testing strategy, debugging approach, acknowledge tradeoffs explicitly.

---

### 5. Resource Pattern Breaking Changes

**Problem:** V2 Resource is incompatible with V1:

**V1:**
```rust
enum Resource<T, E = String> {
    NotAsked,
    Loading,              // No progress
    Success(T),
    Failure(E),           // Generic error
}
```

**V2:**
```rust
enum Resource<T> {
    NotAsked,
    Loading(Progress),    // REQUIRED progress
    Success(T),
    Failure {             // Structured error + retry count
        error: ResourceError,
        retry_count: usize,
    },
}
```

**Questions:**
- **Breaking change justified?** Is the Progress requirement worth breaking V1 compatibility?
- **Always have progress?** What if I just want a spinner? Must I use `Progress::Indeterminate`?
- **Generic error removed:** Why force `ResourceError` instead of keeping generic `E`?
- **Retry count location:** Should it be in Resource or in app state?
- **Migration path:** How do V1 apps migrate? Automated tool?

**Impact:** Every V1 app using Resource needs rewriting.

**Resolution needed:** Justify breaking changes or restore backward compatibility. Provide migration guide.

---

### 6. Lifecycle Sync-Only Hooks

**Problem:** Lifecycle hooks are sync-only, but Drop gets 1-second grace period:

```rust
fn on_destroy(&mut self) {
    // SYNC ONLY - no await allowed!
    self.flush_buffers_sync();  // What if this needs async I/O?
}
```

**Questions:**
- **1 second enough?** What if app needs to flush large buffers or wait for network?
- **Drop impl expected?** Docs say "Drop impl handles async cleanup" - how? Drop is also sync!
- **User experience:** If cleanup takes >1 second, does app just... terminate anyway?
- **Alternative design:** Could we have async `on_destroy_async` that shows progress modal?

**Impact:** Apps with complex cleanup logic may lose data.

**Resolution needed:** Clarify async cleanup story or extend grace period. Document Drop behavior.

---

### 7. Event Broadcast vs Work Queue Confusion

**Problem:** Two parallel communication systems with overlapping use cases:

```rust
// Event broadcast - best-effort, multiple subscribers
ctx.broadcast("migration:selected", migration_id);

// Work queue - guaranteed, single consumer, persistent
ctx.send_work("operation_queue", Operation { ... }, Priority::Normal);
```

**Questions:**
- **Decision tree:** When exactly should I use events vs queues?
- **Queue ownership:** What if queue owner app is destroyed? Do items survive? Docs say Background apps can be killed for memory pressure!
- **Type safety:** Both use `serde_json::Value` type erasure. Runtime type errors?
- **Persistent subscriptions:** Events can be persistent via `.persistent(true)`. Doesn't this make them equivalent to queues?
- **Priority inversion:** Can low-priority queue items in queue A starve high-priority items in queue B?

**Impact:** Every cross-app communication decision needs clarity.

**Resolution needed:** Clear decision matrix. Document queue persistence guarantees. Address type safety concerns.

---

## 🟡 DESIGN INCONSISTENCIES

### 8. Layer System vs Stack Widget

**Problem:** Two overlapping concepts:

```rust
// Layers - app-level, returned from update()
fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
    vec![
        Layer::fill(main_ui),
        Layer::centered(60, 20, modal),
    ]
}

// Stack - element-level widget
Stack::new()
    .layer(background, LayerAlignment::Fill)
    .layer(overlay, LayerAlignment::Center)
    .build()
```

**Questions:**
- **When to use which?** Guidelines unclear
- **Focusable elements in Stack?** How does focus traverse stack layers vs app layers?
- **Dimming interaction:** Both support `dim_below()`. What if both are used?
- **Performance:** Does Stack create nested ratatui buffer passes?

**Impact:** Developers won't know which to use when.

**Resolution needed:** Clear usage guidelines. Consider merging or removing one.

---

### 9. Keybind System Complexity

**Problem:** The keybind system is elaborate:
- 3 categories (Navigation, Global, App)
- Alias system (primary + 10 aliases per binding)
- Proc macro for auto-registration
- Priority-based dispatch
- Vim mode presets
- Conflict detection

**Questions:**
- **Is alias system necessary?** Could we just allow multiple primary bindings per action?
- **Proc macro vs inventory:** Docs mention both. Which is better? Proc macro feels fragile (parsing AST to find `.bind()` calls across all code paths).
- **Conflict resolution:** If navigation "k" conflicts with app "k", is priority always clear?
- **Customization UI:** Where's the keybind editor documentation?
- **Can we simplify?** This feels over-engineered for a TUI framework.

**Impact:** High implementation complexity. Risk of bugs.

**Resolution needed:** Simplify or fully justify complexity. Choose proc macro OR inventory, not both.

---

### 10. Modal Pattern Duplication

**Problem:** Two ways to create modals:

```rust
// Raw layers - maximum flexibility
layers.push(
    Layer::centered(60, 20, panel("Settings", |ui| { ... }))
        .dim_below(true)
        .blocks_input(true)
);

// Builder helpers - convenience
layers.push(
    ConfirmationModal::new("Delete?", "Sure?")
        .on_yes(Self::handle_yes)
        .build()
);
```

**Questions:**
- **When to use which?** Both work for same use cases
- **Are builders just sugar?** Or do they add framework behavior?
- **Can we eliminate one?** Having two patterns increases API surface

**Impact:** API confusion for developers.

**Resolution needed:** Clarify builder vs raw layer usage. Consider keeping only one.

---

## 🔵 MISSING DOCUMENTATION

### 11. No Testing Strategy

**Problem:** Zero documentation on how to test V2 apps.

**Questions:**
- **Unit tests:** How do we test `update()` that requires `&mut Context`?
- **Mock Context:** Does framework provide test doubles?
- **Integration tests:** Simulate terminal? Snapshot tests?
- **Property tests:** Can we generate random UI interactions?

**Impact:** Apps will be untestable without guidance.

**Resolution needed:** Complete testing guide with examples. Provide test utilities.

---

### 12. No Migration Guide

**Problem:** Docs list "Migration Guide" as TODO, but this is essential before anyone can port V1 apps.

**Needed:**
- V1 → V2 conversion checklist
- Common patterns mapping (Msg enums → callbacks)
- Resource pattern migration
- Lifecycle hook conversion
- Breaking changes comprehensive list

**Impact:** Cannot evaluate V2 without knowing migration effort.

**Resolution needed:** Write comprehensive migration guide before implementation.

---

### 13. No Performance Characteristics

**Problem:** No performance targets or constraints documented.

**Questions:**
- **Render budget:** What's the target? 16ms @ 60fps?
- **Layer limits:** How many layers before performance degrades?
- **Element counts:** Can we render 1000-element lists efficiently?
- **Virtual scrolling:** Is it planned for large datasets?
- **Profiling:** Tools and strategies?

**Impact:** Can't design apps without knowing limits.

**Resolution needed:** Document performance targets, benchmarks, profiling strategies.

---

### 14. No State Persistence Patterns

**Problem:** No framework guidance on saving/restoring app state.

**Questions:**
- **Where to save:** SQLite? Files? Options system?
- **When to save:** on_background? on_destroy? Continuous?
- **What to save:** Full state? Semantic state only?
- **Serialization:** Does framework help or is it app responsibility?

**Impact:** Every app reinvents state persistence.

**Resolution needed:** Document recommended state persistence patterns. Consider framework support.

---

## 🟢 TECHNICAL DETAILS NEEDED

### 15. Async Coordination & Cancellation

**Problem:** Unclear how async tasks coordinate with runtime.

**Questions:**
- **Simultaneous completions:** What if 10 tasks finish at once? Batched updates?
- **Cancellation:** Can apps cancel spawned tasks?
- **Error propagation:** What if spawned task panics?
- **Lifetime:** Can tasks outlive the app that spawned them?
- **Invalidation:** Does every spawned task get an Invalidator? How?

**Impact:** Affects all async app code.

**Resolution needed:** Complete async task lifecycle documentation. Document cancellation API if supported.

---

### 16. Focus System Edge Cases

**Problem:** Auto-registration has unclear behavior.

**Questions:**
- **Stability:** If render order changes based on state, does focus index shift unpredictably?
- **Conditional rendering:** Button appears/disappears - does focus jump?
- **Layer focus coordination:** Can focus flow across layers or always layer-scoped?
- **Multiple auto-focus:** Two widgets with `.auto_focus(true)` - which wins?
- **Focus restoration:** When modal closes, is focus guaranteed to restore to previous element?

**Impact:** User experience consistency.

**Resolution needed:** Document focus behavior for all edge cases. Add examples.

---

### 17. OKLCH Color System Practicality

**Problem:** Terminal support and performance questions.

**Questions:**
- **Terminal precision:** Most terminals use RGB/256-color. How much OKLCH precision is lost in conversion?
- **Conversion overhead:** Does OKLCH → RGB happen every frame? Cached?
- **User understanding:** How do users configure OKLCH colors? Need visual picker?
- **Accessibility:** Does OKLCH help with colorblind-friendly palettes?
- **Fallback:** What if terminal doesn't support true color?

**Impact:** Visual quality and performance.

**Resolution needed:** Benchmark conversion overhead. Document terminal compatibility. Design color picker UI.

---

### 18. Animation Mode Switching

**Problem:** Runtime switches between event-driven and frame-driven.

**Questions:**
- **Detection:** How does runtime know animations are active? Apps call `ctx.animate()`?
- **Transition smoothness:** Does mode switch cause visible stutter?
- **Multiple animations:** Do ALL animations need to finish before reverting to event-driven?
- **Battery claim verification:** Is "1-2% CPU" accurate under all conditions?

**Impact:** Performance and battery life guarantees.

**Resolution needed:** Document animation detection mechanism. Validate CPU usage claims with benchmarks.

---

## 📋 ACTION ITEMS

### Must complete before implementation:

1. **Write Options System spec** *(BLOCKER)*
   - Complete schema, API, validation, defaults
   - Document settings UI

2. **Clarify component state ownership** *(BLOCKER)*
   - Choose ONE consistent pattern
   - Document when to use each approach

3. **Document callback signatures** *(BLOCKER)*
   - Async support, closures, error handling
   - Show complete examples

4. **Testing strategy** *(CRITICAL)*
   - Mock Context, test harness, examples
   - Unit testing patterns

5. **Migration guide** *(CRITICAL)*
   - V1→V2 conversion steps
   - Breaking changes comprehensive list

6. **Keybind simplification analysis** *(HIGH PRIORITY)*
   - Justify complexity or simplify
   - Choose proc macro OR inventory

7. **Performance targets** *(HIGH PRIORITY)*
   - Document render budget, limits
   - Create benchmark suite

8. **State persistence patterns** *(MEDIUM PRIORITY)*
   - Framework pattern or app responsibility
   - Document recommended approach

9. **Async cancellation** *(MEDIUM PRIORITY)*
   - Complete lifecycle documentation
   - Document error handling

10. **Build prototype** *(VALIDATION)*
    - Simple app using V2 design
    - Validate API ergonomics before full implementation

### Should discuss before deciding:

1. **Philosophy:** Are you comfortable abandoning Elm's pure functions for testability concerns?

2. **Complexity:** Is the keybind system worth its implementation complexity?

3. **Breaking changes:** Is V2 incompatibility with V1 worth the ergonomic gains?

4. **Scope:** Should V2 include ALL these features or MVP first?

5. **Component patterns:** Direct mutation vs state objects vs events - which should dominate?

6. **Resource pattern:** Keep V1 compatibility or accept breaking changes?

7. **Lifecycle hooks:** Accept sync-only limitation or add async support?

8. **Communication systems:** Keep both events and queues or simplify to one?

---

## Resolution Tracking

Use this section to track decisions as they're made:

### Resolved Items

*(Empty - add resolutions here as discussions conclude)*

---

**See Also:**
- [Open Questions](open-questions.md) - Future considerations (nice-to-have)
- [Overview](../00-overview.md) - V2 design goals and philosophy
- [Next Steps](../00-overview.md#next-steps) - Implementation roadmap
