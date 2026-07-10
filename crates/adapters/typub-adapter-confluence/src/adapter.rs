use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use reqwest::multipart::{Form, Part};

use typub_adapters_core::{
    AdapterContext, AdapterPayload, ContentInfo, OutputFormat, PlatformAdapter, PlatformBranding,
    RenderConfig, build_unified_preview, convert_png_math_for_strategy, debug, downcast_payload,
    ensure_no_unresolved_image_markers, info, mock_materialize_and_resolve_urls,
    prepare_deferred_assets, render_config_for_png_math, resolve_asset_urls, warn,
};
use typub_config::Config;
use typub_core::{AssetStrategy, MathRendering};
use typub_ir::Document;
use typub_storage::{PublishResult, build_resolved_url_map, mime_type_from_path};
use typub_theme::{Theme, ThemeRegistry, load_theme};

use crate::config::{ID, resolve_math_rendering, resolve_strategy};
use crate::format;
use crate::model::*;

pub struct ConfluenceAdapter {
    client: Client,
    base_url: String,
    default_space: String,
    default_parent_id: Option<String>,
    api_key: Option<String>,
    email: Option<String>,
    fallback_theme: Theme,
    theme_registry: ThemeRegistry,
    asset_strategy: AssetStrategy,
    /// Math rendering strategy: Latex (ADF extension) or Png (attachment).
    /// Per [[WI-2026-02-17-002]].
    math_rendering: MathRendering,
    /// LaTeX Math plugin app ID (for ADF extension format).
    /// If not configured, uses default Appfire LaTeX Math plugin ID.
    latex_math_app_id: Option<String>,
    /// LaTeX Math plugin environment ID (for ADF extension format).
    /// If not configured, uses default Appfire LaTeX Math plugin environment ID.
    latex_math_env_id: Option<String>,
}

#[derive(Debug)]
pub struct ConfluencePayload {
    pub title: String,
    pub space: String,
    pub parent_id: Option<String>,
    pub existing_page_id: Option<String>,
    pub page_id: String,
    pub version: u32,
    pub page_url: String,
    pub is_update: bool,
    pub status: String,
    pub asset_map: HashMap<PathBuf, String>,
    pub confluence_body: String,
}

struct PageUpdate<'a> {
    page_id: &'a str,
    title: &'a str,
    content: &'a str,
    version: u32,
    parent_id: Option<&'a str>,
    status: &'a str,
    labels: &'a [String],
}

impl ConfluenceAdapter {
    pub fn new(config: &Config) -> Result<Self> {
        let platform_config = config.get_platform(ID);

        let base_url = platform_config
            .and_then(|c| c.get_str("base_url"))
            .unwrap_or_else(|| "https://confluence.atlassian.net".to_string());

        // Support both 'space' (new standard) and 'default_space' (legacy) for backward compatibility
        let default_space = platform_config
            .and_then(|c| c.get_str("space"))
            .or_else(|| platform_config.and_then(|c| c.get_str("default_space")))
            .unwrap_or_else(|| "DOCS".to_string());

        let api_key = platform_config
            .and_then(|c| c.get_str("api_key"))
            .or_else(|| std::env::var("CONFLUENCE_API_KEY").ok());
        let email = platform_config
            .and_then(|c| c.get_str("email"))
            .or_else(|| std::env::var("CONFLUENCE_EMAIL").ok());

        let registry = ThemeRegistry::new()?;
        let fallback_theme = registry.get_or_default("tech")?.clone();
        let asset_strategy = resolve_strategy(platform_config)?;
        let math_rendering = resolve_math_rendering(platform_config)?;

        // LaTeX Math plugin configuration (optional)
        let latex_math_app_id = platform_config.and_then(|c| c.get_str("latex_math_app_id"));
        let latex_math_env_id = platform_config.and_then(|c| c.get_str("latex_math_env_id"));

        // Default parent page ID for new pages (can be overridden per-post)
        let default_parent_id = platform_config.and_then(|c| c.get_str("parent_id"));

        Ok(Self {
            client: Client::new(),
            base_url,
            default_space,
            default_parent_id,
            api_key,
            email,
            fallback_theme,
            theme_registry: registry,
            asset_strategy,
            math_rendering,
            latex_math_app_id,
            latex_math_env_id,
        })
    }

    #[cfg(test)]
    pub fn new_for_test() -> Self {
        Self::new_for_test_with(
            "https://confluence.atlassian.net",
            "DOCS",
            AssetStrategy::Upload,
            MathRendering::Latex,
        )
    }

    #[cfg(test)]
    #[allow(clippy::expect_used)]
    pub fn new_for_test_with(
        base_url: &str,
        default_space: &str,
        asset_strategy: AssetStrategy,
        math_rendering: MathRendering,
    ) -> Self {
        let registry = ThemeRegistry::new().expect("registry");
        let fallback_theme = registry.get_or_default("tech").expect("theme").clone();
        Self {
            client: Client::new(),
            base_url: base_url.to_string(),
            default_space: default_space.to_string(),
            default_parent_id: None,
            api_key: None,
            email: None,
            fallback_theme,
            theme_registry: registry,
            asset_strategy,
            math_rendering,
            latex_math_app_id: None,
            latex_math_env_id: None,
        }
    }

    fn get_auth(&self) -> Result<(&str, &str)> {
        match (self.email.as_ref(), self.api_key.as_ref()) {
            (Some(email), Some(token)) => Ok((email, token)),
            (email, token) => {
                let mut missing = Vec::new();
                if token.is_none() {
                    missing.push(
                        "api_key (platforms.confluence.api_key or CONFLUENCE_API_KEY env var)",
                    );
                }
                if email.is_none() {
                    missing.push("email (platforms.confluence.email or CONFLUENCE_EMAIL env var)");
                }
                anyhow::bail!("Confluence credentials missing: {}", missing.join("; "))
            }
        }
    }

    /// Get configured LaTeX Math app ID.
    /// Returns None if not configured (will fallback to legacy macro format).
    pub fn latex_math_app_id(&self) -> Option<&str> {
        self.latex_math_app_id.as_deref()
    }

    /// Get configured LaTeX Math environment ID.
    /// Returns None if not configured (will fallback to legacy macro format).
    pub fn latex_math_env_id(&self) -> Option<&str> {
        self.latex_math_env_id.as_deref()
    }

    async fn find_page_by_title(
        &self,
        title: &str,
        space: &str,
    ) -> Result<Option<(String, u32, String)>> {
        let (email, token) = self.get_auth()?;

        let url = format!(
            "{}/wiki/rest/api/content?title={}&spaceKey={}&expand=version",
            self.base_url,
            urlencoding::encode(title),
            urlencoding::encode(space)
        );

        let response = self
            .client
            .get(&url)
            .basic_auth(email, Some(token))
            .send()
            .await
            .context("Failed to search Confluence")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Confluence search error ({}): {}", status, body);
        }

        let result: SearchResponse = response
            .json()
            .await
            .context("Failed to parse Confluence search response")?;

        if let Some(page) = result.results.into_iter().find(|p| p.title == title) {
            let full_url = format!("{}/wiki{}", self.base_url, page.links.webui);
            Ok(Some((page.id, page.version.number, full_url)))
        } else {
            Ok(None)
        }
    }

    async fn find_page_by_id(&self, page_id: &str) -> Result<Option<(String, u32, String)>> {
        let (email, token) = self.get_auth()?;
        let url = format!(
            "{}/wiki/rest/api/content/{}?expand=version",
            self.base_url,
            urlencoding::encode(page_id)
        );
        let response = self
            .client
            .get(&url)
            .basic_auth(email, Some(token))
            .send()
            .await
            .context("Failed to query Confluence page by id")?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Confluence query-by-id error ({}): {}", status, body);
        }

        let page: PageResponse = response
            .json()
            .await
            .context("Failed to parse Confluence query-by-id response")?;
        let full_url = format!("{}/wiki{}", self.base_url, page.links.webui);
        Ok(Some((page.id, page.version.number, full_url)))
    }

    async fn create_page(
        &self,
        title: &str,
        space: &str,
        parent_id: Option<&str>,
        status: &str,
    ) -> Result<(String, u32, String)> {
        let (email, token) = self.get_auth()?;

        let placeholder = "<p>Publishing in progress...</p>";
        let request = CreatePageRequest {
            page_type: "page".to_string(),
            title: title.to_string(),
            space: SpaceKey {
                key: space.to_string(),
            },
            body: PageBody {
                storage: StorageContent {
                    value: placeholder.to_string(),
                    representation: "storage".to_string(),
                },
            },
            ancestors: parent_id.map(|id| vec![Ancestor { id: id.to_string() }]),
            status: Some(status.to_string()),
        };

        let url = format!("{}/wiki/rest/api/content", self.base_url);
        let response = self
            .client
            .post(&url)
            .basic_auth(email, Some(token))
            .json(&request)
            .send()
            .await
            .context("Failed to create Confluence page")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Confluence create page error ({}): {}", status, body);
        }

        let result: PageResponse = response
            .json()
            .await
            .context("Failed to parse Confluence response")?;

        let full_url = format!("{}/wiki{}", self.base_url, result.links.webui);
        Ok((result.id, result.version.number, full_url))
    }

    async fn upload_attachment(&self, page_id: &str, file_path: &Path) -> Result<String> {
        let (email, token) = self.get_auth()?;

        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid file name"))?;

        let file_data = std::fs::read(file_path).with_context(|| {
            format!(
                "Failed to read staged asset '{}' (local file, nothing was sent to Confluence)",
                file_path.display()
            )
        })?;

        let mime_type = mime_type_from_path(file_path);

        let part = Part::bytes(file_data)
            .file_name(file_name.to_string())
            .mime_str(mime_type)?;

        let form = Form::new().part("file", part);

        let url = format!(
            "{}/wiki/rest/api/content/{}/child/attachment",
            self.base_url, page_id
        );

        let response = self
            .client
            .put(&url)
            .basic_auth(email, Some(&token))
            .header("X-Atlassian-Token", "nocheck")
            .multipart(form)
            .send()
            .await
            .with_context(|| format!("Confluence attachment request failed for '{file_name}'"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "Confluence rejected attachment '{}' ({}): {}",
                file_name,
                status,
                body
            );
        }

        let upload: AttachmentResponse = response
            .json()
            .await
            .context("Failed to parse Confluence attachment upload response")?;
        let attachment =
            upload.results.into_iter().next().ok_or_else(|| {
                anyhow::anyhow!("Confluence attachment upload returned no results")
            })?;

        debug!("Uploaded: id={} title={}", attachment.id, attachment.title);
        Ok(attachment.title)
    }

    async fn update_page(&self, update: PageUpdate<'_>) -> Result<()> {
        let (email, token) = self.get_auth()?;

        // Ancestors are asserted on every update so a page adopted by title
        // during provision moves under the planned parent instead of keeping
        // whatever parent it had before adoption.
        let request = UpdatePageRequest {
            page_type: "page".to_string(),
            title: update.title.to_string(),
            body: PageBody {
                storage: StorageContent {
                    value: update.content.to_string(),
                    representation: "storage".to_string(),
                },
            },
            version: PageVersion {
                number: update.version + 1,
            },
            // Confluence replaces the page's labels when this field is present;
            // an empty array therefore clears labels left by earlier publishes.
            metadata: PageMetadata {
                labels: update
                    .labels
                    .iter()
                    .map(|name| PageLabel {
                        prefix: "global".to_string(),
                        name: name.clone(),
                    })
                    .collect(),
            },
            ancestors: update
                .parent_id
                .map(|id| vec![Ancestor { id: id.to_string() }]),
            status: Some(update.status.to_string()),
        };

        let url = format!("{}/wiki/rest/api/content/{}", self.base_url, update.page_id);

        let response = self
            .client
            .put(&url)
            .basic_auth(email, Some(token))
            .json(&request)
            .send()
            .await
            .context("Failed to update Confluence page")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Confluence update page error ({}): {}", status, body);
        }

        Ok(())
    }

    pub fn normalize_labels(tags: &[String]) -> Vec<String> {
        let mut labels: Vec<String> = tags
            .iter()
            .map(|tag| {
                let mut out = String::new();
                for ch in tag.trim().chars() {
                    if ch.is_ascii_alphanumeric() {
                        out.push(ch.to_ascii_lowercase());
                    } else if ch == '-' || ch == '_' || ch == '.' || ch.is_ascii_whitespace() {
                        if out.ends_with('-') {
                            continue;
                        }
                        out.push('-');
                    }
                }
                out.trim_matches('-').to_string()
            })
            .filter(|label| !label.is_empty())
            .map(|mut label| {
                if label.len() > 255 {
                    label.truncate(255);
                }
                label
            })
            .collect();

        labels.sort();
        labels.dedup();
        labels
    }
}

#[async_trait(?Send)]
impl PlatformAdapter for ConfluenceAdapter {
    fn id(&self) -> &'static str {
        ID
    }

    fn name(&self) -> &'static str {
        "Confluence"
    }

    fn required_format(&self) -> OutputFormat {
        OutputFormat::HtmlFragment
    }

    fn asset_strategy(&self) -> AssetStrategy {
        self.asset_strategy
    }

    fn validate_config(&self, _config: &typub_config::PlatformConfig) -> Result<()> {
        // Report every missing credential at once; the lookup chain is the
        // platform config field first, then the environment variable.
        let mut missing = Vec::new();
        if self.api_key.is_none() {
            missing.push("api_key (platforms.confluence.api_key or CONFLUENCE_API_KEY env var)");
        }
        if self.email.is_none() {
            missing.push("email (platforms.confluence.email or CONFLUENCE_EMAIL env var)");
        }
        if !missing.is_empty() {
            anyhow::bail!("Confluence credentials missing: {}", missing.join("; "));
        }
        Ok(())
    }

    fn supports_shared_link_rewrite(&self) -> bool {
        true
    }

    fn render_config(&self, _content_info: &ContentInfo) -> RenderConfig {
        let mut config = render_config_for_png_math(self.asset_strategy, self.math_rendering);
        config.preamble = "#set raw(theme: none)".to_string();
        config
    }

    async fn specialize_payload(
        &self,
        document: Document,
        ctx: &dyn AdapterContext,
    ) -> Result<AdapterPayload> {
        let content_info = ctx.content_info();
        let space = content_info
            .get_platform_str("space")
            .unwrap_or_else(|| self.default_space.clone());
        let parent_id = content_info
            .get_platform_str("parent_id")
            .or_else(|| self.default_parent_id.clone());

        let published = ctx.published();
        let status = if published { "current" } else { "draft" };
        debug!(
            "Confluence: resolved status='{}' for '{}'",
            status, content_info.slug
        );

        let mut elements = document;

        // Handle PNG math rendering: convert SVG to PNG markers for deferred upload.
        // Per [[WI-2026-02-17-002]].
        (elements, _) = convert_png_math_for_strategy(
            elements,
            self.asset_strategy,
            self.math_rendering,
            &content_info.path,
            &content_info.slug,
        )?;

        let deferred = prepare_deferred_assets(self.asset_strategy, &elements, &content_info.path);

        Ok(AdapterPayload::new(
            ConfluencePayload {
                title: content_info.title.clone(),
                space,
                parent_id,
                existing_page_id: ctx.get_platform_id(&content_info.slug, ID)?,
                page_id: String::new(),
                version: 0,
                page_url: String::new(),
                is_update: false,
                status: status.to_string(),
                asset_map: HashMap::new(),
                confluence_body: String::new(),
            },
            content_info.clone(),
            deferred,
            elements,
        ))
    }

    async fn provision_target(
        &self,
        mut payload: AdapterPayload,
        _ctx: &dyn AdapterContext,
    ) -> Result<AdapterPayload> {
        let inner = payload
            .downcast_mut::<ConfluencePayload>()
            .ok_or_else(|| anyhow::anyhow!("Invalid Confluence payload type"))?;

        info!("[1/4] Finding or creating Confluence page...");

        let existing_page = if let Some(existing_page_id) = inner.existing_page_id.as_deref() {
            match self.find_page_by_id(existing_page_id).await? {
                Some(page) => Some(page),
                None => {
                    warn!(
                        "Cached Confluence page id '{}' not found; falling back to title lookup",
                        existing_page_id
                    );
                    self.find_page_by_title(&inner.title, &inner.space).await?
                }
            }
        } else {
            self.find_page_by_title(&inner.title, &inner.space).await?
        };

        match existing_page {
            Some((id, ver, url)) => {
                debug!("Found existing page id={}, version={}", id, ver);
                inner.page_id = id;
                inner.version = ver;
                inner.page_url = url;
                inner.is_update = true;
            }
            None => {
                let (id, ver, url) = self
                    .create_page(
                        &inner.title,
                        &inner.space,
                        inner.parent_id.as_deref(),
                        &inner.status,
                    )
                    .await?;
                debug!("Created new page id={}", id);
                inner.page_id = id;
                inner.version = ver;
                inner.page_url = url;
                inner.is_update = false;
            }
        };

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

        let page_id = payload
            .downcast_ref::<ConfluencePayload>()
            .ok_or_else(|| anyhow::anyhow!("Invalid Confluence payload type"))?
            .page_id
            .clone();

        let mut asset_map: HashMap<PathBuf, String> = HashMap::new();

        // Upload all image markers (block + inline, unified via [[ADR-0008]])
        if payload.assets.needs_materialize() && self.asset_strategy == AssetStrategy::Upload {
            let asset_count = payload.assets.pending.assets.len();
            info!(
                "[2/4] {}",
                format!("Uploading {} attachments...", asset_count)
            );

            let mut resolved = HashMap::new();
            for asset in &payload.assets.pending.assets {
                let filename = self
                    .upload_attachment(&page_id, &asset.local_path)
                    .await
                    .with_context(|| {
                        format!(
                            "attachment '{}' (staged at '{}')",
                            asset.original_ref,
                            asset.local_path.display()
                        )
                    })?;
                resolved.insert(asset.index, filename.clone());
                asset_map.insert(asset.local_path.clone(), filename);
            }
            payload.assets.resolved = resolved;
        } else if self.asset_strategy == AssetStrategy::Upload {
            info!("[2/4] No attachments to upload");
        } else {
            info!("[2/4] Skipping attachment upload (asset_strategy != upload)");
        }

        let math_mode_str = match self.math_rendering {
            MathRendering::Latex => "LaTeX macros",
            MathRendering::Png => "PNG attachments",
            MathRendering::Svg => "SVG",
        };
        info!("[3/4] Math formulas processed via {}", math_mode_str);

        let inner = payload
            .downcast_mut::<ConfluencePayload>()
            .ok_or_else(|| anyhow::anyhow!("Invalid Confluence payload type"))?;
        inner.asset_map = asset_map;

        if !payload.assets.resolved.is_empty() {
            let url_map = build_resolved_url_map(&payload.assets, &payload.content_info.path);
            resolve_asset_urls(&mut payload.document, &url_map);
        }

        Ok(payload)
    }

    async fn serialize_payload(
        &self,
        mut payload: AdapterPayload,
        _ctx: &dyn AdapterContext,
    ) -> Result<AdapterPayload> {
        ensure_no_unresolved_image_markers(self.id(), self.asset_strategy, &payload.document)?;
        let asset_map = payload
            .downcast_ref::<ConfluencePayload>()
            .ok_or_else(|| anyhow::anyhow!("Invalid Confluence payload type"))?
            .asset_map
            .clone();
        let app_id = self.latex_math_app_id();
        let env_id = self.latex_math_env_id();
        let confluence_body = format::elements_to_confluence_html(
            &payload.document,
            &payload.content_info.path,
            &asset_map,
            app_id,
            env_id,
        );

        let inner = payload
            .downcast_mut::<ConfluencePayload>()
            .ok_or_else(|| anyhow::anyhow!("Invalid Confluence payload type"))?;
        inner.confluence_body = confluence_body;
        Ok(payload)
    }

    async fn publish_payload(
        &self,
        payload: AdapterPayload,
        _ctx: &dyn AdapterContext,
    ) -> Result<PublishResult> {
        let labels = Self::normalize_labels(&payload.content_info.tags);
        let payload = downcast_payload::<ConfluencePayload>(payload, "Confluence")?;

        let action = if payload.is_update {
            "Updating"
        } else {
            "Setting"
        };
        info!("[4/4] {}", format!("{} page content...", action));

        self.update_page(PageUpdate {
            page_id: &payload.page_id,
            title: &payload.title,
            content: &payload.confluence_body,
            version: payload.version,
            parent_id: payload.parent_id.as_deref(),
            status: &payload.status,
            labels: &labels,
        })
        .await?;

        let action = if payload.is_update {
            "Updated"
        } else {
            "Published"
        };
        debug!("{}: {}", action, payload.page_url);

        Ok(PublishResult {
            url: Some(payload.page_url),
            platform_id: Some(payload.page_id),
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
        let theme = load_theme(
            ctx.theme_id(),
            None,
            &self.theme_registry,
            &self.fallback_theme,
        );

        // Use unified preview builder with MathJax support for LaTeX
        build_unified_preview(
            &elements,
            content_info,
            ID,
            "Confluence",
            Some(&theme.css),
            Some(&PlatformBranding::new("#ffffff", "#0052cc")),
        )
    }

    async fn check_status(&self, _slug: &str) -> Result<bool> {
        if self.api_key.is_none() || self.email.is_none() {
            return Ok(false);
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_creation() {
        let adapter = ConfluenceAdapter::new_for_test();
        assert_eq!(adapter.id(), "confluence");
        assert_eq!(adapter.name(), "Confluence");
        assert_eq!(adapter.asset_strategy(), AssetStrategy::Upload);
    }

    #[test]
    fn test_normalize_labels() {
        let labels = ConfluenceAdapter::normalize_labels(&[
            " Rust ".to_string(),
            "rust".to_string(),
            "Data Science".to_string(),
            "+++".to_string(),
        ]);
        assert_eq!(labels, vec!["data-science".to_string(), "rust".to_string()]);
    }

    #[test]
    fn test_normalize_labels_deduplication() {
        let labels = ConfluenceAdapter::normalize_labels(&["Rust".to_string(), "rust".to_string()]);
        assert_eq!(labels, vec!["rust".to_string()]);
    }

    #[test]
    fn test_normalize_labels_truncation() {
        let long_label = "a".repeat(300);
        let labels = ConfluenceAdapter::normalize_labels(&[long_label]);
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].len(), 255);
    }

    #[test]
    fn test_missing_credentials_reported_once_naming_both() -> Result<()> {
        let adapter = ConfluenceAdapter::new_for_test();
        let msg = match adapter.get_auth() {
            Ok(_) => anyhow::bail!("credentials unexpectedly present"),
            Err(err) => err.to_string(),
        };
        assert!(msg.contains("api_key"), "names api_key: {msg}");
        assert!(msg.contains("email"), "names email: {msg}");
        assert!(msg.contains("CONFLUENCE_API_KEY"), "names env chain: {msg}");
        Ok(())
    }

    fn adapter_with_credentials(base_url: &str) -> ConfluenceAdapter {
        let mut adapter = ConfluenceAdapter::new_for_test_with(
            base_url,
            "DOCS",
            AssetStrategy::Upload,
            MathRendering::Latex,
        );
        adapter.api_key = Some("token".to_string());
        adapter.email = Some("user@example.com".to_string());
        adapter
    }

    fn page_update<'a>(
        title: &'a str,
        parent_id: Option<&'a str>,
        labels: &'a [String],
    ) -> PageUpdate<'a> {
        PageUpdate {
            page_id: "42",
            title,
            content: "<p>body</p>",
            version: 3,
            parent_id,
            status: "current",
            labels,
        }
    }

    #[tokio::test]
    async fn test_upload_attachment_missing_local_file_is_a_local_error() -> Result<()> {
        let adapter = adapter_with_credentials("http://127.0.0.1:9");
        let msg = match adapter
            .upload_attachment("42", Path::new("/nonexistent/dir/figure.png"))
            .await
        {
            Ok(_) => anyhow::bail!("upload of a missing file unexpectedly succeeded"),
            Err(err) => format!("{err:#}"),
        };
        assert!(
            msg.contains("Failed to read staged asset"),
            "local domain: {msg}"
        );
        assert!(
            msg.contains("/nonexistent/dir/figure.png"),
            "names path: {msg}"
        );
        assert!(
            !msg.contains("Confluence rejected"),
            "not a remote error: {msg}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_update_page_asserts_planned_parent_in_ancestors() -> Result<()> {
        use wiremock::matchers::{body_partial_json, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/wiki/rest/api/content/42"))
            .and(body_partial_json(serde_json::json!({
                "ancestors": [{ "id": "777" }]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
            .expect(1)
            .mount(&server)
            .await;

        let adapter = adapter_with_credentials(&server.uri());
        adapter
            .update_page(page_update("Adopted Page", Some("777"), &[]))
            .await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_update_page_without_parent_sends_no_ancestors() -> Result<()> {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, Request, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/wiki/rest/api/content/42"))
            .and(|request: &Request| !String::from_utf8_lossy(&request.body).contains("ancestors"))
            .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
            .expect(1)
            .mount(&server)
            .await;

        let adapter = adapter_with_credentials(&server.uri());
        adapter
            .update_page(page_update("Rootless Page", None, &[]))
            .await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_update_page_replaces_labels_in_same_request() -> Result<()> {
        use wiremock::matchers::{body_partial_json, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/wiki/rest/api/content/42"))
            .and(body_partial_json(serde_json::json!({
                "metadata": {
                    "labels": [
                        { "prefix": "global", "name": "data-science" },
                        { "prefix": "global", "name": "rust" }
                    ]
                }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
            .expect(1)
            .mount(&server)
            .await;

        let adapter = adapter_with_credentials(&server.uri());
        let labels = vec!["data-science".to_string(), "rust".to_string()];
        adapter
            .update_page(page_update("Tagged Page", None, &labels))
            .await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_update_page_sends_empty_labels_to_clear_remote_labels() -> Result<()> {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, Request, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/wiki/rest/api/content/42"))
            .and(|request: &Request| {
                serde_json::from_slice::<serde_json::Value>(&request.body)
                    .ok()
                    .and_then(|body| body.pointer("/metadata/labels").cloned())
                    .and_then(|labels| labels.as_array().cloned())
                    .is_some_and(|labels| labels.is_empty())
            })
            .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
            .expect(1)
            .mount(&server)
            .await;

        let adapter = adapter_with_credentials(&server.uri());
        adapter
            .update_page(page_update("Untagged Page", None, &[]))
            .await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_update_page_label_rejection_is_an_error() -> Result<()> {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/wiki/rest/api/content/42"))
            .respond_with(ResponseTemplate::new(400).set_body_string("invalid labels"))
            .expect(1)
            .mount(&server)
            .await;

        let adapter = adapter_with_credentials(&server.uri());
        let labels = vec!["rust".to_string()];
        let error = adapter
            .update_page(page_update("Tagged Page", None, &labels))
            .await
            .expect_err("rejected label update should fail the publish");
        let message = format!("{error:#}");
        assert!(
            message.contains("Confluence update page error (400"),
            "{message}"
        );
        assert!(message.contains("invalid labels"), "{message}");
        Ok(())
    }

    #[tokio::test]
    async fn test_upload_attachment_remote_rejection_names_file_status_body() -> Result<()> {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/wiki/rest/api/content/42/child/attachment"))
            .respond_with(ResponseTemplate::new(403).set_body_string("attachment quota exceeded"))
            .mount(&server)
            .await;

        let dir = std::env::temp_dir().join("typub-confluence-attach-test");
        std::fs::create_dir_all(&dir)?;
        let file = dir.join("figure.png");
        std::fs::write(&file, b"png")?;

        let adapter = adapter_with_credentials(&server.uri());
        let msg = match adapter.upload_attachment("42", &file).await {
            Ok(_) => anyhow::bail!("rejected upload unexpectedly succeeded"),
            Err(err) => format!("{err:#}"),
        };
        assert!(
            msg.contains("Confluence rejected attachment 'figure.png'"),
            "{msg}"
        );
        assert!(msg.contains("403"), "carries status: {msg}");
        assert!(
            msg.contains("attachment quota exceeded"),
            "carries body: {msg}"
        );
        std::fs::remove_file(&file).ok();
        Ok(())
    }
}
