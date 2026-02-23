use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use std::collections::HashMap;
use std::path::PathBuf;
use typub_adapters_core::{
    AdapterContext, AdapterPayload, ContentInfo, OutputFormat, PlatformAdapter, PlatformBranding,
    RenderConfig, build_unified_preview, convert_png_math_for_strategy, downcast_payload,
    mock_materialize_and_resolve_urls, prepare_deferred_assets, render_config_for_png_math,
    resolve_asset_urls, warn,
};
use typub_assets_ast::ensure_no_unresolved_image_markers;
use typub_config::{Config, PlatformConfig};
use typub_core::{AssetStrategy, MathRendering};
use typub_html::{SerializeOptions, document_to_html_with_options};
use typub_ir::Document;
use typub_storage::{
    PendingAssetList, PublishResult, build_image_marker_url_map, build_resolved_url_map,
    to_data_uri,
};

use crate::client::GhostClient;
use crate::config::{resolve_asset_strategy, resolve_math_rendering};
use crate::model::{GhostPayload, ID};

pub struct GhostAdapter {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    asset_strategy: AssetStrategy,
    math_rendering: MathRendering,
}

impl GhostAdapter {
    pub fn new(config: &Config) -> Result<Self> {
        let platform_config = config.get_platform(ID);
        let base_url = platform_config
            .and_then(|c| c.get_str("base_url"))
            .or_else(|| platform_config.and_then(|c| c.get_str("api_base")))
            .unwrap_or_else(|| "http://localhost:2368".to_string())
            .trim_end_matches('/')
            .to_string();
        let api_key = platform_config
            .and_then(|c| c.get_str("api_key"))
            .or_else(|| std::env::var("GHOST_ADMIN_API_KEY").ok());

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

    #[cfg(test)]
    pub(crate) fn new_for_test() -> GhostAdapter {
        GhostAdapter::new_for_test_with(
            "http://localhost:2368",
            Some("ghost_api_key".to_string()),
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
    ) -> GhostAdapter {
        GhostAdapter {
            client: Client::new(),
            base_url: base_url.to_string(),
            api_key,
            asset_strategy,
            math_rendering,
        }
    }

    fn client(&self, published: bool) -> GhostClient<'_> {
        GhostClient::new(
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

        for asset in &content_info.assets {
            let full = if asset.is_absolute() {
                asset.clone()
            } else {
                content_info.path.join(asset)
            };
            let mapped = match self.asset_strategy {
                AssetStrategy::Embed => {
                    let data = std::fs::read(&full).with_context(|| {
                        format!("Failed to read local asset for ghost: {}", full.display())
                    })?;
                    to_data_uri(&data, &full)
                }
                _ => {
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

    async fn upload_assets(&self, pending: &PendingAssetList) -> Result<HashMap<usize, String>> {
        let client = self.client(false);
        let mut url_map = HashMap::new();
        for asset in &pending.assets {
            let url = client.upload_image(&asset.local_path).await?;
            url_map.insert(asset.index, url);
        }
        Ok(url_map)
    }

    fn html_to_lexical(html: &str) -> Result<String> {
        let escaped_html = serde_json::to_string(html)
            .context("Failed to encode HTML for Ghost Lexical payload")?;
        Ok(format!(
            r#"{{"root":{{"children":[{{"type":"html","version":1,"html":{}}}],"direction":"ltr","format":"","indent":0,"type":"root","version":1}}}}"#,
            escaped_html
        ))
    }
}

#[async_trait(?Send)]
impl PlatformAdapter for GhostAdapter {
    fn id(&self) -> &'static str {
        ID
    }

    fn name(&self) -> &'static str {
        "Ghost"
    }

    fn required_format(&self) -> OutputFormat {
        OutputFormat::Html
    }

    fn asset_strategy(&self) -> AssetStrategy {
        self.asset_strategy
    }

    fn validate_config(&self, _config: &PlatformConfig) -> Result<()> {
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
        let tags: Vec<String> = normalized_tags.into_iter().take(10).collect();
        let existing_id = ctx.get_platform_id(&content_info.slug, ID)?;

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
            let url_map = build_image_marker_url_map(&content_info.path, &asset_map);
            resolve_asset_urls(&mut elements, &url_map);
        }

        // Use helper for deferred assets (handles both deferred and immediate strategies)
        let deferred = prepare_deferred_assets(self.asset_strategy, &elements, &content_info.path);

        Ok(AdapterPayload::new(
            GhostPayload {
                title: content_info.title.clone(),
                lexical: None,
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
        // Dry-run mode: generate mock URLs without file I/O
        if ctx.is_dry_run() {
            mock_materialize_and_resolve_urls(&mut payload, ctx)?;
            return Ok(payload);
        }

        if payload.assets.needs_materialize() {
            match payload.assets.strategy {
                AssetStrategy::Upload => {
                    // Ghost uses its native upload API, not S3
                    let url_map = self.upload_assets(&payload.assets.pending).await?;
                    payload.assets.resolved = url_map;
                }
                AssetStrategy::External => {
                    // External uses S3 storage with standard helper
                    let storage_config = ctx.storage_config().ok_or_else(|| {
                        anyhow::anyhow!(
                            "External asset strategy requires [storage] configuration. See RFC-0004."
                        )
                    })?;
                    typub_storage::materialize_external_assets(&mut payload.assets, storage_config)
                        .await?;
                }
                _ => {}
            }

            if !payload.assets.resolved.is_empty() {
                let url_map = build_resolved_url_map(&payload.assets, &payload.content_info.path);
                resolve_asset_urls(&mut payload.document, &url_map);
            }
        }

        Ok(payload)
    }

    async fn serialize_payload(
        &self,
        mut payload: AdapterPayload,
        _ctx: &dyn AdapterContext,
    ) -> Result<AdapterPayload> {
        // Ensure all local assets have resolved publish URLs before serialize
        // Per [[RFC-0009:C-ASSET-REFERENCE]], assets referenced by ID must have resolved variants
        ensure_no_unresolved_image_markers(ID, self.asset_strategy, &payload.document)
            .context("[{ID}] Serialize stage validation")?;

        let serialize_options = SerializeOptions {
            use_code_highlight: crate::config::CAPABILITY.code_highlight,
            ..Default::default()
        };
        let html = document_to_html_with_options(&payload.document, &serialize_options);
        let lexical = Self::html_to_lexical(&html)?;

        let inner = payload
            .downcast_mut::<GhostPayload>()
            .ok_or_else(|| anyhow::anyhow!("Invalid Ghost payload type"))?;
        inner.lexical = Some(lexical);
        Ok(payload)
    }

    async fn publish_payload(
        &self,
        payload: AdapterPayload,
        ctx: &dyn AdapterContext,
    ) -> Result<PublishResult> {
        let payload = downcast_payload::<GhostPayload>(payload, "Ghost")?;
        let published = ctx.published();
        let lexical = payload.lexical.as_deref().unwrap_or("");

        let post = if let Some(id) = payload.existing_id {
            match self.client(published).get_post(&id).await? {
                Some(current) => {
                    let updated_at = current
                        .updated_at
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("Ghost post missing updated_at field"))?;

                    match self
                        .client(published)
                        .update_post(&id, &payload.title, lexical, &payload.tags, updated_at)
                        .await?
                    {
                        Some(updated) => updated,
                        None => {
                            warn!(
                                "Ghost post id '{}' returned 404 during update; trying title lookup",
                                id
                            );
                            self.client(published)
                                .update_or_create_by_title(&payload.title, lexical, &payload.tags)
                                .await?
                        }
                    }
                }
                None => {
                    warn!(
                        "Cached Ghost post id '{}' for '{}' is no longer valid; trying title lookup",
                        id, payload.title
                    );
                    self.client(published)
                        .update_or_create_by_title(&payload.title, lexical, &payload.tags)
                        .await?
                }
            }
        } else {
            self.client(published)
                .update_or_create_by_title(&payload.title, lexical, &payload.tags)
                .await?
        };

        Ok(PublishResult {
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
        build_unified_preview(
            &elements,
            content_info,
            ID,
            "Ghost",
            None,
            Some(&PlatformBranding::new("#ffffff", "#15171A")),
        )
    }

    async fn check_status(&self, _slug: &str) -> Result<bool> {
        Ok(false)
    }
}
