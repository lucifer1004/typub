use anyhow::Result;

use typub_adapters_core::{
    AdapterCapability, AdapterRegistrar, NodePolicy, NodePolicyAction, PlatformAdapter,
    TaxonomyCapability, resolve_asset_strategy_from_config, resolve_math_rendering_from_config,
};
use typub_config::{Config, PlatformConfig};
use typub_core::{
    AssetStrategy, CapabilityGapBehavior, CapabilitySupport, MathDelimiters, MathRendering,
};

use crate::adapter::DevtoAdapter;

pub const ID: &str = "devto";

pub const CAPABILITY: AdapterCapability = AdapterCapability {
    id: ID,
    name: "Dev.to",
    short_code: "dt",
    local_output: false,
    requires_config: true,
    taxonomy: TaxonomyCapability::new(
        CapabilitySupport::Supported,
        CapabilitySupport::Unsupported(CapabilityGapBehavior::WarnAndDegrade),
        CapabilitySupport::Supported,
        typub_core::DraftSupport::StatusField { reversible: true },
    ),
    asset_strategies: &[AssetStrategy::External, AssetStrategy::Embed],
    math_renderings: &[MathRendering::Png], // Dev.to Markdown doesn't support LaTeX, use PNG
    math_delimiters: &[MathDelimiters::Dollar],
    code_highlight: false,
    notes: "Dev.to REST API. Tags limited to 4. No native image upload. PNG math via external/embedded images.",
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
    Ok(Box::new(DevtoAdapter::new(config)?))
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
    use std::collections::HashMap;

    #[test]
    fn test_resolve_strategy_default() {
        let result = resolve_strategy(None).expect("resolve");
        assert_eq!(result, AssetStrategy::External);
    }

    #[test]
    fn test_resolve_strategy_valid() {
        let cfg = PlatformConfig {
            enabled: true,
            asset_strategy: Some("embed".into()),
            published: None,
            theme: None,
            internal_link_target: None,
            math_rendering: None,
            math_delimiters: None,
            extra: HashMap::new(),
        };
        let result = resolve_strategy(Some(&cfg)).expect("resolve");
        assert_eq!(result, AssetStrategy::Embed);
    }

    #[test]
    fn test_resolve_strategy_invalid_value() {
        let cfg = PlatformConfig {
            enabled: true,
            asset_strategy: Some("invalid".into()),
            published: None,
            theme: None,
            internal_link_target: None,
            math_rendering: None,
            math_delimiters: None,
            extra: HashMap::new(),
        };
        let err = resolve_strategy(Some(&cfg)).expect_err("invalid should fail");
        assert!(err.to_string().contains("Invalid asset strategy"));
    }

    #[test]
    fn test_resolve_strategy_disabled_platform() {
        let cfg = PlatformConfig {
            enabled: false,
            asset_strategy: Some("embed".into()),
            published: None,
            theme: None,
            internal_link_target: None,
            math_rendering: None,
            math_delimiters: None,
            extra: HashMap::new(),
        };
        let result = resolve_strategy(Some(&cfg)).expect("resolve");
        assert_eq!(result, AssetStrategy::External);
    }

    #[test]
    fn test_resolve_strategy_unsupported() {
        let cfg = PlatformConfig {
            enabled: true,
            asset_strategy: Some("upload".into()),
            published: None,
            theme: None,
            internal_link_target: None,
            math_rendering: None,
            math_delimiters: None,
            extra: HashMap::new(),
        };
        let err = resolve_strategy(Some(&cfg)).expect_err("should fail");
        assert!(err.to_string().contains("does not support"));
    }
}
