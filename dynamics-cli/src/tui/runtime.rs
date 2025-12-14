use ratatui::Frame;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, MouseEvent, MouseEventKind, MouseButton};
use anyhow::Result;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};
use serde_json::Value;
use std::pin::Pin;
use std::future::Future;
use std::any::Any;

use crate::tui::{App, AppId, Command, Renderer, InteractionRegistry, Subscription, AppState, KeyBinding, QuitPolicy, SuspendPolicy};
use crate::tui::command::{ParallelConfig, DispatchTarget};
use crate::tui::renderer::{FocusRegistry, FocusableInfo, DropdownRegistry};
use crate::tui::element::FocusId;
use crate::tui::state::{RuntimeConfig, FocusMode};

/// Trait for creating app instances (factory pattern for lazy initialization)
pub trait AppFactory: Send {
    /// Create a new app instance with typed parameters
    fn create(&self, params: Box<dyn Any + Send>) -> Result<Box<dyn AppRuntime>>;

    /// Get the quit policy for this app type
    fn quit_policy(&self) -> QuitPolicy;

    /// Get the suspend policy for this app type
    fn suspend_policy(&self) -> SuspendPolicy;
}

/// Trait for runtime operations, allowing type-erased storage of different Runtime<A> types
pub trait AppRuntime {
    fn handle_key(&mut self, key_event: KeyEvent) -> Result<bool>;
    fn handle_mouse(&mut self, mouse_event: MouseEvent) -> Result<bool>;
    fn render_to_area(&mut self, frame: &mut Frame, area: ratatui::layout::Rect);
    fn get_title(&self) -> &'static str;
    fn get_status(&self) -> Option<ratatui::text::Line<'static>>;
    fn get_key_bindings(&self) -> Vec<(KeyBinding, String)>;
    fn poll_timers(&mut self) -> Result<()>;
    fn poll_async(&mut self) -> Pin<Box<dyn Future<Output = Result<()>> + '_>>;
    fn take_navigation(&mut self) -> Option<AppId>;
    fn peek_navigation(&self) -> Option<AppId>;
    fn take_start_app(&mut self) -> Option<(AppId, Box<dyn Any + Send>)>;
    fn take_publishes(&mut self) -> Vec<(String, Value)>;
    fn handle_publish(&mut self, topic: &str, data: Value) -> Result<()>;
    fn focus_next(&mut self) -> Result<()>;
    fn focus_previous(&mut self) -> Result<()>;

    // Lifecycle methods
    fn can_quit(&self) -> Result<(), String>;
    fn on_suspend(&mut self) -> Result<()>;
    fn on_resume(&mut self) -> Result<()>;
    fn on_destroy(&mut self) -> Result<()>;

    /// Check if the app is capturing raw input (e.g., keybind capture mode)
    /// When true, global keybinds should be bypassed to allow the app to handle all keys
    fn is_capturing_raw_input(&self) -> bool {
        false // Default: apps don't capture raw input
    }
}

/// Tracks the state of a parallel task execution
struct ParallelTaskCoordinator<Msg> {
    total_tasks: usize,
    results: Arc<Mutex<Vec<Option<Box<dyn Any + Send>>>>>,
    msg_mapper: Arc<dyn Fn(usize, Box<dyn Any + Send>) -> Msg + Send>,
    config: ParallelConfig,
}

/// The runtime manages app lifecycle, event routing, and command execution
pub struct Runtime<A: App> {
    /// Current app state
    state: A::State,

    /// Interaction registry for mouse events
    registry: InteractionRegistry<A::Msg>,

    /// Focus registry for keyboard focus
    focus_registry: FocusRegistry<A::Msg>,

    /// Dropdown registry for select widget overlays
    dropdown_registry: DropdownRegistry<A::Msg>,

    /// Currently focused element ID
    focused_id: Option<FocusId>,

    /// Keyboard subscriptions
    key_subscriptions: HashMap<KeyBinding, A::Msg>,

    /// Event bus for pub/sub
    event_bus: HashMap<String, Vec<Box<dyn Fn(Value) -> Option<A::Msg> + Send>>>,

    /// Timer subscriptions: (interval, last_tick, msg)
    timers: Vec<(Duration, Instant, A::Msg)>,

    /// Last hovered element position for tracking hover exits
    last_hover_pos: Option<(u16, u16)>,

    /// Pending navigation request
    navigation_target: Option<AppId>,

    /// Pending async commands - now using tokio spawn with channel
    async_result_rx: tokio::sync::mpsc::UnboundedReceiver<A::Msg>,
    async_result_tx: tokio::sync::mpsc::UnboundedSender<A::Msg>,
    /// Count of pending async tasks (for logging)
    pending_async_count: usize,

    /// Pending parallel coordination tasks (task_index, task_name)
    pending_parallel: Vec<std::pin::Pin<Box<dyn std::future::Future<Output = (usize, String)> + Send>>>,

    /// Pending publish events to broadcast globally
    pending_publishes: Vec<(String, serde_json::Value)>,

    /// Pending app start with params (app_id, params)
    pending_start_app: Option<(AppId, Box<dyn Any + Send>)>,

    /// Active parallel task coordinator
    parallel_coordinator: Option<ParallelTaskCoordinator<A::Msg>>,

    /// Progress receivers for parallel tasks (task_name, receiver)
    progress_receivers: Vec<(String, tokio::sync::mpsc::UnboundedReceiver<String>)>,

    /// Track if user explicitly unfocused via Escape (to prevent auto-restore)
    explicitly_unfocused: bool,

    /// Previous layer count (to detect modal open/close)
    previous_layer_count: usize,
}

impl<A: App> Runtime<A> {
    pub fn new() -> Self {
        Self::with_params(A::InitParams::default())
    }

    pub fn with_params(params: A::InitParams) -> Self {
        let (state, init_command) = A::init(params);
        let (async_result_tx, async_result_rx) = tokio::sync::mpsc::unbounded_channel();

        let mut runtime = Self {
            state,
            registry: InteractionRegistry::new(),
            focus_registry: FocusRegistry::new(),
            dropdown_registry: DropdownRegistry::new(),
            focused_id: None,
            key_subscriptions: HashMap::new(),
            event_bus: HashMap::new(),
            timers: Vec::new(),
            last_hover_pos: None,
            navigation_target: None,
            async_result_rx,
            async_result_tx,
            pending_async_count: 0,
            pending_parallel: Vec::new(),
            pending_publishes: Vec::new(),
            pending_start_app: None,
            parallel_coordinator: None,
            progress_receivers: Vec::new(),
            explicitly_unfocused: false,
            previous_layer_count: 1,  // Start with 1 (base layer)
        };

        // Initialize subscriptions
        runtime.update_subscriptions();

        // Execute init command
        runtime.execute_command(init_command).ok();

        runtime
    }

    /// Take the pending navigation target (if any)
    pub fn take_navigation(&mut self) -> Option<AppId> {
        self.navigation_target.take()
    }

    /// Peek at the pending navigation target without taking it
    pub fn peek_navigation(&self) -> Option<AppId> {
        self.navigation_target
    }

    /// Take pending start app request (if any)
    pub fn take_start_app(&mut self) -> Option<(AppId, Box<dyn Any + Send>)> {
        self.pending_start_app.take()
    }

    /// Take pending publish events
    pub fn take_publishes(&mut self) -> Vec<(String, serde_json::Value)> {
        std::mem::take(&mut self.pending_publishes)
    }

    /// Get keyboard bindings for help menu
    pub fn get_key_bindings(&self) -> Vec<(KeyBinding, String)> {
        use crate::tui::Subscription;

        A::subscriptions(&self.state)
            .into_iter()
            .filter_map(|sub| match sub {
                Subscription::Keyboard { key, description, .. } => Some((key, description)),
                _ => None,
            })
            .collect()
    }

    /// Get the app's title (static string)
    pub fn get_title(&self) -> &'static str {
        A::title()
    }

    /// Get the app's status (optional, dynamic)
    pub fn get_status(&self) -> Option<ratatui::text::Line<'static>> {
        let config = crate::global_runtime_config();
        A::status(&self.state)
    }

    /// Get a reference to the app's state
    pub fn get_state(&self) -> &A::State {
        &self.state
    }

    /// Get the currently focused element ID
    pub fn get_focused_id(&self) -> Option<&FocusId> {
        self.focused_id.as_ref()
    }

    /// Set the interaction registry (for mouse events after rendering)
    pub fn set_registry(&mut self, registry: InteractionRegistry<A::Msg>) {
        self.registry = registry;
    }

    /// Focus the next element (Tab)
    pub fn focus_next(&mut self) -> Result<()> {
        let focusable_ids = self.focus_registry.focusable_ids_in_active_layer();

        if focusable_ids.is_empty() {
            return Ok(());
        }

        let next_id = if let Some(current_id) = &self.focused_id {
            if let Some(pos) = focusable_ids.iter().position(|id| id == current_id) {
                focusable_ids[(pos + 1) % focusable_ids.len()].clone()
            } else {
                focusable_ids[0].clone()
            }
        } else {
            focusable_ids[0].clone()
        };

        let cmd = Command::set_focus(next_id);
        self.execute_command(cmd)?;
        Ok(())
    }

    /// Focus the previous element (Shift-Tab)
    pub fn focus_previous(&mut self) -> Result<()> {
        let focusable_ids = self.focus_registry.focusable_ids_in_active_layer();

        if focusable_ids.is_empty() {
            return Ok(());
        }

        let prev_id = if let Some(current_id) = &self.focused_id {
            if let Some(pos) = focusable_ids.iter().position(|id| id == current_id) {
                let prev_pos = if pos == 0 {
                    focusable_ids.len() - 1
                } else {
                    pos - 1
                };
                focusable_ids[prev_pos].clone()
            } else {
                focusable_ids[0].clone()
            }
        } else {
            focusable_ids[focusable_ids.len() - 1].clone()
        };

        let cmd = Command::set_focus(prev_id);
        self.execute_command(cmd)?;
        Ok(())
    }

    /// Poll timer subscriptions and fire those that are ready
    pub fn poll_timers(&mut self) -> Result<()> {
        let now = Instant::now();
        let mut messages = Vec::new();

        // Check which timers need to fire
        for (interval, last_tick, msg) in &mut self.timers {
            if now.duration_since(*last_tick) >= *interval {
                messages.push(msg.clone());
                *last_tick = now;
            }
        }

        // Execute messages
        let had_messages = !messages.is_empty();
        for msg in messages {
            let command = A::update(&mut self.state, msg);
            self.execute_command(command)?;
        }
        // Refresh subscriptions after timer events
        if had_messages {
            self.update_subscriptions();
        }

        Ok(())
    }

    /// Poll pending async commands and process completed ones
    pub async fn poll_async(&mut self) -> Result<()> {
        use std::future::Future;
        use std::pin::Pin;
        use std::task::{Context, Poll};

        let poll_start = std::time::Instant::now();

        // Log poll state
        if self.pending_async_count > 0 || !self.pending_parallel.is_empty() {
            log::debug!(
                "⏱️ poll_async: {} async, {} parallel tasks pending",
                self.pending_async_count,
                self.pending_parallel.len()
            );
        }

        // Receive completed async results from spawned tasks
        let mut had_completions = false;
        while let Ok(msg) = self.async_result_rx.try_recv() {
            self.pending_async_count = self.pending_async_count.saturating_sub(1);
            log::debug!("⏱️ Async task completed, {} remaining", self.pending_async_count);
            let command = A::update(&mut self.state, msg);
            self.execute_command(command)?;
            had_completions = true;
        }

        // Refresh subscriptions after async completions
        if had_completions {
            self.update_subscriptions();
        }

        // Poll progress receivers for incremental updates
        for (task_name, rx) in &mut self.progress_receivers {
            // Drain all available progress messages
            while let Ok(progress) = rx.try_recv() {
                log::info!("📊 Progress received for '{}': {}", task_name, progress);
                self.pending_publishes.push((
                    "loading:progress".to_string(),
                    serde_json::json!({
                        "task": task_name,
                        "status": "InProgress",
                        "progress": progress,
                    }),
                ));
            }
        }

        // Poll parallel coordination tasks (these are short-lived coordination futures, not I/O)
        let waker = futures::task::noop_waker();
        let mut cx = Context::from_waker(&waker);
        let mut parallel_completed = Vec::new();
        for (i, future) in self.pending_parallel.iter_mut().enumerate() {
            let poll_result = future.as_mut().poll(&mut cx);
            match &poll_result {
                Poll::Ready((task_idx, task_name)) => {
                    log::info!("⏱️ Parallel task {} '{}' completed", task_idx, task_name);
                    parallel_completed.push((i, *task_idx, task_name.clone()));
                }
                Poll::Pending => {
                    // Only log occasionally to avoid spam
                }
            }
        }

        // Log poll duration if there were pending tasks
        if !self.pending_parallel.is_empty() {
            let elapsed = poll_start.elapsed();
            if elapsed.as_millis() > 10 {
                log::warn!("⏱️ poll_async took {}ms with {} parallel tasks", elapsed.as_millis(), self.pending_parallel.len());
            }
        }

        // Process completed parallel tasks - FIRST publish all completion events
        parallel_completed.sort_by(|a, b| b.0.cmp(&a.0));
        for (i, task_idx, task_name) in parallel_completed {
            self.pending_parallel.remove(i);

            // Publish task completion to LoadingScreen
            log::info!("✓ Runtime - task '{}' completed, publishing loading:progress event", task_name);
            self.pending_publishes.push((
                "loading:progress".to_string(),
                serde_json::json!({
                    "task": task_name,
                    "status": "Completed",
                }),
            ));
        }

        // AFTER all completion events are published, check if all tasks are complete
        if let Some(coordinator) = &self.parallel_coordinator {
            let results = coordinator.results.lock().unwrap();
            let all_complete = results.iter().all(|r| r.is_some());

            if all_complete {
                log::info!("✓ Runtime - all parallel tasks complete, applying results");
                // Get all results and apply them via msg_mapper
                let total = coordinator.total_tasks;
                drop(results); // Release lock before taking coordinator

                // Take ownership of coordinator to get results
                if let Some(coordinator) = self.parallel_coordinator.take() {
                    let mut results = coordinator.results.lock().unwrap();

                    // Apply each result via msg_mapper
                    for idx in 0..total {
                        if let Some(result) = results[idx].take() {
                            let msg = (coordinator.msg_mapper)(idx, result);
                            let command = A::update(&mut self.state, msg);
                            self.execute_command(command)?;
                        }
                    }
                    // Refresh subscriptions after parallel task completions
                    self.update_subscriptions();

                    // DON'T set navigation target here!
                    // LoadingScreen will handle navigation after showing completion status
                    // via its countdown timer. The target is already stored in the
                    // "loading:init" event data that was sent to LoadingScreen.

                    // Clear all pending parallel futures and progress receivers
                    self.pending_parallel.clear();
                    self.progress_receivers.clear();
                }
            }
        }

        Ok(())
    }

    /// Update subscriptions based on current state
    fn update_subscriptions(&mut self) {
        self.key_subscriptions.clear();
        self.event_bus.clear();
        self.timers.clear();

        let subscriptions = A::subscriptions(&self.state);
        log::debug!("✓ Runtime - updating subscriptions, count: {}", subscriptions.len());
        for sub in subscriptions {
            match sub {
                Subscription::Keyboard { key, msg, description } => {
                    log::debug!("  Registering keyboard subscription: code={:?}, modifiers={:?} -> {}",
                                key.code, key.modifiers, description);
                    // description is used for help menus, not for runtime lookup
                    self.key_subscriptions.insert(key, msg);
                }
                Subscription::Subscribe { topic, handler } => {
                    self.event_bus
                        .entry(topic)
                        .or_insert_with(Vec::new)
                        .push(handler);
                }
                Subscription::Timer { interval, msg } => {
                    self.timers.push((interval, Instant::now(), msg));
                }
            }
        }
    }

    /// Handle a keyboard event
    pub fn handle_key(&mut self, key_event: KeyEvent) -> Result<bool> {
        if key_event.kind != KeyEventKind::Press {
            return Ok(true);
        }

        // If there's a focused element, try routing the key to it first
        if let Some(focused_id) = &self.focused_id {
            if let Some(focusable) = self.focus_registry.find_in_active_layer(focused_id) {
                match (focusable.on_key)(key_event) {
                    DispatchTarget::WidgetEvent(boxed_event) => {
                        // Try widget auto-dispatch
                        if self.state.dispatch_widget_event(focused_id, boxed_event.as_ref()) {
                            return Ok(true);  // Handled!
                        }
                        // Not handled - widget event was not auto-dispatched
                        // This means the widget has no #[widget] attribute or doesn't match
                        // Just ignore the event (no-op)
                        return Ok(true);
                    }
                    DispatchTarget::AppMsg(msg) => {
                        // Direct to update()
                        let command = A::update(&mut self.state, msg);
                        let result = self.execute_command(command)?;
                        self.update_subscriptions();
                        return Ok(result);
                    }
                    DispatchTarget::PassThrough => {
                        // Widget didn't handle this key
                        // Special case for Esc: progressive unfocus
                        if key_event.code == KeyCode::Esc {
                            // Always unfocus when Esc is pressed and widget returns PassThrough
                            let focused_id = self.focused_id.take().unwrap();
                            if let Some(focusable) = self.focus_registry.find_in_active_layer(&focused_id) {
                                if let Some(on_blur) = focusable.on_blur.clone() {
                                    let command = A::update(&mut self.state, on_blur);
                                    self.execute_command(command)?;
                                }
                            }
                            // Mark as explicitly unfocused (prevents auto-restore on next render)
                            self.explicitly_unfocused = true;
                            // Focus cleared, Escape consumed
                            return Ok(true);
                        }
                        // Fall through to global subscriptions for other keys
                    }
                }
            }
        }

        // No focused element handled it (or it returned PassThrough), check global subscriptions
        let binding = KeyBinding::with_modifiers(key_event.code, key_event.modifiers);
        log::debug!("🔍 Key event received: code={:?}, modifiers={:?}, created binding={:?}",
                    key_event.code, key_event.modifiers, binding);
        log::debug!("🔍 Registered subscriptions count: {}", self.key_subscriptions.len());

        // Debug: show some registered keys for comparison
        if self.key_subscriptions.len() < 20 {
            for (kb, _) in self.key_subscriptions.iter() {
                log::debug!("  - Registered: code={:?}, modifiers={:?}", kb.code, kb.modifiers);
            }
        }

        if let Some(msg) = self.key_subscriptions.get(&binding).cloned() {
            log::debug!("✓ Runtime - global subscription matched key {:?}", binding);
            let command = A::update(&mut self.state, msg);
            let result = self.execute_command(command)?;
            // Refresh subscriptions after state change (subscriptions may depend on state)
            self.update_subscriptions();
            return Ok(result);
        }

        log::debug!("✗ Runtime - no subscription for key {:?}", binding);
        Ok(true)
    }

    /// Handle a mouse event
    pub fn handle_mouse(&mut self, mouse_event: MouseEvent) -> Result<bool> {
        let pos = (mouse_event.column, mouse_event.row);

        match mouse_event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // STEP 1: Handle focus change
                if let Some(clicked_id) = self.focus_registry.find_at_position(pos.0, pos.1) {
                    // Clicked on focusable element - focus it if not already focused
                    if self.focused_id.as_ref() != Some(&clicked_id) {
                        let cmd = Command::set_focus(clicked_id);
                        self.execute_command(cmd)?;
                    }
                } else {
                    // Clicked on non-focusable area - clear focus
                    if self.focused_id.is_some() {
                        let cmd = Command::clear_focus();
                        self.execute_command(cmd)?;
                    }
                }

                // STEP 2: Handle click action
                if let Some(msg) = self.registry.find_click(pos.0, pos.1) {
                    let command = A::update(&mut self.state, msg);
                    let result = self.execute_command(command)?;
                    // Refresh subscriptions after state change
                    self.update_subscriptions();
                    return Ok(result);
                }
            }
            MouseEventKind::Moved => {
                // Handle focus-on-hover based on config
                let config = crate::global_runtime_config();
                match config.focus_mode {
                    FocusMode::Click => {
                        // Do nothing - focus only changes on click
                    }
                    FocusMode::Hover => {
                        // Always focus on hover
                        if let Some(hovered_id) = self.focus_registry.find_at_position(pos.0, pos.1) {
                            if self.focused_id.as_ref() != Some(&hovered_id) {
                                let cmd = Command::set_focus(hovered_id);
                                self.execute_command(cmd)?;
                            }
                        }
                    }
                    FocusMode::HoverWhenUnfocused => {
                        // Only focus on hover if nothing is currently focused
                        if self.focused_id.is_none() {
                            if let Some(hovered_id) = self.focus_registry.find_at_position(pos.0, pos.1) {
                                let cmd = Command::set_focus(hovered_id);
                                self.execute_command(cmd)?;
                            }
                        }
                    }
                }

                self.last_hover_pos = Some(pos);
            }
            MouseEventKind::ScrollUp => {
                // Scroll up - send as Up arrow key (or Left if Shift is held) to focused element
                if let Some(focused_id) = &self.focused_id {
                    if let Some(focusable) = self.focus_registry.find_in_active_layer(focused_id) {
                        // Check if scroll happened over the focused element
                        if pos.0 >= focusable.rect.x
                            && pos.0 < focusable.rect.x + focusable.rect.width
                            && pos.1 >= focusable.rect.y
                            && pos.1 < focusable.rect.y + focusable.rect.height
                        {
                            // If Shift is held, scroll left (horizontal); otherwise scroll up (vertical)
                            let key_code = if mouse_event.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) {
                                KeyCode::Left
                            } else {
                                KeyCode::Up
                            };
                            let scroll_up_event = KeyEvent::new(key_code, mouse_event.modifiers);
                            match (focusable.on_key)(scroll_up_event) {
                                DispatchTarget::WidgetEvent(boxed_event) => {
                                    if self.state.dispatch_widget_event(focused_id, boxed_event.as_ref()) {
                                        return Ok(true);
                                    }
                                    return Ok(true);
                                }
                                DispatchTarget::AppMsg(msg) => {
                                    let command = A::update(&mut self.state, msg);
                                    let result = self.execute_command(command)?;
                                    self.update_subscriptions();
                                    return Ok(result);
                                }
                                DispatchTarget::PassThrough => {
                                    // Scroll wheel on unfocused widget - ignore
                                }
                            }
                        }
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                // Scroll down - send as Down arrow key (or Right if Shift is held) to focused element
                if let Some(focused_id) = &self.focused_id {
                    if let Some(focusable) = self.focus_registry.find_in_active_layer(focused_id) {
                        // Check if scroll happened over the focused element
                        if pos.0 >= focusable.rect.x
                            && pos.0 < focusable.rect.x + focusable.rect.width
                            && pos.1 >= focusable.rect.y
                            && pos.1 < focusable.rect.y + focusable.rect.height
                        {
                            // If Shift is held, scroll right (horizontal); otherwise scroll down (vertical)
                            let key_code = if mouse_event.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) {
                                KeyCode::Right
                            } else {
                                KeyCode::Down
                            };
                            let scroll_down_event = KeyEvent::new(key_code, mouse_event.modifiers);
                            match (focusable.on_key)(scroll_down_event) {
                                DispatchTarget::WidgetEvent(boxed_event) => {
                                    if self.state.dispatch_widget_event(focused_id, boxed_event.as_ref()) {
                                        return Ok(true);
                                    }
                                    return Ok(true);
                                }
                                DispatchTarget::AppMsg(msg) => {
                                    let command = A::update(&mut self.state, msg);
                                    let result = self.execute_command(command)?;
                                    self.update_subscriptions();
                                    return Ok(result);
                                }
                                DispatchTarget::PassThrough => {
                                    // Scroll wheel on unfocused widget - ignore
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        Ok(true)
    }

    /// Handle a published event (from global or local event bus)
    pub fn handle_publish(&mut self, topic: &str, data: serde_json::Value) -> Result<()> {
        // Collect messages first to avoid borrow checker issues
        let messages: Vec<A::Msg> = if let Some(handlers) = self.event_bus.get(topic) {
            handlers
                .iter()
                .filter_map(|handler| handler(data.clone()))
                .collect()
        } else {
            Vec::new()
        };

        // Now execute the messages
        let had_messages = !messages.is_empty();
        for msg in messages {
            let command = A::update(&mut self.state, msg);
            self.execute_command(command)?;
        }
        // Refresh subscriptions after pub/sub events
        if had_messages {
            self.update_subscriptions();
        }
        Ok(())
    }

    /// Execute a command
    fn execute_command(&mut self, command: Command<A::Msg>) -> Result<bool> {
        match command {
            Command::None => Ok(true),

            Command::Batch(commands) => {
                for cmd in commands {
                    if !self.execute_command(cmd)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }

            Command::Quit => Ok(false),

            Command::NavigateTo(app_id) => {
                // Store navigation target to be picked up by multi-runtime
                self.navigation_target = Some(app_id);
                Ok(true)
            }

            Command::StartApp { app_id, params } => {
                // Store the app_id and params for MultiAppRuntime to pick up
                self.pending_start_app = Some((app_id, params));
                self.navigation_target = Some(app_id);
                Ok(true)
            }

            Command::WakeApp(app_id) => {
                // Store navigation target to be picked up by multi-runtime
                self.navigation_target = Some(app_id);
                Ok(true)
            }

            Command::RestartApp { app_id, params } => {
                // Publish special event for MultiAppRuntime to handle
                let encoded = serde_json::to_value(&app_id).unwrap_or(Value::Null);
                self.pending_publishes.push((
                    format!("__lifecycle:restart_app:{:?}", app_id),
                    serde_json::json!({
                        "app_id": encoded,
                        "params_type": std::any::type_name_of_val(&*params),
                    })
                ));
                self.navigation_target = Some(app_id);
                Ok(true)
            }

            Command::QuitSelf => {
                // Signal to MultiAppRuntime that this app wants to quit
                self.pending_publishes.push((
                    "__lifecycle:quit_self".to_string(),
                    Value::Null,
                ));
                Ok(true)
            }

            Command::SleepSelf => {
                // Signal to MultiAppRuntime that this app wants to sleep
                self.pending_publishes.push((
                    "__lifecycle:sleep_self".to_string(),
                    Value::Null,
                ));
                Ok(true)
            }

            Command::Publish { topic, data } => {
                // Handle locally first
                self.handle_publish(&topic, data.clone())?;
                // Store for global broadcasting by MultiAppRuntime
                self.pending_publishes.push((topic, data));
                Ok(true)
            }

            Command::Perform(future) => {
                // Spawn on tokio runtime and send result through channel
                let tx = self.async_result_tx.clone();
                self.pending_async_count += 1;
                tokio::spawn(async move {
                    let msg = future.await;
                    let _ = tx.send(msg);
                });
                Ok(true)
            }

            Command::PerformParallel { tasks, config, msg_mapper } => {
                // Navigate to LoadingScreen immediately
                let task_names: Vec<String> = tasks.iter().map(|t| t.description.clone()).collect();
                let total_tasks = tasks.len();

                // Start LoadingScreen with typed params
                use crate::tui::apps::screens::LoadingScreenParams;
                self.pending_start_app = Some((
                    AppId::LoadingScreen,
                    Box::new(LoadingScreenParams {
                        tasks: task_names.clone(),
                        target: config.on_complete,
                        caller: config.caller,
                        cancellable: config.cancellable,
                    })
                ));
                self.navigation_target = Some(AppId::LoadingScreen);

                // Initialize shared state for task results
                let results = Arc::new(Mutex::new((0..total_tasks).map(|_| None).collect::<Vec<_>>()));
                let msg_mapper = Arc::new(msg_mapper);

                // Set up coordinator
                self.parallel_coordinator = Some(ParallelTaskCoordinator {
                    total_tasks,
                    results: results.clone(),
                    msg_mapper: msg_mapper.clone(),
                    config: config.clone(),
                });

                // Spawn all tasks as separate futures
                for (idx, task) in tasks.into_iter().enumerate() {
                    let results_ref = results.clone();
                    let task_name = task.description.clone();
                    let future = task.future;

                    // Store progress receiver if task has one
                    if let Some(rx) = task.progress_rx {
                        log::info!("📝 Storing progress receiver for task: '{}'", task_name);
                        self.progress_receivers.push((task_name.clone(), rx));
                    }

                    // Immediately publish InProgress status
                    self.pending_publishes.push((
                        "loading:progress".to_string(),
                        serde_json::json!({
                            "task": task_name,
                            "status": "InProgress",
                        }),
                    ));

                    // Create wrapper future that stores result and signals completion
                    let wrapper_future = Box::pin(async move {
                        // Execute the actual task
                        let result = future.await;

                        // Store in shared results
                        {
                            let mut lock = results_ref.lock().unwrap();
                            lock[idx] = Some(result);
                        }

                        // Return task index and name for coordination
                        (idx, task_name)
                    });

                    self.pending_parallel.push(wrapper_future);
                }

                Ok(true)
            }

            Command::SetFocus(id) => {
                log::debug!("Command::SetFocus({:?}) - current focus: {:?}", id, self.focused_id);
                // Send blur to currently focused element (if any)
                if let Some(old_id) = self.focused_id.take() {
                    log::debug!("  Blurring old focus: {:?}", old_id);
                    if let Some(focusable) = self.focus_registry.find_in_active_layer(&old_id) {
                        if let Some(on_blur) = focusable.on_blur.clone() {
                            let cmd = A::update(&mut self.state, on_blur);
                            self.execute_command(cmd)?;
                        }
                    }
                }

                // Set new focus
                log::debug!("  Setting new focus: {:?}", id);
                self.focused_id = Some(id.clone());
                // Clear explicit unfocus flag (focus was set programmatically)
                self.explicitly_unfocused = false;

                // Send focus message to new element
                if let Some(focusable) = self.focus_registry.find_in_active_layer(&id) {
                    if let Some(on_focus) = focusable.on_focus.clone() {
                        let cmd = A::update(&mut self.state, on_focus);
                        self.execute_command(cmd)?;
                    }
                }

                Ok(true)
            }

            Command::ClearFocus => {
                if let Some(old_id) = self.focused_id.take() {
                    if let Some(focusable) = self.focus_registry.find_in_active_layer(&old_id) {
                        if let Some(on_blur) = focusable.on_blur.clone() {
                            let cmd = A::update(&mut self.state, on_blur);
                            self.execute_command(cmd)?;
                        }
                    }
                }
                Ok(true)
            }
        }
    }

    /// Render the current app
    pub fn render(&mut self, frame: &mut Frame) {
        let area = frame.size();
        self.render_to_area(frame, area);
    }

    /// Render the app to a specific area
    pub fn render_to_area(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        log::debug!("=== Runtime::render_to_area START - current focus: {:?} ===", self.focused_id);

        // Sync runtime's focus to active layer ONLY if it exists in the current registry
        // This prevents Command::SetFocus from corrupting layer 0 when focusing modal elements
        // that haven't been rendered yet
        if let Some(ref focused_id) = self.focused_id {
            if self.focus_registry.contains(focused_id) {
                log::debug!("Runtime: saving validated focus {:?} to active layer before clear", focused_id);
                self.focus_registry.save_layer_focus(Some(focused_id.clone()));
            } else {
                log::debug!("Runtime: NOT saving focus {:?} - doesn't exist in current registry", focused_id);
            }
        } else {
            // No focus - clear the active layer's saved focus
            self.focus_registry.save_layer_focus(None);
        }

        // Clear registries for this frame
        self.registry.clear();
        log::debug!("Runtime: calling focus_registry.clear()");
        self.focus_registry.clear();
        self.dropdown_registry.clear();

        // Set current hover position for auto-hover tracking
        self.registry.set_hover_pos(self.last_hover_pos);

        // Get the layered view from the app
        let config = crate::global_runtime_config();
        let layered_view = A::view(&mut self.state);

        log::debug!("Runtime: rendering layers");
        // Render using the new layered API
        Renderer::render_layers(
            frame,
            
            &mut self.registry,
            &mut self.focus_registry,
            self.focused_id.as_ref(),
            &layered_view,
            area,
            None, // No global UI area in single-app runtime
        );

        // Detect layer count changes (modal open/close)
        let current_layer_count = self.focus_registry.active_layer().map(|l| l.layer_index + 1).unwrap_or(1);
        if current_layer_count != self.previous_layer_count {
            log::debug!("Runtime: layer count changed {} -> {}, clearing explicit unfocus flag",
                       self.previous_layer_count, current_layer_count);
            self.explicitly_unfocused = false;
            self.previous_layer_count = current_layer_count;
        }

        log::debug!("Runtime: validating focus");
        // Check if focused element still exists in the tree
        if let Some(focused_id) = &self.focused_id {
            if !self.focus_registry.contains(focused_id) {
                log::debug!("Runtime: focused element {:?} no longer exists, clearing and attempting restore", focused_id);
                // Clear stale focus first
                self.focused_id = None;

                // Try to restore from layer stack (only valid IDs)
                self.focused_id = self.focus_registry.restore_focus_from_layers();
                log::debug!("Runtime: focus after restoration: {:?}", self.focused_id);
            } else {
                log::debug!("Runtime: focused element {:?} still exists", focused_id);
            }
        } else {
            // No focus currently - check if we should restore
            if self.explicitly_unfocused {
                // User explicitly unfocused via Escape - don't restore
                // This allows the second Escape to reach global subscriptions (e.g., close modal)
                log::debug!("Runtime: no current focus, user explicitly unfocused, leaving unfocused");
            } else {
                // Focus lost due to layer change or element disappearing - try to restore
                log::debug!("Runtime: no current focus, attempting restore from layers");
                let restored = self.focus_registry.restore_focus_from_layers();
                if restored.is_some() {
                    log::debug!("Runtime: restored focus from layers: {:?}", restored);
                    self.focused_id = restored;
                } else {
                    log::debug!("Runtime: no focus to restore");
                }
            }
        }

        // Save validated/restored focus to the active layer for next frame
        log::debug!("Runtime: saving focus {:?} to active layer", self.focused_id);
        self.focus_registry.save_layer_focus(self.focused_id.clone());

        // Process any render messages (e.g., from on_render callbacks)
        let render_messages = self.registry.take_render_messages();
        if !render_messages.is_empty() {
            log::debug!("Runtime: processing {} render messages", render_messages.len());
            for msg in render_messages {
                let command = A::update(&mut self.state, msg);
                if let Err(e) = self.execute_command(command) {
                    log::error!("Runtime: error executing render message command: {}", e);
                }
            }
        }

        log::debug!("=== Runtime::render_to_area END ===\n");
    }
}

/// Blanket implementation of AppRuntime for Runtime<A>
/// This allows different Runtime<App> types to be stored in a type-erased collection
impl<A: App + 'static> AppRuntime for Runtime<A>
where
    A::State: 'static,
    A::Msg: 'static,
{
    fn handle_key(&mut self, key_event: KeyEvent) -> Result<bool> {
        Runtime::handle_key(self, key_event)
    }

    fn handle_mouse(&mut self, mouse_event: MouseEvent) -> Result<bool> {
        Runtime::handle_mouse(self, mouse_event)
    }

    fn render_to_area(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        Runtime::render_to_area(self, frame, area)
    }

    fn get_title(&self) -> &'static str {
        Runtime::get_title(self)
    }

    fn get_status(&self) -> Option<ratatui::text::Line<'static>> {
        Runtime::get_status(self)
    }

    fn get_key_bindings(&self) -> Vec<(KeyBinding, String)> {
        Runtime::get_key_bindings(self)
    }

    fn poll_timers(&mut self) -> Result<()> {
        Runtime::poll_timers(self)
    }

    fn poll_async(&mut self) -> Pin<Box<dyn Future<Output = Result<()>> + '_>> {
        Box::pin(Runtime::poll_async(self))
    }

    fn take_navigation(&mut self) -> Option<AppId> {
        Runtime::take_navigation(self)
    }

    fn peek_navigation(&self) -> Option<AppId> {
        Runtime::peek_navigation(self)
    }

    fn take_start_app(&mut self) -> Option<(AppId, Box<dyn Any + Send>)> {
        Runtime::take_start_app(self)
    }

    fn take_publishes(&mut self) -> Vec<(String, Value)> {
        Runtime::take_publishes(self)
    }

    fn handle_publish(&mut self, topic: &str, data: Value) -> Result<()> {
        Runtime::handle_publish(self, topic, data)
    }

    fn focus_next(&mut self) -> Result<()> {
        Runtime::focus_next(self)
    }

    fn focus_previous(&mut self) -> Result<()> {
        Runtime::focus_previous(self)
    }

    fn can_quit(&self) -> Result<(), String> {
        A::can_quit(&self.state)
    }

    fn on_suspend(&mut self) -> Result<()> {
        let command = A::on_suspend(&mut self.state);
        self.execute_command(command)?;
        Ok(())
    }

    fn on_resume(&mut self) -> Result<()> {
        let command = A::on_resume(&mut self.state);
        self.execute_command(command)?;
        Ok(())
    }

    fn on_destroy(&mut self) -> Result<()> {
        let command = A::on_destroy(&mut self.state);
        self.execute_command(command)?;
        Ok(())
    }

    fn is_capturing_raw_input(&self) -> bool {
        A::is_capturing_raw_input(&self.state)
    }
}

/// Blanket implementation of AppFactory for Runtime<A>
impl<A: App + 'static> AppFactory for std::marker::PhantomData<A>
where
    A::State: 'static,
    A::Msg: 'static,
{
    fn create(&self, params: Box<dyn Any + Send>) -> Result<Box<dyn AppRuntime>> {
        // Downcast params to the correct type
        let typed_params = params.downcast::<A::InitParams>()
            .map_err(|_| anyhow::anyhow!("Invalid parameter type for app"))?;

        Ok(Box::new(Runtime::<A>::with_params(*typed_params)))
    }

    fn quit_policy(&self) -> QuitPolicy {
        A::quit_policy()
    }

    fn suspend_policy(&self) -> SuspendPolicy {
        A::suspend_policy()
    }
}