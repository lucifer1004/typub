use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;

use typub_adapters_core::MarkdownRenderOptions;
use typub_adapters_core::{
    AdapterContext, AdapterPayload, ContentInfo, OutputFormat, PlatformAdapter, PlatformBranding,
    RenderConfig, build_unified_preview, debug, default_render_config_for,
    document_to_markdown_with_options, downcast_payload, ensure_no_unresolved_image_markers,
    image_utils, materialize_and_resolve_urls, mock_materialize_and_resolve_urls,
    prepare_deferred_assets, resolve_asset_urls, warn,
};
use typub_config::Config;
use typub_core::AssetStrategy;
use typub_ir::Document;

use crate::client::HashnodeClient;
use crate::config::{CAPABILITY, ID, resolve_strategy};
use crate::model::HashnodePayload;

pub struct HashnodeAdapter {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    publication_id: Option<String>,
    publication_host: Option<String>,
    asset_strategy: AssetStrategy,
}

impl HashnodeAdapter {
    pub fn new(config: &Config) -> Result<Self> {
        let platform_config = config.get_platform(ID);
        let base_url = platform_config
            .and_then(|c| c.get_str("base_url"))
            .or_else(|| platform_config.and_then(|c| c.get_str("api_base")))
            .unwrap_or_else(|| "https://gql.hashnode.com".to_string())
            .trim_end_matches('/')
            .to_string();
        let api_key = platform_config
            .and_then(|c| c.get_str("api_key"))
            .or_else(|| std::env::var("HASHNODE_API_KEY").ok());
        let publication_id = platform_config
            .and_then(|c| c.get_str("publication_id"))
            .or_else(|| std::env::var("HASHNODE_PUBLICATION_ID").ok());
        let publication_host = platform_config
            .and_then(|c| c.get_str("publication_host"))
            .or_else(|| std::env::var("HASHNODE_PUBLICATION_HOST").ok());

        let asset_strategy = resolve_strategy(platform_config)?;

        Ok(Self {
            client: Client::new(),
            base_url,
            api_key,
            publication_id,
            publication_host,
            asset_strategy,
        })
    }

    #[cfg(test)]
    pub fn new_for_test() -> Self {
        Self::new_for_test_with(
            "https://gql.hashnode.com",
            None,
            None,
            None,
            AssetStrategy::External,
        )
    }

    #[cfg(test)]
    pub fn new_for_test_with(
        base_url: &str,
        api_key: Option<String>,
        publication_id: Option<String>,
        publication_host: Option<String>,
        asset_strategy: AssetStrategy,
    ) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.to_string(),
            api_key,
            publication_id,
            publication_host,
            asset_strategy,
        }
    }

    fn client(&self, published: bool) -> HashnodeClient<'_> {
        HashnodeClient::new(
            &self.client,
            &self.base_url,
            self.api_key.as_deref(),
            self.publication_id.as_deref(),
            self.publication_host.as_deref(),
            published,
        )
    }

    async fn build_asset_map(
        &self,
        content_info: &ContentInfo,
    ) -> Result<HashMap<PathBuf, String>> {
        let mut map = HashMap::new();

        for asset in &content_info.assets {
            let full = if asset.is_absolute() {
                asset.clone()
            } else {
                content_info.path.join(asset)
            };
            let mapped = match self.asset_strategy {
                AssetStrategy::External => {
                    if let Ok(rel) = asset.strip_prefix(&content_info.path) {
                        rel.to_string_lossy().replace('\\', "/")
                    } else if asset.is_relative() {
                        asset.to_string_lossy().replace('\\', "/")
                    } else {
                        full.to_string_lossy().replace('\\', "/")
                    }
                }
                _ => unreachable!("unsupported strategy for hashnode"),
            };
            map.insert(asset.clone(), mapped);
        }
        Ok(map)
    }
}

#[async_trait(?Send)]
impl PlatformAdapter for HashnodeAdapter {
    fn id(&self) -> &'static str {
        ID
    }

    fn name(&self) -> &'static str {
        "Hashnode"
    }

    fn required_format(&self) -> OutputFormat {
        OutputFormat::Html
    }

    fn asset_strategy(&self) -> AssetStrategy {
        self.asset_strategy
    }

    fn validate_config(&self, _config: &typub_config::PlatformConfig) -> Result<()> {
        let _ = self.client(true).auth_key()?;
        let _ = self.client(true).publication_id()?;
        Ok(())
    }

    fn supports_shared_link_rewrite(&self) -> bool {
        true
    }

    fn render_config(&self, _content_info: &ContentInfo) -> RenderConfig {
        default_render_config_for(self.asset_strategy, &CAPABILITY)
    }

    async fn specialize_payload(
        &self,
        mut elements: Document,
        ctx: &dyn AdapterContext,
    ) -> Result<AdapterPayload> {
        let content_info = ctx.content_info();
        let normalized_tags = ctx.normalize_terms(&content_info.tags);
        let tags: Vec<String> = normalized_tags.into_iter().take(5).collect();
        let existing_id = ctx.get_platform_id(&content_info.slug, ID)?;

        if !self.asset_strategy.requires_deferred_upload() {
            let asset_map = self.build_asset_map(content_info).await?;
            let url_map = image_utils::build_image_marker_url_map(&content_info.path, &asset_map);
            resolve_asset_urls(&mut elements, &url_map);
        }

        let deferred = prepare_deferred_assets(self.asset_strategy, &elements, &content_info.path);

        Ok(AdapterPayload::new(
            HashnodePayload {
                title: content_info.title.clone(),
                content_markdown: String::new(),
                tags,
                existing_id,
                slug: content_info.slug.clone(),
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
        // Dry-run mode: generate mock URLs without file I/O
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
        let md_options = MarkdownRenderOptions {
            math_delimiters: CAPABILITY.default_math_delimiter(),
            ..Default::default()
        };
        let md = document_to_markdown_with_options(&payload.document, &md_options)?;
        let inner = payload
            .downcast_mut::<HashnodePayload>()
            .ok_or_else(|| anyhow::anyhow!("Invalid HashNode payload type"))?;
        inner.content_markdown = md;
        Ok(payload)
    }

    async fn publish_payload(
        &self,
        payload: AdapterPayload,
        ctx: &dyn AdapterContext,
    ) -> Result<typub_storage::PublishResult> {
        let slug = payload.content_info.slug.clone();
        let payload = downcast_payload::<HashnodePayload>(payload, "HashNode")?;

        let published = ctx.published();
        debug!("Hashnode: resolved published={} for '{}'", published, slug);

        let post = if let Some(id) = payload.existing_id {
            match self
                .client(published)
                .update_post(
                    &id,
                    &payload.title,
                    &payload.content_markdown,
                    &payload.tags,
                )
                .await?
            {
                Some(updated) => updated,
                None => {
                    warn!(
                        "Cached HashNode id '{}' for '{}' is no longer valid",
                        id, payload.title
                    );
                    if published {
                        debug!("Trying slug lookup for published post");
                        self.client(published)
                            .update_or_create_by_slug(
                                &payload.slug,
                                &payload.title,
                                &payload.content_markdown,
                                &payload.tags,
                            )
                            .await?
                    } else {
                        debug!("Creating new draft (drafts have no slug lookup)");
                        self.client(published)
                            .publish_post(&payload.title, &payload.content_markdown, &payload.tags)
                            .await?
                    }
                }
            }
        } else if published {
            self.client(published)
                .update_or_create_by_slug(
                    &payload.slug,
                    &payload.title,
                    &payload.content_markdown,
                    &payload.tags,
                )
                .await?
        } else {
            self.client(published)
                .publish_post(&payload.title, &payload.content_markdown, &payload.tags)
                .await?
        };

        Ok(typub_storage::PublishResult {
            url: Some(post.url),
            platform_id: Some(post.id),
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

        // Use unified preview builder with MathJax support for LaTeX
        build_unified_preview(
            &elements,
            content_info,
            ID,
            "Hashnode",
            None, // Hashnode uses basic styling
            Some(&PlatformBranding::new("#ffffff", "#2962ff")),
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
        let adapter = HashnodeAdapter::new_for_test();
        assert_eq!(adapter.id(), "hashnode");
        assert_eq!(adapter.name(), "Hashnode");
        assert_eq!(adapter.asset_strategy(), AssetStrategy::External);
    }

    #[test]
    fn test_required_format() {
        let adapter = HashnodeAdapter::new_for_test();
        assert_eq!(adapter.required_format(), OutputFormat::Html);
    }

    #[test]
    fn test_supports_shared_link_rewrite() {
        let adapter = HashnodeAdapter::new_for_test();
        assert!(adapter.supports_shared_link_rewrite());
    }

    #[test]
    fn test_render_config() {
        let adapter = HashnodeAdapter::new_for_test();
        let config = adapter.render_config(&ContentInfo::minimal("T", "s", "/p"));
        assert!(config.image_as_marker);
        assert_eq!(config.math_rendering, typub_core::MathRendering::Latex);
    }
}
