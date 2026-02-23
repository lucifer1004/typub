use anyhow::Result;

use typub_adapters_core::{
    AdapterCapability, AdapterRegistrar, NodePolicy, NodePolicyAction, PlatformAdapter,
    TaxonomyCapability,
};
use typub_config::Config;
use typub_core::{AssetStrategy, CapabilityGapBehavior, CapabilitySupport};

use crate::adapter::CopyPasteAdapter;
use crate::model::{all_profiles, find_profile};

/// Register all built-in copy-paste platform adapters.
///
/// Creates one adapter instance per profile in `profiles.toml`.
/// Also registers capabilities for each profile.
pub fn register(registrar: &mut AdapterRegistrar) -> Result<()> {
    for profile in all_profiles() {
        let profile_id = profile.id;
        let profile_name = profile.name;
        let short_code = profile.short_code;

        // Determine supported math renderings based on format
        // HTML: SVG and PNG supported, Markdown: LaTeX only
        let math_renderings: &[typub_core::MathRendering] =
            if profile.format == crate::model::CopyFormat::Markdown {
                &[typub_core::MathRendering::Latex]
            } else {
                const HTML_MATH: &[typub_core::MathRendering] = &[
                    typub_core::MathRendering::Svg,
                    typub_core::MathRendering::Png,
                ];
                HTML_MATH
            };

        // Build taxonomy capability
        let taxonomy = TaxonomyCapability::new(
            CapabilitySupport::Unsupported(CapabilityGapBehavior::WarnAndDegrade),
            CapabilitySupport::Unsupported(CapabilityGapBehavior::WarnAndDegrade),
            CapabilitySupport::Supported,
            typub_core::DraftSupport::None,
        );

        // Build math_delimiters with profile's default as first element
        let math_delimiters: &[typub_core::MathDelimiters] =
            if profile.default_math_delimiters() == typub_core::MathDelimiters::Dollar {
                static DOLLAR_FIRST: &[typub_core::MathDelimiters] = &[
                    typub_core::MathDelimiters::Dollar,
                    typub_core::MathDelimiters::Brackets,
                ];
                DOLLAR_FIRST
            } else {
                static BRACKETS_FIRST: &[typub_core::MathDelimiters] = &[
                    typub_core::MathDelimiters::Brackets,
                    typub_core::MathDelimiters::Dollar,
                ];
                BRACKETS_FIRST
            };

        // Build asset_strategies with profile's default as first element
        let asset_strategies: &[AssetStrategy] = if profile.default_asset_strategy()
            == AssetStrategy::Embed
        {
            static EMBED_FIRST: &[AssetStrategy] = &[AssetStrategy::Embed, AssetStrategy::External];
            EMBED_FIRST
        } else {
            static EXTERNAL_FIRST: &[AssetStrategy] =
                &[AssetStrategy::External, AssetStrategy::Embed];
            EXTERNAL_FIRST
        };

        // Register capability (copypaste adapters have dynamic capabilities)
        // code_highlight is derived from format: HTML platforms need it, Markdown platforms don't
        let code_highlight = profile.format == crate::model::CopyFormat::StyledHtml;
        let capability = AdapterCapability {
            id: profile_id,
            name: profile_name,
            short_code,
            local_output: true, // File-based output
            requires_config: false,
            taxonomy,
            asset_strategies,
            math_renderings,
            math_delimiters,
            code_highlight,
            notes: "Copy-paste adapter for clipboard-based publishing.",
            node_policy: NodePolicy {
                raw: NodePolicyAction::Sanitize,
                unknown: NodePolicyAction::Drop,
            },
        };
        registrar.register_capability(profile_id, capability)?;
    }

    Ok(())
}

/// Create a single adapter for a specific profile.
pub fn create_for_profile(profile_id: &str, config: &Config) -> Result<Box<dyn PlatformAdapter>> {
    let profile = find_profile(profile_id)
        .ok_or_else(|| anyhow::anyhow!("Unknown copypaste profile: {}", profile_id))?;
    Ok(Box::new(CopyPasteAdapter::from_profile(profile, config)?))
}

/// Create an adapter from a manual platform config.
pub fn create_manual(id: &'static str, config: &Config) -> Result<Box<dyn PlatformAdapter>> {
    let platform_config = config
        .get_platform(id)
        .ok_or_else(|| anyhow::anyhow!("Manual platform '{}' not found in config", id))?;
    Ok(Box::new(CopyPasteAdapter::from_manual_config(
        id,
        platform_config,
        config,
    )?))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_register_adds_all_profiles() {
        let mut registrar = AdapterRegistrar::new();
        register(&mut registrar).expect("register");

        // Check that at least wechat and zhihu are registered
        assert!(registrar.capabilities().contains_key("wechat"));
        assert!(registrar.capabilities().contains_key("zhihu"));
    }

    #[test]
    fn test_create_for_profile_wechat() {
        let config = Config::default();
        let adapter = create_for_profile("wechat", &config).expect("create wechat");
        assert_eq!(adapter.id(), "wechat");
        assert_eq!(adapter.name(), "WeChat Official Account");
    }

    #[test]
    fn test_create_for_profile_unknown() {
        let config = Config::default();
        let result = create_for_profile("unknown-profile", &config);
        assert!(result.is_err());
    }
}
