//! URL mapping utilities for image references.
//!
//! Converts local asset references (markers and img src paths) to final URLs.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

fn normalize_separators(path: &str) -> String {
    path.replace('\\', "/")
}

fn trim_dot_prefix(path: &str) -> &str {
    path.strip_prefix("./").unwrap_or(path)
}

fn trim_leading_slash(path: &str) -> &str {
    path.strip_prefix('/').unwrap_or(path)
}

/// Generate candidate keys for matching an image reference.
///
/// Given a path like `./assets/img.png`, returns normalized variants:
/// `assets/img.png`, `./assets/img.png`, `/assets/img.png`, etc.
pub fn key_candidates(value: &str) -> Vec<String> {
    let normalized = normalize_separators(value);
    let trimmed_dot = trim_dot_prefix(&normalized);
    let trimmed_slash = trim_leading_slash(trimmed_dot);
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let candidates = vec![
        normalized.clone(),
        trimmed_dot.to_string(),
        trimmed_slash.to_string(),
        format!("./{trimmed_slash}"),
        format!("/{trimmed_slash}"),
    ];
    for candidate in candidates {
        let c = candidate.trim();
        if !c.is_empty() && seen.insert(c.to_string()) {
            out.push(c.to_string());
        }
    }
    out
}

/// Resolve an image reference to a URL using a map of path variants.
///
/// Tries multiple path normalization variants to find a match.
pub fn resolve_image_reference_url(
    reference: &str,
    url_map: &HashMap<String, String>,
) -> Option<String> {
    key_candidates(reference)
        .into_iter()
        .find_map(|candidate| url_map.get(&candidate).cloned())
}

fn marker_path_candidates(content_root: &Path, asset_path: &Path) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();

    let mut push = |s: String| {
        if !s.is_empty() && seen.insert(s.clone()) {
            out.push(s);
        }
    };

    if let Ok(rel) = asset_path.strip_prefix(content_root) {
        for candidate in key_candidates(&rel.to_string_lossy()) {
            push(candidate);
        }
    }
    if asset_path.is_relative() {
        for candidate in key_candidates(&asset_path.to_string_lossy()) {
            push(candidate);
        }
    }
    for candidate in key_candidates(&asset_path.to_string_lossy()) {
        push(candidate);
    }

    out
}

fn src_path_candidates(content_root: &Path, asset_path: &Path) -> Vec<String> {
    // src candidates include marker candidates plus common web-style leading slash variants.
    marker_path_candidates(content_root, asset_path)
}

/// Build a URL map for image markers.
///
/// Given a map of `PathBuf -> URL`, generates all path variant keys
/// that might be used to reference each asset.
pub fn build_image_marker_url_map(
    content_root: &Path,
    asset_map: &HashMap<PathBuf, String>,
) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for (asset_path, url) in asset_map {
        for candidate in marker_path_candidates(content_root, asset_path) {
            map.insert(candidate, url.clone());
        }
    }
    map
}

/// Build a URL map for `<img src="...">` references.
///
/// Same as `build_image_marker_url_map` but intended for HTML src attributes.
pub fn build_image_src_url_map(
    content_root: &Path,
    asset_map: &HashMap<PathBuf, String>,
) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for (asset_path, url) in asset_map {
        for candidate in src_path_candidates(content_root, asset_path) {
            map.insert(candidate, url.clone());
        }
    }
    map
}

/// Build a map of asset paths to `file://` URLs for local preview.
pub fn build_preview_file_url_map(
    content_root: &Path,
    assets: &[PathBuf],
) -> HashMap<PathBuf, String> {
    let mut map = HashMap::new();
    for asset in assets {
        let absolute = if asset.is_absolute() {
            asset.clone()
        } else {
            content_root.join(asset)
        };
        map.insert(asset.clone(), format!("file://{}", absolute.display()));
    }
    map
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn test_key_candidates_basic() {
        let candidates = key_candidates("assets/image.png");
        assert!(candidates.contains(&"assets/image.png".to_string()));
        assert!(candidates.contains(&"./assets/image.png".to_string()));
    }

    #[test]
    fn test_key_candidates_with_dot_prefix() {
        let candidates = key_candidates("./assets/image.png");
        assert!(candidates.contains(&"assets/image.png".to_string()));
        assert!(candidates.contains(&"./assets/image.png".to_string()));
    }

    #[test]
    fn test_key_candidates_backslash() {
        let candidates = key_candidates("assets\\image.png");
        assert!(candidates.contains(&"assets/image.png".to_string()));
    }

    #[test]
    fn test_resolve_image_reference_url() {
        let mut url_map = HashMap::new();
        url_map.insert(
            "assets/image.png".to_string(),
            "https://cdn.example.com/abc123.png".to_string(),
        );

        assert_eq!(
            resolve_image_reference_url("assets/image.png", &url_map),
            Some("https://cdn.example.com/abc123.png".to_string())
        );
        assert_eq!(
            resolve_image_reference_url("./assets/image.png", &url_map),
            Some("https://cdn.example.com/abc123.png".to_string())
        );
        assert_eq!(
            resolve_image_reference_url("nonexistent.png", &url_map),
            None
        );
    }

    #[test]
    fn test_build_image_marker_url_map() {
        let content_root = PathBuf::from("/project/content/my-post");
        let mut asset_map = HashMap::new();
        asset_map.insert(
            PathBuf::from("/project/content/my-post/image.png"),
            "https://cdn.example.com/abc123.png".to_string(),
        );

        let url_map = build_image_marker_url_map(&content_root, &asset_map);
        assert!(url_map.contains_key("image.png"));
    }

    #[test]
    fn test_build_preview_file_url_map() {
        let content_root = PathBuf::from("/project/content/my-post");
        let assets = vec![PathBuf::from("image.png")];

        let map = build_preview_file_url_map(&content_root, &assets);
        assert_eq!(
            map.get(&PathBuf::from("image.png")),
            Some(&"file:///project/content/my-post/image.png".to_string())
        );
    }
}
