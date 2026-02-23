//! S3-compatible storage client, status tracking, and asset types for typub.
//!
//! This crate provides:
//! - `S3Storage` — S3-compatible storage client
//! - `UploadResult` — result of an asset upload
//! - `PendingAsset`, `PendingAssetList`, `DeferredAssets` — deferred asset types
//! - `StatusTracker` — SQLite-backed publish status tracking
//! - Asset upload orchestration (`materialize_external_assets`, etc.)
//! - Pure utility functions for hash computation, URL construction, encoding, etc.
//!
//! Extracted per [[RFC-0007:C-SHARED-TYPES]] to enable adapter subcrates
//! to handle asset uploads without depending on the main crate.

// ============================================================================
// Modules
// ============================================================================

mod deferred;
mod encoding;
mod s3;
pub mod status;
mod upload;
mod url_mapping;

// ============================================================================
// Re-exports
// ============================================================================

// From typub-core
pub use typub_core::AssetStrategy;

// Deferred asset types
pub use deferred::{DeferredAssets, PendingAsset, PendingAssetList, build_pending_asset_list};

// Encoding utilities
pub use encoding::{base64_encode, to_data_uri};

// S3 storage
pub use s3::{S3Storage, UploadResult};

// URL mapping utilities
pub use url_mapping::{
    build_image_marker_url_map, build_image_src_url_map, build_preview_file_url_map,
    key_candidates, resolve_image_reference_url,
};

// Upload orchestration
pub use upload::{
    AssetAnalysis, AssetInfo, analyze_assets, build_resolved_url_map, materialize_external_assets,
    materialize_external_assets_with_status, materialize_with_analysis, upload_pending_assets,
};

// Status tracking (re-export commonly used types)
pub use status::{
    AssetUploadRecord, LifecycleAction, PlatformStatus, PostStatus, PublishResult, StatusDatabase,
    StatusTracker, determine_lifecycle_action, validate_remote_status,
};

// ============================================================================
// Utility Functions
// ============================================================================

use std::path::Path;

/// Replace placeholder tokens in content with remote URLs.
///
/// Per [[RFC-0004:C-PIPELINE-INTEGRATION]], placeholder tokens are in the format
/// `{{ASSET:<index>}}`. This function replaces all such tokens with the corresponding
/// remote URLs from the provided map.
///
/// # Arguments
///
/// * `content` - The content string containing placeholder tokens
/// * `url_map` - A map from asset index to remote URL
///
/// # Returns
///
/// The content with all placeholder tokens replaced.
pub fn replace_asset_placeholders(
    content: &str,
    url_map: &std::collections::HashMap<usize, String>,
) -> String {
    let mut result = content.to_string();

    for (index, url) in url_map {
        let placeholder = format!("{{{{ASSET:{}}}}}", index);
        result = result.replace(&placeholder, url);
    }

    result
}

/// Determine MIME type from file extension using mime_guess.
/// Returns "application/octet-stream" for unknown types.
pub fn mime_type_from_path(path: &Path) -> &'static str {
    mime_guess::from_path(path)
        .first_raw()
        .unwrap_or("application/octet-stream")
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn test_replace_asset_placeholders_single() {
        let content = "Here is an image: {{ASSET:0}}";
        let mut url_map = std::collections::HashMap::new();
        url_map.insert(0, "https://cdn.example.com/abc123.png".to_string());

        let result = replace_asset_placeholders(content, &url_map);
        assert_eq!(
            result,
            "Here is an image: https://cdn.example.com/abc123.png"
        );
    }

    #[test]
    fn test_replace_asset_placeholders_multiple() {
        let content = "First: {{ASSET:0}}, Second: {{ASSET:1}}, First again: {{ASSET:0}}";
        let mut url_map = std::collections::HashMap::new();
        url_map.insert(0, "https://cdn.example.com/first.png".to_string());
        url_map.insert(1, "https://cdn.example.com/second.jpg".to_string());

        let result = replace_asset_placeholders(content, &url_map);
        assert_eq!(
            result,
            "First: https://cdn.example.com/first.png, Second: https://cdn.example.com/second.jpg, First again: https://cdn.example.com/first.png"
        );
    }

    #[test]
    fn test_replace_asset_placeholders_missing() {
        let content = "Here is an image: {{ASSET:99}}";
        let url_map = std::collections::HashMap::new();

        let result = replace_asset_placeholders(content, &url_map);
        assert_eq!(result, "Here is an image: {{ASSET:99}}");
    }

    #[test]
    fn test_mime_type_from_path() {
        assert_eq!(mime_type_from_path(Path::new("image.png")), "image/png");
        assert_eq!(mime_type_from_path(Path::new("photo.JPEG")), "image/jpeg");
        assert_eq!(mime_type_from_path(Path::new("doc.pdf")), "application/pdf");
        assert_eq!(
            mime_type_from_path(Path::new("unknown.unknownext123")),
            "application/octet-stream"
        );
    }
}
