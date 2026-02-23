//! Markdown post-processing rules.
//!
//! These rules are applied after Markdown serialization to handle
//! platform-specific quirks that cannot be addressed at the AST level.

use enumset::{EnumSet, EnumSetType};

/// Markdown post-processing rules.
///
/// Applied after serialization to handle platform-specific editor quirks.
/// Each rule can be enabled per-platform via the `markdown_processing_rules`
/// field in `profiles.toml`.
#[derive(EnumSetType, Debug)]
pub enum MarkdownProcessingRule {
    /// No-op placeholder for future extension.
    Noop,
}

/// A set of markdown processing rules to apply.
pub type MarkdownProcessingRules = EnumSet<MarkdownProcessingRule>;

/// Parse markdown processing rules from a string slice array.
///
/// Used by build.rs to convert TOML array values to MarkdownProcessingRules.
/// Unknown rule names are silently ignored.
pub fn parse_markdown_processing_rules(names: &[&str]) -> MarkdownProcessingRules {
    let mut rules = MarkdownProcessingRules::empty();
    for name in names {
        if *name == "noop" {
            rules |= MarkdownProcessingRule::Noop;
        }
    }
    rules
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_noop() {
        let rules = parse_markdown_processing_rules(&["noop"]);
        assert!(rules.contains(MarkdownProcessingRule::Noop));
    }

    #[test]
    fn test_parse_unknown_rule() {
        let rules = parse_markdown_processing_rules(&["unknown_rule"]);
        assert!(rules.is_empty());
    }
}
