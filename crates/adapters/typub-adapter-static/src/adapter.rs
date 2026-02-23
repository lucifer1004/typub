use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;

use typub_adapters_core::{
    AdapterContext, AdapterPayload, ContentInfo, OutputFormat, PlatformAdapter, PlatformBranding,
    RenderConfig, build_unified_preview, convert_png_math_for_strategy, debug, downcast_payload,
    ensure_no_unresolved_image_markers, materialize_and_resolve_urls,
    mock_materialize_and_resolve_urls, prepare_deferred_assets, render_config_for_png_math,
    resolve_asset_urls,
};
use typub_config::Config;
use typub_core::{AssetStrategy, MathRendering};
use typub_html::{SerializeOptions, document_to_html_with_options};
use typub_ir::Document;
use typub_storage::{PublishResult, build_image_marker_url_map, to_data_uri};
use typub_theme::{Theme, ThemeRegistry, apply_theme_full_document, load_theme};

use crate::config::{CAPABILITY, ID, resolve_math_rendering, resolve_strategy};

pub struct StaticAdapter {
    output_dir: PathBuf,
    fallback_theme: Theme,
    theme_registry: ThemeRegistry,
    asset_strategy: AssetStrategy,
    math_rendering: MathRendering,
}

#[derive(Debug)]
pub struct StaticPayload {
    pub slug: String,
    pub themed_html: String,
}

impl StaticAdapter {
    pub fn new(config: &Config) -> Result<Self> {
        let platform_config = config.get_platform(ID);

        let output_dir = platform_config
            .and_then(|c| c.get_str("output_dir"))
            .map(PathBuf::from)
            .unwrap_or_else(|| config.output_dir.join(ID));

        let registry = ThemeRegistry::new()?;
        let fallback_theme = registry.get_or_default("minimal")?.clone();

        let asset_strategy = resolve_strategy(platform_config)?;
        let math_rendering = resolve_math_rendering(platform_config)?;

        Ok(Self {
            output_dir,
            fallback_theme,
            theme_registry: registry,
            asset_strategy,
            math_rendering,
        })
    }

    #[cfg(test)]
    #[allow(clippy::expect_used)]
    pub fn new_for_test() -> Self {
        Self::new_for_test_with(
            PathBuf::from("/tmp/static"),
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
        let registry = ThemeRegistry::new().expect("registry");
        let fallback_theme = registry.get_or_default("minimal").expect("theme").clone();
        Self {
            output_dir,
            fallback_theme,
            theme_registry: registry,
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
                // Nothing to do here - build_asset_map is only called for non-deferred strategies
            }
        }

        Ok(url_map)
    }
}

#[async_trait(?Send)]
impl PlatformAdapter for StaticAdapter {
    fn id(&self) -> &'static str {
        ID
    }

    fn name(&self) -> &'static str {
        "Static Site"
    }

    fn required_format(&self) -> OutputFormat {
        OutputFormat::Html
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

        // Handle PNG math rendering based on asset strategy.
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

        // Build deferred assets using helper (handles both deferred and immediate strategies)
        let deferred = prepare_deferred_assets(self.asset_strategy, &elements, &content_info.path);

        Ok(AdapterPayload::new(
            StaticPayload {
                slug,
                themed_html: String::new(),
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
        ctx: &dyn AdapterContext,
    ) -> Result<AdapterPayload> {
        ensure_no_unresolved_image_markers(self.id(), self.asset_strategy, &payload.document)?;

        let serialize_options = SerializeOptions {
            use_code_highlight: CAPABILITY.code_highlight,
            ..Default::default()
        };
        let body_html = document_to_html_with_options(&payload.document, &serialize_options);
        let theme = load_theme(
            ctx.theme_id(),
            None,
            &self.theme_registry,
            &self.fallback_theme,
        );
        let themed_html =
            apply_theme_full_document(&body_html, &theme, &payload.content_info.title, false)?;

        let inner = payload
            .downcast_mut::<StaticPayload>()
            .ok_or_else(|| anyhow::anyhow!("Invalid Static publish payload type"))?;
        inner.themed_html = themed_html;
        Ok(payload)
    }

    async fn publish_payload(
        &self,
        payload: AdapterPayload,
        _ctx: &dyn AdapterContext,
    ) -> Result<PublishResult> {
        let payload = downcast_payload::<StaticPayload>(payload, "Static")?;
        let dest_dir = self.output_dir.join(&payload.slug);
        std::fs::create_dir_all(&dest_dir)?;
        let html_path = dest_dir.join("index.html");
        std::fs::write(&html_path, &payload.themed_html)?;

        Ok(PublishResult {
            url: Some(format!("file://{}", html_path.display())),
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
            "Static Site",
            Some(&theme.css),
            Some(&PlatformBranding::new("#ffffff", "#4a5568")),
        )
    }

    async fn check_status(&self, slug: &str) -> Result<bool> {
        let dest_path = self.output_dir.join(slug).join("index.html");
        Ok(dest_path.exists())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_creation() {
        let adapter = StaticAdapter::new_for_test();
        assert_eq!(adapter.id(), "static");
        assert_eq!(adapter.name(), "Static Site");
        assert_eq!(adapter.asset_strategy(), AssetStrategy::Copy);
    }

    #[test]
    fn test_adapter_with_external_strategy() {
        let adapter = StaticAdapter::new_for_test_with(
            PathBuf::from("/tmp/static"),
            AssetStrategy::External,
            MathRendering::Svg,
        );
        assert_eq!(adapter.asset_strategy(), AssetStrategy::External);
        assert!(adapter.asset_strategy().requires_deferred_upload());
    }
}
