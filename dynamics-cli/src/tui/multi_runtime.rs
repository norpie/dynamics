use ratatui::Frame;
use ratatui::text::{Line, Span};
use ratatui::style::Style;
use ratatui::prelude::Stylize;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use anyhow::Result;
use std::collections::HashMap;
use std::time::Instant;

use crate::tui::{AppId, Runtime, AppRuntime, apps::{AppLauncher, LoadingScreen, ErrorScreen, SettingsApp, UpdateApp, EnvironmentSelectorApp, migration::{MigrationEnvironmentApp, MigrationComparisonSelectApp, EntityComparisonApp}, DeadlinesFileSelectApp, DeadlinesMappingApp, DeadlinesInspectionApp, OperationQueueApp, SelectQuestionnaireApp, copy_questionnaires::{CopyQuestionnaireApp, PushQuestionnaireApp}}, Element, LayoutConstraint, Layer, Theme, ThemeVariant, App, ModalState, KeyBinding, AppLifecycle};
use crate::tui::runtime::AppFactory;
use crate::tui::element::{ColumnBuilder, RowBuilder, FocusId};
use crate::tui::widgets::ScrollableState;
use crate::tui::modals::AppOverviewModal;

/// Group key bindings by description, combining keys with the same description into aliases
fn group_bindings_by_description(bindings: &[(KeyBinding, &str)]) -> Vec<(String, String)> {
    let mut grouped: HashMap<&str, Vec<String>> = HashMap::new();

    for (key, desc) in bindings {
        grouped.entry(*desc)
            .or_insert_with(Vec::new)
            .push(key.to_string());
    }

    let mut result: Vec<(String, String)> = grouped.into_iter()
        .map(|(desc, mut keys)| {
            // Sort keys to ensure consistent ordering (shorter keys first)
            keys.sort_by_key(|k| k.len());
            (keys.join("/"), desc.to_string())
        })
        .collect();

    // Sort by the first key in each group for consistent ordering
    result.sort_by(|a, b| a.0.cmp(&b.0));

    result
}

/// Messages for global UI elements (help menu, quit modal, etc.)
#[derive(Clone)]
enum GlobalMsg {
    // Quit modal
    QuitConfirm,
    QuitCancel,

    // Help modal
    HelpScroll(KeyCode),
    CloseHelp,

    // App overview modal
    CloseAppOverview,
}

/// Manages multiple app runtimes and handles navigation between them
pub struct MultiAppRuntime {
    /// App factories for lazy creation
    factories: HashMap<AppId, Box<dyn AppFactory>>,

    /// Active app instances (lazily created)
    runtimes: HashMap<AppId, Box<dyn AppRuntime>>,

    /// Lifecycle state for each app
    lifecycles: HashMap<AppId, AppLifecycle>,

    /// Timestamp of when each app was last active (for recency ordering)
    last_active_time: HashMap<AppId, Instant>,

    /// Currently active app
    active_app: AppId,

    // Global UI state
    help_modal: ModalState<()>,
    help_scroll_state: ScrollableState,
    quit_modal: ModalState<()>,
    app_overview_modal: ModalState<()>,

    // Global focus system
    global_interaction_registry: crate::tui::InteractionRegistry<GlobalMsg>,
    global_focus_registry: crate::tui::renderer::FocusRegistry<GlobalMsg>,
    global_focused_id: Option<FocusId>,

    /// Last time Tab key was pressed (for debouncing)
    last_tab_press: Option<Instant>,

    // Status scroll animation
    /// Current scroll offset for status text (character position)
    status_scroll_offset: usize,
    /// Last time the status scroll was updated
    status_scroll_timer: Instant,
    /// Cached status text to detect changes
    cached_status_text: Option<String>,
    /// Available width for status text (calculated during render)
    status_available_width: usize,
}

impl MultiAppRuntime {
    pub fn new() -> Self {
        let mut factories: HashMap<AppId, Box<dyn AppFactory>> = HashMap::new();
        let mut lifecycles = HashMap::new();

        // Register all app factories here - this is the ONLY place you need to add new apps!
        factories.insert(AppId::AppLauncher, Box::new(std::marker::PhantomData::<AppLauncher>));
        factories.insert(AppId::LoadingScreen, Box::new(std::marker::PhantomData::<LoadingScreen>));
        factories.insert(AppId::ErrorScreen, Box::new(std::marker::PhantomData::<ErrorScreen>));
        factories.insert(AppId::Settings, Box::new(std::marker::PhantomData::<SettingsApp>));
        factories.insert(AppId::UpdateApp, Box::new(std::marker::PhantomData::<UpdateApp>));
        factories.insert(AppId::EnvironmentSelector, Box::new(std::marker::PhantomData::<EnvironmentSelectorApp>));
        factories.insert(AppId::MigrationEnvironment, Box::new(std::marker::PhantomData::<MigrationEnvironmentApp>));
        factories.insert(AppId::MigrationComparisonSelect, Box::new(std::marker::PhantomData::<MigrationComparisonSelectApp>));
        factories.insert(AppId::EntityComparison, Box::new(std::marker::PhantomData::<EntityComparisonApp>));
        factories.insert(AppId::DeadlinesFileSelect, Box::new(std::marker::PhantomData::<DeadlinesFileSelectApp>));
        factories.insert(AppId::DeadlinesMapping, Box::new(std::marker::PhantomData::<DeadlinesMappingApp>));
        factories.insert(AppId::DeadlinesInspection, Box::new(std::marker::PhantomData::<DeadlinesInspectionApp>));
        factories.insert(AppId::OperationQueue, Box::new(std::marker::PhantomData::<OperationQueueApp>));
        factories.insert(AppId::SelectQuestionnaire, Box::new(std::marker::PhantomData::<SelectQuestionnaireApp>));
        factories.insert(AppId::CopyQuestionnaire, Box::new(std::marker::PhantomData::<CopyQuestionnaireApp>));
        factories.insert(AppId::PushQuestionnaire, Box::new(std::marker::PhantomData::<PushQuestionnaireApp>));

        // Mark all apps as NotCreated initially
        for app_id in factories.keys() {
            lifecycles.insert(*app_id, AppLifecycle::NotCreated);
        }

        let mut runtime = Self {
            factories,
            runtimes: HashMap::new(),
            lifecycles,
            last_active_time: HashMap::new(),
            active_app: AppId::AppLauncher,
            help_modal: ModalState::Closed,
            help_scroll_state: ScrollableState::new(),
            quit_modal: ModalState::Closed,
            app_overview_modal: ModalState::Closed,
            global_interaction_registry: crate::tui::InteractionRegistry::new(),
            global_focus_registry: crate::tui::renderer::FocusRegistry::new(),
            global_focused_id: None,
            last_tab_press: None,
            status_scroll_offset: 0,
            status_scroll_timer: Instant::now(),
            cached_status_text: None,
            status_available_width: 40, // Default, will be updated during render
        };

        // Eagerly create the AppLauncher since it's the starting app
        runtime.ensure_app_exists(AppId::AppLauncher, Box::new(())).ok();
        runtime.last_active_time.insert(AppId::AppLauncher, Instant::now());

        // Eagerly create the OperationQueue so it can receive pub/sub messages from any app
        runtime.ensure_app_exists(AppId::OperationQueue, Box::new(())).ok();

        runtime
    }

    /// Ensure an app exists (create it if not already created)
    fn ensure_app_exists(&mut self, app_id: AppId, params: Box<dyn std::any::Any + Send>) -> Result<()> {
        // If app already exists and is running or background, do nothing
        if matches!(self.lifecycles.get(&app_id), Some(AppLifecycle::Running) | Some(AppLifecycle::Background)) {
            return Ok(());
        }

        // Get the factory for this app
        let factory = self.factories.get(&app_id)
            .ok_or_else(|| anyhow::anyhow!("No factory registered for app {:?}", app_id))?;

        // Create the app instance
        let runtime = factory.create(params)?;

        // Store the runtime and mark as running
        self.runtimes.insert(app_id, runtime);
        self.lifecycles.insert(app_id, AppLifecycle::Running);

        log::info!("Created app {:?}", app_id);
        Ok(())
    }

    pub fn request_quit(&mut self) {
        self.quit_modal.open_empty();
        // Auto-focus the cancel button (first button in the quit modal)
        self.global_focused_id = Some(FocusId::new("quit-cancel-btn"));
    }

    /// Move focus within global UI elements (Tab/Shift-Tab)
    fn move_global_focus(&mut self, forward: bool) {
        self.global_focused_id = if forward {
            self.global_focus_registry.next_focus(self.global_focused_id.as_ref())
        } else {
            self.global_focus_registry.prev_focus(self.global_focused_id.as_ref())
        };
    }

    /// Handle keyboard input for global UI elements (when modals are open)
    /// Returns Some(should_continue) if the key was handled, None if it should pass through
    fn handle_global_key(&mut self, key_event: KeyEvent) -> Result<Option<bool>> {
        // Check if there's a focused element in the global focus registry
        if let Some(focused_id) = &self.global_focused_id {
            // Get the key handler for the focused element
            if let Some(msg) = self.global_focus_registry.dispatch_key(focused_id, key_event) {
                return Ok(Some(self.handle_global_msg(msg)?));
            }
        }

        // Key not handled by focused element
        Ok(None)
    }

    /// Handle global messages from global UI elements
    fn handle_global_msg(&mut self, msg: GlobalMsg) -> Result<bool> {
        match msg {
            GlobalMsg::QuitConfirm => {
                return Ok(false); // Quit application
            }
            GlobalMsg::QuitCancel => {
                self.quit_modal.close();
                self.global_focused_id = None; // Clear focus when closing modal
                return Ok(true);
            }
            GlobalMsg::HelpScroll(key) => {
                // Calculate content height (same as in render_help_menu)
                // Use configured keybinds from global config
                let config = crate::global_runtime_config();
                let help_key = config.get_keybind("global.help");
                let launcher_key = config.get_keybind("global.app_launcher");

                let global_bindings = vec![
                    (help_key, "Toggle help menu"),
                    (launcher_key, "Go to app launcher"),
                    (KeyBinding::new(KeyCode::Esc), "Close help menu"),
                ];

                let mut all_app_bindings: Vec<(AppId, &'static str, Vec<(KeyBinding, String)>)> = vec![];
                for (app_id, runtime) in &self.runtimes {
                    let title = runtime.get_title();
                    let bindings = runtime.get_key_bindings();
                    all_app_bindings.push((*app_id, title, bindings));
                }

                let current_app_data = all_app_bindings.iter()
                    .find(|(id, _, _)| *id == self.active_app)
                    .expect("Active app not found");

                let other_apps: Vec<_> = all_app_bindings.iter()
                    .filter(|(id, _, bindings)| *id != self.active_app && !bindings.is_empty())
                    .collect();

                // Calculate total items using grouped bindings
                let global_grouped = group_bindings_by_description(&global_bindings);
                let current_bindings_ref: Vec<(KeyBinding, &str)> = current_app_data.2.iter()
                    .map(|(key, desc)| (*key, desc.as_str()))
                    .collect();
                let current_grouped = group_bindings_by_description(&current_bindings_ref);

                let other_apps_grouped_count: usize = other_apps.iter()
                    .map(|(_, _, bindings)| {
                        let bindings_ref: Vec<(KeyBinding, &str)> = bindings.iter()
                            .map(|(key, desc)| (*key, desc.as_str()))
                            .collect();
                        let grouped = group_bindings_by_description(&bindings_ref);
                        2 + grouped.len() // section header + bindings
                    })
                    .sum();

                let total_items = 2 + // title + blank
                    (if !global_grouped.is_empty() { 2 + global_grouped.len() } else { 0 }) +
                    (if !current_grouped.is_empty() { 2 + current_grouped.len() } else { 0 }) +
                    other_apps_grouped_count +
                    2; // blank + footer

                // Get viewport height from last render (approximation: 20 lines for modal)
                let content_height = 20usize.saturating_sub(4);
                self.help_scroll_state.handle_key(key, total_items, content_height);
                return Ok(true);
            }
            GlobalMsg::CloseHelp => {
                self.help_modal.close();
                self.global_focused_id = None; // Clear focus when closing modal
                return Ok(true);
            }
            GlobalMsg::CloseAppOverview => {
                self.app_overview_modal.close();
                self.global_focused_id = None; // Clear focus when closing modal
                return Ok(true);
            }
        }
    }

    pub fn handle_key(&mut self, key_event: KeyEvent) -> Result<bool> {
        log::debug!("🎹 MultiRuntime::handle_key: key={:?}, mods={:?}", key_event.code, key_event.modifiers);

        // Priority 1: Global modal keyboard handling (Tab, focused elements)
        if self.quit_modal.is_open() || self.help_modal.is_open() || self.app_overview_modal.is_open() {
            // Tab/Shift-Tab: Move focus within global modal
            if KeyBinding::new(KeyCode::Tab).matches(&key_event) {
                // Check debouncing
                let config = crate::global_runtime_config();
                let now = Instant::now();
                if let Some(last_press) = self.last_tab_press {
                    let elapsed_ms = now.duration_since(last_press).as_millis() as u64;
                    if elapsed_ms < config.tab_debouncing_ms {
                        log::debug!("Tab press debounced in modal ({}ms < {}ms)", elapsed_ms, config.tab_debouncing_ms);
                        return Ok(true);  // Debounced - ignore this press
                    }
                }
                self.last_tab_press = Some(now);

                self.move_global_focus(true);
                return Ok(true);
            }
            if KeyBinding::shift(KeyCode::Tab).matches(&key_event) || key_event.code == KeyCode::BackTab {
                // Check debouncing
                let config = crate::global_runtime_config();
                let now = Instant::now();
                if let Some(last_press) = self.last_tab_press {
                    let elapsed_ms = now.duration_since(last_press).as_millis() as u64;
                    if elapsed_ms < config.tab_debouncing_ms {
                        log::debug!("Shift-Tab press debounced in modal ({}ms < {}ms)", elapsed_ms, config.tab_debouncing_ms);
                        return Ok(true);  // Debounced - ignore this press
                    }
                }
                self.last_tab_press = Some(now);

                self.move_global_focus(false);
                return Ok(true);
            }

            // Dispatch key to focused element
            if let Some(should_continue) = self.handle_global_key(key_event)? {
                return Ok(should_continue);
            }
        }

        // Priority 2: Quit modal hotkeys (Y/N/Esc)
        if self.quit_modal.is_open() {
            match key_event.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    return self.handle_global_msg(GlobalMsg::QuitConfirm);
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    return self.handle_global_msg(GlobalMsg::QuitCancel);
                }
                _ => return Ok(true),  // Consume all other keys
            }
        }

        // Priority 3: Help modal Esc to close
        if self.help_modal.is_open() {
            if key_event.code == KeyCode::Esc {
                return self.handle_global_msg(GlobalMsg::CloseHelp);
            }
            return Ok(true);  // Consume all other keys (except Tab, handled above)
        }

        // Priority 3.5: App overview modal Esc to close
        if self.app_overview_modal.is_open() {
            if key_event.code == KeyCode::Esc {
                return self.handle_global_msg(GlobalMsg::CloseAppOverview);
            }
            return Ok(true);  // Consume all other keys (except Tab, handled above)
        }

        // Priority 3.75: Check if app is capturing raw input (e.g., keybind capture mode)
        // If so, skip global keybind handling and delegate directly to the app
        let runtime = self.runtimes.get(&self.active_app)
            .expect("Active app not found in runtimes");
        if runtime.is_capturing_raw_input() {
            log::debug!("App is capturing raw input, skipping global keybinds");
            // Skip global keybinds - jump to Tab/Shift-Tab or app delegation
            // Since Tab/Shift-Tab are handled earlier (priority 1), we can delegate to app here
            let result = self.runtimes
                .get_mut(&self.active_app)
                .expect("Active app not found in runtimes")
                .handle_key(key_event)?;
            log::debug!("🔄 After delegating key to {:?} (capture mode), checking for side effects", self.active_app);
            self.broadcast_events()?;
            let _ = self.check_navigation()?;
            return Ok(result);
        }

        // Priority 4: Configurable help menu keybind
        let config = crate::global_runtime_config();
        let help_key = config.get_keybind("global.help");
        if help_key.matches(&key_event) {
            self.help_modal.open_empty();
            // Reset scroll state to top
            self.help_scroll_state = ScrollableState::new();
            self.global_focused_id = Some(FocusId::new("help-scroll")); // Auto-focus scrollable
            return Ok(true);
        }

        // Priority 5: Configurable app launcher keybind
        let launcher_key = config.get_keybind("global.app_launcher");
        if launcher_key.matches(&key_event) {
                log::info!("🚀 App launcher keybind pressed - navigating to AppLauncher from {:?}", self.active_app);

            // Clear any pending navigation that would go BACK to the current app (zombie navigations)
            // But preserve legitimate background operations going to other apps
            let current_app = self.active_app;
            for (app_id, runtime) in self.runtimes.iter_mut() {
                if *app_id != current_app && *app_id != AppId::AppLauncher {
                    // Peek at navigation target without taking it
                    if let Some(nav_target) = runtime.peek_navigation() {
                        if nav_target == current_app {
                            // This is a zombie navigation back to where we already are - clear it
                            runtime.take_navigation();
                            log::info!("  🧹 Cleared zombie navigation from {:?} back to current app {:?}", app_id, current_app);
                        }
                    }
                }
            }
            log::debug!("  Cleared zombie navigations");

            // Properly handle lifecycle transitions
            let current_app = self.active_app;
            if current_app != AppId::AppLauncher {
                log::debug!("  Switching from {:?} to AppLauncher", current_app);
                // Handle current app based on quit and suspend policies
                let quit_policy = self.factories.get(&current_app)
                    .map(|f| f.quit_policy())
                    .unwrap_or(crate::tui::QuitPolicy::Sleep);

                log::debug!("  Current app quit_policy: {:?}", quit_policy);

                match quit_policy {
                    crate::tui::QuitPolicy::Sleep | crate::tui::QuitPolicy::QuitOnIdle(_) => {
                        // Check suspend policy
                        let suspend_policy = self.factories.get(&current_app)
                            .map(|f| f.suspend_policy())
                            .unwrap_or(crate::tui::SuspendPolicy::Suspend);

                        log::debug!("  Current app suspend_policy: {:?}", suspend_policy);

                        match suspend_policy {
                            crate::tui::SuspendPolicy::Suspend => {
                                // Keep app in background and call on_suspend
                                log::info!("  🛑 Suspending {:?} (calling on_suspend)", current_app);
                                self.lifecycles.insert(current_app, AppLifecycle::Background);
                                if let Some(runtime) = self.runtimes.get_mut(&current_app) {
                                    runtime.on_suspend().ok();
                                }
                            }
                            crate::tui::SuspendPolicy::AlwaysActive => {
                                // Keep app in background but don't call on_suspend
                                log::info!("  📍 Backgrounding {:?} (AlwaysActive - no on_suspend)", current_app);
                                self.lifecycles.insert(current_app, AppLifecycle::Background);
                            }
                            crate::tui::SuspendPolicy::QuitOnSuspend => {
                                // Destroy app instead of suspending
                                log::info!("  💀 Destroying {:?} (QuitOnSuspend)", current_app);
                                if let Some(mut runtime) = self.runtimes.remove(&current_app) {
                                    runtime.on_destroy().ok();
                                }
                                self.lifecycles.insert(current_app, AppLifecycle::Dead);
                            }
                        }
                    }
                    crate::tui::QuitPolicy::QuitOnExit => {
                        log::info!("  💀 Destroying {:?} (QuitOnExit)", current_app);
                        if let Some(mut runtime) = self.runtimes.remove(&current_app) {
                            runtime.on_destroy().ok();
                        }
                        self.lifecycles.insert(current_app, AppLifecycle::Dead);
                    }
                }

                // Resume app launcher if it was backgrounded
                if matches!(self.lifecycles.get(&AppId::AppLauncher), Some(AppLifecycle::Background)) {
                    log::info!("  ▶️  Resuming AppLauncher from Background");
                    if let Some(runtime) = self.runtimes.get_mut(&AppId::AppLauncher) {
                        runtime.on_resume().ok();
                    }
                }
                self.lifecycles.insert(AppId::AppLauncher, AppLifecycle::Running);
            }

            self.active_app = AppId::AppLauncher;
            self.last_active_time.insert(AppId::AppLauncher, Instant::now());
            log::info!("✅ AppLauncher is now active");
            return Ok(true);
        }

        // Priority 6: Configurable app overview keybind
        let overview_key = config.get_keybind("global.app_overview");
        if overview_key.matches(&key_event) {
            self.app_overview_modal.open_empty();
            self.global_focused_id = Some(FocusId::new("app-overview-close")); // Auto-focus close button
            return Ok(true);
        }

        // When help menu is open, intercept keys for help control
        if self.help_modal.is_open() {
            match key_event.code {
                KeyCode::Esc => {
                    self.help_modal.close();
                    return Ok(true);
                }
                KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown
                | KeyCode::Home | KeyCode::End => {
                    // Delegate to GlobalMsg::HelpScroll handler which knows the dimensions
                    return self.handle_global_msg(GlobalMsg::HelpScroll(key_event.code));
                }
                _ => {
                    // Consume all other keys when help is open
                    return Ok(true);
                }
            }
        }

        // Global Tab/Shift-Tab navigation (before app-specific handling)
        if KeyBinding::new(KeyCode::Tab).matches(&key_event) {
            // Check debouncing
            let config = crate::global_runtime_config();
            let now = Instant::now();
            if let Some(last_press) = self.last_tab_press {
                let elapsed_ms = now.duration_since(last_press).as_millis() as u64;
                if elapsed_ms < config.tab_debouncing_ms {
                    log::debug!("Tab press debounced ({}ms < {}ms)", elapsed_ms, config.tab_debouncing_ms);
                    return Ok(true);  // Debounced - ignore this press
                }
            }
            self.last_tab_press = Some(now);

            let runtime = self.runtimes
                .get_mut(&self.active_app)
                .expect("Active app not found in runtimes");
            runtime.focus_next()?;
            return Ok(true);
        }
        if KeyBinding::shift(KeyCode::Tab).matches(&key_event) || key_event.code == KeyCode::BackTab {
            // Check debouncing
            let config = crate::global_runtime_config();
            let now = Instant::now();
            if let Some(last_press) = self.last_tab_press {
                let elapsed_ms = now.duration_since(last_press).as_millis() as u64;
                if elapsed_ms < config.tab_debouncing_ms {
                    log::debug!("Shift-Tab press debounced ({}ms < {}ms)", elapsed_ms, config.tab_debouncing_ms);
                    return Ok(true);  // Debounced - ignore this press
                }
            }
            self.last_tab_press = Some(now);

            let runtime = self.runtimes
                .get_mut(&self.active_app)
                .expect("Active app not found in runtimes");
            runtime.focus_previous()?;
            return Ok(true);
        }

        // Normal: delegate to active app
        let result = self.runtimes
            .get_mut(&self.active_app)
            .expect("Active app not found in runtimes")
            .handle_key(key_event)?;

        log::debug!("🔄 After delegating key to {:?}, checking for side effects", self.active_app);
        self.broadcast_events()?;
        let _ = self.check_navigation()?;
        Ok(result)
    }

    pub fn handle_mouse(&mut self, mouse_event: MouseEvent) -> Result<bool> {
        use crossterm::event::MouseEventKind;

        // When any global modal is open, check for interactions
        if self.quit_modal.is_open() || self.help_modal.is_open() {
            match mouse_event.kind {
                MouseEventKind::Down(_) => {
                    // Check for button clicks in global interaction registry
                    if let Some(msg) = self.global_interaction_registry.find_click(mouse_event.column, mouse_event.row) {
                        return self.handle_global_msg(msg);
                    }
                }
                MouseEventKind::ScrollUp => {
                    if self.help_modal.is_open() {
                        // Simulate 3 Up key presses for smooth scrolling
                        for _ in 0..3 {
                            let _ = self.handle_global_msg(GlobalMsg::HelpScroll(KeyCode::Up));
                        }
                    }
                    return Ok(true);
                }
                MouseEventKind::ScrollDown => {
                    if self.help_modal.is_open() {
                        // Simulate 3 Down key presses for smooth scrolling
                        for _ in 0..3 {
                            let _ = self.handle_global_msg(GlobalMsg::HelpScroll(KeyCode::Down));
                        }
                    }
                    return Ok(true);
                }
                _ => {}
            }
            return Ok(true); // Consume all mouse events when modal is open
        }

        let result = self.runtimes
            .get_mut(&self.active_app)
            .expect("Active app not found in runtimes")
            .handle_mouse(mouse_event)?;

        self.broadcast_events()?;
        let _ = self.check_navigation()?;
        Ok(result)
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let config = crate::global_runtime_config();
        let theme = &config.theme;
        let full_area = frame.size();

        // Calculate header area (3 lines) and app content area
        let header_height = 3;
        let header_area = ratatui::layout::Rect {
            x: full_area.x,
            y: full_area.y,
            width: full_area.width,
            height: header_height,
        };
        let app_area = ratatui::layout::Rect {
            x: full_area.x,
            y: full_area.y + header_height,
            width: full_area.width,
            height: full_area.height.saturating_sub(header_height),
        };

        // Render global header
        let active_runtime = self.runtimes.get(&self.active_app)
            .expect("Active app not found in runtimes");
        let app_title = active_runtime.get_title();
        let app_status = active_runtime.get_status();
        self.render_header(frame, header_area, app_title, app_status);

        // Render active app content
        self.runtimes.get_mut(&self.active_app)
            .expect("Active app not found in runtimes")
            .render_to_area(frame, app_area);

        // If help menu is open, overlay it on top
        if self.help_modal.is_open() {
            self.render_help_menu(frame, full_area);
        }

        // If app overview modal is open, overlay it on top
        if self.app_overview_modal.is_open() {
            self.render_app_overview(frame, full_area);
        }

        // If quit confirmation is open, overlay it on top (highest priority)
        if self.quit_modal.is_open() {
            self.render_quit_confirm(frame, full_area);
        }
    }

    fn render_header(&mut self, frame: &mut Frame, area: ratatui::layout::Rect, title: &str, status: Option<Line<'static>>) {
        let config = crate::global_runtime_config();
        let theme = &config.theme;

        // Calculate help text width first (needed for available space calculation)
        use unicode_width::UnicodeWidthStr;
        let help_key_str = config.get_keybind("global.help").to_string();
        let help_text = format!("[?] {} Help", help_key_str);
        let help_text_width = help_text.width();

        // Calculate available width for status text
        // Total width - title width - separator " │ " (3 chars) - help text width - spacing (1) - panel borders (2)
        let title_width = title.width();
        let separator_width = 3; // " │ "
        let spacing_width = 1; // Spacing between status and help text
        let panel_borders = 2; // Panel left + right borders (1 + 1)
        let available_for_status = area.width.saturating_sub(title_width as u16)
            .saturating_sub(separator_width as u16)
            .saturating_sub(help_text_width as u16)
            .saturating_sub(spacing_width)
            .saturating_sub(panel_borders) as usize;

        // Store the calculated available width for scroll logic
        self.status_available_width = available_for_status;

        // Build title line with optional status
        let title_line = if let Some(status_line) = status {
            // Apply scroll offset if scrolling is active
            let scrolled_status = if self.status_scroll_offset > 0 {
                self.apply_scroll_to_status(status_line)
            } else {
                status_line
            };

            // Combine title and status with separator
            let mut spans = vec![
                Span::styled(String::from(title), Style::default().fg(theme.accent_secondary).bold()),
                Span::styled(" │ ", Style::default().fg(theme.border_primary)),
            ];
            spans.extend(scrolled_status.spans);
            Line::from(spans)
        } else {
            // Just title
            Line::from(Span::styled(String::from(title), Style::default().fg(theme.accent_secondary).bold()))
        };

        let header_left = Element::styled_text(title_line).build();
        let header_right = Element::styled_text(Line::from(vec![
            Span::styled(help_text, Style::default().fg(theme.border_primary))
        ])).build();

        let header = Element::panel(
            RowBuilder::new()
                .add(header_left, LayoutConstraint::Fill(1))
                .add(header_right, LayoutConstraint::Length(help_text_width as u16))
                .spacing(1)
                .build()
        )
        .build();

        use crate::tui::{Renderer, InteractionRegistry};
        use crate::tui::renderer::{FocusRegistry, DropdownRegistry};
        let mut registry: InteractionRegistry<()> = InteractionRegistry::new();
        let mut focus_registry: FocusRegistry<()> = FocusRegistry::new();
        let mut dropdown_registry: DropdownRegistry<()> = DropdownRegistry::new();
        Renderer::render(frame, &mut registry, &mut focus_registry, &mut dropdown_registry, None, &header, area);
    }

    fn render_help_menu(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let theme = &crate::global_runtime_config().theme;
        // First, render a dim overlay over the entire area
        use ratatui::widgets::Paragraph;
        use ratatui::style::Style;
        let dim_overlay = Paragraph::new("")
            .style(Style::default().bg(theme.bg_surface));
        frame.render_widget(dim_overlay, area);

        // Build help content directly as Element<GlobalMsg>
        // Use configured keybinds from global config
        let config = crate::global_runtime_config();
        let help_key = config.get_keybind("global.help");
        let launcher_key = config.get_keybind("global.app_launcher");

        let global_bindings = vec![
            (help_key, "Toggle help menu"),
            (launcher_key, "Go to app launcher"),
            (KeyBinding::new(KeyCode::Esc), "Close help menu"),
        ];

        // Get all apps' key bindings
        let mut all_app_bindings: Vec<(AppId, &'static str, Vec<(KeyBinding, String)>)> = vec![];
        for (app_id, runtime) in &self.runtimes {
            let title = runtime.get_title();
            let bindings = runtime.get_key_bindings();
            all_app_bindings.push((*app_id, title, bindings));
        }

        let current_app_data = all_app_bindings.iter()
            .find(|(id, _, _)| *id == self.active_app)
            .expect("Active app not found");

        let other_apps: Vec<_> = all_app_bindings.iter()
            .filter(|(id, _, bindings)| *id != self.active_app && !bindings.is_empty())
            .collect();

        // Pre-group all bindings to calculate max width across entire help menu
        let global_grouped = group_bindings_by_description(&global_bindings);

        let current_bindings_ref: Vec<(KeyBinding, &str)> = current_app_data.2.iter()
            .map(|(key, desc)| (*key, desc.as_str()))
            .collect();
        let current_grouped = group_bindings_by_description(&current_bindings_ref);

        let other_apps_grouped: Vec<(AppId, &'static str, Vec<(String, String)>)> = other_apps.iter()
            .map(|(id, title, bindings)| {
                let bindings_ref: Vec<(KeyBinding, &str)> = bindings.iter()
                    .map(|(key, desc)| (*key, desc.as_str()))
                    .collect();
                let grouped = group_bindings_by_description(&bindings_ref);
                (*id, *title, grouped)
            })
            .collect();

        // Calculate max key width across ALL sections
        let max_key_width = global_grouped.iter()
            .chain(current_grouped.iter())
            .chain(other_apps_grouped.iter().flat_map(|(_, _, grouped)| grouped.iter()))
            .map(|(keys, _)| keys.len())
            .max()
            .unwrap_or(0);

        // Build help content
        let mut help_items: Vec<Element<GlobalMsg>> = vec![
            Element::styled_text(Line::from(vec![
                Span::styled("Keyboard Shortcuts", Style::default().fg(theme.accent_primary).bold())
            ])).build(),
            Element::text(""),
        ];

        // Global section
        if !global_grouped.is_empty() {
            help_items.push(Element::styled_text(Line::from(vec![
                Span::styled("▼ Global", Style::default().fg(theme.accent_muted).bold())
            ])).build());

            for (keys, desc) in global_grouped {
                let padding = " ".repeat(max_key_width - keys.len());
                let line = Line::from(vec![
                    Span::styled(format!("  {}{}", keys, padding), Style::default().fg(theme.accent_tertiary)),
                    Span::raw("  "),
                    Span::styled(desc, Style::default().fg(theme.text_primary)),
                ]);
                help_items.push(Element::styled_text(line).build());
            }
            help_items.push(Element::text(""));
        }

        // Current app section (only show if it has bindings)
        if !current_grouped.is_empty() {
            help_items.push(Element::styled_text(Line::from(vec![
                Span::styled(format!("▼ {}", current_app_data.1), Style::default().fg(theme.accent_secondary).bold())
            ])).build());

            for (keys, desc) in current_grouped {
                let padding = " ".repeat(max_key_width - keys.len());
                let line = Line::from(vec![
                    Span::styled(format!("  {}{}", keys, padding), Style::default().fg(theme.accent_success)),
                    Span::raw("  "),
                    Span::styled(desc, Style::default().fg(theme.text_primary)),
                ]);
                help_items.push(Element::styled_text(line).build());
            }
            help_items.push(Element::text(""));
        }

        // Other apps sections
        for (_, app_title, grouped) in other_apps_grouped {
            help_items.push(Element::styled_text(Line::from(vec![
                Span::styled(format!("▼ {}", app_title), Style::default().fg(theme.border_primary).bold())
            ])).build());

            for (keys, desc) in grouped {
                let padding = " ".repeat(max_key_width - keys.len());
                let line = Line::from(vec![
                    Span::styled(format!("  {}{}", keys, padding), Style::default().fg(theme.border_tertiary)),
                    Span::raw("  "),
                    Span::styled(desc, Style::default().fg(theme.text_tertiary)),
                ]);
                help_items.push(Element::styled_text(line).build());
            }
            help_items.push(Element::text(""));
        }

        help_items.push(Element::text(""));
        help_items.push(Element::styled_text(Line::from(vec![
            Span::styled("[ESC to close | ↑↓/PgUp/PgDn/Home/End to scroll]", Style::default().fg(theme.border_primary))
        ])).build());

        // Build column with all items
        let mut column_builder = ColumnBuilder::new();
        for item in help_items {
            column_builder = column_builder.add(item, LayoutConstraint::Length(1));
        }
        let help_column = column_builder.spacing(0).build();

        // Calculate modal dimensions
        let modal_width = area.width.min(60);
        let modal_height = area.height.min(20);
        let modal_area = ratatui::layout::Rect {
            x: area.x + (area.width.saturating_sub(modal_width)) / 2,
            y: area.y + (area.height.saturating_sub(modal_height)) / 2,
            width: modal_width,
            height: modal_height,
        };

        // Clear the modal area to prevent bleed-through from dim overlay
        use ratatui::widgets::Clear;
        frame.render_widget(Clear, modal_area);

        let content_height = modal_height.saturating_sub(4) as usize;

        // Count total items for scroll state
        let total_items = 2 + // title + blank
            (if !global_bindings.is_empty() { 2 + global_bindings.len() } else { 0 }) +
            2 + current_app_data.2.len() +
            other_apps.iter().map(|(_, _, bindings)| 2 + bindings.len()).sum::<usize>() +
            2; // blank + footer

        // Set viewport height for scrolloff calculations
        self.help_scroll_state.set_viewport_height(content_height);
        self.help_scroll_state.update_scroll(content_height, total_items);

        // Wrap in scrollable with on_navigate
        let scrollable = Element::scrollable("help-scroll", help_column, &self.help_scroll_state)
            .on_navigate(GlobalMsg::HelpScroll)
            .build();

        let modal = Element::panel(scrollable)
            .title("Help")
            .build();

        // Render with global registries
        use crate::tui::Renderer;
        use crate::tui::renderer::DropdownRegistry;
        let mut dropdown_registry: DropdownRegistry<GlobalMsg> = DropdownRegistry::new();

        // Sync runtime's focus to active layer BEFORE clearing registries
        self.global_focus_registry.save_layer_focus(self.global_focused_id.clone());

        // Clear and render to global registries
        self.global_interaction_registry = crate::tui::InteractionRegistry::new();
        self.global_focus_registry = crate::tui::renderer::FocusRegistry::new();
        Renderer::render(frame, &mut self.global_interaction_registry, &mut self.global_focus_registry, &mut dropdown_registry, self.global_focused_id.as_ref(), &modal, modal_area);

        // Check if focused element still exists in the tree
        if let Some(focused_id) = &self.global_focused_id {
            if !self.global_focus_registry.contains(focused_id) {
                // Clear stale focus first
                self.global_focused_id = None;

                // Try to restore from layer stack (only valid IDs)
                self.global_focused_id = self.global_focus_registry.restore_focus_from_layers();
            }
        } else {
            // No focus currently - try to restore from layer stack if there are focusables
            self.global_focused_id = self.global_focus_registry.restore_focus_from_layers();
        }

        // Save validated/restored focus to the active layer for next frame
        self.global_focus_registry.save_layer_focus(self.global_focused_id.clone());
    }

    /// Render quit confirmation modal
    fn render_quit_confirm(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let theme = &crate::global_runtime_config().theme;
        use ratatui::widgets::Paragraph;
        use ratatui::style::Style;

        // Render dim overlay
        let dim_overlay = Paragraph::new("")
            .style(Style::default().bg(theme.bg_surface));
        frame.render_widget(dim_overlay, area);

        // Build quit confirmation modal using Element<GlobalMsg>
        let button_row = RowBuilder::new()
            .add(
                Element::button("quit-cancel-btn", "No (N/Esc)")
                    .on_press(GlobalMsg::QuitCancel)
                    .build(),
                LayoutConstraint::Fill(1),
            )
            .add(
                Element::text("  "),
                LayoutConstraint::Length(2),
            )
            .add(
                Element::button("quit-confirm-btn", "Yes (Y)")
                    .on_press(GlobalMsg::QuitConfirm)
                    .build(),
                LayoutConstraint::Fill(1),
            )
            .spacing(0)
            .build();

        let modal_content = ColumnBuilder::new()
            .add(
                Element::text("Quit Application"),
                LayoutConstraint::Length(1),
            )
            .add(
                Element::text(""),
                LayoutConstraint::Length(1),
            )
            .add(
                Element::text("Are you sure you want to quit?"),
                LayoutConstraint::Length(1),
            )
            .add(
                Element::text(""),
                LayoutConstraint::Length(1),
            )
            .add(
                button_row,
                LayoutConstraint::Length(3),
            )
            .spacing(0)
            .build();

        let quit_modal = Element::panel(
            Element::container(modal_content)
                .padding(1)
                .build()
        )
        .title("Confirmation")
        .width(50)
        .height(11)
        .build();

        // Calculate modal position
        let modal_width = 50;
        let modal_height = 11;
        let modal_area = ratatui::layout::Rect {
            x: area.x + (area.width.saturating_sub(modal_width)) / 2,
            y: area.y + (area.height.saturating_sub(modal_height)) / 2,
            width: modal_width,
            height: modal_height,
        };

        // Clear the modal area to prevent bleed-through from dim overlay
        use ratatui::widgets::Clear;
        frame.render_widget(Clear, modal_area);

        // Render with global registries
        use crate::tui::Renderer;
        use crate::tui::renderer::DropdownRegistry;
        let mut dropdown_registry: DropdownRegistry<GlobalMsg> = DropdownRegistry::new();

        // Sync runtime's focus to active layer BEFORE clearing registries
        self.global_focus_registry.save_layer_focus(self.global_focused_id.clone());

        // Clear and render to global registries
        self.global_interaction_registry = crate::tui::InteractionRegistry::new();
        self.global_focus_registry = crate::tui::renderer::FocusRegistry::new();
        Renderer::render(frame, &mut self.global_interaction_registry, &mut self.global_focus_registry, &mut dropdown_registry, self.global_focused_id.as_ref(), &quit_modal, modal_area);

        // Check if focused element still exists in the tree
        if let Some(focused_id) = &self.global_focused_id {
            if !self.global_focus_registry.contains(focused_id) {
                // Clear stale focus first
                self.global_focused_id = None;

                // Try to restore from layer stack (only valid IDs)
                self.global_focused_id = self.global_focus_registry.restore_focus_from_layers();
            }
        } else {
            // No focus currently - try to restore from layer stack if there are focusables
            self.global_focused_id = self.global_focus_registry.restore_focus_from_layers();
        }

        // Save validated/restored focus to the active layer for next frame
        self.global_focus_registry.save_layer_focus(self.global_focused_id.clone());
    }

    /// Render app overview modal
    fn render_app_overview(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let theme = &crate::global_runtime_config().theme;
        use ratatui::widgets::Paragraph;
        use ratatui::style::Style;

        // Render dim overlay
        let dim_overlay = Paragraph::new("")
            .style(Style::default().bg(theme.bg_surface));
        frame.render_widget(dim_overlay, area);

        // Get recent apps first (Running/Background in recency order)
        let recent_app_ids = self.get_recent_apps();
        let mut apps: Vec<(AppId, AppLifecycle)> = recent_app_ids.iter()
            .filter_map(|id| {
                self.lifecycles.get(id).map(|lifecycle| (*id, *lifecycle))
            })
            .collect();

        // Then add other apps (NotCreated, Dead, QuittingRequested) sorted by name
        let mut other_apps: Vec<(AppId, AppLifecycle)> = self.lifecycles.iter()
            .filter(|(id, lifecycle)| {
                !matches!(lifecycle, AppLifecycle::Running | AppLifecycle::Background)
            })
            .map(|(id, lifecycle)| (*id, *lifecycle))
            .collect();
        other_apps.sort_by_key(|(id, _)| format!("{:?}", id));
        apps.extend(other_apps);

        // Calculate modal height before moving apps
        let app_count = apps.len();

        // Build app overview modal using AppOverviewModal
        let modal = AppOverviewModal::new(apps)
            .on_close(GlobalMsg::CloseAppOverview)
            .build();

        // Calculate modal position (centered)
        let modal_width = 80;
        let modal_height = (app_count + 10).min(area.height.saturating_sub(4) as usize) as u16; // Title + blank + apps + blank + button + borders
        let modal_area = ratatui::layout::Rect {
            x: area.x + (area.width.saturating_sub(modal_width)) / 2,
            y: area.y + (area.height.saturating_sub(modal_height)) / 2,
            width: modal_width,
            height: modal_height,
        };

        // Clear the modal area to prevent bleed-through from dim overlay
        use ratatui::widgets::Clear;
        frame.render_widget(Clear, modal_area);

        // Render with global registries
        use crate::tui::Renderer;
        use crate::tui::renderer::DropdownRegistry;
        let mut dropdown_registry: DropdownRegistry<GlobalMsg> = DropdownRegistry::new();

        // Sync runtime's focus to active layer BEFORE clearing registries
        self.global_focus_registry.save_layer_focus(self.global_focused_id.clone());

        // Clear and render to global registries
        self.global_interaction_registry = crate::tui::InteractionRegistry::new();
        self.global_focus_registry = crate::tui::renderer::FocusRegistry::new();
        Renderer::render(frame, &mut self.global_interaction_registry, &mut self.global_focus_registry, &mut dropdown_registry, self.global_focused_id.as_ref(), &modal, modal_area);

        // Check if focused element still exists in the tree
        if let Some(focused_id) = &self.global_focused_id {
            if !self.global_focus_registry.contains(focused_id) {
                // Clear stale focus first
                self.global_focused_id = None;

                // Try to restore from layer stack (only valid IDs)
                self.global_focused_id = self.global_focus_registry.restore_focus_from_layers();
            }
        } else {
            // No focus currently - try to restore from layer stack if there are focusables
            self.global_focused_id = self.global_focus_registry.restore_focus_from_layers();
        }

        // Save validated/restored focus to the active layer for next frame
        self.global_focus_registry.save_layer_focus(self.global_focused_id.clone());
    }

    /// Get apps ordered by recency (most recent first), filtered to Running/Background apps only
    pub fn get_recent_apps(&self) -> Vec<AppId> {
        let mut apps: Vec<(AppId, Instant)> = self.last_active_time
            .iter()
            .filter(|(id, _)| {
                // Only include Running or Background apps
                matches!(self.lifecycles.get(id),
                    Some(AppLifecycle::Running) | Some(AppLifecycle::Background))
            })
            .map(|(id, time)| (*id, *time))
            .collect();

        // Sort by timestamp descending (most recent first)
        apps.sort_by(|a, b| b.1.cmp(&a.1));

        // Return just the app IDs
        apps.into_iter().map(|(id, _)| id).collect()
    }

    pub async fn poll_async(&mut self) -> Result<()> {
        // Poll all apps regardless of which is active
        for runtime in self.runtimes.values_mut() {
            runtime.poll_async().await?;
        }
        Ok(())
    }

    /// Poll timer subscriptions for all apps
    pub fn poll_timers(&mut self) -> Result<()> {
        // Poll all apps regardless of which is active
        for runtime in self.runtimes.values_mut() {
            runtime.poll_timers()?;
        }

        // Update status scroll animation
        self.update_status_scroll();

        Ok(())
    }

    /// Apply scroll offset to status line
    fn apply_scroll_to_status(&self, status_line: Line<'static>) -> Line<'static> {
        use unicode_width::UnicodeWidthStr;
        use unicode_width::UnicodeWidthChar;

        // Calculate character positions for each span
        let mut char_positions: Vec<(usize, usize, &Span)> = Vec::new(); // (start_pos, end_pos, span)
        let mut current_pos = 0;

        for span in &status_line.spans {
            let text = span.content.as_ref();
            let width = text.width();
            char_positions.push((current_pos, current_pos + width, span));
            current_pos += width;
        }

        // Build new spans starting from offset
        let offset = self.status_scroll_offset;
        let mut result_spans: Vec<Span> = Vec::new();

        // Find spans that overlap with the visible region [offset, ...]
        for (start, end, span) in char_positions {
            if end <= offset {
                // Span is completely before the visible region
                continue;
            }

            if start >= offset {
                // Span is completely within or after the visible region - include it
                result_spans.push(span.clone());
            } else {
                // Span starts before offset but ends within/after visible region - slice it
                let text = span.content.as_ref();
                let chars_to_skip = offset - start;

                // Use char indices to properly handle unicode
                let sliced_text: String = text.chars().skip(chars_to_skip).collect();
                result_spans.push(Span::styled(sliced_text, span.style));
            }
        }

        Line::from(result_spans)
    }

    /// Update the status scroll offset for long status text
    fn update_status_scroll(&mut self) {
        const SCROLL_INTERVAL_MS: u128 = 150; // Milliseconds between scroll updates
        const PAUSE_AT_START_MS: u128 = 2000; // Pause at the start before scrolling
        const PAUSE_AT_END_MS: u128 = 1000; // Pause at the end before resetting

        let now = Instant::now();
        let elapsed = now.duration_since(self.status_scroll_timer).as_millis();

        // Get current status text from active app
        if let Some(runtime) = self.runtimes.get(&self.active_app) {
            if let Some(status_line) = runtime.get_status() {
                // Convert status Line to plain text for length calculation
                let status_text: String = status_line.spans.iter()
                    .map(|span| span.content.as_ref())
                    .collect::<Vec<_>>()
                    .join("");

                // If status text changed, reset scroll
                if self.cached_status_text.as_ref() != Some(&status_text) {
                    self.cached_status_text = Some(status_text.clone());
                    self.status_scroll_offset = 0;
                    self.status_scroll_timer = now;
                    return;
                }

                // Calculate text width using unicode-width
                use unicode_width::UnicodeWidthStr;
                let text_width = status_text.width();

                // Use the calculated available width from render_header
                let available_width = self.status_available_width;

                // Only scroll if text is longer than available space
                if text_width > available_width {
                    let max_offset = text_width.saturating_sub(available_width);

                    // Handle pause at start
                    if self.status_scroll_offset == 0 && elapsed < PAUSE_AT_START_MS {
                        return;
                    }

                    // Handle pause at end
                    if self.status_scroll_offset >= max_offset && elapsed < PAUSE_AT_END_MS {
                        return;
                    }

                    // Time to scroll
                    if elapsed >= SCROLL_INTERVAL_MS {
                        self.status_scroll_timer = now;

                        if self.status_scroll_offset >= max_offset {
                            // Reset to beginning
                            self.status_scroll_offset = 0;
                        } else {
                            // Scroll by 1 character
                            self.status_scroll_offset += 1;
                        }
                    }
                } else {
                    // Text fits, no scrolling needed
                    self.status_scroll_offset = 0;
                }
            } else {
                // No status, reset
                self.cached_status_text = None;
                self.status_scroll_offset = 0;
            }
        }
    }

    /// Check if any navigation commands were issued
    fn check_navigation(&mut self) -> Result<bool> {
        // Check ALL apps for navigation requests (not just active app)
        // This allows background apps (like LoadingScreen) to trigger navigation when tasks complete
        let mut start_app_request = None;
        let mut nav_target = None;
        let mut nav_source_app = self.active_app;

        log::debug!("🔍 check_navigation - scanning for navigation requests");
        log::debug!("  Active app: {:?}", self.active_app);

        // Check active app first (priority)
        if let Some(runtime) = self.runtimes.get_mut(&self.active_app) {
            start_app_request = runtime.take_start_app();
            nav_target = runtime.take_navigation();
            if start_app_request.is_some() {
                log::info!("  ✓ Active app {:?} has start_app request", self.active_app);
            }
            if nav_target.is_some() {
                log::info!("  ✓ Active app {:?} has navigation request to {:?}", self.active_app, nav_target);
            }
        }

        // If active app didn't request navigation, check all background apps
        if start_app_request.is_none() && nav_target.is_none() {
            log::debug!("  No navigation from active app, scanning background apps...");
            for (app_id, runtime) in self.runtimes.iter_mut() {
                if *app_id != self.active_app {
                    if let Some(start) = runtime.take_start_app() {
                        log::info!("  🎯 Background app {:?} has start_app request", app_id);
                        start_app_request = Some(start);
                        nav_source_app = *app_id;
                        break;
                    }
                    if let Some(target) = runtime.take_navigation() {
                        log::info!("  🎯 Background app {:?} has navigation request to {:?}", app_id, target);
                        nav_target = Some(target);
                        nav_source_app = *app_id;
                        break;
                    }
                }
            }
        }

        // Handle start_app first (it includes params)
        if let Some((target, params)) = start_app_request {
            log::info!("⚡ Processing start_app navigation: {:?} -> {:?} (from {:?})", self.active_app, target, nav_source_app);
            // Handle current app based on quit and suspend policies
            if let Some(current_runtime) = self.runtimes.get(&self.active_app) {
                let quit_policy = self.factories.get(&self.active_app)
                    .map(|f| f.quit_policy())
                    .unwrap_or(crate::tui::QuitPolicy::Sleep);

                match quit_policy {
                    crate::tui::QuitPolicy::Sleep | crate::tui::QuitPolicy::QuitOnIdle(_) => {
                        // Check suspend policy
                        let suspend_policy = self.factories.get(&self.active_app)
                            .map(|f| f.suspend_policy())
                            .unwrap_or(crate::tui::SuspendPolicy::Suspend);

                        match suspend_policy {
                            crate::tui::SuspendPolicy::Suspend => {
                                // Keep app in background and call on_suspend
                                self.lifecycles.insert(self.active_app, AppLifecycle::Background);
                                if let Some(runtime) = self.runtimes.get_mut(&self.active_app) {
                                    runtime.on_suspend().ok();
                                }
                            }
                            crate::tui::SuspendPolicy::AlwaysActive => {
                                // Keep app in background but don't call on_suspend
                                self.lifecycles.insert(self.active_app, AppLifecycle::Background);
                            }
                            crate::tui::SuspendPolicy::QuitOnSuspend => {
                                // Destroy app instead of suspending
                                if let Some(mut runtime) = self.runtimes.remove(&self.active_app) {
                                    runtime.on_destroy().ok();
                                }
                                self.lifecycles.insert(self.active_app, AppLifecycle::Dead);
                            }
                        }
                    }
                    crate::tui::QuitPolicy::QuitOnExit => {
                        // Remove app immediately
                        if let Some(mut runtime) = self.runtimes.remove(&self.active_app) {
                            runtime.on_destroy().ok();
                        }
                        self.lifecycles.insert(self.active_app, AppLifecycle::Dead);
                    }
                }
            }

            // Always destroy existing instance and create fresh with new params
            if let Some(mut runtime) = self.runtimes.remove(&target) {
                runtime.on_destroy().ok();
            }
            self.lifecycles.insert(target, AppLifecycle::Dead);

            // Create target app with provided params
            self.ensure_app_exists(target, params)?;

            self.active_app = target;
            self.last_active_time.insert(target, Instant::now());
            return Ok(true); // Navigation happened
        }

        if let Some(target) = nav_target {
            log::info!("⚡ Processing navigation: {:?} -> {:?} (from {:?})", self.active_app, target, nav_source_app);
            // Handle current app based on quit and suspend policies
            if let Some(current_runtime) = self.runtimes.get(&self.active_app) {
                let quit_policy = self.factories.get(&self.active_app)
                    .map(|f| f.quit_policy())
                    .unwrap_or(crate::tui::QuitPolicy::Sleep);

                match quit_policy {
                    crate::tui::QuitPolicy::Sleep | crate::tui::QuitPolicy::QuitOnIdle(_) => {
                        // Check suspend policy
                        let suspend_policy = self.factories.get(&self.active_app)
                            .map(|f| f.suspend_policy())
                            .unwrap_or(crate::tui::SuspendPolicy::Suspend);

                        match suspend_policy {
                            crate::tui::SuspendPolicy::Suspend => {
                                // Keep app in background and call on_suspend
                                self.lifecycles.insert(self.active_app, AppLifecycle::Background);
                                if let Some(runtime) = self.runtimes.get_mut(&self.active_app) {
                                    runtime.on_suspend().ok();
                                }
                            }
                            crate::tui::SuspendPolicy::AlwaysActive => {
                                // Keep app in background but don't call on_suspend
                                self.lifecycles.insert(self.active_app, AppLifecycle::Background);
                            }
                            crate::tui::SuspendPolicy::QuitOnSuspend => {
                                // Destroy app instead of suspending
                                if let Some(mut runtime) = self.runtimes.remove(&self.active_app) {
                                    runtime.on_destroy().ok();
                                }
                                self.lifecycles.insert(self.active_app, AppLifecycle::Dead);
                            }
                        }
                    }
                    crate::tui::QuitPolicy::QuitOnExit => {
                        // Remove app immediately
                        if let Some(mut runtime) = self.runtimes.remove(&self.active_app) {
                            runtime.on_destroy().ok();
                        }
                        self.lifecycles.insert(self.active_app, AppLifecycle::Dead);
                    }
                }
            }

            // Ensure target app exists (create with default params if needed)
            self.ensure_app_exists(target, Box::new(())).ok();

            // Resume target app if it was backgrounded
            if matches!(self.lifecycles.get(&target), Some(AppLifecycle::Background)) {
                if let Some(runtime) = self.runtimes.get_mut(&target) {
                    runtime.on_resume().ok();
                }
                self.lifecycles.insert(target, AppLifecycle::Running);
            }

            self.active_app = target;
            self.last_active_time.insert(target, Instant::now());
            log::info!("✅ Navigation complete - now active: {:?}", target);
            Ok(true) // Navigation happened
        } else {
            log::debug!("  No navigation requests found");
            Ok(false) // No navigation
        }
    }

    /// Broadcast events globally to all apps
    fn broadcast_events(&mut self) -> Result<()> {
        // Collect all pending publishes from all apps
        let mut all_events = Vec::new();
        for runtime in self.runtimes.values_mut() {
            all_events.extend(runtime.take_publishes());
        }

        // Broadcast each event to all apps
        for (topic, data) in all_events {
            log::debug!("Broadcasting event '{}' to all apps", topic);
            for runtime in self.runtimes.values_mut() {
                runtime.handle_publish(&topic, data.clone())?;
            }
        }

        Ok(())
    }

    /// Process side effects (navigation, events) from timers and async commands
    pub fn process_side_effects(&mut self) -> Result<()> {
        // IMPORTANT: Broadcast events FIRST, then check navigation ONCE
        // This ensures:
        // 1. Parallel task completion events are broadcast to LoadingScreen before we navigate away
        // 2. Initial navigation still works because we loop until things settle
        //
        // We loop until no more navigation happens:
        // - Iteration 1: Broadcast "migration:selected", then navigate to ComparisonSelect
        //   - ComparisonSelect.Initialize creates PerformParallel which sets navigation to LoadingScreen
        // - Iteration 2: Broadcast "loading:init", then navigate to LoadingScreen
        //   - LoadingScreen.Initialize sets up loading state
        // - Iteration 3: Broadcast parallel completion events, then navigate to target
        //   - LoadingScreen received all events before navigation
        const MAX_LOOPS: usize = 5;
        for _ in 0..MAX_LOOPS {
            // Broadcast events first so LoadingScreen gets completion events before we navigate
            self.broadcast_events()?;

            // Then check if we need to navigate
            let navigated = self.check_navigation()?;

            // If no navigation happened, we're done
            if !navigated {
                break;
            }
        }
        Ok(())
    }
}