use typub_adapters_core::MarkdownProcessingRules;
use typub_core::{AssetStrategy, MathDelimiters, MathRendering};
use typub_html::{SerializeRule, SerializeRules};

// ============================================================================
// Copy format
// ============================================================================

/// How content should be prepared for the clipboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyFormat {
    /// Paste as rich text — inline-styled HTML via ClipboardItem.
    StyledHtml,
    /// Paste into a Markdown editor — plain-text Markdown.
    Markdown,
}

// ============================================================================
// Built-in profiles (generated from profiles.toml by build.rs)
// ============================================================================

/// A built-in copy-paste profile, generated at compile time from
/// `profiles.toml`.  All string fields are `&'static str`
/// literals baked into the binary — zero runtime allocation.
pub struct BuiltinProfile {
    pub id: &'static str,
    pub name: &'static str,
    /// Short code for compact display (2-3 characters).
    pub short_code: &'static str,
    pub editor_url: &'static str,
    pub format: CopyFormat,
    /// Serialization rules to apply per [[RFC-0002:C-PIPELINE-STAGES]].
    /// Replaces the legacy compat function approach.
    pub serialize_rules: SerializeRules,
    /// Default theme for this profile (layer 5 in theme resolution chain).
    pub default_theme: Option<&'static str>,
    /// Supported asset strategies. First element is the default.
    /// Per [[WI-2026-02-18-002]].
    pub asset_strategies: &'static [AssetStrategy],
    /// Supported math delimiter syntaxes. First element is the default.
    /// Only relevant when format is Markdown. Per [[WI-2026-02-18-002]].
    pub math_delimiters: &'static [MathDelimiters],
    /// Supported math rendering options. First element is the default.
    /// Per [[WI-2026-02-13-026]], Png variant rasterizes SVG to PNG for platforms
    /// that support base64 images but not inline SVG. Per [[WI-2026-02-18-002]].
    pub math_renderings: &'static [MathRendering],
    /// Whether to use inline HTML (`<img>` tags) for images with dimensions.
    /// Standard Markdown doesn't support width/height attributes on images.
    /// When true, images with dimensions will be rendered as `<img>` tags.
    /// Only relevant when format is Markdown.
    pub use_inline_html_for_sized_images: bool,
    /// Markdown post-processing rules to apply after serialization.
    /// Used for platform-specific editor quirks like blank line stripping.
    /// Only relevant when format is Markdown.
    pub markdown_processing_rules: MarkdownProcessingRules,
    /// Whether lists should be tight (no blank lines between items).
    /// Default is true. Set to false for loose lists with blank lines.
    /// Only relevant when format is Markdown.
    pub tight_lists: bool,
}

impl BuiltinProfile {
    /// Returns the default asset strategy (first element of `asset_strategies`).
    pub fn default_asset_strategy(&self) -> AssetStrategy {
        self.asset_strategies[0]
    }

    /// Returns the default math delimiters (first element of `math_delimiters`).
    pub fn default_math_delimiters(&self) -> MathDelimiters {
        self.math_delimiters[0]
    }

    /// Returns the default math rendering (first element of `math_renderings`).
    pub fn default_math_rendering(&self) -> MathRendering {
        self.math_renderings[0]
    }

    /// Checks if a given asset strategy is supported by this profile.
    pub fn supports_asset_strategy(&self, strategy: AssetStrategy) -> bool {
        self.asset_strategies.contains(&strategy)
    }

    /// Checks if a given math delimiter is supported by this profile.
    pub fn supports_math_delimiter(&self, delimiter: MathDelimiters) -> bool {
        self.math_delimiters.contains(&delimiter)
    }

    /// Checks if a given math rendering is supported by this profile.
    pub fn supports_math_rendering(&self, rendering: MathRendering) -> bool {
        self.math_renderings.contains(&rendering)
    }
}

// Include the generated static array.
include!(concat!(env!("OUT_DIR"), "/builtin_profiles.rs"));

/// Look up a built-in profile by ID.
pub fn find_profile(id: &str) -> Option<&'static BuiltinProfile> {
    BUILTIN_PROFILES.iter().find(|p| p.id == id)
}

/// Return all built-in profile IDs.
pub fn known_profile_ids() -> impl Iterator<Item = &'static str> {
    BUILTIN_PROFILES.iter().map(|p| p.id)
}

/// Return all built-in profiles.
pub fn all_profiles() -> &'static [BuiltinProfile] {
    BUILTIN_PROFILES
}

// ============================================================================
// Payload types
// ============================================================================

/// Payload produced by `specialize_payload`, consumed by `publish_payload`.
#[derive(Debug)]
pub struct CopyPastePayload {
    pub slug: String,
    pub content: String,
    pub format: CopyFormat,
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use typub_html::SerializeRule;

    #[test]
    fn test_builtin_profiles_not_empty() {
        assert!(!BUILTIN_PROFILES.is_empty());
    }

    #[test]
    fn test_find_profile_wechat() {
        let profile = find_profile("wechat").expect("wechat profile");
        assert_eq!(profile.id, "wechat");
        assert_eq!(profile.format, CopyFormat::StyledHtml);
        assert!(profile.serialize_rules.contains(SerializeRule::LiSpanWrap));
    }

    #[test]
    fn test_find_profile_zhihu() {
        let profile = find_profile("zhihu").expect("zhihu profile");
        assert_eq!(profile.id, "zhihu");
        assert_eq!(profile.format, CopyFormat::Markdown);
        assert_eq!(profile.default_asset_strategy(), AssetStrategy::External);
    }

    #[test]
    fn test_known_profile_ids() {
        let ids: Vec<_> = known_profile_ids().collect();
        assert!(ids.contains(&"wechat"));
        assert!(ids.contains(&"zhihu"));
    }
}
