# Pre-Implementation Discussion Points

**Status:** Design review - Must be resolved before implementation

This document tracks critical questions, inconsistencies, and missing documentation that must be addressed before V2 implementation begins.

---

## 🔴 CRITICAL BLOCKERS

### 1. Component State Ownership Inconsistency

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

### 2. Callback Signature & Async Handlers

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

### 3. Lifecycle Sync-Only Hooks

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

## 🔵 MISSING DOCUMENTATION

### 4. No Testing Strategy

**Problem:** Zero documentation on how to test V2 apps.

**Questions:**
- **Unit tests:** How do we test `update()` that requires `&mut Context`?
- **Mock Context:** Does framework provide test doubles?
- **Integration tests:** Simulate terminal? Snapshot tests?
- **Property tests:** Can we generate random UI interactions?

**Impact:** Apps will be untestable without guidance.

**Resolution needed:** Complete testing guide with examples. Provide test utilities.

---

### 5. No Migration Guide

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

### 6. No Performance Characteristics

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

### 7. No State Persistence Patterns

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

### 8. Async Coordination & Cancellation

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

### 9. Focus System Edge Cases

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

### 10. Animation Mode Switching

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

1. **Clarify component state ownership** *(BLOCKER)*
   - Choose ONE consistent pattern
   - Document when to use each approach

2. **Document callback signatures** *(BLOCKER)*
   - Async support, closures, error handling
   - Show complete examples

3. **State persistence patterns** *(MEDIUM PRIORITY)*
   - Framework pattern or app responsibility
   - Document recommended approach

4. **Async cancellation** *(MEDIUM PRIORITY)*
   - Complete lifecycle documentation
   - Document error handling

5. **Build prototype** *(VALIDATION)*
    - Simple app using V2 design
    - Validate API ergonomics before full implementation

### Should discuss before deciding:

1. **Scope:** Should V2 include ALL these features or MVP first?

2. **Component patterns:** Direct mutation vs state objects vs events - which should dominate?

3. **Lifecycle hooks:** Accept sync-only limitation or add async support?

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
