#![allow(clippy::expect_used)]

use crate::ID;
use crate::adapter::GhostAdapter;
use crate::config::{CAPABILITY, resolve_asset_strategy};
use typub_adapters_core::{
    AdapterRegistrar, OutputFormat, PlatformAdapter, default_render_config_for,
};
use typub_config::PlatformConfig;
use typub_core::{AssetStrategy, MathRendering};

#[test]
fn test_register_adds_capability() {
    let mut registrar = AdapterRegistrar::new();
    crate::register(&mut registrar).expect("register adapter");
    assert!(registrar.capabilities().contains_key(ID));
}

#[test]
fn test_resolve_asset_strategy_default() {
    let resolved = resolve_asset_strategy(None).expect("resolve default");
    assert_eq!(resolved, CAPABILITY.default_asset_strategy());
}

#[test]
fn test_resolve_asset_strategy_invalid_value() {
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
    let err = resolve_asset_strategy(Some(&cfg)).expect_err("invalid strategy should fail");
    assert!(err.to_string().contains("Invalid asset strategy"));
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
    let resolved = resolve_asset_strategy(Some(&cfg)).expect("resolve default");
    assert_eq!(resolved, CAPABILITY.default_asset_strategy());
}

#[test]
fn test_resolve_asset_strategy_unsupported() {
    // Valid but unsupported strategy should error
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
    let err = resolve_asset_strategy(Some(&cfg)).expect_err("unsupported strategy should fail");
    assert!(err.to_string().contains("does not support"));
}

#[test]
fn test_render_config_for_upload_uses_markers() {
    let render = default_render_config_for(AssetStrategy::Upload, &CAPABILITY);
    assert!(render.image_as_marker);
}

#[test]
fn test_trait_methods() {
    let adapter = new_for_test();
    assert_eq!(adapter.id(), ID);
    assert_eq!(adapter.name(), "Ghost");
    assert_eq!(adapter.required_format(), OutputFormat::Html);
    assert_eq!(adapter.asset_strategy(), AssetStrategy::Upload);
}

#[test]
fn test_validate_config_missing_api_key() {
    let adapter = GhostAdapter::new_for_test_with(
        "http://localhost:2368",
        None,
        AssetStrategy::Upload,
        MathRendering::Svg,
    );
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
    let err = adapter
        .validate_config(&cfg)
        .expect_err("missing api key should fail");
    assert!(err.to_string().contains("ghost.api_key"));
}

fn new_for_test() -> GhostAdapter {
    GhostAdapter::new_for_test()
}
