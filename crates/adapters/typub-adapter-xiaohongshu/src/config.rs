use anyhow::Result;

use typub_adapters_core::{
    AdapterCapability, AdapterRegistrar, NodePolicy, NodePolicyAction, PlatformAdapter,
    TaxonomyCapability, resolve_asset_strategy_from_config,
};
use typub_config::{Config, PlatformConfig};
use typub_core::{
    AssetStrategy, CapabilityGapBehavior, CapabilitySupport, MathDelimiters, MathRendering,
};

use crate::adapter::XiaohongshuAdapter;

pub const ID: &str = "xiaohongshu";

pub const CAPABILITY: AdapterCapability = AdapterCapability {
    id: ID,
    name: "Xiaohongshu",
    short_code: "xhs",
    local_output: true,
    requires_config: false,
    taxonomy: TaxonomyCapability::new(
        CapabilitySupport::Unsupported(CapabilityGapBehavior::WarnAndDegrade),
        CapabilitySupport::Unsupported(CapabilityGapBehavior::WarnAndDegrade),
        CapabilitySupport::Unsupported(CapabilityGapBehavior::WarnAndDegrade),
        typub_core::DraftSupport::None,
    ),
    asset_strategies: &[AssetStrategy::Embed],
    math_renderings: &[MathRendering::Svg],
    math_delimiters: &[MathDelimiters::Dollar],
    code_highlight: true,
    notes: "Generates slide images for manual upload to 小红书.",
    node_policy: NodePolicy {
        raw: NodePolicyAction::Sanitize,
        unknown: NodePolicyAction::Drop,
    },
};

pub fn resolve_strategy(platform_config: Option<&PlatformConfig>) -> Result<AssetStrategy> {
    resolve_asset_strategy_from_config(platform_config, &CAPABILITY)
}

pub fn create(config: &Config) -> Result<Box<dyn PlatformAdapter>> {
    Ok(Box::new(XiaohongshuAdapter::new(config)?))
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
        assert_eq!(CAPABILITY.id, "xiaohongshu");
        assert_eq!(CAPABILITY.name, "Xiaohongshu");
        assert_eq!(CAPABILITY.default_asset_strategy(), AssetStrategy::Embed);
        const { assert!(CAPABILITY.local_output) };
    }
}
