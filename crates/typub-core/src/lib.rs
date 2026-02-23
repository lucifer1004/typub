//! Shared capability types and content model for typub.
//!
//! This crate defines the canonical enum types for platform capabilities,
//! asset strategies, theme identifiers, and the core content model.
//!
//! Per [[ADR-0002]], extracting these types into a shared subcrate ensures:
//! - Single source of truth for enum variants
//! - Serde-based validation of TOML config at build time
//! - Type safety via `ThemeId` newtype for theme identifiers

pub mod content;

pub use content::{
    Content, ContentFormat, ContentMeta, DiscoverResult, PostInfo, PostPlatformConfig,
};

use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;
use std::ops::Deref;

// ============================================================================
// MathRendering
// ============================================================================

/// How a platform renders math equations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MathRendering {
    /// Platform supports inline SVG — use Typst's native SVG rendering.
    #[default]
    Svg,
    /// Platform requires LaTeX math macros.
    Latex,
    /// Platform supports base64 images but not SVG — rasterize to PNG via resvg.
    /// Per [[WI-2026-02-13-026]].
    Png,
}

impl MathRendering {
    /// Returns the Rust expression string for code generation.
    pub fn code_expr(&self) -> &'static str {
        match self {
            Self::Svg => "MathRendering::Svg",
            Self::Latex => "MathRendering::Latex",
            Self::Png => "MathRendering::Png",
        }
    }
}

// ============================================================================
// MathDelimiters
// ============================================================================

/// Math delimiter syntax for Markdown output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MathDelimiters {
    /// Dollar sign syntax: `$...$` for inline, `$$...$$` for block.
    #[default]
    Dollar,
    /// Backslash-paren syntax: `\(...\)` for inline, `\[...\]` for block.
    Brackets,
    /// Mixed syntax: `\(...\)` for inline, `$$...$$` for block.
    /// Used by platforms like SegmentFault that don't support `\[...\]`.
    #[serde(rename = "brackets_inline_dollar_block")]
    BracketsInlineDollarBlock,
}

impl MathDelimiters {
    /// Returns the Rust expression string for code generation.
    pub fn code_expr(&self) -> &'static str {
        match self {
            Self::Dollar => "MathDelimiters::Dollar",
            Self::Brackets => "MathDelimiters::Brackets",
            Self::BracketsInlineDollarBlock => "MathDelimiters::BracketsInlineDollarBlock",
        }
    }
}

// ============================================================================
// AssetStrategy
// ============================================================================

/// Strategy for handling assets on each platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetStrategy {
    /// Copy files alongside content (e.g., Astro).
    Copy,
    /// Embed as base64 in HTML.
    Embed,
    /// Upload to platform storage (e.g., Confluence attachments).
    Upload,
    /// Upload to external S3-compatible storage.
    /// Per [[RFC-0004:C-EXTERNAL-STRATEGY]].
    External,
}

impl AssetStrategy {
    /// Parse a user-provided asset strategy string (case-insensitive).
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "copy" => Some(Self::Copy),
            "embed" => Some(Self::Embed),
            "upload" => Some(Self::Upload),
            "external" => Some(Self::External),
            _ => None,
        }
    }

    /// Returns true if this strategy requires deferred upload during Materialize stage.
    /// Per [[RFC-0004:C-PIPELINE-INTEGRATION]], both `Upload` and `External` require
    /// placeholder tokens in Finalize and actual upload in Materialize.
    pub fn requires_deferred_upload(&self) -> bool {
        matches!(self, Self::Upload | Self::External)
    }

    /// Returns the Rust expression string for code generation.
    pub fn code_expr(&self) -> &'static str {
        match self {
            Self::Copy => "AssetStrategy::Copy",
            Self::Embed => "AssetStrategy::Embed",
            Self::Upload => "AssetStrategy::Upload",
            Self::External => "AssetStrategy::External",
        }
    }
}

// ============================================================================
// CapabilityGapBehavior / CapabilitySupport
// ============================================================================

/// Behavior when a capability is not supported by a platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilityGapBehavior {
    WarnAndDegrade,
    HardError,
}

/// Generic policy action for handling non-conforming or unsupported semantic nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodePolicyAction {
    Pass,
    Sanitize,
    Drop,
    Error,
}

/// Whether a platform capability is supported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilitySupport {
    Supported,
    Unsupported(CapabilityGapBehavior),
}

impl CapabilitySupport {
    /// Return the gap behavior if unsupported, or `None` if supported.
    pub fn gap_behavior(&self) -> Option<CapabilityGapBehavior> {
        match self {
            Self::Supported => None,
            Self::Unsupported(behavior) => Some(*behavior),
        }
    }

    /// Returns the Rust expression string for code generation.
    pub fn code_expr(&self) -> &'static str {
        match self {
            Self::Supported => "CapabilitySupport::Supported",
            Self::Unsupported(CapabilityGapBehavior::WarnAndDegrade) => {
                "CapabilitySupport::Unsupported(UnsupportedBehavior::WarnAndDegrade)"
            }
            Self::Unsupported(CapabilityGapBehavior::HardError) => {
                "CapabilitySupport::Unsupported(UnsupportedBehavior::HardError)"
            }
        }
    }
}

/// Custom serde: TOML uses flat strings like `"supported"`, `"unsupported_warn"`.
impl<'de> Deserialize<'de> for CapabilitySupport {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "supported" => Ok(Self::Supported),
            "unsupported_warn" => Ok(Self::Unsupported(CapabilityGapBehavior::WarnAndDegrade)),
            "unsupported_error" => Ok(Self::Unsupported(CapabilityGapBehavior::HardError)),
            other => Err(serde::de::Error::unknown_variant(
                other,
                &["supported", "unsupported_warn", "unsupported_error"],
            )),
        }
    }
}

impl Serialize for CapabilitySupport {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = match self {
            Self::Supported => "supported",
            Self::Unsupported(CapabilityGapBehavior::WarnAndDegrade) => "unsupported_warn",
            Self::Unsupported(CapabilityGapBehavior::HardError) => "unsupported_error",
        };
        serializer.serialize_str(s)
    }
}

// ============================================================================
// DraftSupport
// ============================================================================

/// Draft support capability per [[RFC-0005:C-DRAFT-SUPPORT]].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftSupport {
    /// Platform has no draft concept. Content is always published immediately.
    None,
    /// Same object with a status field that can be toggled.
    /// `reversible` indicates whether publish → draft transition is supported.
    StatusField { reversible: bool },
    /// Draft and published content are separate objects with different IDs.
    SeparateObjects,
}

impl DraftSupport {
    /// Returns the Rust expression string for code generation.
    pub fn code_expr(&self) -> &'static str {
        match self {
            Self::None => "DraftSupport::None",
            Self::StatusField { reversible: true } => {
                "DraftSupport::StatusField { reversible: true }"
            }
            Self::StatusField { reversible: false } => {
                "DraftSupport::StatusField { reversible: false }"
            }
            Self::SeparateObjects => "DraftSupport::SeparateObjects",
        }
    }
}

/// Custom serde: TOML uses flat strings like `"none"`, `"status_field_reversible"`.
impl<'de> Deserialize<'de> for DraftSupport {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "none" => Ok(Self::None),
            "status_field_reversible" => Ok(Self::StatusField { reversible: true }),
            "status_field_irreversible" => Ok(Self::StatusField { reversible: false }),
            "separate_objects" => Ok(Self::SeparateObjects),
            other => Err(serde::de::Error::unknown_variant(
                other,
                &[
                    "none",
                    "status_field_reversible",
                    "status_field_irreversible",
                    "separate_objects",
                ],
            )),
        }
    }
}

impl Serialize for DraftSupport {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = match self {
            Self::None => "none",
            Self::StatusField { reversible: true } => "status_field_reversible",
            Self::StatusField { reversible: false } => "status_field_irreversible",
            Self::SeparateObjects => "separate_objects",
        };
        serializer.serialize_str(s)
    }
}

// ============================================================================
// TaxonomyCapability
// ============================================================================

/// Taxonomy-related capabilities for content classification and lifecycle.
/// Grouped into a sub-struct for better organization within AdapterCapability.
#[derive(Debug, Clone, Copy)]
pub struct TaxonomyCapability {
    pub tags: CapabilitySupport,
    pub categories: CapabilitySupport,
    pub internal_links: CapabilitySupport,
    pub draft: DraftSupport,
}

impl TaxonomyCapability {
    /// Create a new taxonomy capability with all fields specified.
    pub const fn new(
        tags: CapabilitySupport,
        categories: CapabilitySupport,
        internal_links: CapabilitySupport,
        draft: DraftSupport,
    ) -> Self {
        Self {
            tags,
            categories,
            internal_links,
            draft,
        }
    }

    /// Create a taxonomy capability where all features are fully supported.
    pub const fn full() -> Self {
        Self {
            tags: CapabilitySupport::Supported,
            categories: CapabilitySupport::Supported,
            internal_links: CapabilitySupport::Supported,
            draft: DraftSupport::StatusField { reversible: true },
        }
    }

    /// Create a taxonomy capability with minimal support (no tags/categories/draft).
    pub const fn minimal() -> Self {
        Self {
            tags: CapabilitySupport::Unsupported(CapabilityGapBehavior::WarnAndDegrade),
            categories: CapabilitySupport::Unsupported(CapabilityGapBehavior::WarnAndDegrade),
            internal_links: CapabilitySupport::Supported,
            draft: DraftSupport::None,
        }
    }

    pub fn tags_gap_behavior(&self) -> Option<CapabilityGapBehavior> {
        self.tags.gap_behavior()
    }

    pub fn categories_gap_behavior(&self) -> Option<CapabilityGapBehavior> {
        self.categories.gap_behavior()
    }

    pub fn internal_links_gap_behavior(&self) -> Option<CapabilityGapBehavior> {
        self.internal_links.gap_behavior()
    }

    pub fn draft_support(&self) -> DraftSupport {
        self.draft
    }
}

// ============================================================================
// ThemeId
// ============================================================================

/// Newtype for theme identifiers.
///
/// Prevents accidental confusion between theme IDs and other strings.
/// Implements `Deref<Target = str>` for ergonomic use at call sites.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ThemeId(String);

impl ThemeId {
    /// Create a new `ThemeId` from a string.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Get the inner string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume and return the inner string.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl Deref for ThemeId {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ThemeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for ThemeId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ThemeId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for ThemeId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// ============================================================================
// LinkResolution
// ============================================================================

/// Result of resolving an internal link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkResolution {
    /// The href is not an internal link.
    NonInternal,
    /// The internal link was resolved to a URL.
    InternalResolved { slug: String, url: String },
    /// The internal link target was not found.
    InternalUnresolved { slug: String },
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn test_math_rendering_serde_roundtrip() {
        let json = serde_json::to_string(&MathRendering::Svg).expect("serialize");
        assert_eq!(json, r#""svg""#);
        let parsed: MathRendering = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, MathRendering::Svg);
    }

    #[test]
    fn test_math_rendering_code_expr() {
        assert_eq!(MathRendering::Svg.code_expr(), "MathRendering::Svg");
        assert_eq!(MathRendering::Latex.code_expr(), "MathRendering::Latex");
    }

    #[test]
    fn test_math_delimiters_serde_roundtrip() {
        let json = serde_json::to_string(&MathDelimiters::Brackets).expect("serialize");
        assert_eq!(json, r#""brackets""#);
        let parsed: MathDelimiters = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, MathDelimiters::Brackets);
    }

    #[test]
    fn test_asset_strategy_serde() {
        let json = serde_json::to_string(&AssetStrategy::External).expect("serialize");
        assert_eq!(json, r#""external""#);
        let parsed: AssetStrategy = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, AssetStrategy::External);
    }

    #[test]
    fn test_asset_strategy_parse_aliases() {
        assert_eq!(
            AssetStrategy::parse("external"),
            Some(AssetStrategy::External)
        );
        assert_eq!(AssetStrategy::parse("upload"), Some(AssetStrategy::Upload));
        assert_eq!(AssetStrategy::parse("COPY"), Some(AssetStrategy::Copy));
        assert_eq!(AssetStrategy::parse("unknown"), None);
    }

    #[test]
    fn test_capability_support_serde() {
        let json =
            serde_json::to_string(&CapabilitySupport::Supported).expect("serialize supported");
        assert_eq!(json, r#""supported""#);

        let warn = CapabilitySupport::Unsupported(CapabilityGapBehavior::WarnAndDegrade);
        let json = serde_json::to_string(&warn).expect("serialize warn");
        assert_eq!(json, r#""unsupported_warn""#);

        let parsed: CapabilitySupport =
            serde_json::from_str(r#""unsupported_error""#).expect("deserialize error");
        assert_eq!(
            parsed,
            CapabilitySupport::Unsupported(CapabilityGapBehavior::HardError)
        );
    }

    #[test]
    fn test_capability_support_invalid() {
        let result = serde_json::from_str::<CapabilitySupport>(r#""garbage""#);
        assert!(result.is_err());
    }

    #[test]
    fn test_draft_support_serde() {
        let json = serde_json::to_string(&DraftSupport::None).expect("serialize none");
        assert_eq!(json, r#""none""#);

        let reversible = DraftSupport::StatusField { reversible: true };
        let json = serde_json::to_string(&reversible).expect("serialize reversible");
        assert_eq!(json, r#""status_field_reversible""#);

        let parsed: DraftSupport =
            serde_json::from_str(r#""separate_objects""#).expect("deserialize");
        assert_eq!(parsed, DraftSupport::SeparateObjects);
    }

    #[test]
    fn test_theme_id_deref_and_display() {
        let id = ThemeId::new("wechat-green");
        assert_eq!(&*id, "wechat-green");
        assert_eq!(id.as_str(), "wechat-green");
        assert_eq!(format!("{}", id), "wechat-green");
    }

    #[test]
    fn test_theme_id_serde() {
        let id = ThemeId::new("elegant");
        let json = serde_json::to_string(&id).expect("serialize");
        assert_eq!(json, r#""elegant""#);
        let parsed: ThemeId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, id);
    }

    #[test]
    fn test_theme_id_from() {
        let id: ThemeId = "dark".into();
        assert_eq!(id.as_str(), "dark");

        let id: ThemeId = String::from("github").into();
        assert_eq!(id.as_str(), "github");
    }
}
