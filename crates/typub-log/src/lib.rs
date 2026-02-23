//! Logging foundation for typub based on `tracing`.
//!
//! This crate provides:
//! - Re-exported `tracing` macros (`debug!`, `info!`, `warn!`, `error!`)
//! - A custom `CliLayer` for CLI-formatted output with icons and colors
//! - The `ProgressReporter` trait for decoupling progress reporting from UI
//!
//! Per [[ADR-0004]], this crate is Layer 0 (no internal typub dependencies).
//!
//! # Usage
//!
//! ```rust,ignore
//! use typub_log::{debug, info, warn, error};
//!
//! // Structured logging with tracing
//! info!(file = %path.display(), "Processing file");
//! debug!(count = 42, "Items processed");
//! warn!(platform = "ghost", "Rate limit approaching");
//! error!(error = %e, "Upload failed");
//! ```
//!
//! # Initialization
//!
//! Call `init()` at CLI startup to install the CLI subscriber:
//!
//! ```rust,ignore
//! typub_log::init(verbose);
//! ```

mod cli_layer;
mod progress;

pub use cli_layer::{CliLayer, init, is_verbose};
pub use progress::{FnReporter, NullReporter, ProgressReporter};

// Re-export tracing macros for convenient use
pub use tracing::{debug, error, info, trace, warn};

// Re-export tracing types for advanced use
pub use tracing::{Level, Span, span};
