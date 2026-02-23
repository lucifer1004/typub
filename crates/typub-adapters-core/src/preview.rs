//! Unified preview building for adapters.
//!
//! Provides a common preview HTML builder that handles:
//! - MathJax rendering for LaTeX math (data-latex-src attributes)
//! - Highlight.js for client-side syntax highlighting
//! - Theme styling and code highlighting
//! - Platform-specific branding

use anyhow::Result;
use std::path::PathBuf;
use typub_ir::Document;

use crate::{ContentInfo, write_preview_file};

/// Build a unified preview HTML file for any platform.
///
/// This function generates a preview HTML file with:
/// - MathJax support for rendering LaTeX math formulas
/// - Highlight.js for client-side syntax highlighting
/// - Theme CSS for styling
/// - Platform-specific branding (name, optional styling)
///
/// # Arguments
/// * `document` - The semantic document to render
/// * `content_info` - Content metadata (slug, title)
/// * `platform_id` - Platform identifier (e.g., "confluence", "wordpress")
/// * `platform_name` - Human-readable platform name (e.g., "Confluence")
/// * `theme_css` - Optional theme CSS for styling
/// * `branding` - Optional platform-specific branding (logo, colors)
///
/// # Returns
/// Path to the generated preview file
pub fn build_unified_preview(
    document: &Document,
    content_info: &ContentInfo,
    platform_id: &str,
    platform_name: &str,
    theme_css: Option<&str>,
    branding: Option<&PlatformBranding>,
) -> Result<PathBuf> {
    // Serialize semantic document to HTML (client-side hljs handles highlighting fallback).
    let body = typub_html::document_to_html(document);

    let branding = branding.cloned().unwrap_or_default();
    let theme_css = theme_css.unwrap_or("");

    // MathJax configuration for rendering data-latex-src attributes
    let mathjax_script = r#"
<script>
window.MathJax = {
  tex: {
    inlineMath: [['$', '$']],
    displayMath: [['$$', '$$']]
  },
  startup: {
    ready: function() {
      // Extract LaTeX from data-latex-src attributes (from MathRendering::Latex)
      document.querySelectorAll('[data-latex-src]').forEach(el => {
        const latex = el.getAttribute('data-latex-src');
        const isBlock = el.classList.contains('typst-svg-block');
        el.innerHTML = isBlock ? '$$' + latex + '$$' : '$' + latex + '$';
      });
      MathJax.startup.defaultReady();
    }
  }
};
</script>
<script src="https://cdn.jsdelivr.net/npm/mathjax@3/es5/tex-mml-chtml.js" async></script>
"#;

    // Highlight.js for client-side syntax highlighting
    let highlight_script = r#"
<link rel="stylesheet" href="https://cdn.jsdelivr.net/gh/highlightjs/cdn-release@11.9.0/build/styles/github.min.css">
<script src="https://cdn.jsdelivr.net/gh/highlightjs/cdn-release@11.9.0/build/highlight.min.js"></script>
<script>
document.addEventListener('DOMContentLoaded', function() {
  // Highlight all code blocks that don't already have highlighting
  document.querySelectorAll('pre code').forEach(block => {
    // Only highlight if not already highlighted (no span children from server-side)
    if (!block.querySelector('span')) {
      hljs.highlightElement(block);
    }
  });
});
</script>
"#;

    let preview_html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>{title} - {platform_name} Preview</title>
    <style>
        /* Base styles */
        :root {{
            --brand-color: {brand_color};
            --brand-bg: {brand_bg};
        }}
        * {{ box-sizing: border-box; }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
            max-width: 800px;
            margin: 0 auto !important;
            padding: 20px !important;
            color: #333;
            line-height: 1.6;
        }}
        h1 {{ font-size: 2em; font-weight: 700; margin-bottom: 0.5em; }}
        h2 {{ font-size: 1.5em; font-weight: 600; margin-top: 1.5em; }}
        h3 {{ font-size: 1.2em; font-weight: 600; margin-top: 1em; }}
        p {{ margin: 1em 0; }}
        pre {{
            background: #f6f8fa;
            padding: 16px;
            border-radius: 6px;
            overflow-x: auto;
            font-family: "SFMono-Regular", Consolas, "Liberation Mono", Menlo, monospace;
            font-size: 0.875em;
        }}
        code {{
            font-family: "SFMono-Regular", Consolas, "Liberation Mono", Menlo, monospace;
            background: #f6f8fa;
            padding: 0.2em 0.4em;
            border-radius: 3px;
            font-size: 0.875em;
        }}
        pre code {{
            background: none;
            padding: 0;
        }}
        blockquote {{
            border-left: 4px solid #ddd;
            margin: 1em 0;
            padding-left: 1em;
            color: #666;
        }}
        img {{ max-width: 100%; height: auto; }}
        table {{
            border-collapse: collapse;
            width: 100%;
            margin: 1em 0;
        }}
        th, td {{
            border: 1px solid #ddd;
            padding: 8px 12px;
            text-align: left;
        }}
        th {{ background: #f6f8fa; font-weight: 600; }}
        ul, ol {{ padding-left: 2em; }}
        li {{ margin: 0.25em 0; }}
        hr {{ border: none; border-top: 1px solid #eee; margin: 2em 0; }}

        /* Platform branding header */
        .platform-header {{
            display: flex;
            align-items: center;
            gap: 12px;
            padding-bottom: 16px;
            border-bottom: 1px solid #eee;
            margin-bottom: 24px;
        }}
        .platform-badge {{
            background: var(--brand-bg);
            color: var(--brand-color);
            padding: 4px 12px;
            border-radius: 4px;
            font-size: 0.75em;
            font-weight: 600;
            text-transform: uppercase;
            letter-spacing: 0.5px;
        }}

        /* SVG math styling */
        .typst-svg-block svg {{
            display: block;
            margin: 1em auto;
            max-width: 100%;
        }}

        /* Theme CSS (for code highlighting) */
{theme_css}
    </style>
    {highlight_script}
    {mathjax_script}
</head>
<body>
    <div class="platform-header">
        <span class="platform-badge">{platform_name}</span>
    </div>
    <article class="content">
        <h1>{title}</h1>
        {body}
    </article>
</body>
</html>"#,
        title = content_info.title,
        platform_name = platform_name,
        brand_color = branding.brand_color,
        brand_bg = branding.brand_bg,
        theme_css = theme_css,
        mathjax_script = mathjax_script,
        highlight_script = highlight_script,
        body = body
    );

    write_preview_file(&content_info.slug, platform_id, &preview_html)
}

/// Platform-specific branding for preview.
#[derive(Debug, Clone, Default)]
pub struct PlatformBranding {
    /// Brand color (text)
    pub brand_color: String,
    /// Brand background color
    pub brand_bg: String,
}

impl PlatformBranding {
    /// Create a new branding with custom colors.
    pub fn new(brand_color: impl Into<String>, brand_bg: impl Into<String>) -> Self {
        Self {
            brand_color: brand_color.into(),
            brand_bg: brand_bg.into(),
        }
    }
}
