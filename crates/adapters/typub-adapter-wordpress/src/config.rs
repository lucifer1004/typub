use anyhow::Result;
use typub_adapters_core::{
    AdapterCapability, AdapterRegistrar, NodePolicy, NodePolicyAction, PlatformAdapter,
    TaxonomyCapability, register_adapter, resolve_asset_strategy_from_config,
    resolve_math_rendering_from_config,
};
use typub_config::{Config, PlatformConfig};
use typub_core::{AssetStrategy, MathDelimiters, MathRendering};

use crate::adapter::WordPressAdapter;
use crate::model::ID;

pub const CAPABILITY: AdapterCapability = AdapterCapability {
    id: ID,
    name: "WordPress",
    short_code: "wp",
    local_output: false,
    requires_config: true,
    taxonomy: TaxonomyCapability::full(),
    asset_strategies: &[
        AssetStrategy::Upload,
        AssetStrategy::Embed,
        AssetStrategy::External,
    ],
    math_renderings: &[MathRendering::Svg, MathRendering::Png], // Inline SVG or media upload
    math_delimiters: &[MathDelimiters::Dollar],
    code_highlight: true,
    notes: "Tags/categories sync to WP taxonomies; internal links resolved from local status DB.",
    node_policy: NodePolicy {
        raw: NodePolicyAction::Sanitize,
        unknown: NodePolicyAction::Drop,
    },
};

pub fn create(config: &Config) -> Result<Box<dyn PlatformAdapter>> {
    Ok(Box::new(WordPressAdapter::new(config)?))
}

pub fn register(registrar: &mut AdapterRegistrar) -> Result<()> {
    register_adapter(registrar, &CAPABILITY, create)
}

pub fn resolve_asset_strategy(platform_config: Option<&PlatformConfig>) -> Result<AssetStrategy> {
    resolve_asset_strategy_from_config(platform_config, &CAPABILITY)
}

/// Resolve math rendering strategy from platform configuration.
pub fn resolve_math_rendering(platform_config: Option<&PlatformConfig>) -> Result<MathRendering> {
    resolve_math_rendering_from_config(platform_config, &CAPABILITY)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use std::collections::HashMap;

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
        let err =
            resolve_asset_strategy(Some(&cfg)).expect_err("already configured invalid strategy");
        assert!(err.to_string().contains("Invalid asset strategy"));
    }

    #[test]
    fn test_resolve_strategy_disabled_platform() {
        let cfg = PlatformConfig {
            enabled: false,
            asset_strategy: Some("embed".to_string()),
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
