use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;

use typub_adapters_core::{
    AdapterContext, AdapterPayload, ContentInfo, MarkdownRenderOptions, OutputFormat,
    PlatformAdapter, PlatformBranding, RenderConfig, build_unified_preview,
    convert_png_math_for_strategy, debug, document_to_markdown_with_options, downcast_payload,
    ensure_no_unresolved_image_markers, image_utils, materialize_and_resolve_urls,
    mock_materialize_and_resolve_urls, prepare_deferred_assets, render_config_for_png_math,
    resolve_asset_urls, warn,
};
use typub_config::Config;
use typub_core::{AssetStrategy, MathRendering};
use typub_ir::Document;
use typub_storage::{PublishResult, to_data_uri};

use crate::client::DevtoClient;
use crate::config::{ID, resolve_math_rendering, resolve_strategy};
use crate::model::DevtoPayload;

pub struct DevtoAdapter {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    asset_strategy: AssetStrategy,
    math_rendering: MathRendering,
}

impl DevtoAdapter {
    pub fn new(config: &Config) -> Result<Self> {
        let platform_config = config.get_platform(ID);
        let base_url = platform_config
            .and_then(|c| c.get_str("base_url"))
            .or_else(|| platform_config.and_then(|c| c.get_str("api_base")))
            .unwrap_or_else(|| "https://dev.to/api".to_string())
            .trim_end_matches('/')
            .to_string();
        let api_key = platform_config
            .and_then(|c| c.get_str("api_key"))
            .or_else(|| std::env::var("DEVTO_API_KEY").ok());

        let asset_strategy = resolve_strategy(platform_config)?;
        let math_rendering = resolve_math_rendering(platform_config)?;

        Ok(Self {
            client: Client::new(),
            base_url,
            api_key,
            asset_strategy,
            math_rendering,
        })
    }

    #[cfg(test)]
    pub fn new_for_test() -> Self {
        Self::new_for_test_with(
            "https://dev.to/api",
            None,
            AssetStrategy::External,
            MathRendering::Png,
        )
    }

    #[cfg(test)]
    pub fn new_for_test_with(
        base_url: &str,
        api_key: Option<String>,
        asset_strategy: AssetStrategy,
        math_rendering: MathRendering,
    ) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.to_string(),
            api_key,
            asset_strategy,
            math_rendering,
        }
    }

    fn client(&self, published: bool) -> DevtoClient<'_> {
        DevtoClient::new(
            &self.client,
            &self.base_url,
            self.api_key.as_deref(),
            published,
        )
    }

    async fn build_asset_map(
        &self,
        content_info: &ContentInfo,
    ) -> Result<HashMap<PathBuf, String>> {
        let mut map = HashMap::new();
        let mut warned = false;

        for asset in &content_info.assets {
            let full = if asset.is_absolute() {
                asset.clone()
            } else {
                content_info.path.join(asset)
            };
            let mapped = match self.asset_strategy {
                AssetStrategy::Embed => {
                    if !warned {
                        warn!(
                            "devto embed strategy is accepted by API but not reliably rendered by DEV; images may be dropped."
                        );
                        warned = true;
                    }
                    let data = std::fs::read(&full).with_context(|| {
                        format!("Failed to read local asset for devto: {}", full.display())
                    })?;
                    to_data_uri(&data, &full)
                }
                AssetStrategy::Copy => {
                    if !warned {
                        warn!(
                            "devto copy strategy keeps local paths; DEV cannot fetch local files."
                        );
                        warned = true;
                    }
                    if let Ok(rel) = asset.strip_prefix(&content_info.path) {
                        rel.to_string_lossy().replace('\\', "/")
                    } else if asset.is_relative() {
                        asset.to_string_lossy().replace('\\', "/")
                    } else {
                        full.to_string_lossy().replace('\\', "/")
                    }
                }
                AssetStrategy::Upload => {
                    anyhow::bail!("Upload strategy not supported for Dev.to")
                }
                AssetStrategy::External => {
                    if let Ok(rel) = asset.strip_prefix(&content_info.path) {
                        rel.to_string_lossy().replace('\\', "/")
                    } else if asset.is_relative() {
                        asset.to_string_lossy().replace('\\', "/")
                    } else {
                        full.to_string_lossy().replace('\\', "/")
                    }
                }
            };
            map.insert(asset.clone(), mapped);
        }
        Ok(map)
    }
}

#[async_trait(?Send)]
impl PlatformAdapter for DevtoAdapter {
    fn id(&self) -> &'static str {
        ID
    }

    fn name(&self) -> &'static str {
        "Dev.to"
    }

    fn required_format(&self) -> OutputFormat {
        OutputFormat::Html
    }

    fn asset_strategy(&self) -> AssetStrategy {
        self.asset_strategy
    }

    fn validate_config(&self, _config: &typub_config::PlatformConfig) -> Result<()> {
        let _ = self.client(true).auth_key()?;
        Ok(())
    }

    fn supports_shared_link_rewrite(&self) -> bool {
        true
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
        let normalized_tags = ctx.normalize_terms(&content_info.tags);
        let tags: Vec<String> = normalized_tags.into_iter().take(4).collect();
        let existing_id = ctx.get_platform_id(&content_info.slug, ID)?;

        // Handle PNG math rendering based on asset strategy.
        // Dev.to doesn't support LaTeX, so use PNG images.
        // Per [[WI-2026-02-17-002]].
        (elements, _) = convert_png_math_for_strategy(
            elements,
            self.asset_strategy,
            self.math_rendering,
            &content_info.path,
            &content_info.slug,
        )?;

        if !self.asset_strategy.requires_deferred_upload() {
            let asset_map = self.build_asset_map(content_info).await?;
            let url_map = image_utils::build_image_marker_url_map(&content_info.path, &asset_map);
            resolve_asset_urls(&mut elements, &url_map);
        }

        let deferred = prepare_deferred_assets(self.asset_strategy, &elements, &content_info.path);

        Ok(AdapterPayload::new(
            DevtoPayload {
                title: content_info.title.clone(),
                body_markdown: String::new(),
                tags,
                existing_id,
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
        // Dev.to supports inline HTML, so use <img> tags for sized images
        let md_options = MarkdownRenderOptions::default();
        let md = document_to_markdown_with_options(&payload.document, &md_options)?;
        let inner = payload
            .downcast_mut::<DevtoPayload>()
            .ok_or_else(|| anyhow::anyhow!("Invalid Dev.to payload type"))?;
        inner.body_markdown = md;
        Ok(payload)
    }

    async fn publish_payload(
        &self,
        payload: AdapterPayload,
        ctx: &dyn AdapterContext,
    ) -> Result<PublishResult> {
        use chrono::Utc;

        let slug = payload.content_info.slug.clone();
        let payload = downcast_payload::<DevtoPayload>(payload, "Dev.to")?;

        let published = ctx.published();
        debug!("Dev.to: resolved published={} for '{}'", published, slug);

        let article = if let Some(id) = payload.existing_id {
            match self
                .client(published)
                .update_article(&id, &payload.title, &payload.body_markdown, &payload.tags)
                .await?
            {
                Some(updated) => updated,
                None => {
                    warn!(
                        "Cached Dev.to article id '{}' for '{}' is no longer valid; trying title lookup",
                        id, payload.title
                    );
                    self.client(published)
                        .update_or_create_by_title(
                            &payload.title,
                            &payload.body_markdown,
                            &payload.tags,
                        )
                        .await?
                }
            }
        } else {
            self.client(published)
                .update_or_create_by_title(&payload.title, &payload.body_markdown, &payload.tags)
                .await?
        };

        Ok(PublishResult {
            url: Some(article.url),
            platform_id: Some(article.id.to_string()),
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
        warn!("Dev.to preview shows locally rewritten image links only.");

        // Use unified preview builder with MathJax support for LaTeX
        build_unified_preview(
            &elements,
            content_info,
            ID,
            "Dev.to",
            None, // Dev.to uses basic styling
            Some(&PlatformBranding::new("#ffffff", "#0a0a0a")),
        )
    }

    async fn check_status(&self, _slug: &str) -> Result<bool> {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_creation() {
        let adapter = DevtoAdapter::new_for_test();
        assert_eq!(adapter.id(), "devto");
        assert_eq!(adapter.name(), "Dev.to");
        assert_eq!(adapter.asset_strategy(), AssetStrategy::External);
    }

    #[test]
    fn test_required_format() {
        let adapter = DevtoAdapter::new_for_test();
        assert_eq!(adapter.required_format(), OutputFormat::Html);
    }

    #[test]
    fn test_supports_shared_link_rewrite() {
        let adapter = DevtoAdapter::new_for_test();
        assert!(adapter.supports_shared_link_rewrite());
    }

    #[test]
    fn test_render_config() {
        let adapter = DevtoAdapter::new_for_test();
        let config = adapter.render_config(&ContentInfo::minimal("T", "s", "/p"));
        assert!(config.image_as_marker);
        // Dev.to uses PNG math (default), render_config returns Svg for PNG conversion
        assert_eq!(config.math_rendering, typub_core::MathRendering::Svg);
    }
}
