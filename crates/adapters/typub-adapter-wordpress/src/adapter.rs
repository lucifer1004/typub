use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use std::collections::HashMap;
use std::path::PathBuf;
use typub_adapters_core::{
    AdapterContext, AdapterPayload, ContentInfo, OutputFormat, PlatformAdapter, PlatformBranding,
    RenderConfig, build_unified_preview, convert_png_math_for_strategy, debug, downcast_payload,
    ensure_no_unresolved_image_markers, image_utils, info, mock_materialize_and_resolve_urls,
    prepare_deferred_assets, render_config_for_png_math, resolve_asset_urls, warn,
};
use typub_config::{Config, PlatformConfig};
use typub_core::{AssetStrategy, MathRendering};
use typub_html::{SerializeOptions, document_to_html_with_options};
use typub_ir::Document;
use typub_storage::{PublishResult, build_resolved_url_map, to_data_uri};

use crate::client::WordPressClient;
use crate::config::{CAPABILITY, resolve_asset_strategy, resolve_math_rendering};
use crate::model::ID;
use crate::types::WordPressPayload;

pub struct WordPressAdapter {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    asset_strategy: AssetStrategy,
    math_rendering: MathRendering,
}

impl WordPressAdapter {
    pub fn new(config: &Config) -> Result<Self> {
        let platform_config = config.get_platform(ID);

        let base_url = platform_config
            .and_then(|c| c.get_str("base_url"))
            .unwrap_or_else(|| "https://example.com".to_string())
            .trim_end_matches('/')
            .to_string();

        let api_key = platform_config
            .and_then(|c| c.get_str("api_key"))
            .or_else(|| std::env::var("WORDPRESS_API_KEY").ok());

        let asset_strategy = resolve_asset_strategy(platform_config)?;
        let math_rendering = resolve_math_rendering(platform_config)?;

        Ok(Self {
            client: Client::new(),
            base_url,
            api_key,
            asset_strategy,
            math_rendering,
        })
    }

    fn client(&self) -> WordPressClient<'_> {
        WordPressClient::new(&self.client, &self.base_url, self.api_key.as_deref())
    }

    async fn build_asset_map(
        &self,
        content_info: &ContentInfo,
    ) -> Result<HashMap<PathBuf, String>> {
        let mut url_map = HashMap::new();
        for asset_path in &content_info.assets {
            let absolute_path = if asset_path.is_absolute() {
                asset_path.clone()
            } else {
                content_info.path.join(asset_path)
            };
            let url = match self.asset_strategy {
                AssetStrategy::Upload => self.client().upload_media(&absolute_path).await?,
                AssetStrategy::Embed => {
                    let data = std::fs::read(&absolute_path).with_context(|| {
                        format!(
                            "Failed to read local asset for wordpress: {}",
                            absolute_path.display()
                        )
                    })?;
                    to_data_uri(&data, &absolute_path)
                }
                AssetStrategy::Copy | AssetStrategy::External => {
                    if let Ok(rel) = asset_path.strip_prefix(&content_info.path) {
                        rel.to_string_lossy().replace('\\', "/")
                    } else if asset_path.is_relative() {
                        asset_path.to_string_lossy().replace('\\', "/")
                    } else {
                        absolute_path.to_string_lossy().replace('\\', "/")
                    }
                }
            };
            url_map.insert(asset_path.clone(), url);
        }
        Ok(url_map)
    }

    async fn upload_assets_impl(
        &self,
        pending: &typub_storage::PendingAssetList,
    ) -> Result<std::collections::HashMap<usize, String>> {
        let mut url_map = std::collections::HashMap::new();
        for asset in &pending.assets {
            let url = self.client().upload_media(&asset.local_path).await?;
            url_map.insert(asset.index, url);
        }
        Ok(url_map)
    }

    #[cfg(test)]
    pub(crate) fn new_for_test() -> WordPressAdapter {
        WordPressAdapter::new_for_test_with(
            "http://localhost",
            Some("token".to_string()),
            AssetStrategy::Upload,
            MathRendering::Svg,
        )
    }

    #[cfg(test)]
    pub(crate) fn new_for_test_with(
        base_url: &str,
        api_key: Option<String>,
        asset_strategy: AssetStrategy,
        math_rendering: MathRendering,
    ) -> WordPressAdapter {
        WordPressAdapter {
            client: Client::new(),
            base_url: base_url.to_string(),
            api_key,
            asset_strategy,
            math_rendering,
        }
    }
}

#[async_trait(?Send)]
impl PlatformAdapter for WordPressAdapter {
    fn id(&self) -> &'static str {
        ID
    }

    fn name(&self) -> &'static str {
        "WordPress"
    }

    fn required_format(&self) -> OutputFormat {
        OutputFormat::Html
    }

    fn asset_strategy(&self) -> AssetStrategy {
        self.asset_strategy
    }

    fn validate_config(&self, config: &PlatformConfig) -> Result<()> {
        let base_url = config.get_str("base_url").unwrap_or_default();
        if base_url.trim().is_empty() {
            anyhow::bail!("wordpress.base_url is required");
        }

        let api_key = config.get_str("api_key").unwrap_or_default();
        if api_key.trim().is_empty() && self.api_key.is_none() {
            anyhow::bail!(
                "WORDPRESS_API_KEY not set (configure wordpress.api_key or set WORDPRESS_API_KEY env var)"
            );
        }

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
        let slug = content_info
            .get_platform_str("slug")
            .unwrap_or_else(|| content_info.slug.clone());
        let tags = ctx.normalize_terms(&content_info.tags);
        let categories = ctx.normalize_terms(&content_info.categories);

        // Handle PNG math rendering based on asset strategy.
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
            WordPressPayload {
                title: content_info.title.clone(),
                slug,
                tags,
                categories,
                tag_ids: Vec::new(),
                category_ids: Vec::new(),
                final_body: None,
                existing_id: ctx.get_platform_id(&content_info.slug, ID)?,
            },
            content_info.clone(),
            deferred,
            elements,
        ))
    }

    async fn upload_assets(
        &self,
        pending: &typub_storage::PendingAssetList,
    ) -> Result<std::collections::HashMap<usize, String>> {
        self.upload_assets_impl(pending).await
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

        if payload.assets.needs_materialize() {
            match payload.assets.strategy {
                AssetStrategy::Upload => {
                    info!(
                        "[1/2] Uploading {} assets to WordPress...",
                        payload.assets.pending.assets.len()
                    );
                    let url_map = self.upload_assets_impl(&payload.assets.pending).await?;
                    payload.assets.resolved = url_map;
                    info!("[2/2] Assets uploaded");
                }
                AssetStrategy::External => {
                    let _storage_config = ctx.storage_config().ok_or_else(|| {
                        anyhow::anyhow!(
                            "External asset strategy requires [storage] configuration. See RFC-0004."
                        )
                    })?;
                    debug!("External asset strategy defers to pipeline for StatusTracker access");
                }
                _ => {}
            }

            if !payload.assets.resolved.is_empty() {
                let url_map = build_resolved_url_map(&payload.assets, &payload.content_info.path);
                resolve_asset_urls(&mut payload.document, &url_map);
            }
        }

        let inner = payload
            .downcast_mut::<WordPressPayload>()
            .ok_or_else(|| anyhow::anyhow!("Invalid WordPress payload type"))?;
        inner.tag_ids = self.client().resolve_tag_ids(&inner.tags).await?;
        inner.category_ids = self
            .client()
            .resolve_category_ids(&inner.categories)
            .await?;

        Ok(payload)
    }

    async fn serialize_payload(
        &self,
        mut payload: AdapterPayload,
        _ctx: &dyn AdapterContext,
    ) -> Result<AdapterPayload> {
        ensure_no_unresolved_image_markers(self.id(), self.asset_strategy, &payload.document)?;
        let serialize_options = SerializeOptions {
            use_code_highlight: CAPABILITY.code_highlight,
            ..Default::default()
        };
        let final_body = document_to_html_with_options(&payload.document, &serialize_options);

        let inner = payload
            .downcast_mut::<WordPressPayload>()
            .ok_or_else(|| anyhow::anyhow!("Invalid WordPress payload type"))?;
        inner.final_body = Some(final_body);

        Ok(payload)
    }

    async fn publish_payload(
        &self,
        payload: AdapterPayload,
        ctx: &dyn AdapterContext,
    ) -> Result<PublishResult> {
        let slug = payload.content_info.slug.clone();
        let payload = downcast_payload::<WordPressPayload>(payload, "WordPress")?;

        let final_body = payload.final_body.as_deref().unwrap_or("");

        let status = if ctx.published() { "publish" } else { "draft" };
        debug!("WordPress: resolved status='{}' for '{}'", status, slug);

        let update_target_id = if let Some(existing_id) = payload.existing_id.as_deref() {
            match self.client().find_post_by_id(existing_id).await? {
                Some((id, _)) => Some(id),
                None => {
                    warn!(
                        "Cached WordPress post id '{}' for slug '{}' not found; falling back to slug lookup",
                        existing_id, payload.slug
                    );
                    None
                }
            }
        } else {
            None
        };

        let update_target_id = match update_target_id {
            Some(id) => Some(id),
            None => self
                .client()
                .find_post_by_slug(&payload.slug)
                .await?
                .map(|(id, _url)| id),
        };

        let (post_id, url) = self
            .client()
            .upsert_post(
                update_target_id.as_deref(),
                &payload.title,
                &payload.slug,
                final_body,
                &payload.tag_ids,
                &payload.category_ids,
                status,
            )
            .await?;

        Ok(PublishResult {
            url: Some(url),
            platform_id: Some(post_id),
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
        if self.asset_strategy == AssetStrategy::Upload {
            warn!(
                "WordPress preview uses local file:// image URLs; publish uploads media and rewrites to remote URLs."
            );
        }

        // Use unified preview builder with MathJax support for LaTeX
        build_unified_preview(
            &elements,
            content_info,
            ID,
            "WordPress",
            None, // WordPress doesn't use theme CSS in preview
            Some(&PlatformBranding::new("#ffffff", "#21759b")),
        )
    }

    async fn check_status(&self, slug: &str) -> Result<bool> {
        if self.api_key.is_none() {
            return Ok(false);
        }
        Ok(self.client().find_post_by_slug(slug).await?.is_some())
    }
}
