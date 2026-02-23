//! Custom tracing layer for CLI-formatted output.
//!
//! Implements [[ADR-0004]] Phase 1: CliLayer for custom CLI formatting.

use owo_colors::OwoColorize;
use std::io::{self, IsTerminal};
use std::sync::OnceLock;
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

// ============ Global State ============

static VERBOSE: OnceLock<bool> = OnceLock::new();

/// Check if verbose mode is enabled
pub fn is_verbose() -> bool {
    VERBOSE.get().copied().unwrap_or(false)
}

/// Check if colors should be used
fn use_colors() -> bool {
    io::stderr().is_terminal() && std::env::var("NO_COLOR").is_err()
}

// ============ Icons ============

mod icons {
    pub const DEBUG: &str = "[debug]";
    pub const INFO: &str = "ℹ";
    pub const WARN: &str = "⚠";
    pub const ERROR: &str = "✗";
    pub const TRACE: &str = "[trace]";
}

// ============ CliLayer ============

/// A tracing layer that formats events for CLI output.
///
/// Features:
/// - Icons for each log level
/// - Colors respecting NO_COLOR and TTY detection
/// - Debug/trace messages only shown in verbose mode
pub struct CliLayer {
    verbose: bool,
}

impl CliLayer {
    /// Create a new CLI layer.
    ///
    /// If `verbose` is false, debug and trace events are suppressed.
    pub fn new(verbose: bool) -> Self {
        Self { verbose }
    }
}

impl<S> Layer<S> for CliLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        let level = meta.level();

        // Filter debug/trace in non-verbose mode
        if !self.verbose && (*level == Level::DEBUG || *level == Level::TRACE) {
            return;
        }

        // Format the message
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        let message = visitor.message.unwrap_or_default();

        // Get icon and color based on level
        let (icon, colored_icon, colored_message) = match *level {
            Level::ERROR => (
                icons::ERROR,
                format_colored(icons::ERROR, "red"),
                format_colored(&message, "red"),
            ),
            Level::WARN => (
                icons::WARN,
                format_colored(icons::WARN, "yellow"),
                format_colored(&message, "yellow"),
            ),
            Level::INFO => (
                icons::INFO,
                format_colored(icons::INFO, "blue"),
                message.clone(),
            ),
            Level::DEBUG => (
                icons::DEBUG,
                format_colored(icons::DEBUG, "bright_black"),
                message.clone(),
            ),
            Level::TRACE => (
                icons::TRACE,
                format_colored(icons::TRACE, "bright_black"),
                format_colored(&message, "bright_black"),
            ),
        };

        // Output to stderr (logs go to stderr per rust-cli conventions)
        if use_colors() {
            eprintln!("{} {}", colored_icon, colored_message);
        } else {
            eprintln!("{} {}", icon, message);
        }
    }
}

/// Format text with color if colors are enabled.
fn format_colored(text: &str, color: &str) -> String {
    if !use_colors() {
        return text.to_string();
    }

    match color {
        "red" => text.red().to_string(),
        "yellow" => text.yellow().to_string(),
        "blue" => text.blue().to_string(),
        "green" => text.green().to_string(),
        "cyan" => text.cyan().to_string(),
        "bright_black" => text.bright_black().to_string(),
        _ => text.to_string(),
    }
}

// ============ Message Visitor ============

/// Visitor to extract the message field from tracing events.
#[derive(Default)]
struct MessageVisitor {
    message: Option<String>,
}

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{:?}", value));
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        }
    }
}

// ============ Initialization ============

/// Initialize the CLI logging subscriber.
///
/// This should be called once at CLI startup.
///
/// # Arguments
///
/// * `verbose` - If true, debug and trace messages are shown.
///
/// # Example
///
/// ```rust,ignore
/// fn main() {
///     typub_log::init(args.verbose);
///     // ... rest of CLI
/// }
/// ```
pub fn init(verbose: bool) {
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    // Store verbose setting for is_verbose()
    let _ = VERBOSE.set(verbose);

    // Build filter: respect RUST_LOG, default to info (or debug if verbose)
    let default_level = if verbose { "debug" } else { "info" };
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));

    // Build and install subscriber
    tracing_subscriber::registry()
        .with(filter)
        .with(CliLayer::new(verbose))
        .init();
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_is_verbose_default_false() {
        // Note: This test might be affected by other tests setting VERBOSE
        // In a fresh state, is_verbose should return false
        // We can't easily test this without resetting global state
    }

    #[test]
    fn test_format_colored_no_color() {
        // When NO_COLOR is set or not a TTY, colors should be stripped
        // This is hard to test without mocking environment
    }
}
