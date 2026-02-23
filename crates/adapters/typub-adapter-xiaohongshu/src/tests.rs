#![allow(clippy::expect_used)]

use crate::{ID, XiaohongshuAdapter};
use typub_adapters_core::{ContentTransform, OutputFormat, PlatformAdapter};
use typub_config::PlatformConfig;
use typub_core::AssetStrategy;

fn new_for_test() -> XiaohongshuAdapter {
    XiaohongshuAdapter::new_for_test()
}

#[test]
fn test_resolve_asset_strategy_default() {
    let resolved = crate::config::resolve_strategy(None).expect("resolve default");
    assert_eq!(resolved, crate::config::CAPABILITY.default_asset_strategy());
}

#[test]
fn test_resolve_asset_strategy_embed() {
    let cfg = PlatformConfig {
        enabled: true,
        asset_strategy: Some("embed".to_string()),
        published: None,
        theme: None,
        internal_link_target: None,
        math_rendering: None,
        math_delimiters: None,
        extra: std::collections::HashMap::new(),
    };
    let resolved = crate::config::resolve_strategy(Some(&cfg)).expect("resolve embed");
    assert_eq!(resolved, AssetStrategy::Embed);
}

#[test]
fn test_resolve_asset_strategy_invalid() {
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
    let err = crate::config::resolve_strategy(Some(&cfg)).expect_err("invalid should fail");
    assert!(err.to_string().contains("Invalid asset strategy"));
}

#[test]
fn test_resolve_asset_strategy_unsupported() {
    let cfg = PlatformConfig {
        enabled: true,
        asset_strategy: Some("copy".to_string()),
        published: None,
        theme: None,
        internal_link_target: None,
        math_rendering: None,
        math_delimiters: None,
        extra: std::collections::HashMap::new(),
    };
    let err = crate::config::resolve_strategy(Some(&cfg)).expect_err("unsupported should fail");
    assert!(err.to_string().contains("does not support"));
}

#[test]
fn test_resolve_asset_strategy_disabled_platform() {
    let cfg = PlatformConfig {
        enabled: false,
        asset_strategy: Some("embed".to_string()),
        published: None,
        theme: None,
        internal_link_target: None,
        math_rendering: None,
        math_delimiters: None,
        extra: std::collections::HashMap::new(),
    };
    let resolved = crate::config::resolve_strategy(Some(&cfg)).expect("resolve default");
    assert_eq!(resolved, crate::config::CAPABILITY.default_asset_strategy());
}

#[test]
fn test_render_config_contains_no_imports() {
    let adapter = new_for_test();
    let content_info = typub_adapters_core::ContentInfo::minimal(
        "Test",
        "test-slug",
        std::path::PathBuf::from("/tmp/test.typ"),
    );
    let render = adapter.render_config(&content_info);
    assert!(render.imports.is_empty());
}

#[test]
fn test_render_config_has_show_rules() {
    let adapter = new_for_test();
    let content_info = typub_adapters_core::ContentInfo::minimal(
        "Test",
        "test-slug",
        std::path::PathBuf::from("/tmp/test.typ"),
    );
    let render = adapter.render_config(&content_info);
    assert!(matches!(
        render.content_transform,
        ContentTransform::ShowRules(_)
    ));
}

#[test]
fn test_render_config_no_marker() {
    let adapter = new_for_test();
    let content_info = typub_adapters_core::ContentInfo::minimal(
        "Test",
        "test-slug",
        std::path::PathBuf::from("/tmp/test.typ"),
    );
    let render = adapter.render_config(&content_info);
    assert!(!render.image_as_marker);
}

#[test]
fn test_register_adds_capability() {
    let mut registrar = typub_adapters_core::AdapterRegistrar::new();
    crate::config::register(&mut registrar).expect("register adapter");
    assert!(registrar.capabilities().contains_key(ID));
}

#[test]
fn test_validate_config_ok() {
    let adapter = new_for_test();
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
    assert!(adapter.validate_config(&cfg).is_ok());
}

#[test]
fn test_trait_methods() {
    let adapter = new_for_test();
    assert_eq!(adapter.id(), ID);
    assert_eq!(adapter.name(), "Xiaohongshu (小红书)");
    assert_eq!(adapter.required_format(), OutputFormat::Png);
    assert_eq!(adapter.asset_strategy(), AssetStrategy::Embed);
}
