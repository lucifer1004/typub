//! Content parsing and discovery

use crate::ThemeId;
use anyhow::{Context, Result};
use chrono::NaiveDate;
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Source format of the content file
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentFormat {
    /// Native Typst format (.typ)
    Typst,
    /// Markdown format (.md) - rendered via cmarker
    Markdown,
}

/// Represents a single content post
#[derive(Debug)]
pub struct Content {
    /// Path to the post directory
    pub path: PathBuf,
    /// Parsed metadata from meta.toml
    pub meta: ContentMeta,
    /// Path to the main content file (content.typ or content.md)
    pub content_file: PathBuf,
    /// Source format of the content
    pub source_format: ContentFormat,
    /// Optional path to slides file (slides.typ)
    pub slides_file: Option<PathBuf>,
    /// List of asset files
    pub assets: Vec<PathBuf>,
}

/// Deserialize a TOML date or date string into NaiveDate
fn deserialize_date<'de, D>(deserializer: D) -> std::result::Result<NaiveDate, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    // Try to deserialize as toml::value::Date first, then as string
    let value = toml::Value::deserialize(deserializer)?;

    match value {
        toml::Value::Datetime(dt) => {
            // TOML datetime - extract date part
            let date = dt
                .date
                .ok_or_else(|| D::Error::custom("datetime missing date"))?;
            NaiveDate::from_ymd_opt(date.year as i32, date.month as u32, date.day as u32)
                .ok_or_else(|| D::Error::custom("invalid date"))
        }
        toml::Value::String(s) => {
            // Parse as string
            NaiveDate::parse_from_str(&s, "%Y-%m-%d")
                .map_err(|e| D::Error::custom(format!("invalid date string: {}", e)))
        }
        _ => Err(D::Error::custom("expected date or string")),
    }
}

/// Deserialize an optional TOML date
fn deserialize_optional_date<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<NaiveDate>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    let value: Option<toml::Value> = Option::deserialize(deserializer)?;

    match value {
        None => Ok(None),
        Some(toml::Value::Datetime(dt)) => {
            let date = dt
                .date
                .ok_or_else(|| D::Error::custom("datetime missing date"))?;
            Ok(NaiveDate::from_ymd_opt(
                date.year as i32,
                date.month as u32,
                date.day as u32,
            ))
        }
        Some(toml::Value::String(s)) => Ok(NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
        _ => Err(D::Error::custom("expected date or string")),
    }
}

/// Metadata from meta.toml
#[derive(Debug, Deserialize)]
pub struct ContentMeta {
    pub title: String,
    #[serde(deserialize_with = "deserialize_date")]
    pub created: NaiveDate,
    #[serde(default, deserialize_with = "deserialize_optional_date")]
    pub updated: Option<NaiveDate>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    /// Per-content default for published state (layer 2 per [[RFC-0005:C-RESOLUTION-ORDER]])
    #[serde(default)]
    pub published: Option<bool>,
    /// Per-content default theme (layer 2 in theme resolution chain)
    #[serde(default)]
    pub theme: Option<ThemeId>,
    /// Preferred platform for internal link resolution in copypaste adapters.
    /// Per-post override (layer 1). Falls back to global config or auto-selection.
    #[serde(default)]
    pub internal_link_target: Option<String>,
    /// Per-content Typst render preamble override (layer 2 per [[RFC-0005:C-RESOLUTION-ORDER]]).
    #[serde(default)]
    pub preamble: Option<String>,
    #[serde(default)]
    pub platforms: HashMap<String, PostPlatformConfig>,
}

/// Per-post platform configuration (overrides global)
#[derive(Debug, Clone, Deserialize)]
pub struct PostPlatformConfig {
    /// Per-content platform-specific published setting (layer 1 per [[RFC-0005:C-RESOLUTION-ORDER]])
    #[serde(default)]
    pub published: Option<bool>,
    /// Per-content platform-specific internal link target (layer 1 in resolution chain)
    #[serde(default)]
    pub internal_link_target: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, toml::Value>,
}

impl PostPlatformConfig {
    /// Get a string value from config, expanding environment variables.
    ///
    /// Supports shell-like variable substitution via the `subst` crate:
    /// - `$VAR` or `${VAR}` — substitute from environment
    /// - `${VAR:default}` — use default if VAR is unset
    pub fn get_str(&self, key: &str) -> Option<String> {
        self.extra
            .get(key)
            .and_then(|v| v.as_str())
            .map(|s| subst::substitute(s, &subst::Env).unwrap_or_else(|_| s.to_string()))
    }

    /// Get a raw string value without environment variable expansion.
    pub fn get_str_raw(&self, key: &str) -> Option<&str> {
        self.extra.get(key).and_then(|v| v.as_str())
    }

    /// Get an integer value from config
    pub fn get_int(&self, key: &str) -> Option<i64> {
        self.extra.get(key).and_then(|v| v.as_integer())
    }
}

/// Lightweight post info for display and sorting.
///
/// This is a projection of `Content` that avoids the Clone requirement
/// and contains only the fields needed for listing, sorting, and filtering.
#[derive(Debug, Clone)]
pub struct PostInfo {
    /// Path to the post directory
    pub path: PathBuf,
    /// Post title
    pub title: String,
    /// Post slug (directory name)
    pub slug: String,
    /// Created date
    pub created: NaiveDate,
    /// Updated date (defaults to created if not specified in meta.toml)
    pub updated: NaiveDate,
    /// Post tags
    pub tags: Vec<String>,
    /// Platform publish status: platform_id → (is_published, optional_url)
    pub status: HashMap<String, (bool, Option<String>)>,
}

impl PostInfo {
    /// Create a PostInfo from a Content and its publish status.
    pub fn from_content(
        content: &Content,
        status: HashMap<String, (bool, Option<String>)>,
    ) -> Self {
        Self {
            path: content.path.clone(),
            title: content.meta.title.clone(),
            slug: content.slug().to_string(),
            created: content.meta.created,
            updated: content.meta.updated.unwrap_or(content.meta.created),
            tags: content.meta.tags.clone(),
            status,
        }
    }
}

/// Result of discovering content posts in a directory.
///
/// Contains both successfully loaded posts and any errors encountered.
/// The caller can decide how to handle errors (e.g., log them, display warnings).
#[derive(Debug)]
pub struct DiscoverResult {
    /// Successfully loaded content posts, sorted by created date (newest first)
    pub contents: Vec<Content>,
    /// Errors encountered while loading posts: (path, error)
    pub errors: Vec<(PathBuf, anyhow::Error)>,
}

impl Content {
    /// Load a content post from a directory
    pub fn load(path: &Path) -> Result<Self> {
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        // Read meta.toml
        let meta_path = path.join("meta.toml");
        let meta_content = std::fs::read_to_string(&meta_path)
            .with_context(|| format!("Failed to read meta.toml at {}", meta_path.display()))?;
        let meta: ContentMeta = toml::from_str(&meta_content)
            .with_context(|| format!("Failed to parse meta.toml at {}", meta_path.display()))?;

        // Find content file: prefer .typ over .md
        let typ_file = path.join("content.typ");
        let md_file = path.join("content.md");

        let (content_file, source_format) = if typ_file.exists() {
            (typ_file, ContentFormat::Typst)
        } else if md_file.exists() {
            (md_file, ContentFormat::Markdown)
        } else {
            anyhow::bail!(
                "No content file found at {} (expected content.typ or content.md)",
                path.display()
            );
        };

        // Check for slides.typ
        let slides_file = path.join("slides.typ");
        let slides_file = if slides_file.exists() {
            Some(slides_file)
        } else {
            None
        };

        // Discover assets
        let assets_dir = path.join("assets");
        let assets = if assets_dir.exists() {
            Self::discover_assets(&assets_dir)?
        } else {
            Vec::new()
        };

        Ok(Self {
            path,
            meta,
            content_file,
            source_format,
            slides_file,
            assets,
        })
    }

    /// Discover all content posts in a directory.
    ///
    /// Returns a [`DiscoverResult`] containing both successfully loaded posts
    /// and any errors encountered. The caller can decide how to handle errors
    /// (e.g., log them in verbose mode, display warnings).
    pub fn discover_all(content_dir: &Path) -> Result<DiscoverResult> {
        let mut contents = Vec::new();
        let mut errors = Vec::new();

        if !content_dir.exists() {
            return Ok(DiscoverResult { contents, errors });
        }

        for entry in std::fs::read_dir(content_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Check if it has a meta.toml (making it a valid post)
                if path.join("meta.toml").exists() {
                    match Self::load(&path) {
                        Ok(content) => contents.push(content),
                        Err(e) => errors.push((path, e)),
                    }
                }
            }
        }

        // Sort by created date, newest first
        contents.sort_by_key(|item| std::cmp::Reverse(item.meta.created));

        Ok(DiscoverResult { contents, errors })
    }

    /// Discover all assets in the assets directory
    fn discover_assets(assets_dir: &Path) -> Result<Vec<PathBuf>> {
        let mut assets = Vec::new();

        for entry in WalkDir::new(assets_dir).follow_links(true) {
            let entry = entry?;
            if entry.file_type().is_file() {
                assets.push(entry.path().to_path_buf());
            }
        }

        Ok(assets)
    }

    /// Get the post's slug (directory name)
    pub fn slug(&self) -> &str {
        self.path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
    }

    /// Get platform-specific config, if any
    pub fn platform_config(&self, platform: &str) -> Option<&PostPlatformConfig> {
        self.meta.platforms.get(platform)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn test_content_meta_parsing() {
        let toml_str = r#"
title = "Test Post"
created = 2024-01-15
tags = ["rust", "test"]
categories = ["engineering", "rust"]

[platforms.astro]
slug = "test-post"
"#;
        let meta: ContentMeta = toml::from_str(toml_str).expect("parse TOML");
        assert_eq!(meta.title, "Test Post");
        assert_eq!(meta.tags, vec!["rust", "test"]);
        assert_eq!(meta.categories, vec!["engineering", "rust"]);
        assert!(meta.platforms.contains_key("astro"));
    }

    #[test]
    fn test_content_meta_with_updated_date() {
        let toml_str = r#"
title = "Updated Post"
created = 2024-01-15
updated = 2024-06-01
tags = []
"#;
        let meta: ContentMeta = toml::from_str(toml_str).expect("parse TOML");
        assert_eq!(
            meta.updated,
            Some(NaiveDate::from_ymd_opt(2024, 6, 1).expect("valid date"))
        );
    }

    #[test]
    fn test_content_meta_with_root_preamble() {
        let toml_str = r##"
title = "Preamble Post"
created = 2024-01-15
preamble = "#set text(size: 11pt)"
"##;
        let meta: ContentMeta = toml::from_str(toml_str).expect("parse TOML");
        assert_eq!(meta.preamble.as_deref(), Some("#set text(size: 11pt)"));
    }

    #[test]
    fn test_content_meta_minimal() {
        let toml_str = r#"
title = "Minimal"
created = 2024-01-01
"#;
        let meta: ContentMeta = toml::from_str(toml_str).expect("parse TOML");
        assert_eq!(meta.title, "Minimal");
        assert!(meta.updated.is_none());
        assert!(meta.tags.is_empty());
        assert!(meta.categories.is_empty());
        assert!(meta.platforms.is_empty());
    }

    #[test]
    fn test_content_meta_categories_only() {
        let toml_str = r#"
title = "Categories"
created = 2024-01-01
categories = ["backend", "api"]
"#;
        let meta: ContentMeta = toml::from_str(toml_str).expect("parse TOML");
        assert_eq!(meta.categories, vec!["backend", "api"]);
        assert!(meta.tags.is_empty());
    }

    #[test]
    fn test_content_meta_string_date() {
        let toml_str = r#"
title = "String Date"
created = "2024-03-15"
"#;
        let meta: ContentMeta = toml::from_str(toml_str).expect("parse TOML");
        assert_eq!(
            meta.created,
            NaiveDate::from_ymd_opt(2024, 3, 15).expect("valid date")
        );
    }

    #[test]
    fn test_content_meta_invalid_date() {
        let toml_str = r#"
title = "Bad Date"
created = "not-a-date"
"#;
        assert!(toml::from_str::<ContentMeta>(toml_str).is_err());
    }

    #[test]
    fn test_content_meta_missing_title() {
        let toml_str = r#"
created = 2024-01-01
"#;
        assert!(toml::from_str::<ContentMeta>(toml_str).is_err());
    }

    #[test]
    fn test_post_platform_config_accessors() {
        let toml_str = r#"
title = "Test"
created = 2024-01-01

[platforms.notion]
database_id = "abc"
priority = 5
"#;
        let meta: ContentMeta = toml::from_str(toml_str).expect("parse TOML");
        let notion = meta.platforms.get("notion").expect("key should exist");
        assert_eq!(notion.get_str("database_id"), Some("abc".to_string()));
        assert_eq!(notion.get_int("priority"), Some(5));
        assert_eq!(notion.get_str("nonexistent"), None);
        assert_eq!(notion.get_int("database_id"), None); // wrong type
    }

    #[test]
    fn test_get_str_no_env_var() {
        let toml_str = r#"
title = "Test"
created = 2024-01-01

[platforms.test]
key = "plain-value"
"#;
        let meta: ContentMeta = toml::from_str(toml_str).expect("parse TOML");
        let config = meta.platforms.get("test").expect("platform config");
        assert_eq!(config.get_str("key"), Some("plain-value".to_string()));
    }

    #[test]
    fn test_get_str_with_env_var() {
        // SAFETY: Test runs single-threaded via cargo test
        unsafe {
            std::env::set_var("TYPUB_TEST_VAR", "expanded-value");
        }
        let toml_str = r#"
title = "Test"
created = 2024-01-01

[platforms.test]
key = "$TYPUB_TEST_VAR"
"#;
        let meta: ContentMeta = toml::from_str(toml_str).expect("parse TOML");
        let config = meta.platforms.get("test").expect("platform config");
        assert_eq!(config.get_str("key"), Some("expanded-value".to_string()));
        // SAFETY: Test cleanup
        unsafe {
            std::env::remove_var("TYPUB_TEST_VAR");
        }
    }

    #[test]
    fn test_get_str_raw_no_expansion() {
        // SAFETY: Test runs single-threaded via cargo test
        unsafe {
            std::env::set_var("TYPUB_RAW_TEST", "should-not-appear");
        }
        let toml_str = r#"
title = "Test"
created = 2024-01-01

[platforms.test]
key = "$TYPUB_RAW_TEST"
"#;
        let meta: ContentMeta = toml::from_str(toml_str).expect("parse TOML");
        let config = meta.platforms.get("test").expect("platform config");
        // get_str_raw should NOT expand
        assert_eq!(config.get_str_raw("key"), Some("$TYPUB_RAW_TEST"));
        // SAFETY: Test cleanup
        unsafe {
            std::env::remove_var("TYPUB_RAW_TEST");
        }
    }

    #[test]
    fn test_content_load_typ() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let post_dir = dir.path().join("test-post");
        std::fs::create_dir_all(&post_dir).expect("create dir");

        std::fs::write(
            post_dir.join("meta.toml"),
            "title = \"Test\"\ncreated = 2024-01-01\n",
        )
        .expect("write file");
        std::fs::write(post_dir.join("content.typ"), "#set text(size: 12pt)").expect("write file");

        let content = Content::load(&post_dir).expect("load content");
        assert_eq!(content.source_format, ContentFormat::Typst);
        assert_eq!(content.meta.title, "Test");
        assert!(content.slides_file.is_none());
        assert!(content.assets.is_empty());
    }

    #[test]
    fn test_content_load_md() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let post_dir = dir.path().join("my-md-post");
        std::fs::create_dir_all(&post_dir).expect("create dir");

        std::fs::write(
            post_dir.join("meta.toml"),
            "title = \"MD\"\ncreated = 2024-02-01\n",
        )
        .expect("write file");
        std::fs::write(post_dir.join("content.md"), "# Hello").expect("write file");

        let content = Content::load(&post_dir).expect("load content");
        assert_eq!(content.source_format, ContentFormat::Markdown);
    }

    #[test]
    fn test_content_load_prefers_typ_over_md() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let post_dir = dir.path().join("dual-post");
        std::fs::create_dir_all(&post_dir).expect("create dir");

        std::fs::write(
            post_dir.join("meta.toml"),
            "title = \"Dual\"\ncreated = 2024-01-01\n",
        )
        .expect("write file");
        std::fs::write(post_dir.join("content.typ"), "typst content").expect("write file");
        std::fs::write(post_dir.join("content.md"), "markdown content").expect("write file");

        let content = Content::load(&post_dir).expect("load content");
        assert_eq!(content.source_format, ContentFormat::Typst);
    }

    #[test]
    fn test_content_load_no_content_file() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let post_dir = dir.path().join("empty-post");
        std::fs::create_dir_all(&post_dir).expect("create dir");

        std::fs::write(
            post_dir.join("meta.toml"),
            "title = \"Empty\"\ncreated = 2024-01-01\n",
        )
        .expect("write file");

        assert!(Content::load(&post_dir).is_err());
    }

    #[test]
    fn test_content_load_no_meta() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let post_dir = dir.path().join("no-meta");
        std::fs::create_dir_all(&post_dir).expect("create dir");
        std::fs::write(post_dir.join("content.typ"), "hello").expect("write file");

        assert!(Content::load(&post_dir).is_err());
    }

    #[test]
    fn test_content_load_with_assets() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let post_dir = dir.path().join("asset-post");
        let assets_dir = post_dir.join("assets");
        std::fs::create_dir_all(&assets_dir).expect("create dir");

        std::fs::write(
            post_dir.join("meta.toml"),
            "title = \"Assets\"\ncreated = 2024-01-01\n",
        )
        .expect("write file");
        std::fs::write(post_dir.join("content.typ"), "content").expect("write file");
        std::fs::write(assets_dir.join("photo.png"), "fake png").expect("write file");
        std::fs::write(assets_dir.join("diagram.svg"), "fake svg").expect("write file");

        let content = Content::load(&post_dir).expect("load content");
        assert_eq!(content.assets.len(), 2);
    }

    #[test]
    fn test_content_load_with_slides() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let post_dir = dir.path().join("slides-post");
        std::fs::create_dir_all(&post_dir).expect("create dir");

        std::fs::write(
            post_dir.join("meta.toml"),
            "title = \"Slides\"\ncreated = 2024-01-01\n",
        )
        .expect("write file");
        std::fs::write(post_dir.join("content.typ"), "content").expect("write file");
        std::fs::write(post_dir.join("slides.typ"), "slide content").expect("write file");

        let content = Content::load(&post_dir).expect("load content");
        assert!(content.slides_file.is_some());
    }

    #[test]
    fn test_slug() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let post_dir = dir.path().join("2024-01-15-my-great-post");
        std::fs::create_dir_all(&post_dir).expect("create dir");

        std::fs::write(
            post_dir.join("meta.toml"),
            "title = \"Great\"\ncreated = 2024-01-15\n",
        )
        .expect("write file");
        std::fs::write(post_dir.join("content.typ"), "x").expect("write file");

        let content = Content::load(&post_dir).expect("load content");
        assert_eq!(content.slug(), "2024-01-15-my-great-post");
    }

    #[test]
    fn test_platform_config() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let post_dir = dir.path().join("pc-post");
        std::fs::create_dir_all(&post_dir).expect("create dir");

        std::fs::write(
            post_dir.join("meta.toml"),
            r#"
title = "PC"
created = 2024-01-01

[platforms.astro]
slug = "custom-slug"
"#,
        )
        .expect("write file");
        std::fs::write(post_dir.join("content.typ"), "x").expect("write file");

        let content = Content::load(&post_dir).expect("load content");
        assert!(content.platform_config("astro").is_some());
        assert!(content.platform_config("notion").is_none());
    }

    #[test]
    fn test_discover_all() {
        let dir = tempfile::tempdir().expect("create temp dir");

        // Create two valid posts
        for name in ["2024-01-01-first", "2024-02-01-second"] {
            let post = dir.path().join(name);
            std::fs::create_dir_all(&post).expect("create dir");
            std::fs::write(
                post.join("meta.toml"),
                format!("title = \"{name}\"\ncreated = {}\n", &name[..10]),
            )
            .expect("write file");
            std::fs::write(post.join("content.typ"), "x").expect("write file");
        }

        // Create a directory without meta.toml (should be skipped)
        let invalid = dir.path().join("not-a-post");
        std::fs::create_dir_all(&invalid).expect("create dir");

        let result = Content::discover_all(dir.path()).expect("discover all");
        assert_eq!(result.contents.len(), 2);
        assert!(result.errors.is_empty());
        // Newest first
        assert_eq!(result.contents[0].slug(), "2024-02-01-second");
        assert_eq!(result.contents[1].slug(), "2024-01-01-first");
    }

    #[test]
    fn test_discover_all_with_errors() {
        let dir = tempfile::tempdir().expect("create temp dir");

        // Create a valid post
        let valid = dir.path().join("valid-post");
        std::fs::create_dir_all(&valid).expect("create dir");
        std::fs::write(
            valid.join("meta.toml"),
            "title = \"Valid\"\ncreated = 2024-01-01\n",
        )
        .expect("write file");
        std::fs::write(valid.join("content.typ"), "x").expect("write file");

        // Create a post with invalid meta.toml
        let invalid = dir.path().join("invalid-post");
        std::fs::create_dir_all(&invalid).expect("create dir");
        std::fs::write(invalid.join("meta.toml"), "invalid toml {{{{").expect("write file");

        let result = Content::discover_all(dir.path()).expect("discover all");
        assert_eq!(result.contents.len(), 1);
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].0.ends_with("invalid-post"));
    }

    #[test]
    fn test_discover_all_empty_dir() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let result = Content::discover_all(dir.path()).expect("discover all");
        assert!(result.contents.is_empty());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_discover_all_nonexistent_dir() {
        let result = Content::discover_all(Path::new("/tmp/nonexistent-contents-dir"))
            .expect("discover all");
        assert!(result.contents.is_empty());
        assert!(result.errors.is_empty());
    }
}
