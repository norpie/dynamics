//! Spinner component for showing progress during long-running operations

use std::io::{self, Write};
use std::sync::{Arc, atomic::AtomicBool};
use std::time::Duration;
use tokio::sync::oneshot;

const SPINNER_CHARS: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const SPINNER_INTERVAL: Duration = Duration::from_millis(80);

/// A spinner that displays an animated progress indicator
///
/// The spinner automatically starts when created and stops when dropped.
/// Uses RAII pattern for automatic cleanup.
///
/// # Example
///
/// ```rust
/// {
///     let _spinner = Spinner::start("Loading data...");
///     // Do some long-running work
///     tokio::time::sleep(Duration::from_secs(2)).await;
/// } // Spinner automatically stops here
/// ```
pub struct Spinner {
    stop_tx: Option<oneshot::Sender<()>>,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl Spinner {
    /// Start a new spinner with the given message
    pub fn start(message: impl Into<String>) -> Self {
        let message = message.into();
        let (stop_tx, stop_rx) = oneshot::channel();

        let handle = tokio::spawn(Self::run_spinner(message, stop_rx));

        Self {
            stop_tx: Some(stop_tx),
            handle: Some(handle),
        }
    }

    /// Manually stop the spinner (usually not needed due to Drop impl)
    pub fn stop(mut self) {
        self.stop_internal();
    }

    /// Internal stop implementation
    fn stop_internal(&mut self) {
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }

        if let Some(handle) = self.handle.take() {
            // Just abort the task - we can't await in Drop
            handle.abort();
        }

        // Clear the spinner line
        Self::clear_line();
    }

    /// The main spinner loop
    async fn run_spinner(message: String, mut stop_rx: oneshot::Receiver<()>) {
        let mut frame = 0;
        let mut stdout = io::stdout();

        loop {
            // Check if we should stop
            if stop_rx.try_recv().is_ok() {
                break;
            }

            // Show current frame
            let spinner_char = SPINNER_CHARS[frame % SPINNER_CHARS.len()];
            print!("\r{} — {}", spinner_char, message);
            let _ = stdout.flush();

            frame += 1;

            // Wait for next frame or stop signal
            tokio::select! {
                _ = tokio::time::sleep(SPINNER_INTERVAL) => {},
                _ = &mut stop_rx => break,
            }
        }

        // Clear the line when stopping
        Self::clear_line();
    }

    /// Clear the current line
    fn clear_line() {
        print!("\r\x1b[K");
        let _ = io::stdout().flush();
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.stop_internal();
    }
}

/// Convenience function to run a future with a spinner
///
/// # Example
///
/// ```rust
/// let result = with_spinner("Loading data...", async {
///     // Do some work
///     expensive_operation().await
/// }).await?;
/// ```
pub async fn with_spinner<F, T>(message: impl Into<String>, future: F) -> T
where
    F: std::future::Future<Output = T>,
{
    let _spinner = Spinner::start(message);
    future.await
}
