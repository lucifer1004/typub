//! Unified configuration resolution
//!
//! Implements [[RFC-0005:C-RESOLUTION-ORDER]] with a consistent 4-layer resolution
//! chain for all configurable fields.
//!
//! Resolution order (highest to lowest priority):
//! 1. `meta.toml[platforms.<platform>].{field}` — per-content platform-specific
//! 2. `meta.toml.{field}` — per-content default
//! 3. `typub.toml[platforms.<platform>].{field}` — global platform-specific
//! 4. `typub.toml.{field}` — global default
//! 5. Caller-provided default (optional)

use crate::assets::AssetStrategy;
use crate::content::Content;
use anyhow::Result;
use typub_adapters_core::ResolvedConfigDefaults;
use typub_config::{Config, PlatformConfig, StorageConfig};
use typub_core::{NodePolicyAction, ThemeId};

// Re-export from typub_adapters_core for backward compatibility

/// Fully resolved configuration for a (content, platform) pair.
///
/// This struct centralizes all configuration resolution, implementing the
/// 4-layer resolution chain defined in [[RFC-0005:C-RESOLUTION-ORDER]].
///
/// Compute once per (content, platform), then pass throughout the pipeline.
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    /// Whether content should be published to this platform
    pub published: bool,
    /// Resolved theme ID (not the Theme object itself)
    pub theme_id: Option<ThemeId>,
    /// Preferred platform for internal link resolution (for copypaste adapters)
    pub internal_link_target: Option<String>,
    /// Asset handling strategy for this platform
    pub asset_strategy: AssetStrategy,
    /// Optional user-provided Typst preamble resolved from config layers.
    pub render_preamble: Option<String>,
    /// Resolved storage configuration (merged global + platform)
    pub storage: Option<StorageConfig>,
    /// Optional node policy overrides from config layers.
    pub node_policy_override: Option<NodePolicyOverride>,
}

/// Partial node policy override from configuration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NodePolicyOverride {
    pub raw: Option<NodePolicyAction>,
    pub unknown: Option<NodePolicyAction>,
}

impl ResolvedConfig {
    /// Resolve all configuration fields for a (content, platform) pair.
    ///
    /// Implements [[RFC-0005:C-RESOLUTION-ORDER]]:
    /// 1. `meta.toml[platforms.<platform>].{field}`
    /// 2. `meta.toml.{field}`
    /// 3. `typub.toml[platforms.<platform>].{field}`
    /// 4. `typub.toml.{field}`
    /// 5. Default from `defaults` parameter
    pub fn resolve(
        content: &Content,
        platform: &str,
        config: &Config,
        defaults: ResolvedConfigDefaults,
    ) -> Result<Self> {
        Ok(Self {
            published: Self::resolve_published(content, platform, config, defaults.published),
            theme_id: Self::resolve_theme_id(content, platform, config, defaults.theme),
            internal_link_target: Self::resolve_internal_link_target(content, platform, config),
            asset_strategy: Self::resolve_asset_strategy(
                content,
                platform,
                config,
                defaults.asset_strategy,
            )?,
            render_preamble: Self::resolve_render_preamble(content, platform, config),
            storage: Self::resolve_storage(platform, config),
            node_policy_override: Self::resolve_node_policy_override(content, platform, config)?,
        })
    }

    pub fn resolve_internal_link_target_for(
        content: &Content,
        platform: &str,
        config: &Config,
    ) -> Option<String> {
        Self::resolve_internal_link_target(content, platform, config)
    }

    pub fn resolve_asset_strategy_for(
        content: &Content,
        platform: &str,
        config: &Config,
        default: AssetStrategy,
    ) -> Result<AssetStrategy> {
        Self::resolve_asset_strategy(content, platform, config, default)
    }

    pub fn resolve_asset_strategy_from_platform_config(
        platform: &str,
        platform_config: Option<&PlatformConfig>,
        default: AssetStrategy,
    ) -> Result<AssetStrategy> {
        let Some(strategy_str) = platform_config.and_then(|c| c.asset_strategy.as_deref()) else {
            return Ok(default);
        };

        parse_asset_strategy(platform, strategy_str)
    }

    /// Resolve `published` using 4-layer chain + default.
    ///
    /// Implements [[RFC-0005:C-RESOLUTION-ORDER]].
    fn resolve_published(
        content: &Content,
        platform: &str,
        config: &Config,
        default: bool,
    ) -> bool {
        // Layer 1: meta.toml[platforms.<platform>].published
        content
            .meta
            .platforms
            .get(platform)
            .and_then(|p| p.published)
            // Layer 2: meta.toml.published
            .or(content.meta.published)
            // Layer 3: typub.toml[platforms.<platform>].published
            .or(config.platforms.get(platform).and_then(|p| p.published))
            // Layer 4: typub.toml.published
            .or(config.published)
            // Layer 5: default
            .unwrap_or(default)
    }

    /// Resolve `theme` ID using 4-layer chain + default.
    ///
    /// Returns the theme ID string, not the Theme object. Caller should
    /// load the actual Theme from ThemeRegistry using this ID.
    fn resolve_theme_id(
        content: &Content,
        platform: &str,
        config: &Config,
        default: Option<ThemeId>,
    ) -> Option<ThemeId> {
        // Layer 1: meta.toml[platforms.<platform>].theme (via extra)
        content
            .platform_config(platform)
            .and_then(|c| c.get_str("theme"))
            .map(ThemeId::from)
            // Layer 2: meta.toml.theme
            .or_else(|| content.meta.theme.clone())
            // Layer 3: typub.toml[platforms.<platform>].theme
            .or_else(|| config.platforms.get(platform).and_then(|p| p.theme.clone()))
            // Layer 4: typub.toml.theme
            .or_else(|| config.theme.clone())
            // Layer 5: default
            .or(default)
    }

    /// Resolve `internal_link_target` using 4-layer chain.
    ///
    /// Returns `None` if no explicit preference is set (caller should auto-select).
    fn resolve_internal_link_target(
        content: &Content,
        platform: &str,
        config: &Config,
    ) -> Option<String> {
        // Layer 1: meta.toml[platforms.<platform>].internal_link_target
        content
            .meta
            .platforms
            .get(platform)
            .and_then(|p| p.internal_link_target.clone())
            // Layer 2: meta.toml.internal_link_target
            .or_else(|| content.meta.internal_link_target.clone())
            // Layer 3: typub.toml[platforms.<platform>].internal_link_target
            .or_else(|| {
                config
                    .platforms
                    .get(platform)
                    .and_then(|p| p.internal_link_target.clone())
            })
            // Layer 4: typub.toml.internal_link_target
            .or_else(|| config.internal_link_target.clone())
        // No default — None means auto-select
    }

    /// Resolve `asset_strategy` using 4-layer chain + default.
    ///
    /// Note: Currently `meta.toml` doesn't have explicit asset_strategy fields,
    /// so layers 1-2 use the `extra` map. We check for the string value and parse it.
    fn resolve_asset_strategy(
        content: &Content,
        platform: &str,
        config: &Config,
        default: AssetStrategy,
    ) -> Result<AssetStrategy> {
        // Layer 1: meta.toml[platforms.<platform>].asset_strategy (via extra)
        let strategy_str = content
            .platform_config(platform)
            .and_then(|c| c.get_str("asset_strategy"))
            // Layer 2: not applicable (no global asset_strategy in meta.toml)
            // Layer 3: typub.toml[platforms.<platform>].asset_strategy
            .or_else(|| {
                config
                    .platforms
                    .get(platform)
                    .and_then(|p| p.asset_strategy.clone())
            });
        // Layer 4: not applicable (no global asset_strategy in typub.toml root)

        match strategy_str {
            Some(s) => parse_asset_strategy(platform, &s),
            None => Ok(default),
        }
    }

    /// Resolve Typst render preamble using 5-layer chain.
    fn resolve_render_preamble(
        content: &Content,
        platform: &str,
        config: &Config,
    ) -> Option<String> {
        content
            .platform_config(platform)
            .and_then(|c| c.get_str("preamble"))
            .or_else(|| content.meta.preamble.clone())
            .or_else(|| {
                config
                    .platforms
                    .get(platform)
                    .and_then(|p| p.get_str("preamble"))
            })
            .or_else(|| config.preamble.clone())
    }

    /// Resolve storage configuration by merging global and platform-specific config.
    ///
    /// Uses [[RFC-0004:C-STORAGE-CONFIG]] precedence ladder for each field:
    /// 1. Platform-specific env var (e.g., `HASHNODE_S3_BUCKET`)
    /// 2. Platform-specific config value
    /// 3. Global env var (e.g., `S3_BUCKET`)
    /// 4. Global config value
    fn resolve_storage(platform: &str, config: &Config) -> Option<StorageConfig> {
        let global = config.storage.as_ref();
        let platform_storage = config.platforms.get(platform).and_then(|p| p.get_storage());

        // Only return Some if there's any storage config at any level
        if global.is_none() && platform_storage.is_none() {
            return None;
        }

        Some(StorageConfig::resolve(
            global,
            platform_storage.as_ref(),
            platform,
        ))
    }

    fn resolve_node_policy_override(
        content: &Content,
        platform: &str,
        config: &Config,
    ) -> Result<Option<NodePolicyOverride>> {
        let post_override = content
            .platform_config(platform)
            .and_then(|p| p.extra.get("node_policy"))
            .map(|v| {
                parse_node_policy_override(
                    v,
                    &format!("meta.toml platforms.{platform}.node_policy"),
                )
            })
            .transpose()?;

        let config_override = config
            .platforms
            .get(platform)
            .and_then(|p| p.extra.get("node_policy"))
            .map(|v| {
                parse_node_policy_override(
                    v,
                    &format!("typub.toml platforms.{platform}.node_policy"),
                )
            })
            .transpose()?;

        let raw = post_override
            .and_then(|p| p.raw)
            .or(config_override.and_then(|p| p.raw));
        let unknown = post_override
            .and_then(|p| p.unknown)
            .or(config_override.and_then(|p| p.unknown));

        if raw.is_none() && unknown.is_none() {
            Ok(None)
        } else {
            Ok(Some(NodePolicyOverride { raw, unknown }))
        }
    }
}

/// Resolve a platform-specific string field using 4-layer resolution.
///
/// Implements [[RFC-0005:C-RESOLUTION-ORDER]]:
/// 1. `meta.toml[platforms.<platform>].{field}` — per-content platform-specific
/// 2. `meta.toml.{field}` — per-content default (not applicable for most platform-specific fields)
/// 3. `typub.toml[platforms.<platform>].{field}` — global platform-specific
/// 4. `typub.toml.{field}` — global default (not applicable for most platform-specific fields)
/// 5. `default` — caller-provided default
///
/// This function is used by adapters to resolve fields like `space`, `parent_id`,
/// `slug`, `subtitle`, etc. that are specific to a platform but may be overridden
/// at the post level.
pub fn resolve_platform_field(
    content: &Content,
    platform: &str,
    config: &Config,
    field: &str,
    default: Option<String>,
) -> Option<String> {
    // Layer 1: meta.toml[platforms.<platform>].{field} (via extra)
    content
        .platform_config(platform)
        .and_then(|c| c.get_str(field))
        // Layer 2: not applicable (no per-content default for platform-specific fields)
        // Layer 3: typub.toml[platforms.<platform>].{field}
        .or_else(|| {
            config
                .platforms
                .get(platform)
                .and_then(|p| p.get_str(field))
        })
        // Layer 4: not applicable (no global default for platform-specific fields)
        // Layer 5: caller-provided default
        .or(default)
}

fn parse_asset_strategy(platform: &str, strategy: &str) -> Result<AssetStrategy> {
    AssetStrategy::parse(strategy).ok_or_else(|| {
        anyhow::anyhow!(
            "Invalid asset strategy '{}' for platform '{}'. \
             Expected one of: copy, embed, upload, external.",
            strategy,
            platform
        )
    })
}

fn parse_node_policy_override(value: &toml::Value, context: &str) -> Result<NodePolicyOverride> {
    let table = value
        .as_table()
        .ok_or_else(|| anyhow::anyhow!("{context} must be a table with raw/unknown keys"))?;

    let raw = table
        .get("raw")
        .map(|v| parse_node_policy_action(v, context, "raw"))
        .transpose()?;
    let unknown = table
        .get("unknown")
        .map(|v| parse_node_policy_action(v, context, "unknown"))
        .transpose()?;

    Ok(NodePolicyOverride { raw, unknown })
}

fn parse_node_policy_action(
    value: &toml::Value,
    context: &str,
    key: &str,
) -> Result<NodePolicyAction> {
    let Some(raw) = value.as_str() else {
        anyhow::bail!("{context}.{key} must be a string");
    };
    match raw {
        "pass" => Ok(NodePolicyAction::Pass),
        "sanitize" => Ok(NodePolicyAction::Sanitize),
        "drop" => Ok(NodePolicyAction::Drop),
        "error" => Ok(NodePolicyAction::Error),
        _ => anyhow::bail!(
            "{context}.{key} has invalid value '{}'; expected one of: pass, sanitize, drop, error",
            raw
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{ContentFormat, ContentMeta, PostPlatformConfig};
    use anyhow::Result;
    use chrono::NaiveDate;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn make_content(
        published: Option<bool>,
        theme: Option<ThemeId>,
        internal_link_target: Option<String>,
        platforms: HashMap<String, PostPlatformConfig>,
    ) -> Content {
        Content {
            path: PathBuf::from("/tmp/test-post"),
            meta: ContentMeta {
                title: "Test".to_string(),
                created: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap_or_default(),
                updated: None,
                tags: vec![],
                categories: vec![],
                published,
                theme,
                internal_link_target,
                preamble: None,
                platforms,
            },
            content_file: PathBuf::from("/tmp/test-post/content.typ"),
            source_format: ContentFormat::Typst,
            slides_file: None,
            assets: vec![],
        }
    }

    fn make_post_platform_config(
        published: Option<bool>,
        internal_link_target: Option<String>,
        extra: HashMap<String, toml::Value>,
    ) -> PostPlatformConfig {
        PostPlatformConfig {
            published,
            internal_link_target,
            extra,
        }
    }

    #[test]
    fn test_resolve_published_layer_1_wins() {
        let mut platforms = HashMap::new();
        platforms.insert(
            "wechat".to_string(),
            make_post_platform_config(Some(false), None, HashMap::new()),
        );

        let content = make_content(Some(true), None, None, platforms);

        let config = Config {
            published: Some(true),
            ..Default::default()
        };

        let result = ResolvedConfig::resolve_published(&content, "wechat", &config, true);
        assert!(!result); // Layer 1 wins (false)
    }

    #[test]
    fn test_resolve_published_uses_default_when_all_none() {
        let content = make_content(None, None, None, HashMap::new());
        let config = Config::default();

        let result = ResolvedConfig::resolve_published(&content, "wechat", &config, false);
        assert!(!result); // Uses default (false)

        let result = ResolvedConfig::resolve_published(&content, "wechat", &config, true);
        assert!(result); // Uses default (true)
    }

    #[test]
    fn test_resolve_theme_id_layer_precedence() {
        // Layer 2 set, others None
        let content = make_content(None, Some(ThemeId::new("elegant")), None, HashMap::new());
        let config = Config::default();

        let result = ResolvedConfig::resolve_theme_id(
            &content,
            "wechat",
            &config,
            Some(ThemeId::new("minimal")),
        );
        assert_eq!(result, Some(ThemeId::new("elegant"))); // Layer 2 wins
    }

    #[test]
    fn test_resolve_internal_link_target_4_layer() {
        let content = make_content(None, None, None, HashMap::new());

        let config = Config {
            internal_link_target: Some("ghost".to_string()),
            ..Default::default()
        };

        let result = ResolvedConfig::resolve_internal_link_target(&content, "wechat", &config);
        assert_eq!(result, Some("ghost".to_string())); // Layer 4
    }

    #[test]
    fn test_resolve_internal_link_target_returns_none_when_unset() {
        let content = make_content(None, None, None, HashMap::new());
        let config = Config::default();

        let result = ResolvedConfig::resolve_internal_link_target(&content, "wechat", &config);
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_asset_strategy_uses_default() -> Result<()> {
        let content = make_content(None, None, None, HashMap::new());
        let config = Config::default();

        let result = ResolvedConfig::resolve_asset_strategy(
            &content,
            "wechat",
            &config,
            AssetStrategy::Copy,
        )?;
        assert_eq!(result, AssetStrategy::Copy);
        Ok(())
    }

    #[test]
    fn test_resolve_asset_strategy_platform_override() -> Result<()> {
        let content = make_content(None, None, None, HashMap::new());

        let mut platforms = HashMap::new();
        platforms.insert(
            "wechat".to_string(),
            PlatformConfig {
                enabled: true,
                asset_strategy: Some("embed".to_string()),
                published: None,
                theme: None,
                internal_link_target: None,
                math_rendering: None,
                math_delimiters: None,
                extra: HashMap::new(),
            },
        );
        let config = Config {
            platforms,
            ..Default::default()
        };

        let result = ResolvedConfig::resolve_asset_strategy(
            &content,
            "wechat",
            &config,
            AssetStrategy::Copy,
        )?;
        assert_eq!(result, AssetStrategy::Embed);
        Ok(())
    }

    #[test]
    fn test_resolve_full_config() -> Result<()> {
        let content = make_content(
            Some(true),
            Some(ThemeId::new("elegant")),
            Some("ghost".to_string()),
            HashMap::new(),
        );
        let config = Config::default();

        let defaults =
            ResolvedConfigDefaults::new(false, Some(ThemeId::new("minimal")), AssetStrategy::Copy);
        let resolved = ResolvedConfig::resolve(&content, "wechat", &config, defaults)?;

        assert!(resolved.published);
        assert_eq!(resolved.theme_id, Some(ThemeId::new("elegant")));
        assert_eq!(resolved.internal_link_target, Some("ghost".to_string()));
        assert_eq!(resolved.asset_strategy, AssetStrategy::Copy);
        assert!(resolved.render_preamble.is_none());
        assert!(resolved.storage.is_none());
        assert!(resolved.node_policy_override.is_none());
        Ok(())
    }

    #[test]
    fn test_resolve_render_preamble_layer_1_wins() -> Result<()> {
        let mut post_extra = HashMap::new();
        post_extra.insert(
            "preamble".to_string(),
            toml::Value::String("#set text(fill: red)".to_string()),
        );
        let mut post_platforms = HashMap::new();
        post_platforms.insert(
            "wechat".to_string(),
            make_post_platform_config(None, None, post_extra),
        );
        let mut content = make_content(None, None, None, post_platforms);
        content.meta.preamble = Some("#set text(fill: blue)".to_string());

        let mut cfg_extra = HashMap::new();
        cfg_extra.insert(
            "preamble".to_string(),
            toml::Value::String("#set text(fill: green)".to_string()),
        );
        let mut platforms = HashMap::new();
        platforms.insert(
            "wechat".to_string(),
            PlatformConfig {
                enabled: true,
                asset_strategy: None,
                published: None,
                theme: None,
                internal_link_target: None,
                math_rendering: None,
                math_delimiters: None,
                extra: cfg_extra,
            },
        );
        let config = Config {
            preamble: Some("#set text(fill: purple)".to_string()),
            platforms,
            ..Default::default()
        };

        let defaults = ResolvedConfigDefaults::new(true, None, AssetStrategy::Embed);
        let resolved = ResolvedConfig::resolve(&content, "wechat", &config, defaults)?;
        assert_eq!(
            resolved.render_preamble.as_deref(),
            Some("#set text(fill: red)")
        );
        Ok(())
    }

    #[test]
    fn test_resolve_render_preamble_falls_through_layers() -> Result<()> {
        let mut content = make_content(None, None, None, HashMap::new());
        content.meta.preamble = Some("#set text(size: 11pt)".to_string());

        let config = Config {
            preamble: Some("#set text(size: 9pt)".to_string()),
            ..Default::default()
        };
        let defaults = ResolvedConfigDefaults::new(true, None, AssetStrategy::Embed);

        let resolved = ResolvedConfig::resolve(&content, "wechat", &config, defaults)?;
        assert_eq!(
            resolved.render_preamble.as_deref(),
            Some("#set text(size: 11pt)")
        );
        Ok(())
    }

    #[test]
    fn test_resolve_node_policy_override_layer_1_overrides_layer_3() -> Result<()> {
        let mut post_extra = HashMap::new();
        post_extra.insert(
            "node_policy".to_string(),
            toml::Value::Table(
                [
                    ("raw".to_string(), toml::Value::String("drop".to_string())),
                    (
                        "unknown".to_string(),
                        toml::Value::String("error".to_string()),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
        );
        let mut post_platforms = HashMap::new();
        post_platforms.insert(
            "wechat".to_string(),
            make_post_platform_config(None, None, post_extra),
        );
        let content = make_content(None, None, None, post_platforms);

        let mut cfg_extra = HashMap::new();
        cfg_extra.insert(
            "node_policy".to_string(),
            toml::Value::Table(
                [
                    (
                        "raw".to_string(),
                        toml::Value::String("sanitize".to_string()),
                    ),
                    (
                        "unknown".to_string(),
                        toml::Value::String("drop".to_string()),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
        );
        let mut platforms = HashMap::new();
        platforms.insert(
            "wechat".to_string(),
            PlatformConfig {
                enabled: true,
                asset_strategy: None,
                published: None,
                theme: None,
                internal_link_target: None,
                math_rendering: None,
                math_delimiters: None,
                extra: cfg_extra,
            },
        );
        let config = Config {
            platforms,
            ..Default::default()
        };

        let defaults = ResolvedConfigDefaults::new(true, None, AssetStrategy::Embed);
        let resolved = ResolvedConfig::resolve(&content, "wechat", &config, defaults)?;
        assert_eq!(
            resolved.node_policy_override,
            Some(NodePolicyOverride {
                raw: Some(NodePolicyAction::Drop),
                unknown: Some(NodePolicyAction::Error)
            })
        );
        Ok(())
    }

    #[test]
    fn test_resolve_node_policy_override_partial_fallback() -> Result<()> {
        let mut post_extra = HashMap::new();
        post_extra.insert(
            "node_policy".to_string(),
            toml::Value::Table(
                [("raw".to_string(), toml::Value::String("error".to_string()))]
                    .into_iter()
                    .collect(),
            ),
        );
        let mut post_platforms = HashMap::new();
        post_platforms.insert(
            "wechat".to_string(),
            make_post_platform_config(None, None, post_extra),
        );
        let content = make_content(None, None, None, post_platforms);

        let mut cfg_extra = HashMap::new();
        cfg_extra.insert(
            "node_policy".to_string(),
            toml::Value::Table(
                [(
                    "unknown".to_string(),
                    toml::Value::String("sanitize".to_string()),
                )]
                .into_iter()
                .collect(),
            ),
        );
        let mut platforms = HashMap::new();
        platforms.insert(
            "wechat".to_string(),
            PlatformConfig {
                enabled: true,
                asset_strategy: None,
                published: None,
                theme: None,
                internal_link_target: None,
                math_rendering: None,
                math_delimiters: None,
                extra: cfg_extra,
            },
        );
        let config = Config {
            platforms,
            ..Default::default()
        };

        let defaults = ResolvedConfigDefaults::new(true, None, AssetStrategy::Embed);
        let resolved = ResolvedConfig::resolve(&content, "wechat", &config, defaults)?;
        assert_eq!(
            resolved.node_policy_override,
            Some(NodePolicyOverride {
                raw: Some(NodePolicyAction::Error),
                unknown: Some(NodePolicyAction::Sanitize)
            })
        );
        Ok(())
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_resolve_node_policy_override_invalid_value_errors() {
        let content = make_content(None, None, None, HashMap::new());

        let mut extra = HashMap::new();
        extra.insert(
            "node_policy".to_string(),
            toml::Value::Table(
                [(
                    "raw".to_string(),
                    toml::Value::String("invalid".to_string()),
                )]
                .into_iter()
                .collect(),
            ),
        );
        let mut platforms = HashMap::new();
        platforms.insert(
            "wechat".to_string(),
            PlatformConfig {
                enabled: true,
                asset_strategy: None,
                published: None,
                theme: None,
                internal_link_target: None,
                math_rendering: None,
                math_delimiters: None,
                extra,
            },
        );
        let config = Config {
            platforms,
            ..Default::default()
        };

        let defaults = ResolvedConfigDefaults::new(true, None, AssetStrategy::Embed);
        let err = ResolvedConfig::resolve(&content, "wechat", &config, defaults)
            .expect_err("invalid node_policy should error");
        assert!(err.to_string().contains("invalid value"));
    }

    // --- resolve_platform_field tests ---

    #[test]
    fn test_resolve_platform_field_layer_1_post_platform_specific() {
        let mut post_extra = HashMap::new();
        post_extra.insert(
            "space".to_string(),
            toml::Value::String("POSTSPACE".to_string()),
        );
        let mut post_platforms = HashMap::new();
        post_platforms.insert(
            "confluence".to_string(),
            make_post_platform_config(None, None, post_extra),
        );
        let content = make_content(None, None, None, post_platforms);

        let mut global_extra = HashMap::new();
        global_extra.insert(
            "space".to_string(),
            toml::Value::String("GLOBALSPACE".to_string()),
        );
        let mut global_platforms = HashMap::new();
        global_platforms.insert(
            "confluence".to_string(),
            PlatformConfig {
                enabled: true,
                asset_strategy: None,
                published: None,
                theme: None,
                internal_link_target: None,
                math_rendering: None,
                math_delimiters: None,
                extra: global_extra,
            },
        );
        let config = Config {
            platforms: global_platforms,
            ..Default::default()
        };

        // Layer 1 should win
        let result = resolve_platform_field(&content, "confluence", &config, "space", None);
        assert_eq!(result, Some("POSTSPACE".to_string()));
    }

    #[test]
    fn test_resolve_platform_field_layer_3_global_platform_specific() {
        let content = make_content(None, None, None, HashMap::new());

        let mut global_extra = HashMap::new();
        global_extra.insert(
            "space".to_string(),
            toml::Value::String("GLOBALSPACE".to_string()),
        );
        let mut global_platforms = HashMap::new();
        global_platforms.insert(
            "confluence".to_string(),
            PlatformConfig {
                enabled: true,
                asset_strategy: None,
                published: None,
                theme: None,
                internal_link_target: None,
                math_rendering: None,
                math_delimiters: None,
                extra: global_extra,
            },
        );
        let config = Config {
            platforms: global_platforms,
            ..Default::default()
        };

        // Layer 3 should be used when Layer 1 is not set
        let result = resolve_platform_field(&content, "confluence", &config, "space", None);
        assert_eq!(result, Some("GLOBALSPACE".to_string()));
    }

    #[test]
    fn test_resolve_platform_field_layer_5_default() {
        let content = make_content(None, None, None, HashMap::new());
        let config = Config::default();

        // Layer 5 (caller default) should be used when no config is found
        let result = resolve_platform_field(
            &content,
            "confluence",
            &config,
            "space",
            Some("DEFAULT".to_string()),
        );
        assert_eq!(result, Some("DEFAULT".to_string()));
    }

    #[test]
    fn test_resolve_platform_field_none_when_not_found() {
        let content = make_content(None, None, None, HashMap::new());
        let config = Config::default();

        // Should return None when field is not found at any layer and no default
        let result = resolve_platform_field(&content, "confluence", &config, "space", None);
        assert_eq!(result, None);
    }
}
