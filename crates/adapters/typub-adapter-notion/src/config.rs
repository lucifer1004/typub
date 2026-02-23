use anyhow::Result;
use typub_adapters_core::{
    AdapterCapability, AdapterRegistrar, NodePolicy, NodePolicyAction, PlatformAdapter,
    RenderConfig, TaxonomyCapability, default_render_config_for, register_adapter,
    resolve_asset_strategy_from_config,
};
use typub_config::{Config, PlatformConfig};
use typub_core::{
    AssetStrategy, CapabilityGapBehavior, CapabilitySupport, MathDelimiters, MathRendering,
};

use crate::adapter::NotionAdapter;
use crate::model::ID;

pub const CAPABILITY: AdapterCapability = AdapterCapability {
    id: ID,
    name: "Notion",
    short_code: "no",
    local_output: false,
    requires_config: true,
    taxonomy: TaxonomyCapability::new(
        CapabilitySupport::Supported,
        CapabilitySupport::Unsupported(CapabilityGapBehavior::WarnAndDegrade),
        CapabilitySupport::Supported,
        typub_core::DraftSupport::None,
    ),
    asset_strategies: &[AssetStrategy::Upload, AssetStrategy::External],
    math_renderings: &[MathRendering::Latex], // Notion API requires LaTeX source
    math_delimiters: &[MathDelimiters::Dollar],
    code_highlight: false,
    notes: "Notion REST API publishing with HTML to blocks conversion.",
    node_policy: NodePolicy {
        raw: NodePolicyAction::Sanitize,
        unknown: NodePolicyAction::Drop,
    },
};

pub fn create(config: &Config) -> Result<Box<dyn PlatformAdapter>> {
    Ok(Box::new(NotionAdapter::new(config)?))
}

pub fn register(registrar: &mut AdapterRegistrar) -> Result<()> {
    register_adapter(registrar, &CAPABILITY, create)
}

pub fn resolve_asset_strategy(platform_config: Option<&PlatformConfig>) -> Result<AssetStrategy> {
    resolve_asset_strategy_from_config(platform_config, &CAPABILITY)
}

pub fn render_config_for(strategy: AssetStrategy) -> RenderConfig {
    default_render_config_for(strategy, &CAPABILITY)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_capability_math_rendering_is_latex() {
        // Notion requires LaTeX source to create equation blocks,
        // not rendered SVG. See renderer.rs for MathRendering behavior.
        assert_eq!(CAPABILITY.default_math_rendering(), MathRendering::Latex);
    }

    #[allow(clippy::expect_used)]
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
            extra: HashMap::new(),
        };
        let err = resolve_asset_strategy(Some(&cfg)).expect_err("invalid should fail");
        assert!(err.to_string().contains("Invalid asset strategy"));
    }

    #[allow(clippy::expect_used)]
    #[test]
    fn test_resolve_strategy_disabled_platform() {
        let cfg = PlatformConfig {
            enabled: false,
            asset_strategy: Some("external".to_string()),
            published: None,
            theme: None,
            internal_link_target: None,
            math_rendering: None,
            math_delimiters: None,
            extra: HashMap::new(),
        };
        let resolved = resolve_asset_strategy(Some(&cfg)).expect("resolve");
        assert_eq!(resolved, CAPABILITY.default_asset_strategy());
    }
}
