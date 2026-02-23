#![allow(clippy::expect_used)]

use crate::{ID, WordPressAdapter};
use std::collections::HashMap;
use typub_adapters_core::{OutputFormat, PlatformAdapter};
use typub_config::PlatformConfig;
use typub_core::{AssetStrategy, MathRendering};

fn new_for_test() -> WordPressAdapter {
    WordPressAdapter::new_for_test()
}

fn make_platform_config_with(base_url: &str, api_key: &str) -> PlatformConfig {
    let mut extra = HashMap::new();
    extra.insert(
        "base_url".to_string(),
        toml::Value::String(base_url.to_string()),
    );
    extra.insert(
        "api_key".to_string(),
        toml::Value::String(api_key.to_string()),
    );
    PlatformConfig {
        enabled: true,
        asset_strategy: None,
        published: None,
        theme: None,
        internal_link_target: None,
        math_rendering: None,
        math_delimiters: None,
        extra,
    }
}

#[test]
fn test_resolve_asset_strategy_default() {
    let resolved = crate::config::resolve_asset_strategy(None).expect("resolve default");
    assert_eq!(resolved, crate::config::CAPABILITY.default_asset_strategy());
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
    let err = crate::config::resolve_asset_strategy(Some(&cfg))
        .expect_err("invalid strategy should fail");
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
    let resolved = crate::config::resolve_asset_strategy(Some(&cfg)).expect("resolve default");
    assert_eq!(resolved, crate::config::CAPABILITY.default_asset_strategy());
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
    let err = crate::config::resolve_asset_strategy(Some(&cfg))
        .expect_err("unsupported strategy should fail");
    assert!(err.to_string().contains("does not support"));
}

#[test]
fn test_resolve_asset_strategy_embed_ok() {
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
    let resolved = crate::config::resolve_asset_strategy(Some(&cfg)).expect("embed strategy");
    assert_eq!(resolved, AssetStrategy::Embed);
}

#[test]
fn test_render_config_for_upload_uses_markers() {
    let render = typub_adapters_core::default_render_config_for(
        AssetStrategy::Upload,
        &crate::config::CAPABILITY,
    );
    assert!(render.image_as_marker);
}

#[test]
fn test_render_config_for_embed_no_markers() {
    let render = typub_adapters_core::default_render_config_for(
        AssetStrategy::Embed,
        &crate::config::CAPABILITY,
    );
    assert!(!render.image_as_marker);
}

#[test]
fn test_register_adds_capability() {
    let mut registrar = typub_adapters_core::AdapterRegistrar::new();
    crate::config::register(&mut registrar).expect("register adapter");
    assert!(registrar.capabilities().contains_key(crate::ID));
}

#[test]
fn test_validate_config_missing_base_url() {
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
    let err = adapter
        .validate_config(&cfg)
        .expect_err("should fail without base_url");
    assert!(err.to_string().contains("base_url"));
}

#[test]
fn test_validate_config_missing_api_key() {
    // Create adapter without api_key (no env var, no config)
    let adapter = WordPressAdapter::new_for_test_with(
        "http://localhost",
        None, // No api_key
        AssetStrategy::Upload,
        MathRendering::Svg,
    );
    let mut extra = HashMap::new();
    extra.insert(
        "base_url".to_string(),
        toml::Value::String("https://example.com".to_string()),
    );
    let cfg = PlatformConfig {
        enabled: true,
        asset_strategy: None,
        published: None,
        theme: None,
        internal_link_target: None,
        math_rendering: None,
        math_delimiters: None,
        extra,
    };
    let err = adapter
        .validate_config(&cfg)
        .expect_err("should fail without api_key");
    assert!(err.to_string().contains("WORDPRESS_API_KEY"));
}

#[test]
fn test_validate_config_ok() {
    let adapter = new_for_test();
    let cfg = make_platform_config_with("https://example.com", "secret-token");
    assert!(adapter.validate_config(&cfg).is_ok());
}

#[test]
fn test_trait_methods() {
    let adapter = new_for_test();
    assert_eq!(adapter.id(), ID);
    assert_eq!(adapter.name(), "WordPress");
    assert_eq!(adapter.required_format(), OutputFormat::Html);
    assert_eq!(adapter.asset_strategy(), AssetStrategy::Upload);
    assert!(adapter.supports_shared_link_rewrite());
}

#[test]
fn test_capability_fields() {
    let cap = &crate::config::CAPABILITY;
    assert_eq!(cap.id, "wordpress");
    assert_eq!(cap.short_code, "wp");
    assert!(cap.code_highlight);
}
