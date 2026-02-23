//! Platform-specific serialization options.
//!
//! These rules control HTML output generation at serialization time.
//! SerializeRule provides a declarative way to specify which output
//! variations each platform needs.

use enumset::{EnumSet, EnumSetType};

/// Platform-specific serialization rules.
///
/// These rules are applied during Stage-5 (Serialize) to control HTML output.
/// Each rule can be enabled per-platform via the `serialize_rules` field
/// in `profiles.toml`.
#[derive(EnumSetType, Debug)]
pub enum SerializeRule {
    /// Wrap `<li>` content in `<span style="display:inline;">`.
    ///
    /// Needed for: WeChat (prevents text splitting in list items).
    /// WeChat's editor sometimes breaks list item content across multiple
    /// elements; wrapping in an inline span keeps the content together.
    LiSpanWrap,
    /// Use `<blockquote>` for admonitions (instead of `<div>`).
    ///
    /// Needed for: WeChat, Weibo (these platforms strip `<div>` and `<section>` tags).
    /// When enabled, admonitions render as `<blockquote class="admonition note">`
    /// instead of `<div class="admonition note">`. The `<blockquote>` tag is
    /// preserved by editors that filter other container elements.
    BlockquoteForAdmonition,
    /// Use sibling `<ul>`/`<ol>` instead of nested children of `<li>`.
    ///
    /// Needed for: WeChat (its ProseMirror editor transforms nested `<ul>` incorrectly,
    /// causing reordering for 3+ levels of nesting).
    ///
    /// Standard HTML: `<ul><li>Item<ul><li>Nested</li></ul></li></ul>`
    /// WeChat's structure: `<ul><li>Item</li><ul><li>Nested</li></ul></ul>`
    ///
    /// Note: This produces technically invalid HTML (per spec, `<ul>` can only
    /// contain `<li>` as direct children), but it's what WeChat's editor expects.
    SiblingNestedLists,
    /// Convert definition lists (`<dl>`) to paragraphs.
    ///
    /// Needed for: WeChat, Weibo (these platforms don't support `<dl>`, `<dt>`, `<dd>` tags).
    /// When enabled, each definition item renders as:
    /// `<p><strong>Term</strong>: Definition</p>`
    /// instead of:
    /// `<dl><dt>Term</dt><dd>Definition</dd></dl>`
    DefinitionListToParagraph,
}

/// A set of serialization rules to apply.
pub type SerializeRules = EnumSet<SerializeRule>;

/// Parse serialization rules from a string slice array.
///
/// Used by build.rs to convert TOML array values to SerializeRules.
/// Unknown rule names are silently ignored.
pub fn parse_serialize_rules(names: &[&str]) -> SerializeRules {
    let mut rules = SerializeRules::empty();
    for name in names {
        // Unknown rules are silently ignored
        if *name == "li_span_wrap" {
            rules |= SerializeRule::LiSpanWrap;
        }
        if *name == "blockquote_for_admonition" {
            rules |= SerializeRule::BlockquoteForAdmonition;
        }
        if *name == "sibling_nested_lists" {
            rules |= SerializeRule::SiblingNestedLists;
        }
        if *name == "definition_list_to_paragraph" {
            rules |= SerializeRule::DefinitionListToParagraph;
        }
    }
    rules
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_serialize_rules() {
        let rules = parse_serialize_rules(&["li_span_wrap"]);
        assert!(rules.contains(SerializeRule::LiSpanWrap));
    }

    #[test]
    fn test_parse_blockquote_for_admonition() {
        let rules = parse_serialize_rules(&["blockquote_for_admonition"]);
        assert!(rules.contains(SerializeRule::BlockquoteForAdmonition));
    }

    #[test]
    fn test_parse_sibling_nested_lists() {
        let rules = parse_serialize_rules(&["sibling_nested_lists"]);
        assert!(rules.contains(SerializeRule::SiblingNestedLists));
    }

    #[test]
    fn test_parse_definition_list_to_paragraph() {
        let rules = parse_serialize_rules(&["definition_list_to_paragraph"]);
        assert!(rules.contains(SerializeRule::DefinitionListToParagraph));
    }

    #[test]
    fn test_parse_unknown_rule() {
        let rules = parse_serialize_rules(&["unknown_rule"]);
        assert!(rules.is_empty());
    }

    #[test]
    fn test_parse_multiple_rules() {
        let rules =
            parse_serialize_rules(&["li_span_wrap", "blockquote_for_admonition", "unknown"]);
        assert_eq!(rules.len(), 2);
        assert!(rules.contains(SerializeRule::LiSpanWrap));
        assert!(rules.contains(SerializeRule::BlockquoteForAdmonition));
    }
}
