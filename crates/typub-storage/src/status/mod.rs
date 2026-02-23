//! Publish status tracking module.
//!
//! This module provides status tracking for content publishing, including:
//! - Platform status and publish results
//! - Lifecycle management for API-based platforms
//! - SQLite-backed status tracker
//! - Asset upload record tracking

mod lifecycle;
mod tracker;
mod types;

// Re-export all public types
pub use lifecycle::{LifecycleAction, determine_lifecycle_action, validate_remote_status};
pub use tracker::StatusTracker;
pub use types::{AssetUploadRecord, PlatformStatus, PostStatus, PublishResult, StatusDatabase};
