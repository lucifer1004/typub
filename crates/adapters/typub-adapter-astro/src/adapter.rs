//! Astro adapter - outputs Markdown with YAML front-matter for Astro Content Collections.
//!
//! Per [[ADR-0009]], this adapter generates Markdown files compatible with
//! Astro's Content Collections system, enabling users to integrate typub output
//! into their Astro projects.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

use typub_adapters_core::{
    AdapterContext, AdapterPayload, ContentInfo, MarkdownRenderOptions, OutputFormat,
    PlatformAdapter, PlatformBranding, RenderConfig, build_unified_preview,
    convert_png_math_for_strategy, debug, document_to_markdown_with_options, downcast_payload,
    ensure_no_unresolved_image_markers, materialize_and_resolve_urls,
    mock_materialize_and_resolve_urls, prepare_deferred_assets, render_config_for_png_math,
    resolve_asset_urls,
};
use typub_config::Config;
use typub_core::{AssetStrategy, MathDelimiters, MathRendering};
use typub_ir::Document;
use typub_storage::{PublishResult, build_image_marker_url_map, to_data_uri};

use crate::config::{ID, resolve_math_rendering, resolve_strategy};

pub struct AstroAdapter {
    output_dir: PathBuf,
    asset_strategy: AssetStrategy,
    math_rendering: MathRendering,
}

#[derive(Debug)]
pub struct AstroPayload {
    pub slug: String,
    pub markdown: String,
    pub frontmatter: FrontMatter,
}

#[derive(Debug, Default)]
pub struct FrontMatter {
    pub title: String,
    pub date: Option<DateTime<Utc>>,
    pub draft: bool,
    pub tags: Vec<String>,
    pub categories: Vec<String>,
}

impl AstroAdapter {
    pub fn new(config: &Config) -> Result<Self> {
        let platform_config = config.get_platform(ID);

        let output_dir = platform_config
            .and_then(|c| c.get_str("output_dir"))
            .map(PathBuf::from)
            .unwrap_or_else(|| config.output_dir.join(ID));

        let asset_strategy = resolve_strategy(platform_config)?;
        let math_rendering = resolve_math_rendering(platform_config)?;

        Ok(Self {
            output_dir,
            asset_strategy,
            math_rendering,
        })
    }

    #[cfg(test)]
    #[allow(clippy::expect_used)]
    pub fn new_for_test() -> Self {
        Self::new_for_test_with(
            PathBuf::from("/tmp/astro"),
            AssetStrategy::Copy,
            MathRendering::Svg,
        )
    }

    #[cfg(test)]
    #[allow(clippy::expect_used)]
    pub fn new_for_test_with(
        output_dir: PathBuf,
        asset_strategy: AssetStrategy,
        math_rendering: MathRendering,
    ) -> Self {
        Self {
            output_dir,
            asset_strategy,
            math_rendering,
        }
    }

    async fn build_asset_map(
        &self,
        content_info: &ContentInfo,
    ) -> Result<HashMap<PathBuf, String>> {
        let mut url_map = HashMap::new();

        match self.asset_strategy {
            AssetStrategy::Copy => {
                let dest_dir = self.output_dir.join(&content_info.slug).join("assets");
                std::fs::create_dir_all(&dest_dir)?;

                for asset_path in &content_info.assets {
                    let file_name = asset_path.file_name().ok_or_else(|| {
                        anyhow::anyhow!("Invalid asset filename: {}", asset_path.display())
                    })?;
                    let dest_path = dest_dir.join(file_name);
                    std::fs::copy(asset_path, &dest_path)?;
                    url_map.insert(
                        asset_path.clone(),
                        format!("./assets/{}", file_name.to_string_lossy()),
                    );
                }
            }
            AssetStrategy::Embed => {
                for asset_path in &content_info.assets {
                    let data = std::fs::read(asset_path)?;
                    let data_uri = to_data_uri(&data, asset_path);
                    url_map.insert(asset_path.clone(), data_uri);
                }
            }
            AssetStrategy::External | AssetStrategy::Upload => {
                // These strategies require deferred upload, handled in materialize_payload
            }
        }

        Ok(url_map)
    }

    fn format_frontmatter(fm: &FrontMatter) -> String {
        let mut lines = vec!["---".to_string()];

        // Title - escape if contains special characters
        let title = if fm.title.contains(':') || fm.title.contains('\n') || fm.title.contains('"') {
            format!("title: {:?}", fm.title)
        } else {
            format!("title: {}", fm.title)
        };
        lines.push(title);

        if let Some(date) = &fm.date {
            lines.push(format!("date: {}", date.format("%Y-%m-%d")));
        }

        if fm.draft {
            lines.push("draft: true".to_string());
        }

        if !fm.tags.is_empty() {
            lines.push("tags:".to_string());
            for tag in &fm.tags {
                lines.push(format!("  - {}", tag));
            }
        }

        if !fm.categories.is_empty() {
            lines.push("categories:".to_string());
            for cat in &fm.categories {
                lines.push(format!("  - {}", cat));
            }
        }

        lines.push("---".to_string());
        lines.push(String::new()); // Empty line after frontmatter

        lines.join("\n")
    }
}

#[async_trait(?Send)]
impl PlatformAdapter for AstroAdapter {
    fn id(&self) -> &'static str {
        ID
    }

    fn name(&self) -> &'static str {
        "Astro Content Collection"
    }

    fn required_format(&self) -> OutputFormat {
        OutputFormat::Html // We still render HTML first, then convert to Markdown
    }

    fn asset_strategy(&self) -> AssetStrategy {
        self.asset_strategy
    }

    fn validate_config(&self, _config: &typub_config::PlatformConfig) -> Result<()> {
        Ok(())
    }

    fn render_config(&self, _content_info: &ContentInfo) -> RenderConfig {
        render_config_for_png_math(self.asset_strategy, self.math_rendering)
    }

    async fn specialize_payload(
        &self,
        mut elements: Document,
        ctx: &dyn AdapterContext,
    ) -> Result<AdapterPayload> {
        let content_info = ctx.content_info();
        let slug = content_info
            .get_platform_str("slug")
            .unwrap_or_else(|| content_info.slug.clone());

        // Handle PNG math rendering based on asset strategy
        (elements, _) = convert_png_math_for_strategy(
            elements,
            self.asset_strategy,
            self.math_rendering,
            &content_info.path,
            &content_info.slug,
        )?;

        // For strategies that don't need deferred upload, resolve assets immediately
        if !self.asset_strategy.requires_deferred_upload() {
            let url_map_raw = self.build_asset_map(content_info).await?;
            debug!(count = url_map_raw.len(), "Processed assets");
            let url_map = build_image_marker_url_map(&content_info.path, &url_map_raw);
            resolve_asset_urls(&mut elements, &url_map);
        }

        // Build deferred assets using helper
        let deferred = prepare_deferred_assets(self.asset_strategy, &elements, &content_info.path);

        // Build frontmatter from content info
        let frontmatter = FrontMatter {
            title: content_info.title.clone(),
            date: Some(Utc::now()),
            draft: false,
            tags: content_info.tags.clone(),
            categories: content_info.categories.clone(),
        };

        Ok(AdapterPayload::new(
            AstroPayload {
                slug,
                markdown: String::new(),
                frontmatter,
            },
            content_info.clone(),
            deferred,
            elements,
        ))
    }

    async fn materialize_payload(
        &self,
        mut payload: AdapterPayload,
        ctx: &dyn AdapterContext,
    ) -> Result<AdapterPayload> {
        // Dry-run mode: mock asset uploads by copying to temp dir
        if ctx.is_dry_run() {
            mock_materialize_and_resolve_urls(&mut payload, ctx)?;
            return Ok(payload);
        }

        materialize_and_resolve_urls(&mut payload, ctx).await?;
        Ok(payload)
    }

    async fn serialize_payload(
        &self,
        mut payload: AdapterPayload,
        _ctx: &dyn AdapterContext,
    ) -> Result<AdapterPayload> {
        ensure_no_unresolved_image_markers(self.id(), self.asset_strategy, &payload.document)?;

        // Convert HTML elements to Markdown
        let md_options = MarkdownRenderOptions {
            math_delimiters: MathDelimiters::Dollar,
            use_inline_html_for_sized_images: true,
            ..Default::default()
        };
        let md = document_to_markdown_with_options(&payload.document, &md_options)?;

        let inner = payload
            .downcast_mut::<AstroPayload>()
            .ok_or_else(|| anyhow::anyhow!("Invalid Astro publish payload type"))?;
        inner.markdown = md;
        Ok(payload)
    }

    async fn publish_payload(
        &self,
        payload: AdapterPayload,
        _ctx: &dyn AdapterContext,
    ) -> Result<PublishResult> {
        let payload = downcast_payload::<AstroPayload>(payload, "Astro")?;

        // Create output directory
        let dest_dir = self.output_dir.join(&payload.slug);
        std::fs::create_dir_all(&dest_dir)?;

        // Write Markdown file with frontmatter
        let frontmatter_str = Self::format_frontmatter(&payload.frontmatter);
        let content = format!("{}{}", frontmatter_str, payload.markdown);
        let md_path = dest_dir.join("index.md");
        std::fs::write(&md_path, &content)?;

        Ok(PublishResult {
            url: Some(format!(
                "file://{}",
                md_path.to_string_lossy().replace('\\', "/")
            )),
            platform_id: Some(payload.slug),
            published_at: Utc::now(),
        })
    }

    fn build_preview(
        &self,
        _title: &str,
        elements: Document,
        ctx: &dyn AdapterContext,
    ) -> Result<PathBuf> {
        let content_info = ctx.content_info();

        // Preview as rendered HTML (not Markdown) for browser viewing
        build_unified_preview(
            &elements,
            content_info,
            ID,
            "Astro",
            None,
            Some(&PlatformBranding::new("#ffffff", "#ff5d01")),
        )
    }

    async fn check_status(&self, slug: &str) -> Result<bool> {
        let dest_path = self.output_dir.join(slug).join("index.md");
        Ok(dest_path.exists())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_creation() {
        let adapter = AstroAdapter::new_for_test();
        assert_eq!(adapter.id(), "astro");
        assert_eq!(adapter.name(), "Astro Content Collection");
        assert_eq!(adapter.asset_strategy(), AssetStrategy::Copy);
    }

    #[test]
    fn test_adapter_with_external_strategy() {
        let adapter = AstroAdapter::new_for_test_with(
            PathBuf::from("/tmp/astro"),
            AssetStrategy::External,
            MathRendering::Svg,
        );
        assert_eq!(adapter.asset_strategy(), AssetStrategy::External);
        assert!(adapter.asset_strategy().requires_deferred_upload());
    }

    #[test]
    fn test_format_frontmatter() {
        let fm = FrontMatter {
            title: "Hello World".to_string(),
            date: Some(
                DateTime::parse_from_rfc3339("2026-02-17T12:00:00Z")
                    .expect("parse date")
                    .with_timezone(&Utc),
            ),
            draft: false,
            tags: vec!["rust".to_string(), "typst".to_string()],
            categories: vec!["programming".to_string()],
        };

        let yaml = AstroAdapter::format_frontmatter(&fm);
        assert!(yaml.starts_with("---\n"));
        assert!(yaml.contains("title:"));
        assert!(yaml.contains("Hello World"));
        assert!(yaml.contains("date: 2026-02-17"));
        assert!(yaml.contains("tags:"));
        assert!(yaml.contains("categories:"));
        // Should end with closing --- and newline for markdown content
        assert!(yaml.contains("---\n"));
        // Check there's a blank line after frontmatter
        let parts: Vec<&str> = yaml.split("---").collect();
        assert!(parts.len() >= 3, "Should have opening and closing ---");
    }

    #[test]
    fn test_format_frontmatter_draft() {
        let fm = FrontMatter {
            title: "Draft Post".to_string(),
            date: None,
            draft: true,
            tags: vec![],
            categories: vec![],
        };

        let yaml = AstroAdapter::format_frontmatter(&fm);
        assert!(yaml.contains("draft: true"));
        assert!(!yaml.contains("tags:"));
        assert!(!yaml.contains("categories:"));
    }
}
