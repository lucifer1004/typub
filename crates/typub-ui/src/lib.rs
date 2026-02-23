//! Unified UI module for CLI output
//!
//! Following rust-cli standards:
//! - Data → stdout, logs/errors → stderr
//! - Respects NO_COLOR environment variable
//! - TTY detection before coloring
//! - Uses owo-colors for styling
//! - Uses indicatif for progress bars
//!
//! Per [[ADR-0004]], this crate re-exports `typub-log` for convenience and
//! provides `IndicatifReporter` implementing `ProgressReporter`.

use comfy_table::Table as ComfyTable;
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use std::io::{self, IsTerminal};
use std::time::Duration;

// Re-export typub-log for convenience
pub use typub_log::{FnReporter, NullReporter, ProgressReporter};
pub use typub_log::{debug, error, info, init, is_verbose, trace, warn};

pub mod i18n;

/// Check if colors should be used
fn use_colors() -> bool {
    io::stdout().is_terminal() && std::env::var("NO_COLOR").is_err()
}

// ============ Status Icons ============

mod icons {
    pub const SUCCESS: &str = "✓";
    pub const ERROR: &str = "✗";
    pub const WARNING: &str = "⚠";
    pub const INFO: &str = "ℹ";
    pub const ARROW: &str = "→";
    pub const PENDING: &str = "○";
    pub const DONE: &str = "●";
    pub const SKIP: &str = "⊘";
}

// ============ Styled Output Helpers ============

/// Apply color conditionally based on terminal and NO_COLOR
macro_rules! styled {
    ($text:expr, $color:ident) => {
        if use_colors() {
            format!("{}", $text.$color())
        } else {
            $text.to_string()
        }
    };
    ($text:expr, $color:ident, bold) => {
        if use_colors() {
            format!("{}", $text.$color().bold())
        } else {
            $text.to_string()
        }
    };
}

// ============ Public API - Messages ============

/// Print a success message (to stderr for logging)
pub fn success(message: &str) {
    eprintln!("{} {}", styled!(icons::SUCCESS, green), message);
}

/// Print an error message (to stderr)
pub fn error(message: &str) {
    eprintln!("{} {}", styled!(icons::ERROR, red), styled!(message, red));
}

/// Print a warning message (to stderr)
pub fn warn(message: &str) {
    eprintln!(
        "{} {}",
        styled!(icons::WARNING, yellow),
        styled!(message, yellow)
    );
}

/// Print an info message (to stderr for logging)
pub fn info(message: &str) {
    eprintln!("{} {}", styled!(icons::INFO, blue), message);
}

/// Print a debug message (only in verbose mode, to stderr)
pub fn debug(message: &str) {
    if is_verbose() {
        eprintln!("{} {}", styled!("[debug]", bright_black), message);
    }
}

// ============ Public API - Structured Output ============

/// Print a header/section title (to stderr)
pub fn header(title: &str) {
    eprintln!();
    eprintln!("{}", styled!(title, cyan, bold));
    let separator = "─".repeat(title.len().max(40));
    eprintln!("{}", styled!(&separator, bright_black));
}

/// Print a step in a multi-step process (to stderr)
pub fn step(number: usize, total: usize, message: &str) {
    let step_info = format!("[{}/{}]", number, total);
    eprintln!("{} {}", styled!(&step_info, cyan), message);
}

/// Print a sub-item with label and value (to stderr)
pub fn item(label: &str, value: &str) {
    eprintln!(
        "  {} {}: {}",
        styled!(icons::ARROW, bright_black),
        styled!(label, cyan),
        value
    );
}

/// Print platform status line (to stderr)
pub fn platform_status(platform: &str, published: bool, url: Option<&str>) {
    let (icon, platform_styled) = if published {
        (styled!(icons::DONE, green), styled!(platform, green))
    } else {
        (
            styled!(icons::PENDING, bright_black),
            styled!(platform, bright_black),
        )
    };

    if let Some(url) = url {
        eprintln!(
            "  {} {} {}",
            icon,
            platform_styled,
            styled!(url, bright_black)
        );
    } else {
        eprintln!("  {} {}", icon, platform_styled);
    }
}

// ============ Public API - Publish Logging ============

/// Log start of publish operation
pub fn log_publish_start(title: &str, platforms: &[&str]) {
    header(&format!("Publishing: {}", title));
    info(&format!("Targets: {}", platforms.join(", ")));
}

/// Log successful publish
pub fn log_publish_success(platform: &str, url: Option<&str>) {
    if let Some(url) = url {
        eprintln!(
            "  {} {} {} {}",
            styled!(icons::SUCCESS, green),
            styled!(platform, green, bold),
            styled!(icons::ARROW, bright_black),
            styled!(url, cyan)
        );
    } else {
        eprintln!(
            "  {} {}",
            styled!(icons::SUCCESS, green),
            styled!(platform, green, bold)
        );
    }
}

/// Log skipped operation
pub fn log_skip(platform: &str, reason: &str) {
    eprintln!(
        "  {} {} ({})",
        styled!(icons::SKIP, bright_black),
        styled!(platform, bright_black),
        styled!(reason, bright_black)
    );
}

/// Log dry run
pub fn log_dry_run(platform: &str) {
    eprintln!(
        "  {} Would publish to: {}",
        styled!("[DRY RUN]", yellow),
        platform
    );
}

// ============ Progress Bars (using indicatif) ============

/// Create a spinner for operations with unknown duration
pub fn spinner(message: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    let mut style = ProgressStyle::default_spinner();
    if let Ok(s) = style.clone().template("{spinner:.cyan} {msg}") {
        style = s.tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏");
    }
    pb.set_style(style);
    pb.set_message(message.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}

/// Finish spinner with success
pub fn spinner_success(pb: ProgressBar, message: &str) {
    if let Ok(style) = ProgressStyle::default_spinner().template("{msg}") {
        pb.set_style(style);
    }
    pb.finish_with_message(format!("{} {}", styled!(icons::SUCCESS, green), message));
}

/// Finish spinner with error
pub fn spinner_error(pb: ProgressBar, message: &str) {
    if let Ok(style) = ProgressStyle::default_spinner().template("{msg}") {
        pb.set_style(style);
    }
    pb.finish_with_message(format!(
        "{} {}",
        styled!(icons::ERROR, red),
        styled!(message, red)
    ));
}

/// Create a progress bar for known-length operations
pub fn progress_bar(len: u64, message: &str) -> ProgressBar {
    let pb = ProgressBar::new(len);
    let mut style = ProgressStyle::default_bar();
    if let Ok(s) = style
        .clone()
        .template("{msg} [{bar:30.cyan/bright_black}] {pos}/{len}")
    {
        style = s.progress_chars("━━─");
    }
    pb.set_style(style);
    pb.set_message(message.to_string());
    pb
}

// ============ Multi-Step Progress ============

/// Progress tracker for multi-step operations
pub struct MultiProgress {
    name: String,
    current: usize,
    total: usize,
    spinner: Option<ProgressBar>,
}

impl MultiProgress {
    pub fn new(name: &str, total: usize) -> Self {
        header(name);
        Self {
            name: name.to_string(),
            current: 0,
            total,
            spinner: None,
        }
    }

    /// Start a new step
    pub fn step(&mut self, message: &str) {
        // Finish previous spinner if any
        if let Some(pb) = self.spinner.take() {
            spinner_success(pb, "Done");
        }

        self.current += 1;
        let step_msg = format!("[{}/{}] {}", self.current, self.total, message);
        self.spinner = Some(spinner(&step_msg));
    }

    /// Finish all steps successfully
    pub fn finish(self) {
        if let Some(pb) = self.spinner {
            spinner_success(pb, "Done");
        }
        success(&format!("{} completed", self.name));
    }

    /// Finish with error
    pub fn finish_error(self, err: &str) {
        if let Some(pb) = self.spinner {
            spinner_error(pb, err);
        }
        error(&format!("{} failed: {}", self.name, err));
    }
}

// ============ Table Output (to stdout for data) ============

/// Print a simple table (data goes to stdout)
pub struct Table {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    widths: Vec<usize>,
}

impl Table {
    pub fn new(headers: &[&str]) -> Self {
        let headers: Vec<String> = headers.iter().map(|s| s.to_string()).collect();
        let widths = headers.iter().map(|h| h.len()).collect();
        Self {
            headers,
            rows: Vec::new(),
            widths,
        }
    }

    pub fn add_row(&mut self, row: &[&str]) {
        let row: Vec<String> = row.iter().map(|s| s.to_string()).collect();
        for (i, cell) in row.iter().enumerate() {
            if i < self.widths.len() {
                self.widths[i] = self.widths[i].max(cell.len());
            }
        }
        self.rows.push(row);
    }

    /// Print table to stdout (data output)
    pub fn print(&self) {
        // Header
        let header_cells: Vec<String> = self
            .headers
            .iter()
            .enumerate()
            .map(|(i, h)| format!("{:width$}", h, width = self.widths[i]))
            .collect();
        let header_line = header_cells.join("  ");
        println!("{}", styled!(&header_line, bold));

        // Separator
        let sep: Vec<String> = self.widths.iter().map(|w| "─".repeat(*w)).collect();
        println!("{}", styled!(&sep.join("──"), bright_black));

        // Rows
        for row in &self.rows {
            let cells: Vec<String> = row
                .iter()
                .enumerate()
                .map(|(i, cell)| {
                    let width = self.widths.get(i).copied().unwrap_or(cell.len());
                    format!("{:width$}", cell, width = width)
                })
                .collect();
            println!("{}", cells.join("  "));
        }
    }
}

// ============ IndicatifReporter ============

/// A progress reporter using indicatif progress bars.
///
/// Implements [[ADR-0004]] Phase 4: ProgressReporter for typub-ui.
pub struct IndicatifReporter {
    bar: ProgressBar,
}

impl IndicatifReporter {
    /// Create a new reporter with a spinner (indeterminate progress).
    pub fn spinner(message: &str) -> Self {
        let bar = spinner(message);
        Self { bar }
    }

    /// Create a new reporter with a progress bar (known length).
    pub fn progress(len: u64, message: &str) -> Self {
        let bar = progress_bar(len, message);
        Self { bar }
    }
}

impl ProgressReporter for IndicatifReporter {
    fn set_message(&self, message: &str) {
        self.bar.set_message(message.to_string());
    }

    fn set_progress(&self, current: u64, total: u64) {
        if total > 0 {
            self.bar.set_length(total);
            self.bar.set_position(current);
        }
    }

    fn finish_success(&self, message: &str) {
        spinner_success_ref(&self.bar, message);
    }

    fn finish_error(&self, message: &str) {
        spinner_error_ref(&self.bar, message);
    }

    fn inc(&self, delta: u64) {
        self.bar.inc(delta);
    }
}

/// Finish spinner with success (by reference).
fn spinner_success_ref(pb: &ProgressBar, message: &str) {
    if let Ok(style) = ProgressStyle::default_spinner().template("{msg}") {
        pb.set_style(style);
    }
    pb.finish_with_message(format!("{} {}", styled!(icons::SUCCESS, green), message));
}

/// Finish spinner with error (by reference).
fn spinner_error_ref(pb: &ProgressBar, message: &str) {
    if let Ok(style) = ProgressStyle::default_spinner().template("{msg}") {
        pb.set_style(style);
    }
    pb.finish_with_message(format!(
        "{} {}",
        styled!(icons::ERROR, red),
        styled!(message, red)
    ));
}

// ============ Asset Analysis Display ============

/// Format bytes into human-readable string.
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Display asset analysis in a formatted table.
///
/// Shows summary of assets: total count, new (will upload), cached (reused).
pub fn log_asset_analysis(
    title: &str,
    total_count: usize,
    new_count: usize,
    new_size_bytes: u64,
    cached_count: usize,
    cached_size_bytes: u64,
) {
    let mut table = ComfyTable::new();
    table.load_preset(comfy_table::presets::NOTHING);

    // Header
    table.set_header(vec!["", "Count", "Size"]);

    // Total row
    table.add_row(vec![
        "📦 Total",
        &total_count.to_string(),
        &format_size(new_size_bytes + cached_size_bytes),
    ]);

    // New row (will upload)
    table.add_row(vec![
        "✅ New (will upload)",
        &new_count.to_string(),
        &format_size(new_size_bytes),
    ]);

    // Cached row (will skip)
    table.add_row(vec![
        "🔄 Cached (will skip)",
        &cached_count.to_string(),
        &format_size(cached_size_bytes),
    ]);

    eprintln!();
    eprintln!("{}", styled!(title, cyan, bold));
    eprintln!("{}", table);
}
