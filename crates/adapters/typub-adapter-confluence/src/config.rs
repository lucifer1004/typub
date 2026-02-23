use anyhow::Result;

use typub_adapters_core::{
    AdapterCapability, AdapterRegistrar, NodePolicy, NodePolicyAction, PlatformAdapter,
    TaxonomyCapability, resolve_asset_strategy_from_config, resolve_math_rendering_from_config,
};
use typub_config::{Config, PlatformConfig};
use typub_core::{
    AssetStrategy, CapabilityGapBehavior, CapabilitySupport, MathDelimiters, MathRendering,
};

use crate::adapter::ConfluenceAdapter;

pub const ID: &str = "confluence";

pub const CAPABILITY: AdapterCapability = AdapterCapability {
    id: ID,
    name: "Confluence",
    short_code: "cf",
    local_output: false,
    requires_config: true,
    taxonomy: TaxonomyCapability::new(
        CapabilitySupport::Supported,
        CapabilitySupport::Unsupported(CapabilityGapBehavior::WarnAndDegrade),
        CapabilitySupport::Supported,
        typub_core::DraftSupport::StatusField { reversible: true },
    ),
    asset_strategies: &[AssetStrategy::Upload],
    math_renderings: &[MathRendering::Latex, MathRendering::Png], // LaTeX via ADF, or PNG attachment
    math_delimiters: &[MathDelimiters::Dollar],
    code_highlight: false,
    notes: "Confluence REST API. Tags map to labels. CDATA requires plain text code blocks.",
    node_policy: NodePolicy {
        raw: NodePolicyAction::Sanitize,
        unknown: NodePolicyAction::Drop,
    },
};

pub fn resolve_strategy(platform_config: Option<&PlatformConfig>) -> Result<AssetStrategy> {
    resolve_asset_strategy_from_config(platform_config, &CAPABILITY)
}

/// Resolve math rendering strategy from platform configuration.
pub fn resolve_math_rendering(platform_config: Option<&PlatformConfig>) -> Result<MathRendering> {
    resolve_math_rendering_from_config(platform_config, &CAPABILITY)
}

pub fn create(config: &Config) -> Result<Box<dyn PlatformAdapter>> {
    Ok(Box::new(ConfluenceAdapter::new(config)?))
}

pub fn register(registrar: &mut AdapterRegistrar) -> Result<()> {
    registrar.register_factory(ID, create)?;
    registrar.register_capability(ID, CAPABILITY)?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_values() {
        assert_eq!(CAPABILITY.id, "confluence");
        assert_eq!(CAPABILITY.default_math_rendering(), MathRendering::Latex);
        assert_eq!(CAPABILITY.default_asset_strategy(), AssetStrategy::Upload);
        const { assert!(!CAPABILITY.code_highlight) };
    }

    #[test]
    fn test_resolve_strategy_invalid_value() {
        let cfg = PlatformConfig {
            enabled: true,
            asset_strategy: Some("invalid".to_string()),
            published: None,
            theme: None,
            internal_link_target: None,
            math_rendering: None,
            math_delimiters: None,
            extra: std::collections::HashMap::new(),
        };
        let err = resolve_strategy(Some(&cfg)).expect_err("invalid should fail");
        assert!(err.to_string().contains("Invalid asset strategy"));
    }

    #[test]
    fn test_resolve_strategy_disabled_platform() {
        let cfg = PlatformConfig {
            enabled: false,
            asset_strategy: Some("upload".to_string()),
            published: None,
            theme: None,
            internal_link_target: None,
            math_rendering: None,
            math_delimiters: None,
            extra: std::collections::HashMap::new(),
        };
        let resolved = resolve_strategy(Some(&cfg)).expect("resolve");
        assert_eq!(resolved, CAPABILITY.default_asset_strategy());
    }

    #[test]
    fn test_resolve_math_rendering_default() {
        let cfg = PlatformConfig {
            enabled: true,
            asset_strategy: None,
            published: None,
            theme: None,
            internal_link_target: None,
            math_rendering: None,
            math_delimiters: None,
            extra: std::collections::HashMap::new(),
        };
        let resolved = resolve_math_rendering(Some(&cfg)).expect("resolve");
        assert_eq!(resolved, MathRendering::Latex);
    }

    #[test]
    fn test_resolve_math_rendering_png() {
        let cfg = PlatformConfig {
            enabled: true,
            asset_strategy: None,
            published: None,
            theme: None,
            internal_link_target: None,
            math_rendering: Some("png".to_string()),
            math_delimiters: None,
            extra: std::collections::HashMap::new(),
        };
        let resolved = resolve_math_rendering(Some(&cfg)).expect("resolve");
        assert_eq!(resolved, MathRendering::Png);
    }
}
