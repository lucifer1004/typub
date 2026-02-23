#![allow(clippy::expect_used)]

use crate::capability::{AdapterCapability, NodePolicy};
use crate::context::AdapterContext;
use crate::helpers::{
    default_render_config_for, register_adapter, resolve_asset_strategy_from_config,
};
use crate::payload::{AdapterPayload, downcast_payload};
use crate::registrar::AdapterRegistrar;
use crate::types::{ContentInfo, OutputFormat, ResolvedConfigDefaults};
use crate::{AdapterFactory, PlatformAdapter, TaxonomyCapability};
use crate::{
    build_pending_asset_list_from_document, ensure_no_unresolved_image_markers, resolve_asset_urls,
};
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::PathBuf;
use typub_core::{
    AssetStrategy, CapabilityGapBehavior, CapabilitySupport, MathDelimiters, MathRendering,
    NodePolicyAction,
};
use typub_ir::{Asset, AssetId, AssetSource, DocMeta, Document, ImageAsset, RelativePath};

fn empty_document() -> Document {
    Document {
        blocks: Vec::new(),
        footnotes: BTreeMap::new(),
        assets: BTreeMap::new(),
        meta: DocMeta::default(),
    }
}

fn local_image_asset(path: &str) -> Asset {
    Asset::Image(ImageAsset {
        source: AssetSource::LocalPath {
            path: RelativePath::new(path.to_string()).expect("valid relative path"),
        },
        meta: None,
        variants: Vec::new(),
    })
}

#[test]
fn test_downcast_payload() {
    let payload = AdapterPayload::simple("hello".to_string(), "demo");
    let inner = downcast_payload::<String>(payload, "demo").expect("downcast");
    assert_eq!(inner, "hello");
}

#[test]
fn test_build_pending_asset_list_from_document() {
    let mut document = empty_document();
    document.assets.insert(
        AssetId("asset-1".to_string()),
        local_image_asset("image1.png"),
    );
    document.assets.insert(
        AssetId("asset-2".to_string()),
        local_image_asset("image2.png"),
    );
    let result =
        build_pending_asset_list_from_document(&document, PathBuf::from("/content").as_path());
    assert_eq!(result.assets.len(), 2);
}

#[test]
fn test_resolve_asset_urls() {
    let mut document = empty_document();
    document.assets.insert(
        AssetId("asset-1".to_string()),
        local_image_asset("image.png"),
    );
    let mut url_map = HashMap::new();
    url_map.insert("image.png".into(), "https://example.com/image.png".into());
    let resolved = resolve_asset_urls(&mut document, &url_map);
    assert_eq!(resolved, 1);
}

#[test]
fn test_ensure_no_unresolved_image_markers() {
    let mut document = empty_document();
    document.assets.insert(
        AssetId("asset-1".to_string()),
        local_image_asset("image.png"),
    );
    let result = ensure_no_unresolved_image_markers("test", AssetStrategy::Upload, &document);
    assert!(result.is_err());
}

fn make_demo_capability() -> AdapterCapability {
    AdapterCapability {
        id: "demo",
        name: "Demo",
        short_code: "dm",
        local_output: false,
        requires_config: true,
        taxonomy: TaxonomyCapability::new(
            CapabilitySupport::Supported,
            CapabilitySupport::Unsupported(CapabilityGapBehavior::WarnAndDegrade),
            CapabilitySupport::Supported,
            typub_core::DraftSupport::None,
        ),
        asset_strategies: &[AssetStrategy::Embed],
        math_renderings: &[MathRendering::Svg],
        math_delimiters: &[MathDelimiters::Dollar],
        code_highlight: false,
        notes: "demo",
        node_policy: NodePolicy {
            raw: NodePolicyAction::Pass,
            unknown: NodePolicyAction::Pass,
        },
    }
}

#[test]
fn test_default_render_config_for() {
    let capability = make_demo_capability();
    let cfg = default_render_config_for(AssetStrategy::Embed, &capability);
    assert_eq!(cfg.math_rendering, MathRendering::Svg);
}

#[test]
fn test_resolve_asset_strategy_from_config() {
    let config = typub_config::PlatformConfig {
        enabled: true,
        asset_strategy: None,
        published: None,
        theme: None,
        internal_link_target: None,
        math_rendering: None,
        math_delimiters: None,
        extra: HashMap::new(),
    };
    let capability = AdapterCapability {
        id: "devto",
        name: "Dev.to",
        short_code: "dt",
        local_output: false,
        requires_config: true,
        taxonomy: TaxonomyCapability::new(
            CapabilitySupport::Supported,
            CapabilitySupport::Supported,
            CapabilitySupport::Supported,
            typub_core::DraftSupport::None,
        ),
        asset_strategies: &[AssetStrategy::Upload, AssetStrategy::Embed],
        math_renderings: &[MathRendering::Svg],
        math_delimiters: &[MathDelimiters::Dollar],
        code_highlight: false,
        notes: "demo",
        node_policy: NodePolicy {
            raw: NodePolicyAction::Pass,
            unknown: NodePolicyAction::Pass,
        },
    };
    let resolved =
        resolve_asset_strategy_from_config(Some(&config), &capability).expect("strategy");
    assert_eq!(resolved, AssetStrategy::Upload);
}

#[test]
fn test_content_info_with_rendered_paths() {
    let info = ContentInfo::minimal("title", "slug", "/path")
        .with_rendered_paths(vec![PathBuf::from("/tmp/rendered.html")]);
    assert_eq!(info.rendered_paths.len(), 1);
}

#[test]
fn test_resolved_config_defaults() {
    let defaults = ResolvedConfigDefaults::new(false, None, AssetStrategy::Embed);
    assert!(!defaults.published);
}

#[test]
fn test_adapter_capability_policy() {
    let cap = make_demo_capability();
    let policy = cap.asset_strategy_policy();
    assert!(policy.supported.contains(&AssetStrategy::Embed));
}

struct DummyAdapter;

#[async_trait::async_trait(?Send)]
impl PlatformAdapter for DummyAdapter {
    fn id(&self) -> &'static str {
        "demo"
    }

    fn name(&self) -> &'static str {
        "Demo"
    }

    fn required_format(&self) -> OutputFormat {
        OutputFormat::Html
    }

    fn asset_strategy(&self) -> AssetStrategy {
        AssetStrategy::Embed
    }

    fn validate_config(&self, _config: &typub_config::PlatformConfig) -> anyhow::Result<()> {
        Ok(())
    }

    async fn specialize_payload(
        &self,
        document: Document,
        _ctx: &dyn AdapterContext,
    ) -> anyhow::Result<AdapterPayload> {
        Ok(AdapterPayload::simple(
            typub_html::document_to_html(&document),
            "demo",
        ))
    }

    async fn check_status(&self, _slug: &str) -> anyhow::Result<bool> {
        Ok(true)
    }
}

#[test]
fn test_adapter_registrar_register_and_build() {
    let mut registrar = AdapterRegistrar::new();
    let factory: AdapterFactory = |_config| Ok(Box::new(DummyAdapter));
    let capability = AdapterCapability {
        id: "demo",
        name: "Demo",
        short_code: "dm",
        local_output: false,
        requires_config: true,
        taxonomy: TaxonomyCapability::full(),
        asset_strategies: &[AssetStrategy::Embed],
        math_renderings: &[MathRendering::Svg],
        math_delimiters: &[MathDelimiters::Dollar],
        code_highlight: false,
        notes: "demo",
        node_policy: NodePolicy {
            raw: NodePolicyAction::Pass,
            unknown: NodePolicyAction::Pass,
        },
    };
    register_adapter(&mut registrar, &capability, factory).expect("register");
    assert!(registrar.capabilities().contains_key("demo"));
}
