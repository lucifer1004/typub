//! HTML processing utilities for typub semantic IR v2.

mod builders;
mod parse;
mod serialize;
mod serialize_rules;

pub use builders::*;
pub use parse::parse_html_document;
pub use serialize::{
    SerializeOptions, document_to_html, document_to_html_with_options, escape_html_attr,
    escape_html_text, inlines_text, inlines_to_html, inlines_to_html_with_options,
};
pub use serialize_rules::{SerializeRule, SerializeRules, parse_serialize_rules};
