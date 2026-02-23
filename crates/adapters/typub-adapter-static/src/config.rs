use anyhow::Result;

use typub_adapters_core::{
    AdapterCapability, AdapterRegistrar, NodePolicy, NodePolicyAction, PlatformAdapter,
    TaxonomyCapability, resolve_asset_strategy_from_config, resolve_math_rendering_from_config,
};
use typub_config::{Config, PlatformConfig};
use typub_core::{
    AssetStrategy, CapabilityGapBehavior, CapabilitySupport, MathDelimiters, MathRendering,
};

use crate::adapter::StaticAdapter;

pub const ID: &str = "static";

pub const CAPABILITY: AdapterCapability = AdapterCapability {
    id: ID,
    name: "Static Site",
    short_code: "st",
    local_output: true,
    requires_config: false,
    taxonomy: TaxonomyCapability::new(
        CapabilitySupport::Unsupported(CapabilityGapBehavior::WarnAndDegrade),
        CapabilitySupport::Unsupported(CapabilityGapBehavior::WarnAndDegrade),
        CapabilitySupport::Supported,
        typub_core::DraftSupport::None,
    ),
    asset_strategies: &[
        AssetStrategy::Copy,
        AssetStrategy::Embed,
        AssetStrategy::External,
    ],
    math_renderings: &[MathRendering::Svg, MathRendering::Png],
    math_delimiters: &[MathDelimiters::Dollar],
    code_highlight: true,
    notes: "Standalone HTML output for static hosting. Generates index.html with themed styling.",
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
    Ok(Box::new(StaticAdapter::new(config)?))
}

pub fn register(registrar: &mut AdapterRegistrar) -> Result<()> {
    registrar.register_factory(ID, create)?;
    registrar.register_capability(ID, CAPABILITY)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_fields() {
        assert_eq!(CAPABILITY.id, "static");
        assert_eq!(CAPABILITY.name, "Static Site");
        assert_eq!(CAPABILITY.default_asset_strategy(), AssetStrategy::Copy);
        const { assert!(CAPABILITY.local_output) };
    }
}
