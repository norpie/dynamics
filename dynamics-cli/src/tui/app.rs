use crate::tui::element::FocusId;
use crate::tui::{Command, Element, LayeredView, QuitPolicy, Subscription, SuspendPolicy, Theme};
use ratatui::text::Line;
use std::any::Any;

/// Trait for state types that can auto-dispatch widget events
///
/// This trait enables automatic routing of widget events to Field types,
/// eliminating the need for manual Msg variants and update handlers.
///
/// Usually implemented via `#[derive(AppState)]` with `#[widget("id")]` attributes.
pub trait AppState {
    /// Try to handle widget event internally by dispatching to Field types
    ///
    /// Returns true if handled, false if event should be passed to update()
    fn dispatch_widget_event(&mut self, _id: &FocusId, _event: &dyn Any) -> bool {
        false // Default: not handled
    }
}

/// The main trait that all TUI apps must implement.
///
/// This follows the Elm architecture:
/// - State: immutable data that represents the app's current state
/// - Msg: events/actions that can happen
/// - update: pure function that handles messages and returns commands
/// - view: pure function that renders the current state
/// - subscriptions: declares what inputs the app wants to receive
///
/// New lifecycle features:
/// - InitParams: typed parameters for app initialization
/// - Lifecycle hooks: on_suspend, on_resume, on_destroy
/// - QuitPolicy: control what happens when navigating away
pub trait App: Sized + Send + 'static {
    /// The app's state type
    type State: Default + Send + AppState;

    /// The app's message type
    type Msg: Clone + Send + 'static;

    /// Initialization parameters (use () if app takes no params)
    type InitParams: Default + Send + 'static;

    /// Update the state based on a message and return a command
    fn update(state: &mut Self::State, msg: Self::Msg) -> Command<Self::Msg>;

    /// Render the current state to layered UI elements
    /// Note: Takes &mut for internal optimizations (e.g., cache rebuilds)
    fn view(state: &mut Self::State) -> LayeredView<Self::Msg>;

    /// Declare what inputs this app wants to receive
    fn subscriptions(state: &Self::State) -> Vec<Subscription<Self::Msg>>;

    /// Return the app's title (static string for help menu, etc.)
    fn title() -> &'static str;

    /// Return optional status text (dynamic, styled based on state)
    fn status(state: &Self::State) -> Option<Line<'static>> {
        None
    }

    /// Initialize the app with typed parameters
    fn init(params: Self::InitParams) -> (Self::State, Command<Self::Msg>) {
        let _ = params; // Suppress unused warning for apps that don't use params
        (Self::State::default(), Command::None)
    }

    /// Policy for what happens when navigating away from this app
    fn quit_policy() -> QuitPolicy {
        QuitPolicy::Sleep
    }

    /// Policy for what happens when app is suspended (backgrounded)
    fn suspend_policy() -> SuspendPolicy {
        SuspendPolicy::Suspend
    }

    /// Check if app can quit (return Err to veto)
    fn can_quit(_state: &Self::State) -> Result<(), String> {
        Ok(())
    }

    /// Called when app goes to background
    fn on_suspend(_state: &mut Self::State) -> Command<Self::Msg> {
        Command::None
    }

    /// Called when app returns to foreground
    fn on_resume(_state: &mut Self::State) -> Command<Self::Msg> {
        Command::None
    }

    /// Called before app is destroyed
    fn on_destroy(_state: &mut Self::State) -> Command<Self::Msg> {
        Command::None
    }

    /// Check if the app is capturing raw input (e.g., keybind capture mode)
    /// When true, global keybinds should be bypassed to allow the app to handle all keys
    fn is_capturing_raw_input(_state: &Self::State) -> bool {
        false // Default: apps don't capture raw input
    }
}
