use anyhow::Result;

use crate::capability::LinkResolution;
use crate::types::ContentInfo;

/// Trait for looking up publish status.
pub trait StatusLookup: Send + Sync {
    fn get_platform_url(&self, slug: &str, platform: &str) -> Result<Option<String>>;
    fn get_platform_id(&self, slug: &str, platform: &str) -> Result<Option<String>>;
}

/// Trait for resolving internal links.
pub trait LinkResolver: Send + Sync {
    fn resolve_href(&self, href: &str, platform: &str) -> Result<LinkResolution>;
}

/// Context provided by pipeline to adapters.
pub trait AdapterContext: Send {
    fn get_platform_id(&self, slug: &str, platform: &str) -> Result<Option<String>>;
    fn normalize_terms(&self, terms: &[String]) -> Vec<String>;
    fn published(&self) -> bool;
    fn storage_config(&self) -> Option<&typub_config::StorageConfig>;
    fn theme_id(&self) -> Option<&str>;
    fn content_info(&self) -> &ContentInfo;

    /// Get the status tracker for asset upload caching.
    /// Per [[RFC-0004:C-UPLOAD-TRACKING]].
    fn status_tracker(&self) -> Option<&typub_storage::StatusTracker> {
        None
    }

    /// Whether we're in dry-run mode.
    /// In dry-run mode, asset uploads should be mocked (copied to temp dir)
    /// instead of actually uploading to remote storage.
    fn is_dry_run(&self) -> bool {
        false
    }
}

// Re-export StatusTracker for convenience
pub use typub_storage::StatusTracker;
