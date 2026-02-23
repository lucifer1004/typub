use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;

use typub_adapters_core::{
    AdapterContext, AdapterPayload, ContentInfo, ContentTransform, OutputFormat, PlatformAdapter,
    RenderConfig, default_render_config_for, downcast_payload, info, write_preview_file,
};
use typub_config::Config;

/// Embedded Xiaohongshu theme Typst script
const XIAOHONGSHU_TYP: &str = include_str!("../typst-scripts/xiaohongshu.typ");
use typub_core::AssetStrategy;
use typub_ir::{DocMeta, Document};
use typub_storage::{DeferredAssets, PublishResult};

use crate::config::{CAPABILITY, ID, resolve_strategy};

pub struct XiaohongshuAdapter {
    output_dir: PathBuf,
    asset_strategy: AssetStrategy,
}

#[derive(Debug)]
pub struct XiaohongshuPayload {
    pub slug: String,
    pub slide_paths: Vec<PathBuf>,
}

impl XiaohongshuAdapter {
    pub fn new(config: &Config) -> Result<Self> {
        let platform_config = config.get_platform(ID);

        let output_dir = platform_config
            .and_then(|c| c.get_str("output_dir"))
            .map(PathBuf::from)
            .unwrap_or_else(|| config.output_dir.join(ID));
        let asset_strategy = resolve_strategy(platform_config)?;

        Ok(Self {
            output_dir,
            asset_strategy,
        })
    }

    #[cfg(test)]
    pub fn new_for_test() -> Self {
        Self::new_for_test_with(PathBuf::from("/tmp/xiaohongshu"), AssetStrategy::Embed)
    }

    #[cfg(test)]
    pub fn new_for_test_with(output_dir: PathBuf, asset_strategy: AssetStrategy) -> Self {
        Self {
            output_dir,
            asset_strategy,
        }
    }
}

#[async_trait(?Send)]
impl PlatformAdapter for XiaohongshuAdapter {
    fn id(&self) -> &'static str {
        ID
    }

    fn name(&self) -> &'static str {
        "Xiaohongshu (小红书)"
    }

    fn required_format(&self) -> OutputFormat {
        OutputFormat::Png
    }

    fn asset_strategy(&self) -> AssetStrategy {
        self.asset_strategy
    }

    fn validate_config(&self, _config: &typub_config::PlatformConfig) -> Result<()> {
        Ok(())
    }

    fn render_config(&self, content_info: &ContentInfo) -> RenderConfig {
        let title = &content_info.title;
        let subtitle = content_info
            .get_platform_str("subtitle")
            .unwrap_or_default();
        let author = content_info
            .get_platform_str("author")
            .unwrap_or_else(|| "author".to_string());

        // Post structure is fixed: posts/YYYY-MM-DD-slug/
        // cover_image in meta.toml is relative to post dir
        // Path must start with "/" for typst to resolve from project root (--root)
        let cover_image = content_info.get_platform_str("cover_image");
        let cover_image_path = cover_image
            .filter(|s| !s.is_empty())
            .map(|img| {
                format!(
                    "/posts/{}/{}",
                    content_info.slug,
                    img.trim_start_matches('/')
                )
            })
            .unwrap_or_default();

        let cover_image_call = if cover_image_path.is_empty() {
            String::new()
        } else {
            format!(r#"  image-content: image("{cover_image_path}"),"#)
        };

        // Optional accent color override (default uses template's colors.accent)
        let accent_color = content_info.get_platform_str("accent_color");
        let accent_color_arg = accent_color
            .filter(|s| !s.is_empty())
            .map(|c| format!(r#"accent-color: rgb("{c}"),"#))
            .unwrap_or_default();

        let mut config = default_render_config_for(self.asset_strategy, &CAPABILITY);
        // Use embedded xiaohongshu.typ instead of external rewind-note package
        config.preamble = format!(
            r#"{xiaohongshu_typ}

#show: rewind-theme.with({accent_color_arg})
#cover(
{cover_image_call}
  title: [{title}],
  subtitle: [{subtitle}],
  author: "{author}",
  {accent_color_arg}
)"#,
            xiaohongshu_typ = XIAOHONGSHU_TYP,
        );
        config.content_transform = ContentTransform::ShowRules(String::new());
        config.template_before = String::new();
        config.template_after = String::new();
        config
    }

    async fn specialize_payload(
        &self,
        _elements: Document,
        ctx: &dyn AdapterContext,
    ) -> Result<AdapterPayload> {
        let content_info = ctx.content_info();
        if content_info.rendered_paths.is_empty() {
            anyhow::bail!("No slide images generated. Make sure slides.typ exists.");
        }
        Ok(AdapterPayload::new(
            XiaohongshuPayload {
                slug: content_info.slug.clone(),
                slide_paths: content_info.rendered_paths.clone(),
            },
            content_info.clone(),
            DeferredAssets::empty(),
            Document {
                blocks: Vec::new(),
                footnotes: Default::default(),
                assets: Default::default(),
                meta: DocMeta::default(),
            },
        ))
    }

    async fn publish_payload(
        &self,
        payload: AdapterPayload,
        _ctx: &dyn AdapterContext,
    ) -> Result<PublishResult> {
        let payload = downcast_payload::<XiaohongshuPayload>(payload, "Xiaohongshu")?;

        let dest_dir = self.output_dir.join(&payload.slug);
        std::fs::create_dir_all(&dest_dir)?;

        for (i, src_path) in payload.slide_paths.iter().enumerate() {
            let dest_path = dest_dir.join(format!("slide-{}.png", i + 1));
            std::fs::copy(src_path, &dest_path)?;
        }

        info!(
            "Generated {} slides at: {}",
            payload.slide_paths.len(),
            dest_dir.display()
        );
        info!("Upload these images manually to 小红书");

        Ok(PublishResult {
            url: Some(format!("file://{}", dest_dir.display())),
            platform_id: Some(payload.slug),
            published_at: Utc::now(),
        })
    }

    fn build_preview(
        &self,
        _title: &str,
        _elements: Document,
        ctx: &dyn AdapterContext,
    ) -> Result<PathBuf> {
        let content_info = ctx.content_info();
        if content_info.rendered_paths.is_empty() {
            anyhow::bail!("No slide images to preview");
        }

        let temp_dir = std::env::temp_dir().join("typub-preview");
        std::fs::create_dir_all(&temp_dir)?;

        // Copy slide images to temp preview directory for dev server to serve
        for (i, path) in content_info.rendered_paths.iter().enumerate() {
            let temp_img = temp_dir.join(format!("slide-{}.png", i + 1));
            std::fs::copy(path, &temp_img)?;
        }

        let slide_count = content_info.rendered_paths.len();

        // Dynamic HTML that generates slides based on count
        let preview_html = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>{title} - 小红书 Preview</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Helvetica Neue", sans-serif;
            background: linear-gradient(135deg, #ff4b5c 0%, #ff8c69 100%);
            margin: 0;
            padding: 40px;
            min-height: 100vh;
        }}
        h1 {{ color: white; text-align: center; margin-bottom: 30px; }}
        .slides {{ display: flex; flex-wrap: wrap; gap: 20px; justify-content: center; max-width: 1200px; margin: 0 auto; }}
        .slide {{ position: relative; background: white; border-radius: 12px; overflow: hidden; box-shadow: 0 4px 20px rgba(0,0,0,0.2); }}
        .slide img {{ display: block; max-width: 300px; height: auto; }}
        .number {{ position: absolute; top: 10px; right: 10px; background: rgba(0,0,0,0.5); color: white; padding: 4px 10px; border-radius: 12px; font-size: 12px; }}
        .info {{ color: white; text-align: center; margin-top: 30px; font-size: 14px; }}
    </style>
</head>
<body>
    <h1>{title}</h1>
    <div class="slides" id="slides"></div>
    <div class="info">
        <p>{slide_count} slides ready</p>
        <p>Images saved at: {output_path}</p>
    </div>
    <script>
    (function() {{
        // Server tells us the count
        const slideCount = {slide_count};
        // Cache buster to force image refresh
        const v = Date.now();

        const slidesContainer = document.getElementById('slides');
        let html = '';
        for (let i = 1; i <= slideCount; i++) {{
            html += `<div class="slide"><img src="/slide-${{i}}.png?v=${{v}}" alt="Slide ${{i}}"><span class="number">${{i}}</span></div>`;
        }}
        slidesContainer.innerHTML = html;

        // SSE for live reload
        const eventSource = new EventSource('/__sse__');
        eventSource.onmessage = () => location.reload();
    }})();
    </script>
</body>
</html>"#,
            title = content_info.title,
            slide_count = slide_count,
            output_path = self.output_dir.join(&content_info.slug).display(),
        );

        write_preview_file(&content_info.slug, ID, &preview_html)
    }

    async fn check_status(&self, slug: &str) -> Result<bool> {
        let dest_dir = self.output_dir.join(slug);
        Ok(dest_dir.exists() && dest_dir.join("slide-1.png").exists())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_creation() {
        let adapter = XiaohongshuAdapter::new_for_test();
        assert_eq!(adapter.id(), "xiaohongshu");
        assert_eq!(adapter.name(), "Xiaohongshu (小红书)");
        assert_eq!(adapter.asset_strategy(), AssetStrategy::Embed);
    }
}
