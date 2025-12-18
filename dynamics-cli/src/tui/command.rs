use std::future::Future;
use std::pin::Pin;
use std::any::Any;
use serde_json::Value;
use tokio::sync::mpsc;
use crate::tui::element::FocusId;
use serde::{Serialize, Deserialize};

/// Sender for reporting progress from within a parallel task
pub type ProgressSender = mpsc::UnboundedSender<String>;

/// Target for event dispatch - either widget auto-routing or app message
///
/// This enum allows the runtime to distinguish between:
/// - Widget events that should be auto-dispatched to Field types via AppState
/// - App messages that should go directly to the update function
/// - Unhandled keys that should pass through to global subscriptions
pub enum DispatchTarget<Msg> {
    /// Widget event - runtime tries auto-dispatch via AppState::dispatch_widget_event
    WidgetEvent(Box<dyn Any + Send>),

    /// App message - goes directly to update() as before
    AppMsg(Msg),

    /// Pass through to global subscriptions without blurring focus
    PassThrough,
}

/// Commands represent side effects that apps want to perform.
/// They are returned from the update() function and executed by the runtime.
pub enum Command<Msg> {
    /// Do nothing
    None,

    /// Execute multiple commands in sequence
    Batch(Vec<Command<Msg>>),

    /// Navigate to a different app (wake if sleeping, create if not exists with default params)
    NavigateTo(AppId),

    /// Start app with typed parameters (always create fresh instance)
    StartApp { app_id: AppId, params: Box<dyn Any + Send> },

    /// Wake background app (error if not already created)
    WakeApp(AppId),

    /// Destroy and recreate app with new params
    RestartApp { app_id: AppId, params: Box<dyn Any + Send> },

    /// Request to destroy this app (graceful quit)
    QuitSelf,

    /// Request to go to background (if not already)
    SleepSelf,

    /// Perform an async operation and send the result as a message
    Perform(Pin<Box<dyn Future<Output = Msg> + Send>>),

    /// Perform multiple async operations in parallel with automatic LoadingScreen management
    PerformParallel {
        tasks: Vec<ParallelTask>,
        config: ParallelConfig,
        msg_mapper: Box<dyn Fn(usize, Box<dyn Any + Send>) -> Msg + Send>,
    },

    /// Publish an event to the event bus
    Publish { topic: String, data: Value },

    /// Set focus to a specific element
    SetFocus(FocusId),

    /// Clear focus from all elements
    ClearFocus,

    /// Quit the application
    Quit,
}

/// A single task in a parallel operation
pub struct ParallelTask {
    pub description: String,
    pub future: Pin<Box<dyn Future<Output = Box<dyn Any + Send>> + Send>>,
    /// Optional receiver for progress updates from the task
    pub progress_rx: Option<mpsc::UnboundedReceiver<String>>,
}

/// Configuration for parallel task execution
#[derive(Clone)]
pub struct ParallelConfig {
    pub title: Option<String>,
    pub on_complete: Option<AppId>,
    pub caller: Option<AppId>,
    pub cancellable: bool,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        Self {
            title: None,
            on_complete: None,
            caller: None,
            cancellable: false,
        }
    }
}

/// Unique identifier for each app
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AppId {
    AppLauncher,
    LoadingScreen,
    ErrorScreen,
    Settings,
    UpdateApp,
    EnvironmentSelector,
    MigrationEnvironment,
    MigrationComparisonSelect,
    EntityComparison,
    DeadlinesFileSelect,
    DeadlinesMapping,
    DeadlinesInspection,
    OperationQueue,
    SelectQuestionnaire,
    CopyQuestionnaire,
    PushQuestionnaire,
    EntitySync,
    TransferConfigList,
    TransferMappingEditor,
    TransferPreview,
    TransferLuaScript,
}

impl<Msg> Command<Msg> {
    /// Helper to create a command that performs an async operation
    pub fn perform<F, T>(future: F, to_msg: impl Fn(T) -> Msg + Send + 'static) -> Self
    where
        F: Future<Output = T> + Send + 'static,
        Msg: Send + 'static,
    {
        Command::Perform(Box::pin(async move {
            let result = future.await;
            to_msg(result)
        }))
    }

    /// Helper to navigate to another app
    pub fn navigate_to(app_id: AppId) -> Self {
        Command::NavigateTo(app_id)
    }

    /// Helper to start an app with typed parameters
    pub fn start_app<P: Send + 'static>(app_id: AppId, params: P) -> Self {
        Command::StartApp {
            app_id,
            params: Box::new(params),
        }
    }

    /// Helper to wake a background app
    pub fn wake_app(app_id: AppId) -> Self {
        Command::WakeApp(app_id)
    }

    /// Helper to restart an app with new parameters
    pub fn restart_app<P: Send + 'static>(app_id: AppId, params: P) -> Self {
        Command::RestartApp {
            app_id,
            params: Box::new(params),
        }
    }

    /// Helper to quit this app
    pub fn quit_self() -> Self {
        Command::QuitSelf
    }

    /// Helper to put this app to sleep
    pub fn sleep_self() -> Self {
        Command::SleepSelf
    }

    /// Helper to publish an event
    pub fn publish<T: serde::Serialize>(topic: impl Into<String>, data: T) -> Self {
        Command::Publish {
            topic: topic.into(),
            data: serde_json::to_value(data).unwrap_or(Value::Null),
        }
    }

    /// Helper to batch multiple commands
    pub fn batch(commands: Vec<Command<Msg>>) -> Self {
        Command::Batch(commands)
    }

    /// Helper to set focus to an element
    pub fn set_focus(id: FocusId) -> Self {
        Command::SetFocus(id)
    }

    /// Helper to clear focus from all elements
    pub fn clear_focus() -> Self {
        Command::ClearFocus
    }

    /// Start building a parallel task execution command
    pub fn perform_parallel() -> ParallelBuilder<Msg>
    where
        Msg: Send + 'static,
    {
        ParallelBuilder::new()
    }
}

/// Builder for parallel task execution
pub struct ParallelBuilder<Msg> {
    tasks: Vec<ParallelTask>,
    config: ParallelConfig,
    _phantom: std::marker::PhantomData<Msg>,
}

impl<Msg: Send + 'static> ParallelBuilder<Msg> {
    fn new() -> Self {
        Self {
            tasks: Vec::new(),
            config: ParallelConfig::default(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Add a task to execute in parallel
    pub fn add_task<F, T>(mut self, description: impl Into<String>, future: F) -> Self
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let task = ParallelTask {
            description: description.into(),
            future: Box::pin(async move {
                let result = future.await;
                Box::new(result) as Box<dyn Any + Send>
            }),
            progress_rx: None,
        };
        self.tasks.push(task);
        self
    }

    /// Add a task with progress reporting capability
    ///
    /// The task factory receives a `ProgressSender` that can be used to send
    /// progress updates (e.g., "50 records fetched", "Processing...").
    /// These updates are displayed on the loading screen.
    pub fn add_task_with_progress<F, Fut, T>(
        mut self,
        description: impl Into<String>,
        task_factory: F,
    ) -> Self
    where
        F: FnOnce(ProgressSender) -> Fut,
        Fut: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let (tx, rx) = mpsc::unbounded_channel();
        let future = task_factory(tx);

        let task = ParallelTask {
            description: description.into(),
            future: Box::pin(async move {
                let result = future.await;
                Box::new(result) as Box<dyn Any + Send>
            }),
            progress_rx: Some(rx),
        };
        self.tasks.push(task);
        self
    }

    /// Set the title shown on the loading screen
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.config.title = Some(title.into());
        self
    }

    /// Set the app to navigate to when all tasks complete
    pub fn on_complete(mut self, app_id: AppId) -> Self {
        self.config.on_complete = Some(app_id);
        self
    }

    /// Set the app to navigate to when loading is cancelled
    pub fn on_cancel(mut self, app_id: AppId) -> Self {
        self.config.caller = Some(app_id);
        self
    }

    /// Enable or disable cancellation (ESC key on loading screen)
    pub fn cancellable(mut self, cancellable: bool) -> Self {
        self.config.cancellable = cancellable;
        self
    }

    /// Build the command with a message mapper that converts task results to messages
    /// The mapper receives (task_index, result) and should downcast the result to the expected type
    pub fn build(self, msg_mapper: impl Fn(usize, Box<dyn Any + Send>) -> Msg + Send + 'static) -> Command<Msg> {
        Command::PerformParallel {
            tasks: self.tasks,
            config: self.config,
            msg_mapper: Box::new(msg_mapper),
        }
    }
}

impl<Msg> Default for Command<Msg> {
    fn default() -> Self {
        Command::None
    }
}