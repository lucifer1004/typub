//! Test utilities shared across modules.

#![allow(clippy::expect_used)]

/// Macro to run snapshot tests with centralized snapshot directory.
///
/// All snapshots are stored in `tests/snapshots/` at the project root.
/// Module name is NOT prepended to snapshot filenames - use explicit snapshot
/// names that include the adapter/module prefix for disambiguation.
///
/// # Usage
///
/// ```ignore
/// with_snapshot_settings!(|| {
///     insta::assert_snapshot!("adapter__test_name", value);
/// });
/// ```
#[macro_export]
macro_rules! with_snapshot_settings {
    ($body:expr) => {{
        let mut settings = insta::Settings::clone_current();
        // Use CARGO_MANIFEST_DIR to get absolute path to project root
        let snapshot_path =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots");
        settings.set_snapshot_path(snapshot_path);
        // Disable module name prefix - we use explicit snapshot names instead
        settings.set_prepend_module_to_snapshot(false);
        settings.bind($body)
    }};
}
