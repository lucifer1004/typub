//! Markdown rendering options.
//!
//! Provides configuration options for the Markdown renderer.

use std::collections::BTreeMap;
use typub_core::MathDelimiters;
use typub_ir::{AssetId, Url};

use crate::processing::MarkdownProcessingRules;

/// Options for Markdown rendering.
pub struct MarkdownRenderOptions<'a> {
    /// Optional mapping from asset id to a resolved URL.
    /// If provided, this takes precedence over IR asset variants/source.
    pub asset_urls: Option<&'a BTreeMap<AssetId, Url>>,
    /// Math delimiter syntax to use for LaTeX math.
    /// Default is `Dollar` ($...$ and $$...$$).
    pub math_delimiters: MathDelimiters,
    /// Whether to use inline HTML (`<img>` tags) for images with dimensions.
    /// Standard Markdown doesn't support width/height attributes on images.
    /// When true, images with width/height attrs will be rendered as `<img>` tags.
    /// When false (default), dimensions are ignored and standard `![alt](url)` syntax is used.
    pub use_inline_html_for_sized_images: bool,
    /// Whether lists should be tight (no blank lines between items).
    /// Default is true. Set to false for loose lists with blank lines.
    pub tight_lists: bool,
    /// Markdown post-processing rules.
    /// Applied after serialization to handle platform-specific editor quirks.
    pub processing_rules: MarkdownProcessingRules,
}

impl<'a> Default for MarkdownRenderOptions<'a> {
    fn default() -> Self {
        Self {
            asset_urls: None,
            math_delimiters: MathDelimiters::Dollar,
            use_inline_html_for_sized_images: false,
            tight_lists: true,
            processing_rules: MarkdownProcessingRules::empty(),
        }
    }
}
