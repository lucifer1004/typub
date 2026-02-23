//! Output caching for rendered content

use crate::content::Content;
use crate::renderer::RenderedOutput;
use anyhow::Result;
use std::path::PathBuf;
use typub_adapters_core::OutputFormat;
use typub_config::Config;

/// Cache manager for rendered outputs
pub struct Cache<'a> {
    config: &'a Config,
}

impl<'a> Cache<'a> {
    pub fn new(config: &'a Config) -> Self {
        Self { config }
    }

    /// Get the cache directory for a post
    pub fn cache_dir(&self, content: &Content) -> PathBuf {
        self.config.output_dir.join(content.slug())
    }

    /// Check if cached output exists and is fresh
    pub fn is_fresh(&self, content: &Content, format: OutputFormat) -> Result<bool> {
        let cache_path = self.output_path(content, format);

        if !cache_path.exists() {
            return Ok(false);
        }

        // Compare modification times
        let cache_mtime = std::fs::metadata(&cache_path)?.modified()?;
        let content_mtime = std::fs::metadata(&content.content_file)?.modified()?;

        // Cache is fresh if it's newer than content
        Ok(cache_mtime > content_mtime)
    }

    /// Get the path where cached output would be stored
    pub fn output_path(&self, content: &Content, format: OutputFormat) -> PathBuf {
        let dir = self.cache_dir(content);
        match format {
            OutputFormat::Html | OutputFormat::HtmlFragment => dir.join("content.html"),
            OutputFormat::Png => dir.join("slide-1.png"), // Check first slide
            OutputFormat::Pdf => dir.join("content.pdf"),
        }
    }

    /// Load cached output if available
    pub fn load(&self, content: &Content, format: OutputFormat) -> Result<Option<RenderedOutput>> {
        if !self.is_fresh(content, format)? {
            return Ok(None);
        }

        let path = self.output_path(content, format);

        match format {
            OutputFormat::Html | OutputFormat::HtmlFragment => {
                let html = std::fs::read_to_string(&path)?;
                Ok(Some(RenderedOutput {
                    format,
                    paths: vec![path],
                    html: Some(html),
                }))
            }
            OutputFormat::Png => {
                // Collect all PNG files
                let dir = self.cache_dir(content);
                let mut paths = Vec::new();
                for entry in std::fs::read_dir(&dir)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.extension().is_some_and(|ext| ext == "png") {
                        paths.push(path);
                    }
                }
                paths.sort();
                Ok(Some(RenderedOutput {
                    format,
                    paths,
                    html: None,
                }))
            }
            OutputFormat::Pdf => Ok(Some(RenderedOutput {
                format,
                paths: vec![path],
                html: None,
            })),
        }
    }

    /// Clear cached outputs for a post
    pub fn clear(&self, content: &Content) -> Result<()> {
        let dir = self.cache_dir(content);
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
        }
        Ok(())
    }

    /// Clear all cached outputs
    pub fn clear_all(&self) -> Result<()> {
        let dir = &self.config.output_dir;
        if dir.exists() {
            std::fs::remove_dir_all(dir)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use crate::content::{Content, ContentFormat, ContentMeta};
    use chrono::NaiveDate;
    use std::collections::HashMap;
    use typub_config::Config;

    fn make_config(output_dir: &std::path::Path) -> Config {
        Config {
            content_dir: std::path::PathBuf::from("posts"),
            output_dir: output_dir.to_path_buf(),
            storage: None,
            published: None,
            theme: None,
            internal_link_target: None,
            preamble: None,
            platforms: HashMap::new(),
        }
    }

    fn make_content(dir: &std::path::Path, slug: &str) -> Content {
        let post_dir = dir.join(slug);
        std::fs::create_dir_all(&post_dir).expect("create dir");
        let content_file = post_dir.join("content.typ");
        std::fs::write(&content_file, "test content").expect("write file");

        Content {
            path: post_dir,
            meta: ContentMeta {
                title: slug.to_string(),
                created: NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid date"),
                updated: None,
                tags: vec![],
                categories: vec![],
                published: None,
                theme: None,
                internal_link_target: None,
                preamble: None,
                platforms: HashMap::new(),
            },
            content_file,
            source_format: ContentFormat::Typst,
            slides_file: None,
            assets: vec![],
        }
    }

    #[test]
    fn test_cache_dir() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let config = make_config(dir.path().join("out").as_ref());
        let content = make_content(dir.path(), "my-post");
        let cache = Cache::new(&config);

        assert_eq!(
            cache.cache_dir(&content),
            dir.path().join("out").join("my-post")
        );
    }

    #[test]
    fn test_output_path_html() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let config = make_config(dir.path().join("out").as_ref());
        let content = make_content(dir.path(), "post");
        let cache = Cache::new(&config);

        assert_eq!(
            cache.output_path(&content, OutputFormat::Html),
            dir.path().join("out/post/content.html")
        );
        assert_eq!(
            cache.output_path(&content, OutputFormat::HtmlFragment),
            dir.path().join("out/post/content.html")
        );
    }

    #[test]
    fn test_output_path_png() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let config = make_config(dir.path().join("out").as_ref());
        let content = make_content(dir.path(), "post");
        let cache = Cache::new(&config);

        assert_eq!(
            cache.output_path(&content, OutputFormat::Png),
            dir.path().join("out/post/slide-1.png")
        );
    }

    #[test]
    fn test_output_path_pdf() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let config = make_config(dir.path().join("out").as_ref());
        let content = make_content(dir.path(), "post");
        let cache = Cache::new(&config);

        assert_eq!(
            cache.output_path(&content, OutputFormat::Pdf),
            dir.path().join("out/post/content.pdf")
        );
    }

    #[test]
    fn test_is_fresh_no_cache() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let config = make_config(dir.path().join("out").as_ref());
        let content = make_content(dir.path(), "post");
        let cache = Cache::new(&config);

        assert!(
            !cache
                .is_fresh(&content, OutputFormat::Html)
                .expect("check freshness")
        );
    }

    #[test]
    fn test_is_fresh_stale_cache() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let config = make_config(dir.path().join("out").as_ref());
        let content = make_content(dir.path(), "post");
        let cache = Cache::new(&config);

        // Create cache file FIRST (older)
        let cache_path = cache.output_path(&content, OutputFormat::Html);
        std::fs::create_dir_all(cache_path.parent().expect("cache path has parent"))
            .expect("create dir");
        std::fs::write(&cache_path, "old html").expect("write file");

        // Sleep to ensure mtime difference
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Touch content file (newer)
        std::fs::write(&content.content_file, "updated content").expect("write file");

        assert!(
            !cache
                .is_fresh(&content, OutputFormat::Html)
                .expect("check freshness")
        );
    }

    #[test]
    fn test_is_fresh_valid_cache() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let config = make_config(dir.path().join("out").as_ref());
        let content = make_content(dir.path(), "post");
        let cache = Cache::new(&config);

        // Content file already exists from make_content
        // Sleep to ensure mtime difference
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Create cache file AFTER (newer)
        let cache_path = cache.output_path(&content, OutputFormat::Html);
        std::fs::create_dir_all(cache_path.parent().expect("cache path has parent"))
            .expect("create dir");
        std::fs::write(&cache_path, "<html>cached</html>").expect("write file");

        assert!(
            cache
                .is_fresh(&content, OutputFormat::Html)
                .expect("check freshness")
        );
    }

    #[test]
    fn test_load_fresh_html() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let config = make_config(dir.path().join("out").as_ref());
        let content = make_content(dir.path(), "post");
        let cache = Cache::new(&config);

        std::thread::sleep(std::time::Duration::from_millis(50));

        let cache_path = cache.output_path(&content, OutputFormat::Html);
        std::fs::create_dir_all(cache_path.parent().expect("cache path has parent"))
            .expect("create dir");
        std::fs::write(&cache_path, "<html>cached</html>").expect("write file");

        let loaded = cache
            .load(&content, OutputFormat::Html)
            .expect("load cache")
            .expect("cache should exist");
        assert_eq!(loaded.format, OutputFormat::Html);
        assert_eq!(loaded.html.as_deref(), Some("<html>cached</html>"));
    }

    #[test]
    fn test_load_stale_returns_none() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let config = make_config(dir.path().join("out").as_ref());
        let content = make_content(dir.path(), "post");
        let cache = Cache::new(&config);

        // Create stale cache
        let cache_path = cache.output_path(&content, OutputFormat::Html);
        std::fs::create_dir_all(cache_path.parent().expect("cache path has parent"))
            .expect("create dir");
        std::fs::write(&cache_path, "old").expect("write file");

        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(&content.content_file, "new content").expect("write file");

        assert!(
            cache
                .load(&content, OutputFormat::Html)
                .expect("load cache")
                .is_none()
        );
    }

    #[test]
    fn test_clear() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let config = make_config(dir.path().join("out").as_ref());
        let content = make_content(dir.path(), "post");
        let cache = Cache::new(&config);

        let cache_dir = cache.cache_dir(&content);
        std::fs::create_dir_all(&cache_dir).expect("create dir");
        std::fs::write(cache_dir.join("content.html"), "html").expect("write file");

        cache.clear(&content).expect("clear cache");
        assert!(!cache_dir.exists());
    }

    #[test]
    fn test_clear_nonexistent_is_ok() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let config = make_config(dir.path().join("out").as_ref());
        let content = make_content(dir.path(), "post");
        let cache = Cache::new(&config);

        // Should not error even if directory doesn't exist
        cache.clear(&content).expect("clear cache");
    }

    #[test]
    fn test_clear_all() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let out_dir = dir.path().join("out");
        std::fs::create_dir_all(out_dir.join("post1")).expect("create dir");
        std::fs::create_dir_all(out_dir.join("post2")).expect("create dir");

        let config = make_config(&out_dir);
        let cache = Cache::new(&config);

        cache.clear_all().expect("clear all cache");
        assert!(!out_dir.exists());
    }

    #[test]
    fn test_load_fresh_png() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let config = make_config(dir.path().join("out").as_ref());
        let content = make_content(dir.path(), "slides");
        let cache = Cache::new(&config);

        std::thread::sleep(std::time::Duration::from_millis(50));

        let cache_dir = cache.cache_dir(&content);
        std::fs::create_dir_all(&cache_dir).expect("create dir");
        std::fs::write(cache_dir.join("slide-1.png"), "png1").expect("write file");
        std::fs::write(cache_dir.join("slide-2.png"), "png2").expect("write file");

        let loaded = cache
            .load(&content, OutputFormat::Png)
            .expect("load cache")
            .expect("cache should exist");
        assert_eq!(loaded.format, OutputFormat::Png);
        assert_eq!(loaded.paths.len(), 2);
        assert!(loaded.html.is_none());
    }
}
