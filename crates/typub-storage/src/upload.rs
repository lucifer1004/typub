//! Asset upload orchestration.
//!
//! Per [[RFC-0004:C-PIPELINE-INTEGRATION]].

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use typub_config::StorageConfig;
use typub_core::AssetStrategy;
use typub_log::{debug, info};

use crate::deferred::{DeferredAssets, PendingAssetList};
use crate::s3::S3Storage;
use crate::status::{AssetUploadRecord, StatusTracker};
use crate::url_mapping::key_candidates;

/// Upload all pending assets with caching support.
///
/// Per [[RFC-0004:C-UPLOAD-TRACKING]].
///
/// # Arguments
///
/// * `storage` - S3-compatible storage client
/// * `pending` - List of pending assets to upload
/// * `status` - Status tracker for caching upload records
///
/// # Returns
///
/// A HashMap mapping asset index to remote URL.
pub async fn upload_pending_assets(
    storage: &S3Storage,
    pending: &PendingAssetList,
    status: &StatusTracker,
) -> Result<HashMap<usize, String>> {
    let mut url_map = HashMap::new();

    for asset in &pending.assets {
        let url = upload_asset_with_cache(storage, &asset.local_path, status).await?;
        url_map.insert(asset.index, url);
    }

    Ok(url_map)
}

/// Upload a single asset with caching support.
///
/// Checks the status tracker for an existing upload record (by content hash).
/// If found and the storage config matches, returns the cached URL.
/// Otherwise, uploads the asset and records it.
async fn upload_asset_with_cache(
    storage: &S3Storage,
    local_path: &Path,
    status: &StatusTracker,
) -> Result<String> {
    // Read file data
    let data = std::fs::read(local_path)
        .with_context(|| format!("Failed to read asset: {}", local_path.display()))?;

    // Compute content hash and extension
    let content_hash = S3Storage::compute_hash(&data);
    let extension = S3Storage::normalize_extension(local_path);

    // Check content index for cache hit
    if let Some(record) =
        status.get_asset_by_content(storage.config_id(), &content_hash, &extension)?
    {
        debug!(
            asset = %local_path.display(),
            hash = &content_hash[..8],
            "Cache hit for asset"
        );
        return Ok(record.remote_url);
    }

    // Upload to S3
    debug!(asset = %local_path.display(), "Uploading asset to S3");
    let result = storage.upload(local_path, &data).await?;

    // Record upload with normalized relative path per [[RFC-0005:C-PROJECT-ROOT]]
    let normalized_path = status.normalize_path(local_path)?;
    let record = AssetUploadRecord {
        local_path: normalized_path,
        content_hash: result.content_hash.clone(),
        extension: result.extension.clone(),
        storage_config_id: storage.config_id().to_string(),
        remote_key: result.remote_key.clone(),
        remote_url: result.remote_url.clone(),
        uploaded_at: chrono::Utc::now().to_rfc3339(),
    };
    status.record_asset_upload(&record)?;

    Ok(result.remote_url)
}

/// Materialize assets for External strategy (without caching).
///
/// This version uploads assets without StatusTracker caching support,
/// suitable for adapter subcrates that don't have access to StatusTracker.
///
/// Per [[RFC-0004:C-PIPELINE-INTEGRATION]] v0.2.0 and [[RFC-0007:C-ADAPTER-CRATE]].
///
/// # Arguments
///
/// * `assets` - Mutable reference to DeferredAssets (will be populated with resolved URLs)
/// * `storage_config` - S3-compatible storage configuration
///
/// # Returns
///
/// `Ok(())` on success, with `assets.resolved` populated.
pub async fn materialize_external_assets(
    assets: &mut DeferredAssets,
    storage_config: &StorageConfig,
) -> Result<()> {
    if !assets.needs_materialize() {
        return Ok(());
    }

    if assets.strategy != AssetStrategy::External {
        return Ok(()); // Only handle External strategy
    }

    let storage = S3Storage::new(storage_config)?;

    info!(
        count = assets.pending.assets.len(),
        "[1/2] Uploading assets to external storage"
    );

    let url_map = upload_pending_assets_uncached(&storage, &assets.pending).await?;
    assets.resolved = url_map;

    info!("[2/2] Assets uploaded and URLs resolved");

    Ok(())
}

/// Materialize assets for External strategy (with caching).
///
/// This version uses StatusTracker for caching upload records, avoiding
/// re-uploads of assets that were previously uploaded with the same content hash.
///
/// Per [[RFC-0004:C-PIPELINE-INTEGRATION]] v0.2.0.
///
/// # Arguments
///
/// * `assets` - Mutable reference to DeferredAssets (will be populated with resolved URLs)
/// * `storage_config` - S3-compatible storage configuration
/// * `status` - Status tracker for caching upload records
///
/// # Returns
///
/// `Ok(())` on success, with `assets.resolved` populated.
pub async fn materialize_external_assets_with_status(
    assets: &mut DeferredAssets,
    storage_config: &StorageConfig,
    status: &StatusTracker,
) -> Result<()> {
    if !assets.needs_materialize() {
        return Ok(());
    }

    if assets.strategy != AssetStrategy::External {
        return Ok(()); // Only handle External strategy
    }

    let storage = S3Storage::new(storage_config)?;

    info!(
        count = assets.pending.assets.len(),
        "[1/2] Uploading assets to external storage"
    );

    let url_map = upload_pending_assets(&storage, &assets.pending, status).await?;
    assets.resolved = url_map;

    info!("[2/2] Assets uploaded and URLs resolved");

    Ok(())
}

/// Upload all pending assets without caching.
///
/// This is a simpler version of `upload_pending_assets` that doesn't use
/// StatusTracker for cache lookups or recording. Suitable for adapter subcrates.
async fn upload_pending_assets_uncached(
    storage: &S3Storage,
    pending: &PendingAssetList,
) -> Result<HashMap<usize, String>> {
    let mut url_map = HashMap::new();

    for asset in &pending.assets {
        let url = upload_asset_uncached(storage, &asset.local_path).await?;
        url_map.insert(asset.index, url);
    }

    Ok(url_map)
}

/// Upload a single asset without caching.
async fn upload_asset_uncached(storage: &S3Storage, local_path: &Path) -> Result<String> {
    // Read file data
    let data = std::fs::read(local_path)
        .with_context(|| format!("Failed to read asset: {}", local_path.display()))?;

    // Upload to S3
    debug!(asset = %local_path.display(), "Uploading asset to S3");
    let result = storage.upload(local_path, &data).await?;

    Ok(result.remote_url)
}

/// Build a map from asset reference paths to resolved remote URLs.
///
/// This converts the index-based `resolved` map in `DeferredAssets` into a
/// path-based map suitable for `resolve_asset_urls()`.
///
/// Per [[RFC-0004:C-PIPELINE-INTEGRATION]], this bridges the gap between
/// the upload system (which uses indices) and the IR modification system
/// (which uses path strings to match asset source paths).
pub fn build_resolved_url_map(
    assets: &DeferredAssets,
    content_path: &Path,
) -> HashMap<String, String> {
    let mut url_map = HashMap::new();
    for asset in &assets.pending.assets {
        if let Some(url) = assets.resolved.get(&asset.index) {
            // Add the original_ref as-is
            url_map.insert(asset.original_ref.clone(), url.clone());

            // Also add candidate key variants so resolve_image_reference_url can match
            for candidate in key_candidates(&asset.original_ref) {
                url_map.entry(candidate).or_insert_with(|| url.clone());
            }

            // Add local_path-based variants (relative to content_path)
            if let Ok(rel) = asset.local_path.strip_prefix(content_path) {
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                url_map.entry(rel_str).or_insert_with(|| url.clone());
            }

            // Add the full local_path as a key (for temp files with absolute paths)
            let local_path_str = asset.local_path.to_string_lossy().replace('\\', "/");
            url_map.entry(local_path_str).or_insert_with(|| url.clone());
        }
    }
    url_map
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use crate::deferred::{PendingAsset, PendingAssetList};
    use std::path::PathBuf;

    #[test]
    fn test_build_resolved_url_map_empty() {
        let assets = DeferredAssets::empty();
        let content_path = PathBuf::from("/project/content/my-post");
        let map = build_resolved_url_map(&assets, &content_path);
        assert!(map.is_empty());
    }

    #[test]
    fn test_build_resolved_url_map_with_resolved() {
        let pending = PendingAssetList {
            assets: vec![PendingAsset {
                index: 0,
                local_path: PathBuf::from("/project/content/my-post/image.png"),
                original_ref: "image.png".to_string(),
            }],
        };
        let mut assets = DeferredAssets::new(pending, AssetStrategy::External);
        assets
            .resolved
            .insert(0, "https://cdn.example.com/abc123.png".to_string());

        let content_path = PathBuf::from("/project/content/my-post");
        let map = build_resolved_url_map(&assets, &content_path);

        assert_eq!(
            map.get("image.png"),
            Some(&"https://cdn.example.com/abc123.png".to_string())
        );
    }
}

// ============================================================================
// Asset Analysis (shared by dry-run and real publish)
// ============================================================================

/// Information about a single asset in analysis.
#[derive(Debug, Clone)]
pub struct AssetInfo {
    /// Asset index in the pending list.
    pub index: usize,
    /// Path to the asset file.
    pub path: std::path::PathBuf,
    /// Size of the asset in bytes.
    pub size_bytes: u64,
    /// SHA256 content hash (full 64-char hex string).
    pub content_hash: String,
    /// Whether this is a new asset (not cached).
    pub is_new: bool,
    /// Cached remote URL (only set if is_new is false).
    pub cached_url: Option<String>,
}

/// Result of asset analysis.
///
/// This provides detailed information about which assets will be uploaded
/// and which will use cached URLs. Used by both dry-run and real publish.
#[derive(Debug, Clone)]
pub struct AssetAnalysis {
    /// Total number of assets.
    pub total_count: usize,
    /// Number of new assets (will be uploaded).
    pub new_count: usize,
    /// Number of cached assets (will use existing URLs).
    pub cached_count: usize,
    /// Total size of new assets in bytes.
    pub new_size_bytes: u64,
    /// Total size of cached assets in bytes.
    pub cached_size_bytes: u64,
    /// Detailed information for each asset.
    pub assets: Vec<AssetInfo>,
}

impl AssetAnalysis {
    /// Create an empty analysis result.
    pub fn empty() -> Self {
        Self {
            total_count: 0,
            new_count: 0,
            cached_count: 0,
            new_size_bytes: 0,
            cached_size_bytes: 0,
            assets: Vec::new(),
        }
    }

    /// Get human-readable size string.
    pub fn format_size(bytes: u64) -> String {
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
}

/// Analyze assets without materializing.
///
/// This function:
/// 1. Computes SHA256 hash for each pending asset
/// 2. Checks StatusTracker for cached URLs
/// 3. Returns detailed analysis of new vs cached assets
///
/// If StatusTracker is not available, all assets are treated as new.
///
/// # Arguments
///
/// * `assets` - The deferred assets to analyze
/// * `storage_config_id` - The storage config ID for cache lookup
/// * `tracker` - Optional status tracker for cache lookup
pub fn analyze_assets(
    assets: &DeferredAssets,
    storage_config_id: &str,
    tracker: Option<&StatusTracker>,
) -> Result<AssetAnalysis> {
    use sha2::{Digest, Sha256};

    if !assets.needs_materialize() {
        return Ok(AssetAnalysis::empty());
    }

    let pending = &assets.pending;

    let mut analysis = AssetAnalysis {
        total_count: pending.assets.len(),
        new_count: 0,
        cached_count: 0,
        new_size_bytes: 0,
        cached_size_bytes: 0,
        assets: Vec::with_capacity(pending.assets.len()),
    };

    for asset in &pending.assets {
        // Read file and compute hash
        let data = std::fs::read(&asset.local_path)
            .with_context(|| format!("Failed to read asset: {}", asset.local_path.display()))?;

        let size_bytes = data.len() as u64;
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let content_hash = hex::encode(hasher.finalize());

        let extension = std::path::Path::new(&asset.local_path)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        // Check cache if tracker available
        let (is_new, cached_url) = if let Some(t) = tracker {
            match t.get_asset_by_content(storage_config_id, &content_hash, &extension) {
                Ok(Some(record)) => (false, Some(record.remote_url)),
                _ => (true, None),
            }
        } else {
            (true, None)
        };

        let info = AssetInfo {
            index: asset.index,
            path: asset.local_path.clone(),
            size_bytes,
            content_hash,
            is_new,
            cached_url,
        };

        if info.is_new {
            analysis.new_count += 1;
            analysis.new_size_bytes += size_bytes;
        } else {
            analysis.cached_count += 1;
            analysis.cached_size_bytes += size_bytes;
        }

        analysis.assets.push(info);
    }

    Ok(analysis)
}

/// Materialize assets with optional mock mode.
///
/// If `mock: true`: generates mock URLs without file I/O (for dry-run)
/// If `mock: false`: actually uploads to S3 (for real publish)
///
/// The `mock_url_prefix` is used to generate mock URLs when `mock: true`.
/// If not provided, defaults to `https://mock-cdn.example.com`.
/// For External strategy, this can use the storage's url_prefix.
/// For Upload strategy (adapter-specific), adapters pass their own prefix.
///
/// Returns AssetAnalysis for UI logging.
pub async fn materialize_with_analysis(
    assets: &mut DeferredAssets,
    storage_config: &StorageConfig,
    tracker: Option<&StatusTracker>,
    mock: bool,
    mock_url_prefix: Option<&str>,
) -> Result<AssetAnalysis> {
    // 1. Always analyze first (compute hashes, check cache)
    let config_id = storage_config.config_id();
    let analysis = analyze_assets(assets, &config_id, tracker)?;

    if analysis.total_count == 0 {
        return Ok(analysis);
    }

    // 2. Either mock or real upload
    if mock {
        // Generate mock URLs (no file I/O)
        let prefix = mock_url_prefix.unwrap_or("https://mock-cdn.example.com");
        for info in &analysis.assets {
            let extension = info
                .path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("bin");
            let stem = info
                .path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("asset");

            let mock_url = format!(
                "{}/{}/{}.{}",
                prefix,
                &info.content_hash[..8],
                stem,
                extension
            );
            assets.resolved.insert(info.index, mock_url);
        }
        info!(
            "Dry-run: {} asset(s) analyzed ({} new, {} cached)",
            analysis.total_count, analysis.new_count, analysis.cached_count
        );
    } else {
        // Real S3 upload
        let storage = S3Storage::new(storage_config)?;

        if analysis.new_count > 0 {
            info!(
                "Uploading {} new asset(s) to external storage...",
                analysis.new_count
            );
        }

        let url_map = if let Some(t) = tracker {
            upload_pending_assets(&storage, &assets.pending, t).await?
        } else {
            // Without tracker, we can't use caching - upload each asset
            let mut url_map = std::collections::HashMap::new();
            for asset in &assets.pending.assets {
                let data = std::fs::read(&asset.local_path).with_context(|| {
                    format!("Failed to read asset: {}", asset.local_path.display())
                })?;
                let result = storage.upload(&asset.local_path, &data).await?;
                url_map.insert(asset.index, result.remote_url);
            }
            url_map
        };
        assets.resolved = url_map;

        info!(
            "Assets uploaded: {} new, {} cached",
            analysis.new_count, analysis.cached_count
        );
    }

    Ok(analysis)
}
