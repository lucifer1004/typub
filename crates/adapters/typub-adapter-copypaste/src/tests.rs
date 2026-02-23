#![allow(clippy::expect_used)]

use std::path::PathBuf;

use typub_adapters_core::{ContentInfo, OutputFormat, PlatformAdapter};
use typub_config::PlatformConfig;
use typub_core::AssetStrategy;

use crate::{CopyFormat, CopyPasteAdapter, all_profiles, find_profile, register};

#[test]
fn test_all_profiles_not_empty() {
    let profiles = all_profiles();
    assert!(!profiles.is_empty());
    // Should have at least wechat, zhihu
    let ids: Vec<_> = profiles.iter().map(|p| p.id).collect();
    assert!(ids.contains(&"wechat"));
    assert!(ids.contains(&"zhihu"));
}

#[test]
fn test_find_profile_wechat() {
    let profile = find_profile("wechat").expect("wechat profile");
    assert_eq!(profile.id, "wechat");
    assert_eq!(profile.name, "WeChat Official Account");
    assert_eq!(profile.format, CopyFormat::StyledHtml);
    // code_highlight is derived from format: StyledHtml => true
    let adapter = CopyPasteAdapter::new_for_test("wechat").expect("adapter");
    assert!(adapter.code_highlight());
}

#[test]
fn test_find_profile_zhihu() {
    let profile = find_profile("zhihu").expect("zhihu profile");
    assert_eq!(profile.id, "zhihu");
    assert_eq!(profile.format, CopyFormat::Markdown);
    assert_eq!(profile.default_asset_strategy(), AssetStrategy::External);
}

#[test]
fn test_find_profile_not_found() {
    let profile = find_profile("nonexistent");
    assert!(profile.is_none());
}

#[test]
fn test_register_creates_adapters() {
    let mut registrar = typub_adapters_core::AdapterRegistrar::new();
    register(&mut registrar).expect("register");

    // Verify some key profiles are registered
    let caps = registrar.capabilities();
    assert!(caps.contains_key("wechat"));
    assert!(caps.contains_key("zhihu"));
    assert!(caps.contains_key("medium"));
}

#[test]
fn test_adapter_wechat_trait_methods() {
    let adapter = CopyPasteAdapter::new_for_test("wechat").expect("create wechat");

    assert_eq!(adapter.id(), "wechat");
    assert_eq!(adapter.name(), "WeChat Official Account");
    assert_eq!(adapter.required_format(), OutputFormat::Html);
    assert_eq!(adapter.asset_strategy(), AssetStrategy::Embed);
}

#[test]
fn test_adapter_zhihu_trait_methods() {
    let adapter = CopyPasteAdapter::new_for_test("zhihu").expect("create zhihu");

    assert_eq!(adapter.id(), "zhihu");
    assert_eq!(adapter.name(), "Zhihu");
    assert_eq!(adapter.required_format(), OutputFormat::Html);
    assert_eq!(adapter.asset_strategy(), AssetStrategy::External);
}

#[test]
fn test_adapter_validate_config() {
    let adapter = CopyPasteAdapter::new_for_test("wechat").expect("create wechat");

    let platform_config = PlatformConfig {
        enabled: true,
        asset_strategy: None,
        published: None,
        theme: None,
        internal_link_target: None,
        math_rendering: None,
        math_delimiters: None,
        extra: std::collections::HashMap::new(),
    };

    assert!(adapter.validate_config(&platform_config).is_ok());
}

#[test]
fn test_adapter_render_config_embed_no_marker() {
    let adapter = CopyPasteAdapter::new_for_test("wechat").expect("create wechat");

    let content_info = ContentInfo::minimal("Test", "test-slug", PathBuf::from("/tmp/test.typ"));
    let render = adapter.render_config(&content_info);

    // Embed strategy should not use markers
    assert!(!render.image_as_marker);
}

#[test]
fn test_adapter_render_config_external_uses_marker() {
    let adapter = CopyPasteAdapter::new_for_test("zhihu").expect("create zhihu");

    let content_info = ContentInfo::minimal("Test", "test-slug", PathBuf::from("/tmp/test.typ"));
    let render = adapter.render_config(&content_info);

    // External strategy should use markers
    assert!(render.image_as_marker);
}

#[test]
fn test_adapter_supports_link_rewrite() {
    let adapter = CopyPasteAdapter::new_for_test("wechat").expect("create wechat");

    // All copypaste adapters support internal link rewriting
    assert!(adapter.supports_shared_link_rewrite());
}

#[test]
fn test_html_platforms_use_styled_html() {
    let html_platforms = ["wechat", "toutiao", "bilibili", "medium"];
    for id in html_platforms {
        let profile = find_profile(id).unwrap_or_else(|| panic!("{} profile", id));
        assert_eq!(
            profile.format,
            CopyFormat::StyledHtml,
            "platform {} should use StyledHtml",
            id
        );
    }
}

#[test]
fn test_markdown_platforms_use_markdown() {
    let md_platforms = ["zhihu", "csdn", "juejin", "cnblogs"];
    for id in md_platforms {
        let profile = find_profile(id).unwrap_or_else(|| panic!("{} profile", id));
        assert_eq!(
            profile.format,
            CopyFormat::Markdown,
            "platform {} should use Markdown",
            id
        );
    }
}
