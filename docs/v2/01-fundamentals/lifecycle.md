# Lifecycle & App Management

**Prerequisites:** [App & Context API](app-and-context.md)

## Simplified Lifecycle Model

V2's immediate mode architecture dramatically simplifies lifecycle compared to V1.

**V1 problems:**
- Two policies (QuitPolicy + SuspendPolicy) for one event
- Dead code (LoadingScreen's `on_suspend` never called due to `AlwaysActive`)
- Unused features (QuittingRequested, QuitOnIdle, KillReason)
- Unclear semantics (what does "Sleep" mean?)

**V2 solution:** One policy, clear semantics.

## Lifecycle Policy

```rust
trait App: 'static {
    fn new(ctx: &AppContext) -> Self;
    fn update(&mut self, ctx: &mut Context) -> Vec<Layer>;

    // Lifecycle policy (static)
    fn lifecycle() -> Lifecycle {
        Lifecycle::Destroy  // Default: destroy when navigating away
    }

    // Lifecycle hooks (sync only)
    fn can_quit(&self) -> Result<(), String> { Ok(()) }
    fn quit_requested(&mut self) { }
    fn on_background(&mut self) { }
    fn on_foreground(&mut self) { }
    fn on_destroy(&mut self) { }
}

enum Lifecycle {
    /// Destroy immediately when navigating away
    Destroy,

    /// Keep in background - still receive events and call update()
    /// (pub/sub, timers, async completions - but NOT keyboard/mouse)
    Background,
}
```

## When update() Is Called

**Foreground app:** User input, pub/sub, timers, async completion
**Background app:** Pub/sub, timers, async completion (no user input)

## Runtime Navigation Behavior

```rust
fn navigate_to(&mut self, target_app: AppId) {
    match current.lifecycle() {
        Lifecycle::Destroy => {
            current.on_destroy();
            self.apps.remove(current);  // Destroyed
        }
        Lifecycle::Background => {
            current.on_background();
            self.background_apps.insert(current);  // Keep alive
        }
    }

    // Switch to target
    if self.background_apps.contains(target) {
        target.on_foreground();  // Resuming from background
    } else {
        target = Target::new(ctx);  // Create new instance
    }
}
```

## Global Quit Coordination

Runtime checks all apps (foreground + background) before quitting:

1. Check foreground app's `can_quit()`
2. Check all background apps' `can_quit()`
3. If any veto → navigate to that app and call `quit_requested()`
4. If all OK → call `on_destroy()` on all apps, give 1 second for Drop cleanup, then quit

**Example: OperationQueue blocks quit**
```rust
impl App for OperationQueue {
    fn lifecycle() -> Lifecycle {
        Lifecycle::Background
    }

    fn can_quit(&self) -> Result<(), String> {
        if !self.currently_running.is_empty() {
            Err(format!("{} operations in progress", self.currently_running.len()))
        } else {
            Ok(())
        }
    }

    fn quit_requested(&mut self) {
        // Runtime brought us to foreground - show modal
        self.show_quit_confirm = true;
    }
}
```

## Memory Pressure Management

Runtime automatically cleans up old background apps when limit exceeded (MAX_BACKGROUND_APPS = 10). **Apps don't specify cleanup timers** - that's runtime policy.

## Hook Details

### can_quit() - Veto Quit Attempts

```rust
fn can_quit(&self) -> Result<(), String> {
    if self.has_unsaved_changes {
        Err("You have unsaved changes".to_string())
    } else {
        Ok(())
    }
}
```

Called by runtime when user tries to quit. First app to veto is brought to foreground.

### quit_requested() - Handle Veto

```rust
fn quit_requested(&mut self) {
    self.show_quit_confirm = true;  // Show modal
}
```

Only called if `can_quit()` returned `Err`. App is now foreground.

### on_background() - Moved to Background

```rust
fn on_background(&mut self) {
    self.countdown_ticks = None;
    self.show_temporary_modal = false;
}
```

Called when user navigates away. Only called if `lifecycle() == Background`. App still receives pub/sub, timers, async completions.

### on_foreground() - Returned to Foreground

```rust
fn on_foreground(&mut self) {
    if self.last_refresh.elapsed() > Duration::from_secs(60) {
        self.refresh();
    }
}
```

Called when user navigates back. Only called if app was in background (not freshly created).

### on_destroy() - About to Be Destroyed

```rust
fn on_destroy(&mut self) {
    self.flush_buffers_sync();
    self.save_state_sync();
}
```

Called before app is removed from runtime. **Sync only** - no async allowed. Runtime gives 1 second grace period for Drop impls.

## Why Hooks Are Sync

**Problem with async hooks:** Blocks runtime, UI frozen, no progress feedback.

**Solution:** Sync hooks + grace period. Apps use `on_destroy()` for quick sync cleanup, Drop impl handles async cleanup with 1-second grace period.

**Benefits:**
- UI stays responsive
- User can see progress via modals
- Apps control cleanup UX
- Runtime provides grace period for Drop

## Migration from V1

```rust
// v1: QuitPolicy::Sleep + SuspendPolicy::AlwaysActive
impl App for OperationQueue {
    fn lifecycle() -> Lifecycle {
        Lifecycle::Background
    }
}

// v1: QuitPolicy::QuitOnExit or SuspendPolicy::QuitOnSuspend
impl App for ErrorScreen {
    fn lifecycle() -> Lifecycle {
        Lifecycle::Destroy  // Default
    }
}
```

## Benefits

- **Simpler mental model** - One policy (Destroy or Background)
- **No dead code** - LoadingScreen scenario works correctly
- **Clearer semantics** - "Destroy or keep in background" is obvious
- **Background apps block quit** - Queue can veto quit even when not foreground
- **Memory managed by runtime** - Apps don't think about cleanup timers
- **Sync hooks** - UI stays responsive, apps show progress modals

**See Also:**
- [Background Apps](../06-system-features/background-apps.md) - Background processing details
- [Pub/Sub](../03-state-management/pubsub.md) - Background message handling

---

**Next:** Learn about [Event Loop](event-loop.md) or explore [Background Apps](../06-system-features/background-apps.md).
