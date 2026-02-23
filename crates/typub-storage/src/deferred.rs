//! Deferred asset types for pipeline stages.
//!
//! Per [[RFC-0004:C-PIPELINE-INTEGRATION]].

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use typub_core::AssetStrategy;

/// A pending asset reference for deferred upload.
/// Per [[RFC-0004:C-PIPELINE-INTEGRATION]].
#[derive(Debug, Clone)]
pub struct PendingAsset {
    /// Zero-based index for placeholder token
    pub index: usize,
    /// Absolute path to the local asset file
    pub local_path: PathBuf,
    /// Original relative path (as referenced in content)
    pub original_ref: String,
}

impl PendingAsset {
    /// Generate the placeholder token for this asset.
    /// Format: `{{ASSET:<index>}}`
    pub fn placeholder(&self) -> String {
        format!("{{{{ASSET:{}}}}}", self.index)
    }
}

/// Result of building pending assets for deferred upload.
#[derive(Debug, Clone)]
pub struct PendingAssetList {
    /// List of pending assets
    pub assets: Vec<PendingAsset>,
}

impl PendingAssetList {
    /// Create a new empty pending asset list.
    pub fn new() -> Self {
        Self { assets: Vec::new() }
    }

    /// Get the placeholder token for an asset by its original reference.
    pub fn placeholder_for(&self, original_ref: &str) -> Option<String> {
        self.assets
            .iter()
            .find(|a| a.original_ref == original_ref)
            .map(|a| a.placeholder())
    }
}

impl Default for PendingAssetList {
    fn default() -> Self {
        Self::new()
    }
}

/// Deferred asset context carried through pipeline stages.
///
/// This is the "asset layer" that wraps any platform-specific payload.
/// Per [[RFC-0004:C-PIPELINE-INTEGRATION]] v0.2.0.
///
/// # Pipeline Flow
///
/// - **Stage 4 (Finalize)**: Created with `pending` assets and `strategy`
/// - **Stage 6 (Materialize)**: `resolved` is filled with uploaded URLs
/// - **Stage 7 (Publish)**: Content with placeholders replaced by resolved URLs
#[derive(Debug, Clone)]
pub struct DeferredAssets {
    /// Assets pending upload during Materialize stage
    pub pending: PendingAssetList,
    /// Strategy for this batch
    pub strategy: AssetStrategy,
    /// Resolved URLs after Materialize (index → remote URL)
    pub resolved: HashMap<usize, String>,
}

impl DeferredAssets {
    /// Create a new DeferredAssets with pending assets and strategy.
    pub fn new(pending: PendingAssetList, strategy: AssetStrategy) -> Self {
        Self {
            pending,
            strategy,
            resolved: HashMap::new(),
        }
    }

    /// Create an empty DeferredAssets (for adapters that don't use deferred upload).
    pub fn empty() -> Self {
        Self::new(PendingAssetList::new(), AssetStrategy::Copy)
    }

    /// Returns true if this payload needs asset materialization.
    pub fn needs_materialize(&self) -> bool {
        self.strategy.requires_deferred_upload() && !self.pending.assets.is_empty()
    }

    /// Check if all pending assets have been resolved.
    pub fn is_resolved(&self) -> bool {
        self.pending.assets.len() == self.resolved.len()
    }
}

impl Default for DeferredAssets {
    fn default() -> Self {
        Self::empty()
    }
}

/// Build a list of pending assets for deferred upload.
///
/// This function collects all assets from the content and creates a `PendingAssetList`
/// with placeholder tokens. The returned list maps each asset to a unique index,
/// which can be used to generate `{{ASSET:N}}` placeholder tokens.
///
/// # Arguments
///
/// * `assets` - List of asset paths from content
/// * `content_path` - Base path of the content directory (for resolving relative paths)
///
/// # Returns
///
/// A `PendingAssetList` containing all assets with their indices and resolved paths.
pub fn build_pending_asset_list(assets: &[PathBuf], content_path: &Path) -> PendingAssetList {
    let mut pending = Vec::with_capacity(assets.len());

    for (index, asset) in assets.iter().enumerate() {
        // Resolve absolute path
        let local_path = if asset.is_absolute() {
            asset.clone()
        } else {
            content_path.join(asset)
        };

        // Compute original reference (relative path as string)
        let original_ref = if let Ok(rel) = asset.strip_prefix(content_path) {
            rel.to_string_lossy().replace('\\', "/")
        } else if asset.is_relative() {
            asset.to_string_lossy().replace('\\', "/")
        } else {
            local_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| asset.to_string_lossy().to_string())
        };

        pending.push(PendingAsset {
            index,
            local_path,
            original_ref,
        });
    }

    PendingAssetList { assets: pending }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn test_pending_asset_placeholder() {
        let asset = PendingAsset {
            index: 0,
            local_path: PathBuf::from("/tmp/image.png"),
            original_ref: "assets/image.png".to_string(),
        };
        assert_eq!(asset.placeholder(), "{{ASSET:0}}");

        let asset2 = PendingAsset {
            index: 42,
            local_path: PathBuf::from("/tmp/photo.jpg"),
            original_ref: "photo.jpg".to_string(),
        };
        assert_eq!(asset2.placeholder(), "{{ASSET:42}}");
    }

    #[test]
    fn test_pending_asset_list_placeholder_for() {
        let list = PendingAssetList {
            assets: vec![
                PendingAsset {
                    index: 0,
                    local_path: PathBuf::from("/tmp/a.png"),
                    original_ref: "a.png".to_string(),
                },
                PendingAsset {
                    index: 1,
                    local_path: PathBuf::from("/tmp/b.jpg"),
                    original_ref: "b.jpg".to_string(),
                },
            ],
        };
        assert_eq!(
            list.placeholder_for("a.png"),
            Some("{{ASSET:0}}".to_string())
        );
        assert_eq!(
            list.placeholder_for("b.jpg"),
            Some("{{ASSET:1}}".to_string())
        );
        assert_eq!(list.placeholder_for("c.gif"), None);
    }

    #[test]
    fn test_deferred_assets_empty() {
        let da = DeferredAssets::empty();
        assert!(da.pending.assets.is_empty());
        assert_eq!(da.strategy, AssetStrategy::Copy);
        assert!(da.resolved.is_empty());
        assert!(!da.needs_materialize());
        assert!(da.is_resolved()); // empty pending = resolved
    }

    #[test]
    fn test_deferred_assets_needs_materialize() {
        let pending = PendingAssetList {
            assets: vec![PendingAsset {
                index: 0,
                local_path: PathBuf::from("/tmp/a.png"),
                original_ref: "a.png".to_string(),
            }],
        };

        // External strategy with pending assets -> needs materialize
        let da_external = DeferredAssets::new(pending.clone(), AssetStrategy::External);
        assert!(da_external.needs_materialize());

        // Upload strategy with pending assets -> needs materialize
        let da_upload = DeferredAssets::new(pending.clone(), AssetStrategy::Upload);
        assert!(da_upload.needs_materialize());

        // Copy strategy with pending assets -> no materialize
        let da_copy = DeferredAssets::new(pending.clone(), AssetStrategy::Copy);
        assert!(!da_copy.needs_materialize());

        // Embed strategy with pending assets -> no materialize
        let da_embed = DeferredAssets::new(pending, AssetStrategy::Embed);
        assert!(!da_embed.needs_materialize());
    }

    #[test]
    fn test_deferred_assets_is_resolved() {
        let pending = PendingAssetList {
            assets: vec![
                PendingAsset {
                    index: 0,
                    local_path: PathBuf::from("/tmp/a.png"),
                    original_ref: "a.png".to_string(),
                },
                PendingAsset {
                    index: 1,
                    local_path: PathBuf::from("/tmp/b.jpg"),
                    original_ref: "b.jpg".to_string(),
                },
            ],
        };

        let mut da = DeferredAssets::new(pending, AssetStrategy::External);
        assert!(!da.is_resolved());

        da.resolved
            .insert(0, "https://cdn.example.com/a.png".to_string());
        assert!(!da.is_resolved());

        da.resolved
            .insert(1, "https://cdn.example.com/b.jpg".to_string());
        assert!(da.is_resolved());
    }

    #[test]
    fn test_build_pending_asset_list() {
        let content_path = PathBuf::from("/project/content/my-post");
        let assets = vec![
            PathBuf::from("image.png"),
            PathBuf::from("./assets/photo.jpg"),
        ];

        let list = build_pending_asset_list(&assets, &content_path);
        assert_eq!(list.assets.len(), 2);

        assert_eq!(list.assets[0].index, 0);
        assert_eq!(
            list.assets[0].local_path,
            PathBuf::from("/project/content/my-post/image.png")
        );
        assert_eq!(list.assets[0].original_ref, "image.png");

        assert_eq!(list.assets[1].index, 1);
        assert_eq!(
            list.assets[1].local_path,
            PathBuf::from("/project/content/my-post/./assets/photo.jpg")
        );
        assert_eq!(list.assets[1].original_ref, "./assets/photo.jpg");
    }
}
