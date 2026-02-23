use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use serde_json::{Value, json};
use std::path::PathBuf;
use typub_adapters_core::{
    AdapterContext, AdapterPayload, ContentInfo, OutputFormat, PlatformAdapter, PlatformBranding,
    build_unified_preview, downcast_payload, ensure_no_unresolved_image_markers,
    materialize_and_resolve_urls, mock_materialize_and_resolve_urls, prepare_deferred_assets,
    resolve_asset_urls, warn,
};
use typub_config::{Config, PlatformConfig};
use typub_core::AssetStrategy;
use typub_ir::Document;
use typub_storage::{PublishResult, build_resolved_url_map, mime_type_from_path};

use crate::blocks;
use crate::client::{NOTION_API_BASE, NotionClient};
use crate::config::{render_config_for, resolve_asset_strategy};
use crate::model::{DESIRED_TITLE_PROPERTY, ID, NotionPayload, NotionSchema};

pub struct NotionAdapter {
    http_client: Client,
    api_base: String,
    api_key: Option<String>,
    data_source_id: Option<String>,
    has_token: bool,
    tags_property: String,
    asset_strategy: AssetStrategy,
}

impl NotionAdapter {
    pub fn new(config: &Config) -> Result<Self> {
        let platform_config = config.get_platform(ID);

        let data_source_id = platform_config.and_then(|c| c.get_str("data_source_id"));

        let api_key = platform_config
            .and_then(|c| c.get_str("api_key"))
            .or_else(|| std::env::var("NOTION_API_KEY").ok());
        let has_token = api_key.is_some();
        let tags_property = platform_config
            .and_then(|c| c.get_str("tags_property"))
            .unwrap_or_else(|| "Tags".to_string());
        let asset_strategy = resolve_asset_strategy(platform_config)?;

        Ok(Self {
            http_client: Client::new(),
            api_base: NOTION_API_BASE.to_string(),
            api_key,
            data_source_id,
            has_token,
            tags_property,
            asset_strategy,
        })
    }

    fn client(&self) -> NotionClient<'_> {
        NotionClient::new(
            &self.http_client,
            &self.api_base,
            self.api_key.as_deref().unwrap_or(""),
        )
    }

    pub(crate) async fn find_existing_page(
        &self,
        data_source_id: &str,
        title_property: &str,
        title: &str,
    ) -> Result<Option<String>> {
        let filter = json!({
            "filter": {
                "property": title_property,
                "title": { "equals": title }
            }
        });

        let result = self
            .client()
            .query_data_source(data_source_id, filter)
            .await?;

        if let Some(pages) = result["results"].as_array()
            && let Some(page) = pages.first()
            && let Some(id) = page["id"].as_str()
        {
            return Ok(Some(id.to_string()));
        }

        Ok(None)
    }

    async fn upload_file(&self, file_path: &std::path::Path, filename: &str) -> Result<String> {
        let file_data = std::fs::read(file_path)
            .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

        let content_type = mime_type_from_path(file_path);

        let (upload_id, upload_url) = self
            .client()
            .create_file_upload(filename, content_type)
            .await?;

        self.client()
            .send_file_upload(&upload_url, file_data, filename, content_type)
            .await?;

        Ok(upload_id)
    }

    async fn try_update_page(
        &self,
        page_id: &str,
        properties: &Value,
        blocks: &[Value],
    ) -> Result<()> {
        self.client()
            .update_page_properties(page_id, properties.clone())
            .await?;
        self.client().erase_page_content(page_id).await?;
        self.client().append_block_children(page_id, blocks).await?;
        Ok(())
    }

    async fn fallback_update_or_create(
        &self,
        data_source_id: &str,
        title_property: &str,
        title: &str,
        properties: Value,
        blocks: &[Value],
    ) -> Result<(String, String)> {
        if let Some(fallback_id) = self
            .find_existing_page(data_source_id, title_property, title)
            .await?
        {
            self.try_update_page(&fallback_id, &properties, blocks)
                .await?;
            let url = format!("https://www.notion.so/{}", fallback_id.replace("-", ""));
            Ok((fallback_id, url))
        } else {
            self.create_page_with_blocks(data_source_id, properties, blocks)
                .await
        }
    }

    pub(crate) async fn create_page_with_blocks(
        &self,
        data_source_id: &str,
        properties: Value,
        blocks: &[Value],
    ) -> Result<(String, String)> {
        const BLOCK_LIMIT: usize = 100;
        let (initial, remaining) = if blocks.len() > BLOCK_LIMIT {
            blocks.split_at(BLOCK_LIMIT)
        } else {
            (blocks, [].as_slice())
        };

        let result = self
            .client()
            .create_page(data_source_id, properties, initial)
            .await?;

        let page_id = result["id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No page ID in response"))?
            .to_string();
        let url = result["url"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No URL in response"))?
            .to_string();

        if !remaining.is_empty() {
            self.client()
                .append_block_children(&page_id, remaining)
                .await?;
        }

        Ok((page_id, url))
    }

    pub(crate) fn normalized_tags(tags: &[String]) -> Vec<String> {
        let mut normalized: Vec<String> = tags
            .iter()
            .map(|tag| tag.trim())
            .filter(|tag| !tag.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        normalized.sort_by_key(|tag| tag.to_lowercase());
        normalized.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        normalized
    }

    pub(crate) fn build_properties(
        &self,
        schema: &NotionSchema,
        title: &str,
        tags: &[String],
    ) -> Value {
        let title_text = json!([{
            "text": { "content": title }
        }]);
        let normalized_tags = Self::normalized_tags(tags);
        let tags_values: Vec<Value> = normalized_tags
            .into_iter()
            .map(|name| json!({ "name": name }))
            .collect();

        json!({
            &schema.title_property: { "title": title_text },
            &schema.tags_property: { "multi_select": tags_values },
        })
    }

    pub(crate) async fn ensure_data_source_schema(
        &self,
        data_source_id: &str,
    ) -> Result<NotionSchema> {
        let data_source = self.client().get_data_source(data_source_id).await?;
        let properties = data_source["properties"]
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("Notion data source properties missing"))?;

        let mut title_property_id: Option<String> = None;
        for (_name, prop) in properties {
            if prop["type"].as_str() == Some("title") {
                title_property_id = prop["id"].as_str().map(ToOwned::to_owned);
                break;
            }
        }

        if let Some(existing_title) = properties.get(DESIRED_TITLE_PROPERTY) {
            if existing_title["type"].as_str() != Some("title") {
                anyhow::bail!(
                    "Notion property '{}' exists but is type '{}'; expected type 'title'",
                    DESIRED_TITLE_PROPERTY,
                    existing_title["type"].as_str().unwrap_or("unknown")
                );
            }
        } else if let Some(property_id) = title_property_id {
            self.client()
                .update_data_source(
                    data_source_id,
                    json!({
                        "properties": {
                            property_id: { "name": DESIRED_TITLE_PROPERTY }
                        }
                    }),
                )
                .await?;
        } else {
            self.client()
                .update_data_source(
                    data_source_id,
                    json!({
                        "properties": {
                            DESIRED_TITLE_PROPERTY: { "title": {} }
                        }
                    }),
                )
                .await?;
        }

        if let Some(tags_prop) = properties.get(&self.tags_property) {
            if tags_prop["type"].as_str() != Some("multi_select") {
                anyhow::bail!(
                    "Notion property '{}' exists but is type '{}'; expected type 'multi_select'",
                    self.tags_property,
                    tags_prop["type"].as_str().unwrap_or("unknown")
                );
            }
        } else {
            self.client()
                .update_data_source(
                    data_source_id,
                    json!({
                        "properties": {
                            &self.tags_property: { "multi_select": {} }
                        }
                    }),
                )
                .await?;
        }

        Ok(NotionSchema {
            title_property: DESIRED_TITLE_PROPERTY.to_string(),
            tags_property: self.tags_property.clone(),
        })
    }

    #[cfg(test)]
    pub(crate) fn new_for_test() -> NotionAdapter {
        NotionAdapter::new_for_test_with(
            "http://localhost",
            true,
            Some("ds-1".to_string()),
            AssetStrategy::Upload,
        )
    }

    #[cfg(test)]
    pub(crate) fn new_for_test_with(
        api_base: &str,
        has_token: bool,
        data_source_id: Option<String>,
        asset_strategy: AssetStrategy,
    ) -> NotionAdapter {
        NotionAdapter {
            http_client: Client::new(),
            api_base: api_base.to_string(),
            api_key: if has_token {
                Some("test-token".to_string())
            } else {
                None
            },
            data_source_id,
            has_token,
            tags_property: "Tags".to_string(),
            asset_strategy,
        }
    }
}

#[async_trait(?Send)]
impl PlatformAdapter for NotionAdapter {
    fn id(&self) -> &'static str {
        ID
    }

    fn name(&self) -> &'static str {
        "Notion"
    }

    fn required_format(&self) -> OutputFormat {
        OutputFormat::Html
    }

    fn asset_strategy(&self) -> AssetStrategy {
        self.asset_strategy
    }

    fn render_config(&self, _content_info: &ContentInfo) -> typub_adapters_core::RenderConfig {
        render_config_for(self.asset_strategy)
    }

    fn validate_config(&self, _config: &PlatformConfig) -> Result<()> {
        if !self.has_token {
            anyhow::bail!(
                "NOTION_API_KEY not set (configure notion.api_key or set NOTION_API_KEY env var)"
            );
        }
        if self.data_source_id.is_none() {
            anyhow::bail!("notion.data_source_id not configured");
        }
        Ok(())
    }

    fn supports_shared_link_rewrite(&self) -> bool {
        true
    }

    async fn specialize_payload(
        &self,
        document: Document,
        ctx: &dyn AdapterContext,
    ) -> Result<AdapterPayload> {
        let content_info = ctx.content_info();
        let data_source_id = self
            .data_source_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("notion.data_source_id not configured"))?;

        let deferred = prepare_deferred_assets(self.asset_strategy, &document, &content_info.path);

        Ok(AdapterPayload::new(
            NotionPayload {
                data_source_id: data_source_id.clone(),
                title: content_info.title.clone(),
                existing_page_id: ctx.get_platform_id(&content_info.slug, ID)?,
                schema: None,
                blocks: Vec::new(),
            },
            content_info.clone(),
            deferred,
            document,
        ))
    }

    async fn provision_target(
        &self,
        mut payload: AdapterPayload,
        _ctx: &dyn AdapterContext,
    ) -> Result<AdapterPayload> {
        let inner = payload
            .downcast_mut::<NotionPayload>()
            .ok_or_else(|| anyhow::anyhow!("Invalid Notion payload type"))?;
        inner.schema = Some(
            self.ensure_data_source_schema(&inner.data_source_id)
                .await?,
        );
        Ok(payload)
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
                    // Notion uses its native file upload API
                    let mut resolved = std::collections::HashMap::new();
                    for asset in &payload.assets.pending.assets {
                        let filename = asset
                            .local_path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("image");
                        let upload_id = self
                            .upload_file(&asset.local_path, filename)
                            .await
                            .with_context(|| {
                                format!(
                                    "Failed to upload image '{}' to Notion",
                                    asset.local_path.display()
                                )
                            })?;
                        resolved.insert(asset.index, upload_id);
                    }
                    payload.assets.resolved = resolved;
                }
                AssetStrategy::External => {
                    // External uses S3 storage with standard helper
                    materialize_and_resolve_urls(&mut payload, ctx).await?;
                    return Ok(payload);
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
        ensure_no_unresolved_image_markers(self.id(), self.asset_strategy, &payload.document)?;
        let marker_map = build_resolved_url_map(&payload.assets, &payload.content_info.path);
        let notion_blocks = blocks::document_to_blocks(&payload.document, &marker_map);

        let inner = payload
            .downcast_mut::<NotionPayload>()
            .ok_or_else(|| anyhow::anyhow!("Invalid Notion payload type"))?;
        inner.blocks = notion_blocks;
        Ok(payload)
    }

    async fn publish_payload(
        &self,
        payload: AdapterPayload,
        _ctx: &dyn AdapterContext,
    ) -> Result<PublishResult> {
        let content_info = payload.content_info.clone();
        let payload = downcast_payload::<NotionPayload>(payload, "Notion")?;
        let schema = payload
            .schema
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Notion schema not provisioned"))?;
        let properties = self.build_properties(schema, &content_info.title, &content_info.tags);
        let blocks = &payload.blocks;

        let existing_page_id = if payload.existing_page_id.is_some() {
            payload.existing_page_id.clone()
        } else {
            self.find_existing_page(
                &payload.data_source_id,
                &schema.title_property,
                &payload.title,
            )
            .await?
        };

        let (page_id, url) = if let Some(page_id) = existing_page_id {
            match self.try_update_page(&page_id, &properties, blocks).await {
                Ok(()) => {
                    let page_url = format!("https://www.notion.so/{}", page_id.replace("-", ""));
                    (page_id, page_url)
                }
                Err(update_err) if payload.existing_page_id.is_some() => {
                    warn!(
                        "Cached Notion page id '{}' update failed ({}); attempting title lookup fallback",
                        page_id, update_err
                    );
                    self.fallback_update_or_create(
                        &payload.data_source_id,
                        &schema.title_property,
                        &payload.title,
                        properties,
                        blocks,
                    )
                    .await?
                }
                Err(update_err) => return Err(update_err),
            }
        } else {
            self.create_page_with_blocks(&payload.data_source_id, properties, blocks)
                .await?
        };

        Ok(PublishResult {
            url: Some(url),
            platform_id: Some(page_id),
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
            "Notion",
            None, // Notion uses custom inline styles
            Some(&PlatformBranding::new("#ffffff", "#000000")),
        )
    }

    async fn check_status(&self, _slug: &str) -> Result<bool> {
        if !self.has_token {
            return Ok(false);
        }
        Ok(false)
    }
}
