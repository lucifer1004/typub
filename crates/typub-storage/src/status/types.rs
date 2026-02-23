//! Status data types for publish tracking.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Result of a successful publish operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishResult {
    /// URL where the content was published
    pub url: Option<String>,
    /// Platform-specific ID (if any)
    pub platform_id: Option<String>,
    /// When the content was published
    pub published_at: DateTime<Utc>,
}

/// Status entry for a single platform
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformStatus {
    /// Whether content is published
    pub published: bool,
    /// Last publish result (if any)
    pub last_publish: Option<PublishResult>,
    /// Content hash at time of publish (for change detection)
    pub content_hash: Option<String>,
    /// Remote lifecycle state per [[RFC-0005:C-STATUS-TRACKING]]
    /// Values: "draft", "published", or None (unknown/legacy)
    pub remote_status: Option<String>,
}

/// Status entry for a single post
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PostStatus {
    /// Status per platform
    pub platforms: HashMap<String, PlatformStatus>,
}

/// Full status database
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct StatusDatabase {
    /// Status per post (keyed by slug)
    pub posts: HashMap<String, PostStatus>,
}

/// Record of a successfully uploaded asset
/// Per [[RFC-0004:C-UPLOAD-TRACKING]]
#[derive(Debug, Clone)]
pub struct AssetUploadRecord {
    /// Local asset path (relative to project root, using forward slashes)
    /// Per [[RFC-0005:C-PROJECT-ROOT]], paths are stored OS-agnostically.
    pub local_path: String,
    /// Storage configuration identifier (64-char SHA-256 hex)
    pub storage_config_id: String,
    /// Content hash of the file (SHA-256, lowercase hex, 64 chars)
    pub content_hash: String,
    /// Normalized extension (lowercase alphanumeric only)
    pub extension: String,
    /// Remote object key
    pub remote_key: String,
    /// Public URL of the uploaded asset
    pub remote_url: String,
    /// Upload timestamp (ISO 8601)
    pub uploaded_at: String,
}
