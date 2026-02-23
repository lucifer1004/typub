//! typub - Multi-platform content publishing from Typst sources
//!
//! This crate provides the CLI tool for multi-platform publishing.
//! Core functionality is provided by subcrates:
//! - `typub-core`: Content parsing and core types
//! - `typub-config`: Configuration handling
//! - `typub-html`: HTML processing and transforms
//! - `typub-storage`: Asset storage and status tracking
//! - `typub-theme`: Theme system
//! - `typub-ui`: CLI output utilities
//! - `typub-adapters-core`: Adapter infrastructure

pub mod dev_server;
#[cfg(test)]
mod test_utils;

// Re-export core types for convenience
pub use typub_config::{
    Config, ConfigLoadResult, PlatformConfig, StorageConfig, resolve_published,
};
