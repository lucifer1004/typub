#![allow(clippy::expect_used)]

use crate::model::NotionSchema;
use crate::{ID, NotionAdapter};
use serde_json::json;
use typub_adapters_core::{OutputFormat, PlatformAdapter};
use typub_config::PlatformConfig;
use typub_core::AssetStrategy;
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn new_for_test() -> NotionAdapter {
    NotionAdapter::new_for_test()
}

#[test]
fn test_normalized_tags_sorted_and_deduped() {
    let tags = vec![
        " Rust ".to_string(),
        "cms".to_string(),
        "rust".to_string(),
        "".to_string(),
    ];
    let normalized = NotionAdapter::normalized_tags(&tags);
    assert_eq!(normalized, vec!["cms".to_string(), "Rust".to_string()]);
}

#[test]
fn test_build_properties() {
    let adapter = new_for_test();
    let schema = NotionSchema {
        title_property: "Title".to_string(),
        tags_property: "Tags".to_string(),
    };

    let props =
        adapter.build_properties(&schema, "Hello", &["rust".to_string(), "cms".to_string()]);
    assert_eq!(
        props["Title"]["title"][0]["text"]["content"].as_str(),
        Some("Hello")
    );
    assert_eq!(
        props["Tags"]["multi_select"].as_array().map(Vec::len),
        Some(2)
    );

    let props_empty = adapter.build_properties(&schema, "Hello", &[]);
    assert_eq!(
        props_empty["Tags"]["multi_select"].as_array().map(Vec::len),
        Some(0)
    );
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
        asset_strategy: Some("embed".to_string()),
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
fn test_render_config_for_upload_uses_markers() {
    let render = crate::config::render_config_for(AssetStrategy::Upload);
    assert!(render.image_as_marker);
}

#[test]
fn test_register_adds_capability() {
    let mut registrar = typub_adapters_core::AdapterRegistrar::new();
    crate::config::register(&mut registrar).expect("register adapter");
    assert!(registrar.capabilities().contains_key(crate::ID));
}

#[test]
fn test_validate_config_no_token() {
    let adapter = NotionAdapter::new_for_test_with(
        "http://localhost",
        false,
        Some("ds-1".to_string()),
        AssetStrategy::Upload,
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
        .expect_err("should fail without token");
    assert!(err.to_string().contains("NOTION_API_KEY"));
}

#[test]
fn test_validate_config_no_data_source_id() {
    let adapter =
        NotionAdapter::new_for_test_with("http://localhost", true, None, AssetStrategy::Upload);
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
        .expect_err("should fail without data_source_id");
    assert!(err.to_string().contains("data_source_id"));
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
    assert_eq!(adapter.name(), "Notion");
    assert_eq!(adapter.required_format(), OutputFormat::Html);
    assert_eq!(adapter.asset_strategy(), AssetStrategy::Upload);
}

#[tokio::test]
async fn test_find_existing_page_found() {
    let server = MockServer::start().await;
    let adapter = NotionAdapter::new_for_test_with(
        &server.uri(),
        true,
        Some("ds-1".to_string()),
        AssetStrategy::Upload,
    );

    Mock::given(method("POST"))
        .and(path_regex("/data_sources/.*/query"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [{ "id": "page-abc-123" }]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let result = adapter
        .find_existing_page("ds-1", "Title", "My Post")
        .await
        .expect("find page");
    assert_eq!(result, Some("page-abc-123".to_string()));
}

#[tokio::test]
async fn test_find_existing_page_not_found() {
    let server = MockServer::start().await;
    let adapter = NotionAdapter::new_for_test_with(
        &server.uri(),
        true,
        Some("ds-1".to_string()),
        AssetStrategy::Upload,
    );

    Mock::given(method("POST"))
        .and(path_regex("/data_sources/.*/query"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": []
        })))
        .expect(1)
        .mount(&server)
        .await;

    let result = adapter
        .find_existing_page("ds-1", "Title", "Nonexistent")
        .await
        .expect("find page");
    assert!(result.is_none());
}

#[tokio::test]
async fn test_create_page_with_blocks_small_batch() {
    let server = MockServer::start().await;
    let adapter = NotionAdapter::new_for_test_with(
        &server.uri(),
        true,
        Some("ds-1".to_string()),
        AssetStrategy::Upload,
    );

    Mock::given(method("POST"))
        .and(path_regex("/pages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "new-page-1",
            "url": "https://notion.so/new-page-1"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let blocks = vec![json!({ "type": "paragraph", "paragraph": { "rich_text": [] } })];
    let (page_id, url) = adapter
        .create_page_with_blocks("ds-1", json!({}), &blocks)
        .await
        .expect("create page");
    assert_eq!(page_id, "new-page-1");
    assert_eq!(url, "https://notion.so/new-page-1");
}

#[tokio::test]
async fn test_ensure_data_source_schema_already_valid() {
    let server = MockServer::start().await;
    let adapter = NotionAdapter::new_for_test_with(
        &server.uri(),
        true,
        Some("ds-1".to_string()),
        AssetStrategy::Upload,
    );

    Mock::given(method("GET"))
        .and(path_regex("/data_sources/ds-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "properties": {
                "Title": { "type": "title", "id": "title-id" },
                "Tags": { "type": "multi_select", "id": "tags-id" }
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let schema = adapter
        .ensure_data_source_schema("ds-1")
        .await
        .expect("schema");
    assert_eq!(schema.title_property, "Title");
    assert_eq!(schema.tags_property, "Tags");
}
