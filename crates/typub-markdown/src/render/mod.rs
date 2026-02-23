//! Markdown rendering from typub semantic `Document` IR.
//!
//! Converts typub semantic IR into Markdown via a Markdown AST (comrak),
//! then delegates final string serialization to comrak formatter.
//!
//! ## Module Structure
//!
//! - [`options`] - Rendering configuration options
//! - [`inline`] - Inline rendering utilities
//! - [`block`] - Block/document rendering utilities

mod block;
mod inline;
mod options;

#[cfg(test)]
mod tests;

pub use block::{document_to_markdown, document_to_markdown_with_options};
pub use inline::{
    inline_text, inlines_text, push_inline_seq, push_text, resolve_rendered_asset_url,
};
pub use options::MarkdownRenderOptions;
