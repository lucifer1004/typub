//! Progress reporting trait for decoupling storage from UI.
//!
//! Implements [[ADR-0004]] Phase 2: ProgressReporter trait.

/// Trait for reporting progress during long-running operations.
///
/// This trait decouples the storage layer from the UI layer, allowing
/// operations like S3 uploads to report progress without depending on
/// `typub-ui`.
///
/// # Implementations
///
/// - `NullReporter` - Discards all progress reports (default)
/// - `FnReporter` - Wraps a closure for simple progress reporting
/// - `typub-ui` provides `IndicatifReporter` with progress bars (Phase 4)
///
/// # Example
///
/// ```rust
/// use typub_log::{ProgressReporter, NullReporter};
///
/// fn upload_with_progress(reporter: &dyn ProgressReporter) {
///     reporter.set_message("Uploading assets...");
///     for i in 0..10 {
///         // ... upload logic ...
///         reporter.set_progress(i + 1, 10);
///     }
///     reporter.finish_success("Upload complete");
/// }
///
/// // Use null reporter when no UI is available
/// upload_with_progress(&NullReporter);
/// ```
pub trait ProgressReporter: Send + Sync {
    /// Set the current progress message.
    fn set_message(&self, message: &str);

    /// Set progress as (current, total).
    ///
    /// For indeterminate progress, call with (0, 0).
    fn set_progress(&self, current: u64, total: u64);

    /// Mark the operation as successfully completed.
    fn finish_success(&self, message: &str);

    /// Mark the operation as failed.
    fn finish_error(&self, message: &str);

    /// Increment progress by a given amount.
    fn inc(&self, delta: u64) {
        // Default implementation does nothing
        let _ = delta;
    }
}

// ============ NullReporter ============

/// A progress reporter that discards all reports.
///
/// Use this when no progress UI is needed (e.g., in library mode or tests).
#[derive(Debug, Clone, Copy, Default)]
pub struct NullReporter;

impl ProgressReporter for NullReporter {
    fn set_message(&self, _message: &str) {}
    fn set_progress(&self, _current: u64, _total: u64) {}
    fn finish_success(&self, _message: &str) {}
    fn finish_error(&self, _message: &str) {}
}

// ============ () as NullReporter ============

/// Unit type implements ProgressReporter as a null reporter.
///
/// This allows passing `&()` when no progress reporting is needed.
impl ProgressReporter for () {
    fn set_message(&self, _message: &str) {}
    fn set_progress(&self, _current: u64, _total: u64) {}
    fn finish_success(&self, _message: &str) {}
    fn finish_error(&self, _message: &str) {}
}

// ============ FnReporter ============

/// A progress reporter that wraps a closure.
///
/// Useful for simple progress reporting without implementing the full trait.
///
/// # Example
///
/// ```rust
/// use typub_log::{FnReporter, ProgressReporter};
///
/// let reporter = FnReporter::new(|msg| println!("Progress: {}", msg));
/// reporter.set_message("Working...");
/// ```
pub struct FnReporter<F>
where
    F: Fn(&str) + Send + Sync,
{
    callback: F,
}

impl<F> FnReporter<F>
where
    F: Fn(&str) + Send + Sync,
{
    /// Create a new FnReporter with the given callback.
    pub fn new(callback: F) -> Self {
        Self { callback }
    }
}

impl<F> ProgressReporter for FnReporter<F>
where
    F: Fn(&str) + Send + Sync,
{
    fn set_message(&self, message: &str) {
        (self.callback)(message);
    }

    fn set_progress(&self, current: u64, total: u64) {
        let msg = format!("{}/{}", current, total);
        (self.callback)(&msg);
    }

    fn finish_success(&self, message: &str) {
        (self.callback)(message);
    }

    fn finish_error(&self, message: &str) {
        (self.callback)(message);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_null_reporter_is_silent() {
        let reporter = NullReporter;
        reporter.set_message("test");
        reporter.set_progress(1, 10);
        reporter.finish_success("done");
        reporter.finish_error("error");
        // No panic = success
    }

    #[test]
    fn test_unit_as_reporter() {
        let reporter: &dyn ProgressReporter = &();
        reporter.set_message("test");
        reporter.set_progress(1, 10);
        reporter.finish_success("done");
        // No panic = success
    }

    #[test]
    fn test_fn_reporter_calls_callback() {
        let count = Arc::new(AtomicUsize::new(0));
        let count_clone = count.clone();

        let reporter = FnReporter::new(move |_msg| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        reporter.set_message("one");
        reporter.set_progress(1, 10);
        reporter.finish_success("done");

        assert_eq!(count.load(Ordering::SeqCst), 3);
    }
}
