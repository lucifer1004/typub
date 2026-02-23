//! Asset tracking and URL resolution for v2 semantic IR.
//!
//! Per [[RFC-0009:C-ASSET-REFERENCE]], assets are document-indexed and content
//! references them by stable IDs instead of inline marker nodes.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::Result;

use typub_config::project::validate_within_project;
use typub_core::AssetStrategy;
use typub_ir::{Asset, AssetSource, AssetVariant, Document, Url};
use typub_storage::{
    PendingAsset, PendingAssetList, build_pending_asset_list, resolve_image_reference_url,
};

const ORIGINAL_VARIANT_NAME: &str = "original";

pub fn build_pending_asset_list_validated(
    assets: &[PathBuf],
    content_path: &Path,
    project_root: &Path,
) -> Result<PendingAssetList> {
    let mut pending = Vec::with_capacity(assets.len());

    for (index, asset) in assets.iter().enumerate() {
        let local_path = if asset.is_absolute() {
            asset.clone()
        } else {
            content_path.join(asset)
        };

        validate_within_project(&local_path, project_root)?;

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

    Ok(PendingAssetList { assets: pending })
}

fn asset_local_path(asset: &Asset) -> Option<&str> {
    let source = match asset {
        Asset::Image(image) => &image.source,
        Asset::Video(media) | Asset::Audio(media) => &media.source,
        Asset::File(file) => &file.source,
        Asset::Custom(custom) => &custom.source,
    };

    match source {
        AssetSource::LocalPath { path } => Some(path.as_str()),
        AssetSource::RemoteUrl { .. } | AssetSource::DataUri { .. } => None,
    }
}

fn asset_variants_mut(asset: &mut Asset) -> &mut Vec<AssetVariant> {
    match asset {
        Asset::Image(image) => &mut image.variants,
        Asset::Video(media) | Asset::Audio(media) => &mut media.variants,
        Asset::File(file) => &mut file.variants,
        Asset::Custom(custom) => &mut custom.variants,
    }
}

fn asset_variants(asset: &Asset) -> &[AssetVariant] {
    match asset {
        Asset::Image(image) => &image.variants,
        Asset::Video(media) | Asset::Audio(media) => &media.variants,
        Asset::File(file) => &file.variants,
        Asset::Custom(custom) => &custom.variants,
    }
}

fn collect_local_asset_paths(document: &Document) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut seen = HashSet::new();

    for asset in document.assets.values() {
        if let Some(path) = asset_local_path(asset)
            && seen.insert(path.to_string())
        {
            paths.push(PathBuf::from(path));
        }
    }

    paths
}

pub fn build_pending_asset_list_from_document(
    document: &Document,
    content_path: &Path,
) -> PendingAssetList {
    let paths = collect_local_asset_paths(document);
    build_pending_asset_list(&paths, content_path)
}

pub fn build_pending_asset_list_from_document_validated(
    document: &Document,
    content_path: &Path,
    project_root: &Path,
) -> Result<PendingAssetList> {
    let paths = collect_local_asset_paths(document);
    build_pending_asset_list_validated(&paths, content_path, project_root)
}

fn upsert_original_variant(asset: &mut Asset, publish_url: Url) {
    let variants = asset_variants_mut(asset);

    if let Some(existing) = variants
        .iter_mut()
        .find(|variant| variant.name == ORIGINAL_VARIANT_NAME)
    {
        existing.publish_url = publish_url;
        return;
    }

    variants.push(AssetVariant {
        name: ORIGINAL_VARIANT_NAME.to_string(),
        publish_url,
        width: None,
        height: None,
    });
}

pub fn resolve_asset_urls(
    document: &mut Document,
    url_map: &std::collections::HashMap<String, String>,
) -> usize {
    let mut resolved = 0;

    for asset in document.assets.values_mut() {
        let local_path = asset_local_path(asset).map(ToString::to_string);
        if let Some(path) = local_path
            && let Some(url) = resolve_image_reference_url(&path, url_map)
        {
            upsert_original_variant(asset, Url(url));
            resolved += 1;
        }
    }

    resolved
}

fn has_original_publish_url(asset: &Asset) -> bool {
    asset_variants(asset).iter().any(|variant| {
        variant.name == ORIGINAL_VARIANT_NAME && !variant.publish_url.0.trim().is_empty()
    })
}

pub fn ensure_no_unresolved_image_markers(
    adapter_id: &str,
    strategy: AssetStrategy,
    document: &Document,
) -> Result<()> {
    if !strategy.requires_deferred_upload() {
        return Ok(());
    }

    let unresolved = document
        .assets
        .iter()
        .filter_map(|(asset_id, asset)| {
            if asset_local_path(asset).is_some() && !has_original_publish_url(asset) {
                Some(asset_id.0.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    if unresolved.is_empty() {
        return Ok(());
    }

    let preview = unresolved
        .iter()
        .take(3)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    anyhow::bail!(
        "Adapter '{}' has {} unresolved local asset(s) at Serialize stage: {}. Deferred assets must be resolved in Materialize.",
        adapter_id,
        unresolved.len(),
        preview
    );
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::collections::{BTreeMap, HashMap};

    use typub_ir::{AssetId, DocMeta, ImageAsset, RelativePath};

    use super::*;

    fn empty_document() -> Document {
        Document {
            blocks: Vec::new(),
            footnotes: BTreeMap::new(),
            assets: BTreeMap::new(),
            meta: DocMeta::default(),
        }
    }

    fn local_image_asset(path: &str) -> Asset {
        Asset::Image(ImageAsset {
            source: AssetSource::LocalPath {
                path: RelativePath::new(path.to_string()).expect("valid relative path"),
            },
            meta: None,
            variants: Vec::new(),
        })
    }

    fn remote_image_asset(url: &str) -> Asset {
        Asset::Image(ImageAsset {
            source: AssetSource::RemoteUrl {
                url: Url(url.to_string()),
            },
            meta: None,
            variants: Vec::new(),
        })
    }

    #[test]
    fn test_build_pending_asset_list_from_document_empty() {
        let document = empty_document();
        let result =
            build_pending_asset_list_from_document(&document, PathBuf::from("/content").as_path());
        assert!(result.assets.is_empty());
    }

    #[test]
    fn test_build_pending_asset_list_from_document_collects_local_assets() {
        let mut document = empty_document();
        document.assets.insert(
            AssetId("asset-a".to_string()),
            local_image_asset("assets/a.png"),
        );
        document.assets.insert(
            AssetId("asset-b".to_string()),
            local_image_asset("assets/a.png"),
        );
        document.assets.insert(
            AssetId("asset-c".to_string()),
            remote_image_asset("https://example.com/c.png"),
        );

        let result =
            build_pending_asset_list_from_document(&document, PathBuf::from("/content").as_path());
        assert_eq!(result.assets.len(), 1);
        assert_eq!(result.assets[0].original_ref, "assets/a.png");
    }

    #[test]
    fn test_resolve_asset_urls_updates_original_variant() {
        let mut document = empty_document();
        document.assets.insert(
            AssetId("asset-a".to_string()),
            local_image_asset("assets/a.png"),
        );

        let mut url_map = HashMap::new();
        url_map.insert(
            "assets/a.png".to_string(),
            "https://cdn.example.com/a.png".to_string(),
        );

        let resolved = resolve_asset_urls(&mut document, &url_map);
        assert_eq!(resolved, 1);

        let variants = match document.assets.get(&AssetId("asset-a".to_string())) {
            Some(Asset::Image(image)) => &image.variants,
            _ => panic!("expected image asset"),
        };
        assert_eq!(variants.len(), 1);
        assert_eq!(variants[0].name, "original");
        assert_eq!(variants[0].publish_url.0, "https://cdn.example.com/a.png");
    }

    #[test]
    fn test_ensure_no_unresolved_image_markers_embed() {
        let mut document = empty_document();
        document.assets.insert(
            AssetId("asset-a".to_string()),
            local_image_asset("assets/a.png"),
        );
        let result = ensure_no_unresolved_image_markers("test", AssetStrategy::Embed, &document);
        assert!(result.is_ok());
    }

    #[test]
    fn test_ensure_no_unresolved_image_markers_upload_with_unresolved() {
        let mut document = empty_document();
        document.assets.insert(
            AssetId("asset-a".to_string()),
            local_image_asset("assets/a.png"),
        );
        let result = ensure_no_unresolved_image_markers("test", AssetStrategy::Upload, &document);
        assert!(result.is_err());
        let err = result.expect_err("should fail");
        assert!(err.to_string().contains("unresolved"));
    }

    #[test]
    fn test_ensure_no_unresolved_image_markers_upload_resolved() {
        let mut document = empty_document();
        document.assets.insert(
            AssetId("asset-a".to_string()),
            local_image_asset("assets/a.png"),
        );

        let mut url_map = HashMap::new();
        url_map.insert(
            "assets/a.png".to_string(),
            "https://cdn.example.com/a.png".to_string(),
        );
        let resolved = resolve_asset_urls(&mut document, &url_map);
        assert_eq!(resolved, 1);

        let result = ensure_no_unresolved_image_markers("test", AssetStrategy::Upload, &document);
        assert!(result.is_ok());
    }
}
