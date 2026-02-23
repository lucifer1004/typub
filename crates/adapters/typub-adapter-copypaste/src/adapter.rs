use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;

use typub_adapters_core::{
    AdapterContext, AdapterPayload, ContentInfo, MarkdownProcessingRules, MarkdownRenderOptions,
    OutputFormat, PlatformAdapter, RenderConfig, convert_png_math_for_strategy, debug,
    document_to_markdown_with_options, downcast_payload, info, materialize_and_resolve_urls,
    mock_materialize_and_resolve_urls, prepare_deferred_assets, render_config_for_png_math,
    resolve_asset_strategy_with_policy, resolve_asset_urls, warn, write_preview_file,
};
use typub_config::{Config, PlatformConfig};
use typub_core::{AssetStrategy, MathDelimiters, MathRendering};
use typub_html::{SerializeOptions, SerializeRule, document_to_html_with_options};
use typub_ir::Document;
use typub_passes::{FlattenSvgPass, PassCtx, run_passes};
use typub_storage::{DeferredAssets, PublishResult, build_image_marker_url_map, to_data_uri};
use typub_theme::{Theme, ThemeRegistry, apply_theme, load_theme};

use crate::model::{BuiltinProfile, CopyFormat, CopyPastePayload};

/// A generic copy-paste adapter parameterized by a platform profile.
///
/// One Rust type backs all copy-paste platforms.  The registry creates one
/// instance per enabled profile, stored as `Box<dyn PlatformAdapter>`.
pub struct CopyPasteAdapter {
    profile_id: &'static str,
    profile_name: &'static str,
    editor_url: &'static str,
    format: CopyFormat,
    /// Serialization rules per [[RFC-0002:C-PIPELINE-STAGES]].
    serialize_rules: typub_html::SerializeRules,

    output_dir: PathBuf,
    /// Fallback theme (used when no theme resolved from 5-level chain)
    fallback_theme: Theme,
    /// Profile default theme (layer 5 in theme resolution)
    profile_default_theme: Option<&'static str>,
    /// Theme registry for runtime resolution
    theme_registry: ThemeRegistry,
    asset_strategy: AssetStrategy,
    /// Math delimiter syntax for Markdown output.
    math_delimiters: MathDelimiters,
    /// How to render math equations.
    /// Per [[WI-2026-02-13-026]], Png variant rasterizes SVG to PNG.
    math_rendering: MathRendering,
    /// Whether to use inline HTML (`<img>` tags) for images with dimensions.
    /// Only relevant when format is Markdown.
    use_inline_html_for_sized_images: bool,
    /// Markdown post-processing rules.
    /// Used for platform-specific editor quirks like blank line stripping.
    /// Only relevant when format is Markdown.
    markdown_processing_rules: MarkdownProcessingRules,
    /// Whether lists should be tight (no blank lines between items).
    /// Only relevant when format is Markdown.
    tight_lists: bool,
}

impl CopyPasteAdapter {
    /// Create from a built-in profile (generated from `profiles.toml`).
    pub fn from_profile(profile: &'static BuiltinProfile, config: &Config) -> Result<Self> {
        let platform_config = config.get_platform(profile.id);
        Self::build(
            profile.id,
            profile.name,
            profile.editor_url,
            profile.format,
            profile.serialize_rules,
            profile.default_theme,
            profile.default_asset_strategy(),
            profile.default_math_delimiters(),
            profile.default_math_rendering(),
            profile.use_inline_html_for_sized_images,
            profile.markdown_processing_rules,
            profile.tight_lists,
            platform_config,
            config,
        )
    }

    #[cfg(test)]
    pub fn new_for_test(profile_id: &str) -> Result<Self> {
        let config = Config::default();
        let profile = crate::find_profile(profile_id)
            .ok_or_else(|| anyhow::anyhow!("Unknown copypaste profile: {}", profile_id))?;
        Self::from_profile(profile, &config)
    }

    /// Create from a user-defined `type = "manual"` platform in typub.toml.
    pub fn from_manual_config(
        id: &'static str,
        platform_config: &PlatformConfig,
        config: &Config,
    ) -> Result<Self> {
        let name = platform_config
            .get_str("name")
            .unwrap_or_else(|| "Custom Platform".to_string());
        let editor_url = platform_config.get_str("editor_url").unwrap_or_default();
        let format = match platform_config
            .get_str("format")
            .as_deref()
            .unwrap_or("html")
        {
            "markdown" | "md" => CopyFormat::Markdown,
            _ => CopyFormat::StyledHtml,
        };
        // Leak the strings so they have 'static lifetime.
        // These are one-time config-driven allocations per custom platform.
        let name: &'static str = Box::leak(name.into_boxed_str());
        let editor_url: &'static str = Box::leak(editor_url.into_boxed_str());
        // Parse math_rendering from config, fallback to format-based default
        let math_rendering = platform_config
            .get_str("math_rendering")
            .and_then(|s| match s.as_str() {
                "svg" => Some(MathRendering::Svg),
                "latex" => Some(MathRendering::Latex),
                "png" => Some(MathRendering::Png),
                _ => None,
            })
            .unwrap_or(match format {
                CopyFormat::Markdown => MathRendering::Latex,
                CopyFormat::StyledHtml => MathRendering::Svg,
            });
        // Manual platforms default to false for use_inline_html_for_sized_images
        let use_inline_html_for_sized_images = platform_config
            .get_str("use_inline_html_for_sized_images")
            .map(|s| s == "true")
            .unwrap_or(false);
        // Manual platforms default to empty markdown processing rules
        let markdown_processing_rules = MarkdownProcessingRules::empty();
        // Manual platforms default to true for tight_lists
        let tight_lists = platform_config
            .get_str("tight_lists")
            .map(|s| s == "true")
            .unwrap_or(true);
        Self::build(
            id,
            name,
            editor_url,
            format,
            typub_html::SerializeRules::empty(), // manual platforms have no serialize rules
            None,                                // manual platforms have no profile default_theme
            AssetStrategy::Embed,                // manual platforms default to embed
            MathDelimiters::Dollar,              // manual platforms default to dollar
            math_rendering,
            use_inline_html_for_sized_images,
            markdown_processing_rules,
            tight_lists,
            Some(platform_config),
            config,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn build(
        id: &'static str,
        name: &'static str,
        editor_url: &'static str,
        format: CopyFormat,
        serialize_rules: typub_html::SerializeRules,
        profile_default_theme: Option<&'static str>,
        profile_default_asset_strategy: AssetStrategy,
        math_delimiters: MathDelimiters,
        math_rendering: MathRendering,
        use_inline_html_for_sized_images: bool,
        markdown_processing_rules: MarkdownProcessingRules,
        tight_lists: bool,
        platform_config: Option<&PlatformConfig>,
        config: &Config,
    ) -> Result<Self> {
        let output_dir = platform_config
            .and_then(|c| c.get_str("output_dir"))
            .map(PathBuf::from)
            .unwrap_or_else(|| config.output_dir.join(id));

        let registry = ThemeRegistry::new()?;
        // Fallback theme when 5-level resolution finds nothing
        let fallback_theme = registry.get_or_default("elegant")?.clone();

        // Copy-paste platforms support Embed (base64 data URIs) and External (S3/R2 URLs).
        // Per [[RFC-0004:C-EXTERNAL-STRATEGY]]
        // Use profile's default instead of hardcoded Embed
        let asset_strategy = resolve_asset_strategy_with_policy(
            id,
            platform_config,
            profile_default_asset_strategy,
            &[AssetStrategy::Embed, AssetStrategy::External],
        )?;

        Ok(Self {
            profile_id: id,
            profile_name: name,
            editor_url,
            format,
            serialize_rules,
            output_dir,
            fallback_theme,
            profile_default_theme,
            theme_registry: registry,
            asset_strategy,
            math_delimiters,
            math_rendering,
            use_inline_html_for_sized_images,
            markdown_processing_rules,
            tight_lists,
        })
    }

    /// Whether to use syntax-highlighted HTML in code blocks.
    /// Per [[ADR-0001]]. Derived from format: HTML profiles use highlighting,
    /// Markdown profiles don't need it (code blocks are native).
    pub fn code_highlight(&self) -> bool {
        matches!(self.format, CopyFormat::StyledHtml)
    }

    /// Build a base64 data-URI map for all assets.
    fn build_embed_asset_map(
        &self,
        content_info: &ContentInfo,
    ) -> Result<HashMap<PathBuf, String>> {
        let mut url_map = HashMap::new();
        for asset_path in &content_info.assets {
            let data = std::fs::read(asset_path)?;
            let data_uri = to_data_uri(&data, asset_path);
            url_map.insert(asset_path.clone(), data_uri);
        }
        Ok(url_map)
    }

    /// Finalize as styled HTML (WeChat, Zhihu, Toutiao, …).
    ///
    /// Applies theme CSS inlining. Serialization rules are applied at serialization
    /// time via SerializeOptions, not here.
    fn finalize_styled_html(&self, body: &str, theme: &Theme) -> Result<String> {
        apply_theme(body, theme, true)
    }

    /// Load theme using pre-resolved theme_id from ResolvedConfig.
    fn load_theme(&self, theme_id: Option<&str>) -> Theme {
        load_theme(
            theme_id,
            self.profile_default_theme,
            &self.theme_registry,
            &self.fallback_theme,
        )
    }

    /// Generate the preview HTML page with copy button and editor link.
    fn build_preview_html(&self, title: &str, theme_name: &str, content_html: &str) -> String {
        // Hardcoded i18n strings (extracted from main crate)
        let copy_failed_msg = "Copy failed";
        let copy_button = "Copy";
        let copy_success = "Copied!";
        let preview_suffix = "Preview";
        let open_editor = "Open Editor";

        let (copy_script, content_area) = match self.format {
            CopyFormat::StyledHtml => (
                styled_html_copy_script(copy_failed_msg),
                format!(r#"<div id="content-area">{}</div>"#, content_html),
            ),
            CopyFormat::Markdown => (
                markdown_copy_script(copy_failed_msg),
                format!(
                    r#"<pre id="content-area" style="white-space:pre-wrap;word-wrap:break-word;font-family:monospace;font-size:14px;background:#f8f8f8;padding:16px;border-radius:4px;overflow-x:auto;">{}</pre>"#,
                    html_escape(content_html)
                ),
            ),
        };

        let editor_link = if self.editor_url.is_empty() {
            String::new()
        } else {
            format!(
                r#"<a href="{url}" target="_blank" rel="noopener" style="display:inline-block;background:#555;color:white;border:none;padding:10px 20px;border-radius:4px;cursor:pointer;font-size:14px;font-weight:500;text-decoration:none;">{label}</a>"#,
                url = self.editor_url,
                label = open_editor
            )
        };

        // Load preview CSS (builtins embedded at compile time, user override supported)
        let preview_css = self
            .theme_registry
            .load_preview_css("copypaste")
            .unwrap_or_default();

        format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>{title} - {platform} {preview_suffix}</title>
    <style>
{preview_css}
    </style>
</head>
<body>
    <div class="toolbar">
        <button class="copy-btn" onclick="copyContent()">{copy_button}</button>
        {editor_link}
        <span class="theme-info">{platform} · {theme_name}</span>
        <span class="copy-success" id="copy-success">{copy_success}</span>
    </div>
    <div class="preview-container">
        <div class="preview-header">
            <div class="preview-title">{title}</div>
        </div>
        {content_area}
    </div>
    <script>
{copy_script}
    </script>
</body>
</html>"#,
            preview_css = preview_css,
            title = title,
            platform = self.profile_name,
            preview_suffix = preview_suffix,
            theme_name = theme_name,
            copy_button = copy_button,
            copy_success = copy_success,
            editor_link = editor_link,
            content_area = content_area,
            copy_script = copy_script,
        )
    }

    /// Build preview from AST elements.
    ///
    /// Applies serialization rules, theming, then generates preview HTML.
    /// Per [[RFC-0002:C-PIPELINE-STAGES]], serialization rules are applied at serialization.
    fn build_preview_from_elements(
        &self,
        content_info: &ContentInfo,
        elements: Document,
        theme: &Theme,
    ) -> Result<PathBuf> {
        let serialize_options = SerializeOptions {
            li_span_wrap: self.serialize_rules.contains(SerializeRule::LiSpanWrap),
            use_code_highlight: self.code_highlight(),
            blockquote_for_admonition: self
                .serialize_rules
                .contains(SerializeRule::BlockquoteForAdmonition),
            sibling_nested_lists: self
                .serialize_rules
                .contains(SerializeRule::SiblingNestedLists),
            definition_list_to_paragraph: self
                .serialize_rules
                .contains(SerializeRule::DefinitionListToParagraph),
        };
        let body = document_to_html_with_options(&elements, &serialize_options);

        let preview_content = match self.format {
            CopyFormat::StyledHtml => apply_theme(&body, theme, true)?,
            CopyFormat::Markdown => {
                let options = MarkdownRenderOptions {
                    math_delimiters: self.math_delimiters,
                    use_inline_html_for_sized_images: self.use_inline_html_for_sized_images,
                    processing_rules: self.markdown_processing_rules,
                    tight_lists: self.tight_lists,
                    ..Default::default()
                };
                document_to_markdown_with_options(&elements, &options)?
            }
        };

        let preview_html =
            self.build_preview_html(&content_info.title, &theme.name, &preview_content);

        write_preview_file(&content_info.slug, self.profile_id, &preview_html)
    }
}

#[async_trait(?Send)]
impl PlatformAdapter for CopyPasteAdapter {
    fn id(&self) -> &'static str {
        self.profile_id
    }

    fn name(&self) -> &'static str {
        self.profile_name
    }

    fn required_format(&self) -> OutputFormat {
        OutputFormat::Html
    }

    fn asset_strategy(&self) -> AssetStrategy {
        self.asset_strategy
    }

    fn validate_config(&self, _config: &PlatformConfig) -> Result<()> {
        Ok(()) // No API keys needed.
    }

    /// Copypaste adapters support internal link rewriting via cross-platform resolution.
    fn supports_shared_link_rewrite(&self) -> bool {
        true
    }

    fn render_config(&self, _content_info: &ContentInfo) -> RenderConfig {
        render_config_for_png_math(self.asset_strategy, self.math_rendering)
    }

    /// Stage 5 (Specialize): resolve image URLs and flatten SVGs in AST.
    async fn specialize_payload(
        &self,
        elements: Document,
        ctx: &dyn AdapterContext,
    ) -> Result<AdapterPayload> {
        let content_info = ctx.content_info();

        // Flatten SVG <use> references for clipboard compatibility.
        // WeChat and similar platforms may not preserve xlink:href references.
        // Only needed for HTML output; Markdown platforms extract math_src as LaTeX.
        let mut elements = match self.format {
            CopyFormat::StyledHtml => {
                let mut doc = elements;
                let mut pass = FlattenSvgPass;
                run_passes(&mut doc, &mut PassCtx::default(), &mut [&mut pass])?;
                doc
            }
            CopyFormat::Markdown => elements,
        };

        // Handle PNG math rendering based on asset strategy.
        // Per [[WI-2026-02-17-001]].
        (elements, _) = convert_png_math_for_strategy(
            elements,
            self.asset_strategy,
            self.math_rendering,
            &content_info.path,
            &content_info.slug,
        )?;

        // Handle assets based on strategy
        let deferred = if self.asset_strategy == AssetStrategy::External {
            // External strategy: use helper to prepare deferred assets
            prepare_deferred_assets(self.asset_strategy, &elements, &content_info.path)
        } else {
            // Embed strategy: resolve base64 data URIs into AST immediately
            let asset_map = self.build_embed_asset_map(content_info)?;
            let url_map = build_image_marker_url_map(&content_info.path, &asset_map);
            resolve_asset_urls(&mut elements, &url_map);
            DeferredAssets::empty()
        };

        Ok(AdapterPayload::new(
            CopyPastePayload {
                slug: content_info.slug.clone(),
                content: String::new(), // Filled by Serialize stage
                format: self.format,
            },
            content_info.clone(),
            deferred,
            elements,
        ))
    }

    /// Stage 7 (Materialize): upload assets for External strategy.
    /// Per [[RFC-0004:C-PIPELINE-INTEGRATION]]
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

        if self.asset_strategy == AssetStrategy::External {
            materialize_and_resolve_urls(&mut payload, ctx).await?;
        }

        Ok(payload)
    }

    /// Stage 8 (Serialize): convert AST to target format (styled HTML or Markdown).
    async fn serialize_payload(
        &self,
        mut payload: AdapterPayload,
        ctx: &dyn AdapterContext,
    ) -> Result<AdapterPayload> {
        let final_content = match self.format {
            CopyFormat::StyledHtml => {
                // Apply serialization rules and code highlighting from profile config
                let serialize_options = SerializeOptions {
                    li_span_wrap: self.serialize_rules.contains(SerializeRule::LiSpanWrap),
                    use_code_highlight: self.code_highlight(),
                    blockquote_for_admonition: self
                        .serialize_rules
                        .contains(SerializeRule::BlockquoteForAdmonition),
                    sibling_nested_lists: self
                        .serialize_rules
                        .contains(SerializeRule::SiblingNestedLists),
                    definition_list_to_paragraph: self
                        .serialize_rules
                        .contains(SerializeRule::DefinitionListToParagraph),
                };
                let body_html =
                    document_to_html_with_options(&payload.document, &serialize_options);
                // Use pre-resolved theme_id from AdapterContext
                let theme = self.load_theme(ctx.theme_id());
                self.finalize_styled_html(&body_html, &theme)?
            }
            CopyFormat::Markdown => {
                let options = MarkdownRenderOptions {
                    math_delimiters: self.math_delimiters,
                    use_inline_html_for_sized_images: self.use_inline_html_for_sized_images,
                    processing_rules: self.markdown_processing_rules,
                    tight_lists: self.tight_lists,
                    ..Default::default()
                };
                document_to_markdown_with_options(&payload.document, &options)?
            }
        };

        let inner = payload
            .downcast_mut::<CopyPastePayload>()
            .ok_or_else(|| anyhow::anyhow!("Invalid CopyPaste payload type"))?;
        inner.content = final_content;
        Ok(payload)
    }

    async fn publish_payload(
        &self,
        payload: AdapterPayload,
        _ctx: &dyn AdapterContext,
    ) -> Result<PublishResult> {
        let payload = downcast_payload::<CopyPastePayload>(payload, self.profile_name)?;
        std::fs::create_dir_all(&self.output_dir)?;

        let ext = match payload.format {
            CopyFormat::StyledHtml => "html",
            CopyFormat::Markdown => "md",
        };
        let output_path = self.output_dir.join(format!("{}.{}", payload.slug, ext));
        std::fs::write(&output_path, &payload.content)?;

        debug!("Saved to: {}", output_path.display());

        // Copy content to clipboard
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|e| anyhow::anyhow!("Failed to access clipboard: {}", e))?;
        match payload.format {
            CopyFormat::StyledHtml => {
                clipboard
                    .set_html(&payload.content, Some(&payload.content))
                    .map_err(|e| anyhow::anyhow!("Failed to copy to clipboard: {}", e))?;
            }
            CopyFormat::Markdown => {
                clipboard
                    .set_text(&payload.content)
                    .map_err(|e| anyhow::anyhow!("Failed to copy to clipboard: {}", e))?;
            }
        }
        info!("Copied to clipboard for {}", self.profile_name);

        // Open editor URL
        if !self.editor_url.is_empty()
            && let Err(e) = open::that(self.editor_url)
        {
            warn!("Failed to open {}: {}", self.editor_url, e);
        }

        Ok(PublishResult {
            url: Some(self.editor_url.to_string()),
            platform_id: Some(payload.slug),
            published_at: Utc::now(),
        })
    }

    /// Build preview from pre-parsed AST (used by unified pipeline).
    ///
    /// Receives AST that has already been parsed and transformed by shared pipeline stages.
    /// Applies copypaste-specific transforms (SVG flattening, theming) and generates preview.
    fn build_preview(
        &self,
        title: &str,
        elements: Document,
        ctx: &dyn AdapterContext,
    ) -> Result<PathBuf> {
        let _ = title; // We use content_info.title instead for consistency
        let content_info = ctx.content_info();
        let theme = self.load_theme(ctx.theme_id());
        self.build_preview_from_elements(content_info, elements, &theme)
    }

    async fn check_status(&self, slug: &str) -> Result<bool> {
        let ext = match self.format {
            CopyFormat::StyledHtml => "html",
            CopyFormat::Markdown => "md",
        };
        let output_path = self.output_dir.join(format!("{}.{}", slug, ext));
        Ok(output_path.exists())
    }
}

// ============================================================================
// HTML helpers
// ============================================================================

/// Minimal HTML escaping for text rendered inside `<pre>`.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Generate copy script for StyledHtml: writes rich HTML to clipboard via ClipboardItem.
fn styled_html_copy_script(copy_failed_msg: &str) -> String {
    format!(
        r#"
        async function copyContent() {{
            const content = document.getElementById('content-area');
            const htmlContent = content.innerHTML;
            try {{
                await navigator.clipboard.write([
                    new ClipboardItem({{
                        'text/html': new Blob([htmlContent], {{ type: 'text/html' }}),
                        'text/plain': new Blob([content.textContent || ''], {{ type: 'text/plain' }})
                    }})
                ]);
                showSuccess();
            }} catch (err) {{
                console.error('Clipboard API failed:', err);
                const range = document.createRange();
                range.selectNodeContents(content);
                const sel = window.getSelection();
                sel.removeAllRanges();
                sel.addRange(range);
                try {{ document.execCommand('copy'); showSuccess(); }}
                catch (e) {{ alert('{copy_failed_msg}'); }}
                sel.removeAllRanges();
            }}
        }}
        function showSuccess() {{
            const el = document.getElementById('copy-success');
            el.classList.add('show');
            setTimeout(() => el.classList.remove('show'), 2000);
        }}
"#,
        copy_failed_msg = copy_failed_msg
    )
}

/// Generate copy script for Markdown: writes plain text to clipboard.
fn markdown_copy_script(copy_failed_msg: &str) -> String {
    format!(
        r#"
        async function copyContent() {{
            const content = document.getElementById('content-area');
            try {{
                await navigator.clipboard.writeText(content.textContent || '');
                showSuccess();
            }} catch (err) {{
                console.error('Clipboard API failed:', err);
                const range = document.createRange();
                range.selectNodeContents(content);
                const sel = window.getSelection();
                sel.removeAllRanges();
                sel.addRange(range);
                try {{ document.execCommand('copy'); showSuccess(); }}
                catch (e) {{ alert('{copy_failed_msg}'); }}
                sel.removeAllRanges();
            }}
        }}
        function showSuccess() {{
            const el = document.getElementById('copy-success');
            el.classList.add('show');
            setTimeout(() => el.classList.remove('show'), 2000);
        }}
"#,
        copy_failed_msg = copy_failed_msg
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
        assert_eq!(html_escape("a & b"), "a &amp; b");
        assert_eq!(html_escape("say \"hi\""), "say &quot;hi&quot;");
    }
}
