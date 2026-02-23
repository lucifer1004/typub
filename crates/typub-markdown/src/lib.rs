//! Markdown rendering and processing utilities for typub.
//!
//! This crate provides:
//! - Document IR to Markdown conversion (`render` module)
//! - Typst to LaTeX math conversion (`latex` module)
//! - Post-processing rules for platform-specific editor quirks (`processing` module)

pub mod latex;
pub mod processing;
pub mod render;

// Re-export main types at crate root
pub use latex::typst_math_to_latex;
pub use processing::{
    MarkdownProcessingRule, MarkdownProcessingRules, parse_markdown_processing_rules,
};
pub use render::{MarkdownRenderOptions, document_to_markdown, document_to_markdown_with_options};
