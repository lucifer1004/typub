use crate::assets::AssetStrategy;
use crate::content::Content;
use crate::metadata::MetadataService;
use crate::resolved_config::ResolvedConfig;
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use typub_config::{Config, PlatformConfig, StorageConfig};
use typub_storage::StatusTracker;

use typub_adapters_core::{ContentInfo, resolve_asset_strategy_with_policy};

pub fn content_info_from(content: &Content) -> ContentInfo {
    ContentInfo::new(
        content.meta.title.clone(),
        content.slug().to_string(),
        content.path.clone(),
        content.meta.tags.clone(),
        content.meta.categories.clone(),
        content.assets.clone(),
    )
}

pub fn content_info_with_platform(content: &Content, platform_id: &str) -> ContentInfo {
    // Extract string values from platform extra config
    let platform_extra = content
        .platform_config(platform_id)
        .map(|cfg| {
            cfg.extra
                .iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();

    ContentInfo::with_platform_extra(
        content.meta.title.clone(),
        content.slug().to_string(),
        content.path.clone(),
        content.meta.tags.clone(),
        content.meta.categories.clone(),
        content.assets.clone(),
        platform_extra,
    )
}

use typub_core::{
    CapabilityGapBehavior, CapabilitySupport, DraftSupport, MathDelimiters, MathRendering,
    NodePolicyAction,
};

use typub_adapters_core::{AdapterCapability, ImageStrategyPolicy, NodePolicy, TaxonomyCapability};

use CapabilityGapBehavior as UnsupportedBehavior;

pub static BUILTIN_ADAPTERS: &[AdapterCapability] = &[
    typub_adapter_ghost::CAPABILITY,
    typub_adapter_devto::CAPABILITY,
    typub_adapter_wordpress::CAPABILITY,
    typub_adapter_hashnode::CAPABILITY,
    typub_adapter_confluence::CAPABILITY,
    typub_adapter_astro::CAPABILITY,
    typub_adapter_static::CAPABILITY,
    typub_adapter_xiaohongshu::CAPABILITY,
    typub_adapter_notion::CAPABILITY,
];

const COPYPASTE_DEFAULT_CAPABILITY: AdapterCapability = AdapterCapability {
    id: "copypaste",
    name: "Copy-Paste",
    short_code: "cp",
    local_output: true,
    requires_config: false,
    taxonomy: TaxonomyCapability::new(
        CapabilitySupport::Unsupported(UnsupportedBehavior::WarnAndDegrade),
        CapabilitySupport::Unsupported(UnsupportedBehavior::WarnAndDegrade),
        CapabilitySupport::Supported,
        DraftSupport::None,
    ),
    asset_strategies: &[AssetStrategy::Embed, AssetStrategy::External],
    math_renderings: &[MathRendering::Svg, MathRendering::Png],
    math_delimiters: &[MathDelimiters::Dollar, MathDelimiters::Brackets],
    code_highlight: false,
    notes: "Copy-paste format has no metadata sync. Internal links resolve to other published platforms.",
    node_policy: NodePolicy {
        raw: NodePolicyAction::Sanitize,
        unknown: NodePolicyAction::Drop,
    },
};

pub fn all_adapter_capabilities() -> &'static [AdapterCapability] {
    BUILTIN_ADAPTERS
}

pub fn adapter_capability(id: &str) -> Option<&'static AdapterCapability> {
    BUILTIN_ADAPTERS.iter().find(|c| c.id == id).or_else(|| {
        // Any known copy-paste profile → default copy-paste caps.
        if typub_adapter_copypaste::find_profile(id).is_some() {
            Some(&COPYPASTE_DEFAULT_CAPABILITY)
        } else {
            None
        }
    })
}

pub fn resolve_math_rendering(platform_id: &str) -> MathRendering {
    adapter_capability(platform_id)
        .map(|cap| cap.default_math_rendering())
        .unwrap_or(MathRendering::Svg)
}

pub fn resolve_math_delimiters(platform_id: &str) -> MathDelimiters {
    adapter_capability(platform_id)
        .map(|cap| cap.default_math_delimiter())
        .unwrap_or(MathDelimiters::Dollar)
}

pub fn resolve_code_highlight(platform_id: &str) -> bool {
    adapter_capability(platform_id)
        .map(|cap| cap.code_highlight)
        .unwrap_or(false)
}

pub fn is_local_output_platform(id: &str) -> bool {
    // Check API adapters first (using local_output field from TOML)
    if let Some(cap) = BUILTIN_ADAPTERS.iter().find(|c| c.id == id) {
        return cap.local_output;
    }

    // All copy-paste profiles are local-output
    typub_adapter_copypaste::find_profile(id).is_some()
}

pub fn is_copypaste_platform(id: &str) -> bool {
    typub_adapter_copypaste::find_profile(id).is_some()
}

pub fn platform_short_code(id: &str) -> Option<&'static str> {
    // Check API adapters first
    if let Some(cap) = BUILTIN_ADAPTERS.iter().find(|c| c.id == id) {
        return Some(cap.short_code);
    }
    // Check copypaste profiles
    if let Some(profile) = typub_adapter_copypaste::find_profile(id) {
        return Some(profile.short_code);
    }
    None
}

pub fn resolve_platform_asset_strategy(
    platform_id: &str,
    platform_config: Option<&PlatformConfig>,
    default: AssetStrategy,
) -> Result<AssetStrategy> {
    ResolvedConfig::resolve_asset_strategy_from_platform_config(
        platform_id,
        platform_config,
        default,
    )
}

pub fn resolve_asset_strategy_from_capability(
    platform_id: &str,
    platform_config: Option<&PlatformConfig>,
) -> Result<AssetStrategy> {
    let cap = adapter_capability(platform_id)
        .ok_or_else(|| anyhow::anyhow!("Unknown platform: {}", platform_id))?;

    resolve_platform_asset_strategy_with_policy(
        platform_id,
        platform_config,
        cap.default_asset_strategy(),
        cap.asset_strategy_policy(),
    )
}

pub fn resolve_platform_asset_strategy_with_policy(
    platform_id: &str,
    platform_config: Option<&PlatformConfig>,
    default: AssetStrategy,
    policy: ImageStrategyPolicy,
) -> Result<AssetStrategy> {
    // Delegates to shared helper per [[ADR-0005]]
    resolve_asset_strategy_with_policy(platform_id, platform_config, default, policy.supported)
}

pub struct PublishContext {
    pub status: StatusTracker,
    pub metadata: Arc<dyn MetadataService>,
    resolved: Option<crate::resolved_config::ResolvedConfig>,
    content_info: ContentInfo,
    /// Whether we're in dry-run mode (mock asset uploads to temp dir)
    pub dry_run: bool,
}

impl PublishContext {
    pub fn new(_config: &Config) -> Result<Self> {
        let status = StatusTracker::load(std::path::Path::new("."))?;
        let metadata = Arc::new(crate::metadata::DefaultMetadataService {});
        Ok(Self {
            status,
            metadata,
            resolved: None,
            content_info: ContentInfo::minimal("", "", std::path::PathBuf::new()),
            dry_run: false,
        })
    }

    pub fn new_with_root(_config: &Config, project_root: &Path) -> Result<Self> {
        let status = StatusTracker::load(project_root)?;
        let metadata = Arc::new(crate::metadata::DefaultMetadataService {});
        Ok(Self {
            status,
            metadata,
            resolved: None,
            content_info: ContentInfo::minimal("", "", std::path::PathBuf::new()),
            dry_run: false,
        })
    }

    /// Create a new PublishContext for dry-run mode.
    /// In dry-run mode, asset uploads are mocked (copied to temp dir).
    pub fn new_dry_run(_config: &Config, project_root: &Path) -> Result<Self> {
        let status = StatusTracker::load(project_root)?;
        let metadata = Arc::new(crate::metadata::DefaultMetadataService {});
        Ok(Self {
            status,
            metadata,
            resolved: None,
            content_info: ContentInfo::minimal("", "", std::path::PathBuf::new()),
            dry_run: true,
        })
    }

    pub fn set_resolved(&mut self, resolved: crate::resolved_config::ResolvedConfig) {
        self.resolved = Some(resolved);
    }

    pub fn resolved(&self) -> Option<&crate::resolved_config::ResolvedConfig> {
        self.resolved.as_ref()
    }

    pub fn set_content_info(&mut self, content_info: ContentInfo) {
        self.content_info = content_info;
    }

    pub fn content_info(&self) -> &ContentInfo {
        &self.content_info
    }
}

impl typub_adapters_core::AdapterContext for PublishContext {
    fn get_platform_id(&self, slug: &str, platform: &str) -> Result<Option<String>> {
        self.status.get_platform_id(slug, platform)
    }

    fn normalize_terms(&self, terms: &[String]) -> Vec<String> {
        self.metadata.normalize_terms(terms)
    }

    fn published(&self) -> bool {
        self.resolved.as_ref().map(|r| r.published).unwrap_or(false)
    }

    fn storage_config(&self) -> Option<&StorageConfig> {
        self.resolved.as_ref().and_then(|r| r.storage.as_ref())
    }

    fn theme_id(&self) -> Option<&str> {
        self.resolved.as_ref().and_then(|r| r.theme_id.as_deref())
    }

    fn content_info(&self) -> &ContentInfo {
        &self.content_info
    }

    fn status_tracker(&self) -> Option<&StatusTracker> {
        Some(&self.status)
    }

    fn is_dry_run(&self) -> bool {
        self.dry_run
    }
}

pub use crate::assets::ensure_no_unresolved_image_markers;

pub fn write_preview_file(slug: &str, platform: &str, html: &str) -> Result<PathBuf> {
    let dir = std::env::temp_dir().join("typub-preview");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{slug}-{platform}.html"));
    std::fs::write(&path, html)?;
    Ok(path)
}

use typub_adapters_core::PlatformAdapter;

pub struct AdapterRegistry {
    adapters: HashMap<String, Box<dyn PlatformAdapter>>,
}

impl AdapterRegistry {
    pub fn new(config: &Config) -> Result<Self> {
        type Factory = fn(&Config) -> Result<Box<dyn PlatformAdapter>>;

        // Factories with requires_config metadata
        let factories: Vec<(&str, Factory, bool)> = vec![
            ("astro", typub_adapter_astro::create, false),
            ("static", typub_adapter_static::create, false),
            ("xiaohongshu", typub_adapter_xiaohongshu::create, false),
            ("confluence", typub_adapter_confluence::create, true),
            ("devto", typub_adapter_devto::create, true),
            ("ghost", typub_adapter_ghost::create, true),
            ("hashnode", typub_adapter_hashnode::create, true),
            ("notion", typub_adapter_notion::create, true),
            ("wordpress", typub_adapter_wordpress::create, true),
        ];

        let mut adapters: HashMap<String, Box<dyn PlatformAdapter>> = HashMap::new();

        for (id, factory, requires_config) in &factories {
            let platform_config = config.get_platform(id);
            let should_register = if *requires_config {
                // Requires config: only register if config exists and is enabled
                platform_config.is_some_and(|p| p.enabled)
            } else {
                // No config required: register unless explicitly disabled
                platform_config.is_none_or(|p| p.enabled)
            };

            if should_register {
                adapters.insert(id.to_string(), factory(config)?);
            }
        }

        for profile in typub_adapter_copypaste::all_profiles() {
            let explicitly_disabled = config.get_platform(profile.id).is_some_and(|p| !p.enabled);
            if !explicitly_disabled {
                let adapter =
                    typub_adapter_copypaste::CopyPasteAdapter::from_profile(profile, config)?;
                adapters.insert(profile.id.to_string(), Box::new(adapter));
            }
        }

        for (id, pcfg) in &config.platforms {
            if !pcfg.enabled || adapters.contains_key(id) {
                continue;
            }
            if pcfg.get_str("type").as_deref() == Some("manual") {
                let id_static: &'static str = Box::leak(id.clone().into_boxed_str());
                let adapter = typub_adapter_copypaste::CopyPasteAdapter::from_manual_config(
                    id_static, pcfg, config,
                )?;
                adapters.insert(id.clone(), Box::new(adapter));
            }
        }

        Ok(Self { adapters })
    }

    pub fn get(&self, id: &str) -> Result<&dyn PlatformAdapter> {
        self.adapters.get(id).map(|a| a.as_ref()).ok_or_else(|| {
            anyhow::anyhow!(
                "Platform '{}' not found. It may be disabled (enabled = false) or not a known platform ID.",
                id,
            )
        })
    }

    pub fn list(&self) -> Vec<&str> {
        self.adapters.keys().map(|s| s.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use typub_adapters_core::{AdapterPayload, downcast_payload};
    use typub_html::{document, document_with_assets, image_marker};

    #[test]
    fn test_resolve_platform_asset_strategy_uses_default_when_missing() {
        let strategy = resolve_platform_asset_strategy("devto", None, AssetStrategy::Upload)
            .expect("should resolve default strategy");
        assert_eq!(strategy, AssetStrategy::Upload);
    }

    #[test]
    fn test_resolve_platform_asset_strategy_parses_override() {
        let cfg = PlatformConfig {
            enabled: true,
            asset_strategy: Some("embed".to_string()),
            published: None,
            theme: None,
            internal_link_target: None,
            math_rendering: None,
            math_delimiters: None,
            extra: HashMap::new(),
        };
        let strategy = resolve_platform_asset_strategy("wechat", Some(&cfg), AssetStrategy::Copy)
            .expect("should parse strategy override");
        assert_eq!(strategy, AssetStrategy::Embed);
    }

    #[test]
    fn test_resolve_platform_asset_strategy_rejects_invalid_value() {
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
        let err = resolve_platform_asset_strategy("astro", Some(&cfg), AssetStrategy::Copy)
            .expect_err("invalid strategy should error");
        assert!(err.to_string().contains("Invalid asset strategy"));
    }

    #[test]
    fn test_policy_rejects_unsupported_strategy() {
        let cfg = PlatformConfig {
            enabled: true,
            asset_strategy: Some("upload".to_string()),
            published: None,
            theme: None,
            internal_link_target: None,
            math_rendering: None,
            math_delimiters: None,
            extra: HashMap::new(),
        };
        let err = resolve_platform_asset_strategy_with_policy(
            "astro",
            Some(&cfg),
            AssetStrategy::Copy,
            ImageStrategyPolicy {
                supported: &[AssetStrategy::Copy, AssetStrategy::Embed],
            },
        )
        .expect_err("unsupported strategy should error");
        assert!(err.to_string().contains("is not supported"));
    }

    // -- Capability lookup --

    #[test]
    fn test_all_adapter_capabilities_count() {
        let caps = all_adapter_capabilities();
        assert_eq!(caps.len(), 9);
    }

    #[test]
    fn test_adapter_capability_api_platforms() {
        for id in &[
            "astro",
            "static",
            "confluence",
            "devto",
            "ghost",
            "hashnode",
            "notion",
            "wordpress",
            "xiaohongshu",
        ] {
            let cap = adapter_capability(id);
            assert!(cap.is_some(), "capability for '{id}' should exist");
            assert_eq!(cap.expect("checked").id, *id);
        }
    }

    #[test]
    fn test_adapter_capability_has_node_policy() {
        for id in &[
            "astro",
            "static",
            "confluence",
            "devto",
            "ghost",
            "hashnode",
            "notion",
            "wordpress",
            "xiaohongshu",
            "wechat",
        ] {
            let cap = adapter_capability(id).expect("capability should exist");
            let _ = cap.node_policy();
        }
    }

    #[test]
    fn test_adapter_capability_copypaste_fallback() {
        // wechat is a built-in copy-paste profile, should get default capability
        let cap = adapter_capability("wechat");
        assert!(cap.is_some());
        assert_eq!(cap.expect("checked").id, "copypaste");
    }

    #[test]
    fn test_adapter_capability_unknown_returns_none() {
        assert!(adapter_capability("nonexistent").is_none());
    }

    #[test]
    fn test_capability_support_gap_behavior() {
        assert!(CapabilitySupport::Supported.gap_behavior().is_none());
        assert_eq!(
            CapabilitySupport::Unsupported(CapabilityGapBehavior::WarnAndDegrade).gap_behavior(),
            Some(CapabilityGapBehavior::WarnAndDegrade)
        );
        assert_eq!(
            CapabilitySupport::Unsupported(CapabilityGapBehavior::HardError).gap_behavior(),
            Some(CapabilityGapBehavior::HardError)
        );
    }

    #[test]
    fn test_adapter_capability_gap_methods() {
        let cap = adapter_capability("astro").expect("astro exists");
        // Astro has unsupported tags and categories, but supports internal links
        assert!(cap.tags_gap_behavior().is_none());
        assert!(cap.categories_gap_behavior().is_none());
        assert!(cap.internal_links_gap_behavior().is_none()); // internal_links is Supported

        let cap = adapter_capability("wordpress").expect("wordpress exists");
        // WordPress supports everything
        assert!(cap.tags_gap_behavior().is_none());
        assert!(cap.categories_gap_behavior().is_none());
        assert!(cap.internal_links_gap_behavior().is_none());
    }

    #[test]
    fn test_image_strategy_policy_allow_all() {
        let policy = ImageStrategyPolicy::allow_all();
        assert_eq!(policy.supported.len(), 4);
        assert!(policy.supported.contains(&AssetStrategy::Copy));
        assert!(policy.supported.contains(&AssetStrategy::Embed));
        assert!(policy.supported.contains(&AssetStrategy::Upload));
        assert!(policy.supported.contains(&AssetStrategy::External));
    }

    #[test]
    fn test_downcast_payload_success() {
        let payload = AdapterPayload::simple(42u32, "test-slug");
        let value: u32 = downcast_payload(payload, "test").expect("downcast should succeed");
        assert_eq!(value, 42);
    }

    #[test]
    fn test_downcast_payload_wrong_type() {
        let payload = AdapterPayload::simple(42u32, "test-slug");
        let err = downcast_payload::<String>(payload, "test").expect_err("wrong type");
        assert!(err.to_string().contains("Invalid test publish payload"));
    }

    #[test]
    fn test_adapter_payload_with_assets() {
        use crate::assets::{AssetStrategy, DeferredAssets, PendingAsset, PendingAssetList};
        use std::path::PathBuf;

        let pending = PendingAssetList {
            assets: vec![PendingAsset {
                index: 0,
                local_path: PathBuf::from("/tmp/a.png"),
                original_ref: "a.png".to_string(),
            }],
        };
        let assets = DeferredAssets::new(pending, AssetStrategy::External);
        let content_info = ContentInfo::minimal("Test", "test-slug", "/tmp");
        let payload = AdapterPayload::new(42u32, content_info, assets, document(Vec::new()));

        assert!(payload.assets.needs_materialize());
        let value: u32 = payload.downcast("test").expect("downcast");
        assert_eq!(value, 42);
    }

    #[test]
    fn test_adapter_payload_map_inner() {
        let payload = AdapterPayload::simple(42u32, "test-slug");
        let mapped = payload
            .map_inner::<u32, _>("test", |v| v * 2)
            .expect("map inner");
        let value: u32 = mapped.downcast("test").expect("downcast");
        assert_eq!(value, 84);
    }

    #[test]
    fn test_ensure_no_unresolved_image_markers_no_deferred_strategy() {
        let (block, (asset_id, asset)) =
            image_marker("asset-a", "assets/a.png", "").expect("build image marker fixture");
        let document = document_with_assets(vec![block], [(asset_id, asset)]);
        ensure_no_unresolved_image_markers("astro", AssetStrategy::Copy, &document)
            .expect("copy strategy should not enforce marker guard");
    }

    #[test]
    fn test_ensure_no_unresolved_image_markers_errors_for_deferred_strategy() {
        let (block, (asset_id, asset)) =
            image_marker("asset-a", "assets/a.png", "").expect("build image marker fixture");
        let document = document_with_assets(vec![block], [(asset_id, asset)]);
        let err =
            ensure_no_unresolved_image_markers("confluence", AssetStrategy::Upload, &document)
                .expect_err("deferred strategy should fail on unresolved local assets");
        assert!(err.to_string().contains("unresolved local asset"));
    }

    #[test]
    fn test_adapter_registry_with_default_config() {
        // Default config has no platforms — built-in copy-paste profiles
        // and requires_config=false adapters (astro, static, xiaohongshu)
        // should still be registered.
        let config = Config::default();
        let registry = AdapterRegistry::new(&config).expect("create registry");
        let list = registry.list();
        // Should have all 24 built-in copy-paste profiles
        assert!(list.contains(&"wechat"));
        assert!(list.contains(&"zhihu"));
        assert!(list.contains(&"csdn"));
        // Should also have requires_config=false adapters
        assert!(list.contains(&"astro"));
        assert!(list.contains(&"static"));
        assert!(list.contains(&"xiaohongshu"));
        // Should NOT have requires_config=true adapters without config
        assert!(!list.contains(&"devto"));
        assert!(!list.contains(&"ghost"));
    }

    #[test]
    fn test_adapter_registry_get_nonexistent() {
        let config = Config::default();
        let registry = AdapterRegistry::new(&config).expect("create registry");
        match registry.get("nonexistent") {
            Ok(_) => panic!("expected error for nonexistent platform"),
            Err(e) => assert!(e.to_string().contains("not found")),
        }
    }

    #[test]
    fn test_adapter_registry_explicit_disable() {
        let config: Config = toml::from_str(
            r#"
[platforms.wechat]
enabled = false
"#,
        )
        .expect("parse config");
        let registry = AdapterRegistry::new(&config).expect("create registry");
        // wechat should be excluded
        assert!(!registry.list().contains(&"wechat"));
        // But other built-in profiles should still be present
        assert!(registry.list().contains(&"zhihu"));
    }

    /// Per [[ADR-0010]]: requires_config=false adapters can be explicitly disabled
    #[test]
    fn test_adapter_registry_requires_config_false_can_be_disabled() {
        let config: Config = toml::from_str(
            r#"
[platforms.astro]
enabled = false
"#,
        )
        .expect("parse config");
        let registry = AdapterRegistry::new(&config).expect("create registry");
        // astro should be excluded when explicitly disabled
        assert!(!registry.list().contains(&"astro"));
        // Other requires_config=false adapters should still be present
        assert!(registry.list().contains(&"static"));
        assert!(registry.list().contains(&"xiaohongshu"));
    }

    /// Per [[ADR-0010]]: requires_config=true adapter is registered when config exists AND enabled
    #[test]
    fn test_adapter_registry_requires_config_true_registered_when_enabled() {
        let config: Config = toml::from_str(
            r#"
[platforms.devto]
enabled = true
"#,
        )
        .expect("parse config");
        let registry = AdapterRegistry::new(&config).expect("create registry");
        // devto should be registered when config exists and is enabled
        assert!(registry.list().contains(&"devto"));
        // Other requires_config=true adapters without config should NOT be registered
        assert!(!registry.list().contains(&"ghost"));
        assert!(!registry.list().contains(&"wordpress"));
    }

    /// Per [[ADR-0010]]: requires_config=true adapter is NOT registered when config exists but disabled
    #[test]
    fn test_adapter_registry_requires_config_true_not_registered_when_disabled() {
        let config: Config = toml::from_str(
            r#"
[platforms.ghost]
enabled = false
"#,
        )
        .expect("parse config");
        let registry = AdapterRegistry::new(&config).expect("create registry");
        // ghost should NOT be registered when explicitly disabled
        assert!(!registry.list().contains(&"ghost"));
    }

    /// Per [[ADR-0010]]: verify capability requires_config values
    #[test]
    fn test_adapter_capability_requires_config_values() {
        // requires_config=false
        let astro = adapter_capability("astro").expect("astro exists");
        assert!(!astro.requires_config);

        let static_cap = adapter_capability("static").expect("static exists");
        assert!(!static_cap.requires_config);

        let xiaohongshu = adapter_capability("xiaohongshu").expect("xiaohongshu exists");
        assert!(!xiaohongshu.requires_config);

        // requires_config=true
        let devto = adapter_capability("devto").expect("devto exists");
        assert!(devto.requires_config);

        let ghost = adapter_capability("ghost").expect("ghost exists");
        assert!(ghost.requires_config);

        let wordpress = adapter_capability("wordpress").expect("wordpress exists");
        assert!(wordpress.requires_config);

        let hashnode = adapter_capability("hashnode").expect("hashnode exists");
        assert!(hashnode.requires_config);

        let confluence = adapter_capability("confluence").expect("confluence exists");
        assert!(confluence.requires_config);

        let notion = adapter_capability("notion").expect("notion exists");
        assert!(notion.requires_config);
    }
}
