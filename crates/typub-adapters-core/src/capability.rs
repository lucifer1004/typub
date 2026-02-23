use typub_core::{
    AssetStrategy, CapabilityGapBehavior, DraftSupport, MathDelimiters, MathRendering,
    NodePolicyAction, TaxonomyCapability,
};

/// Machine-readable node policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodePolicy {
    pub raw: NodePolicyAction,
    pub unknown: NodePolicyAction,
}

/// Platform capabilities exposed by an adapter.
///
/// Field organization:
/// - Identity: id, name, short_code
/// - Output: local_output, requires_config
/// - Taxonomy: tags, categories, internal_links, draft (grouped)
/// - Assets: asset_strategies (first is default)
/// - Math: math_renderings, math_delimiters (first is default)
/// - Code: code_highlight
/// - Docs: notes
#[derive(Debug, Clone, Copy)]
pub struct AdapterCapability {
    // === Identity ===
    pub id: &'static str,
    pub name: &'static str,
    pub short_code: &'static str,

    // === Output Type ===
    /// Whether this adapter produces local file output (vs remote API).
    pub local_output: bool,
    /// Whether this platform requires configuration in typub.toml.
    /// If false, the adapter can be used with `-p <platform>` without any config entry.
    /// Per [[ADR-0010]].
    pub requires_config: bool,

    // === Taxonomy ===
    pub taxonomy: TaxonomyCapability,

    // === Assets ===
    /// Supported asset strategies. First element is the default.
    pub asset_strategies: &'static [AssetStrategy],

    // === Math ===
    /// Supported math rendering strategies. First element is the default.
    pub math_renderings: &'static [MathRendering],
    /// Supported math delimiter syntaxes. First element is the default.
    pub math_delimiters: &'static [MathDelimiters],

    // === Code ===
    pub code_highlight: bool,

    // === Docs ===
    pub notes: &'static str,

    // === Node Policy ===
    pub node_policy: NodePolicy,
}

impl AdapterCapability {
    /// Get the default asset strategy (first element).
    pub fn default_asset_strategy(&self) -> AssetStrategy {
        self.asset_strategies[0]
    }

    /// Get supported asset strategies (all elements).
    pub fn supported_asset_strategies(&self) -> &'static [AssetStrategy] {
        self.asset_strategies
    }

    /// Get the default math rendering (first element).
    pub fn default_math_rendering(&self) -> MathRendering {
        self.math_renderings[0]
    }

    /// Get supported math renderings (all elements).
    pub fn supported_math_renderings(&self) -> &'static [MathRendering] {
        self.math_renderings
    }

    /// Get the default math delimiter (first element).
    pub fn default_math_delimiter(&self) -> MathDelimiters {
        self.math_delimiters[0]
    }

    /// Get supported math delimiters (all elements).
    pub fn supported_math_delimiters(&self) -> &'static [MathDelimiters] {
        self.math_delimiters
    }

    // === Convenience delegates for taxonomy ===

    pub fn tags_gap_behavior(&self) -> Option<CapabilityGapBehavior> {
        self.taxonomy.tags_gap_behavior()
    }

    pub fn categories_gap_behavior(&self) -> Option<CapabilityGapBehavior> {
        self.taxonomy.categories_gap_behavior()
    }

    pub fn internal_links_gap_behavior(&self) -> Option<CapabilityGapBehavior> {
        self.taxonomy.internal_links_gap_behavior()
    }

    pub fn draft_support(&self) -> DraftSupport {
        self.taxonomy.draft_support()
    }

    // === Asset strategy policy ===

    pub fn asset_strategy_policy(&self) -> ImageStrategyPolicy {
        ImageStrategyPolicy {
            supported: self.asset_strategies,
        }
    }

    pub fn code_highlight(&self) -> bool {
        self.code_highlight
    }

    pub fn node_policy(&self) -> NodePolicy {
        self.node_policy
    }

    /// Check if a math rendering strategy is supported.
    pub fn supports_math_rendering(&self, strategy: MathRendering) -> bool {
        self.math_renderings.contains(&strategy)
    }

    /// Check if a math delimiter syntax is supported.
    pub fn supports_math_delimiter(&self, delim: MathDelimiters) -> bool {
        self.math_delimiters.contains(&delim)
    }
}

/// Policy for allowed asset strategies.
#[derive(Debug, Clone, Copy)]
pub struct ImageStrategyPolicy {
    pub supported: &'static [AssetStrategy],
}

impl ImageStrategyPolicy {
    pub const fn allow_all() -> Self {
        Self {
            supported: &[
                AssetStrategy::Copy,
                AssetStrategy::Embed,
                AssetStrategy::Upload,
                AssetStrategy::External,
            ],
        }
    }

    pub const fn from_capability(cap: &AdapterCapability) -> Self {
        Self {
            supported: cap.asset_strategies,
        }
    }
}

/// Lifecycle action to take per [[RFC-0005:C-LIFECYCLE-TRANSITIONS]].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleAction {
    CreatePublished,
    CreateDraft,
    UpdatePublished,
    UpdateDraft,
    TransitionDraftToPublished,
    TransitionPublishedToDraft,
    WarnCannotUnpublish,
}

/// Result of resolving an internal link.
/// Re-exported from typub_core for backward compatibility.
pub use typub_core::LinkResolution;

#[cfg(test)]
mod tests {
    use super::*;
    use typub_core::{CapabilitySupport, DraftSupport};

    const TAXONOMY: TaxonomyCapability = TaxonomyCapability::new(
        CapabilitySupport::Supported,
        CapabilitySupport::Unsupported(CapabilityGapBehavior::WarnAndDegrade),
        CapabilitySupport::Unsupported(CapabilityGapBehavior::HardError),
        DraftSupport::StatusField { reversible: true },
    );

    const TEST_CAP: AdapterCapability = AdapterCapability {
        id: "test",
        name: "Test Platform",
        short_code: "ts",
        local_output: false,
        requires_config: true,
        taxonomy: TAXONOMY,
        asset_strategies: &[AssetStrategy::Upload, AssetStrategy::Embed],
        math_renderings: &[MathRendering::Svg, MathRendering::Png],
        math_delimiters: &[MathDelimiters::Dollar, MathDelimiters::Brackets],
        code_highlight: true,
        notes: "Test notes",
        node_policy: NodePolicy {
            raw: NodePolicyAction::Pass,
            unknown: NodePolicyAction::Pass,
        },
    };

    #[test]
    fn test_taxonomy_capability_fields() {
        assert!(matches!(TAXONOMY.tags, CapabilitySupport::Supported));
        assert!(matches!(
            TAXONOMY.categories,
            CapabilitySupport::Unsupported(CapabilityGapBehavior::WarnAndDegrade)
        ));
        assert_eq!(
            TAXONOMY.draft,
            DraftSupport::StatusField { reversible: true }
        );
    }

    #[test]
    fn test_taxonomy_capability_full() {
        let full = TaxonomyCapability::full();
        assert!(matches!(full.tags, CapabilitySupport::Supported));
        assert!(matches!(full.categories, CapabilitySupport::Supported));
        assert!(matches!(full.internal_links, CapabilitySupport::Supported));
        assert!(matches!(
            full.draft,
            DraftSupport::StatusField { reversible: true }
        ));
    }

    #[test]
    fn test_taxonomy_capability_minimal() {
        let minimal = TaxonomyCapability::minimal();
        assert!(matches!(
            minimal.tags,
            CapabilitySupport::Unsupported(CapabilityGapBehavior::WarnAndDegrade)
        ));
        assert!(matches!(minimal.draft, DraftSupport::None));
    }

    #[test]
    fn test_adapter_capability_identity_fields() {
        assert_eq!(TEST_CAP.id, "test");
        assert_eq!(TEST_CAP.name, "Test Platform");
        assert_eq!(TEST_CAP.short_code, "ts");
        const { assert!(!TEST_CAP.local_output) };
        const { assert!(TEST_CAP.requires_config) };
    }

    #[test]
    fn test_adapter_capability_asset_strategies() {
        assert_eq!(TEST_CAP.default_asset_strategy(), AssetStrategy::Upload);
        assert_eq!(TEST_CAP.supported_asset_strategies().len(), 2);
        assert!(
            TEST_CAP
                .supported_asset_strategies()
                .contains(&AssetStrategy::Upload)
        );
        assert!(
            TEST_CAP
                .supported_asset_strategies()
                .contains(&AssetStrategy::Embed)
        );
    }

    #[test]
    fn test_adapter_capability_math_renderings() {
        assert_eq!(TEST_CAP.default_math_rendering(), MathRendering::Svg);
        assert_eq!(TEST_CAP.supported_math_renderings().len(), 2);
        assert!(TEST_CAP.supports_math_rendering(MathRendering::Svg));
        assert!(TEST_CAP.supports_math_rendering(MathRendering::Png));
        assert!(!TEST_CAP.supports_math_rendering(MathRendering::Latex));
    }

    #[test]
    fn test_adapter_capability_math_delimiters() {
        assert_eq!(TEST_CAP.default_math_delimiter(), MathDelimiters::Dollar);
        assert!(TEST_CAP.supports_math_delimiter(MathDelimiters::Dollar));
        assert!(TEST_CAP.supports_math_delimiter(MathDelimiters::Brackets));
    }

    #[test]
    fn test_adapter_capability_gap_behaviors() {
        assert!(TEST_CAP.tags_gap_behavior().is_none());
        assert_eq!(
            TEST_CAP.categories_gap_behavior(),
            Some(CapabilityGapBehavior::WarnAndDegrade)
        );
        assert_eq!(
            TEST_CAP.internal_links_gap_behavior(),
            Some(CapabilityGapBehavior::HardError)
        );
    }

    #[test]
    fn test_adapter_capability_draft_support() {
        assert_eq!(
            TEST_CAP.draft_support(),
            DraftSupport::StatusField { reversible: true }
        );
    }

    #[test]
    fn test_adapter_capability_code_highlight() {
        assert!(TEST_CAP.code_highlight());
    }

    #[test]
    fn test_adapter_capability_asset_strategy_policy() {
        let policy = TEST_CAP.asset_strategy_policy();
        assert_eq!(policy.supported.len(), 2);
        assert!(policy.supported.contains(&AssetStrategy::Upload));
        assert!(policy.supported.contains(&AssetStrategy::Embed));
    }

    #[test]
    fn test_image_strategy_policy_allow_all() {
        let policy = ImageStrategyPolicy::allow_all();
        assert_eq!(policy.supported.len(), 4);
    }

    #[test]
    fn test_lifecycle_action_variants() {
        assert!(matches!(
            LifecycleAction::CreatePublished,
            LifecycleAction::CreatePublished
        ));
        assert!(matches!(
            LifecycleAction::TransitionDraftToPublished,
            LifecycleAction::TransitionDraftToPublished
        ));
    }

    #[test]
    fn test_link_resolution_variants() {
        let non_internal = LinkResolution::NonInternal;
        let resolved = LinkResolution::InternalResolved {
            slug: "test".into(),
            url: "https://example.com".into(),
        };
        let unresolved = LinkResolution::InternalUnresolved {
            slug: "test".into(),
        };

        assert!(matches!(non_internal, LinkResolution::NonInternal));
        assert!(matches!(resolved, LinkResolution::InternalResolved { .. }));
        assert!(matches!(
            unresolved,
            LinkResolution::InternalUnresolved { .. }
        ));
    }
}
