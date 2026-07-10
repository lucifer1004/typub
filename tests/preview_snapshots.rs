#![allow(clippy::expect_used)]

//! Integration tests for adapter preview HTML output.
//! Uses cargo-insta for snapshot testing.

use chrono::NaiveDate;
use std::sync::Once;

/// Ensure locale is set to English for deterministic snapshots.
/// Must be called before any preview() calls that use i18n.
static INIT_LOCALE: Once = Once::new();

fn ensure_english_locale() {
    INIT_LOCALE.call_once(|| {
        typub_ui::i18n::set_locale(typub_ui::i18n::Locale::En);
    });
}
use std::collections::HashMap;
use std::path::PathBuf;
use typub_adapters_core::{
    AdapterContext, ContentInfo, DefaultMetadataService, MetadataService, OutputFormat,
    ResolvedConfigDefaults,
};
use typub_engine::adapters::{Document, PlatformAdapter};
use typub_engine::content::{Content, ContentFormat, ContentMeta};
use typub_engine::renderer::RenderedOutput;
use typub_engine::resolved_config::ResolvedConfig;

/// Build a minimal `Content` for testing.
fn make_test_content() -> Content {
    Content {
        path: PathBuf::from("/tmp/test-post"),
        meta: ContentMeta {
            title: "Test Post Title".to_string(),
            created: NaiveDate::from_ymd_opt(2026, 1, 15).expect("valid date"),
            updated: None,
            tags: vec!["rust".to_string(), "test".to_string()],
            categories: vec!["engineering".to_string()],
            published: None,
            theme: None,
            internal_link_target: None,
            preamble: None,
            platforms: HashMap::new(),
        },
        content_file: PathBuf::from("/tmp/test-post/content.typ"),
        source_format: ContentFormat::Typst,
        slides_file: None,
        assets: vec![],
    }
}

/// Sample HTML that exercises all major element types.
const SAMPLE_HTML: &str = r#"<!DOCTYPE html><html><head></head><body>
<h1>Introduction</h1>
<p>This is a <strong>test</strong> paragraph with <em>emphasis</em>.</p>
<h2>Code Example</h2>
<pre><code data-lang="rust">fn main() {
    println!("Hello!");
}</code></pre>
<ul><li>First item</li><li>Second item</li></ul>
<ol><li>Step one</li><li>Step two</li></ol>
<blockquote>A wise quote</blockquote>
<table><tr><th>Name</th><th>Value</th></tr><tr><td>A</td><td>1</td></tr></table>
<hr>
<p>Final paragraph.</p>
</body></html>"#;

/// Build a `RenderedOutput` with sample HTML.
fn make_test_rendered() -> RenderedOutput {
    RenderedOutput {
        format: OutputFormat::Html,
        paths: vec![],
        html: Some(SAMPLE_HTML.to_string()),
    }
}

/// Parse test rendered HTML into semantic document IR (simulating pipeline stages 3-4).
fn make_test_document(rendered: &RenderedOutput) -> Document {
    let html = rendered.html.as_ref().expect("html content");
    typub_html::parse_html_document(html).expect("parse html")
}

struct PreviewContext {
    resolved: ResolvedConfig,
    content_info: ContentInfo,
}

impl AdapterContext for PreviewContext {
    fn get_platform_id(&self, _slug: &str, _platform: &str) -> anyhow::Result<Option<String>> {
        Ok(None)
    }

    fn normalize_terms(&self, terms: &[String]) -> Vec<String> {
        DefaultMetadataService.normalize_terms(terms)
    }

    fn published(&self) -> bool {
        self.resolved.published
    }

    fn storage_config(&self) -> Option<&typub_config::StorageConfig> {
        self.resolved.storage.as_ref()
    }

    fn theme_id(&self) -> Option<&str> {
        self.resolved.theme_id.as_deref()
    }

    fn content_info(&self) -> &ContentInfo {
        &self.content_info
    }
}

/// Build an in-memory adapter context with resolved config for a given platform.
fn make_test_ctx(
    content: &Content,
    platform_id: &str,
    config: &typub_config::Config,
) -> PreviewContext {
    let resolved = ResolvedConfig::resolve(
        content,
        platform_id,
        config,
        ResolvedConfigDefaults::default(),
    )
    .expect("resolve config");
    PreviewContext {
        resolved,
        content_info: typub_engine::content_info_from(content),
    }
}

#[test]
fn test_preview_notion() {
    // Notion adapter can be constructed with default config
    let config = typub_config::Config::default();
    // NotionAdapter::new reads NOTION_API_TOKEN env var; it's fine if it's missing,
    // has_token will be false but build_preview() doesn't need the token.
    let adapter = typub_adapter_notion::NotionAdapter::new(&config).expect("create notion adapter");

    let content = make_test_content();
    let rendered = make_test_rendered();
    let document = make_test_document(&rendered);
    let ctx = make_test_ctx(&content, "notion", &config);

    let preview_path = adapter
        .build_preview(&content.meta.title, document, &ctx)
        .expect("generate preview");
    let preview_html = std::fs::read_to_string(&preview_path).expect("read preview HTML");

    insta::assert_snapshot!("notion_preview", preview_html);

    // Cleanup
    let _ = std::fs::remove_file(&preview_path);
}

#[test]
fn test_preview_astro() {
    let config = typub_config::Config::default();
    let adapter = typub_adapter_astro::AstroAdapter::new(&config).expect("create astro adapter");

    let content = make_test_content();
    let rendered = make_test_rendered();
    let document = make_test_document(&rendered);
    let ctx = make_test_ctx(&content, "astro", &config);

    let preview_path = adapter
        .build_preview(&content.meta.title, document, &ctx)
        .expect("generate preview");
    let preview_html = std::fs::read_to_string(&preview_path).expect("read preview HTML");

    insta::assert_snapshot!("astro_preview", preview_html);

    let _ = std::fs::remove_file(&preview_path);
}

#[test]
fn test_preview_confluence() {
    let config = typub_config::Config::default();
    let adapter = typub_adapter_confluence::create(&config).expect("create confluence adapter");

    let content = make_test_content();
    let rendered = make_test_rendered();
    let document = make_test_document(&rendered);
    let ctx = make_test_ctx(&content, "confluence", &config);

    let preview_path = adapter
        .build_preview(&content.meta.title, document, &ctx)
        .expect("generate preview");
    let preview_html = std::fs::read_to_string(&preview_path).expect("read preview HTML");

    insta::assert_snapshot!("confluence_preview", preview_html);

    let _ = std::fs::remove_file(&preview_path);
}

#[test]
fn test_preview_wechat() {
    ensure_english_locale();
    let config = typub_config::Config::default();
    let profile =
        typub_adapter_copypaste::find_profile("wechat").expect("wechat profile should exist");
    let adapter = typub_adapter_copypaste::CopyPasteAdapter::from_profile(profile, &config)
        .expect("create wechat adapter");

    let content = make_test_content();
    let rendered = make_test_rendered();
    let document = make_test_document(&rendered);
    let ctx = make_test_ctx(&content, "wechat", &config);

    let preview_path = adapter
        .build_preview(&content.meta.title, document, &ctx)
        .expect("generate preview");
    let preview_html = std::fs::read_to_string(&preview_path).expect("read preview HTML");

    insta::assert_snapshot!("wechat_preview", preview_html);

    let _ = std::fs::remove_file(&preview_path);
}

#[test]
fn test_preview_wordpress() {
    let config = typub_config::Config::default();
    let adapter = typub_adapter_wordpress::create(&config).expect("create wordpress adapter");

    let content = make_test_content();
    let rendered = make_test_rendered();
    let document = make_test_document(&rendered);
    let ctx = make_test_ctx(&content, "wordpress", &config);

    let preview_path = adapter
        .build_preview(&content.meta.title, document, &ctx)
        .expect("generate preview");
    let preview_html = std::fs::read_to_string(&preview_path).expect("read preview HTML");

    insta::assert_snapshot!("wordpress_preview", preview_html);

    let _ = std::fs::remove_file(&preview_path);
}

#[test]
fn test_preview_devto() {
    let config = typub_config::Config::default();
    let adapter = typub_adapter_devto::create(&config).expect("create devto adapter");

    let content = make_test_content();
    let rendered = make_test_rendered();
    let document = make_test_document(&rendered);
    let ctx = make_test_ctx(&content, "devto", &config);

    let preview_path = adapter
        .build_preview(&content.meta.title, document, &ctx)
        .expect("generate preview");
    let preview_html = std::fs::read_to_string(&preview_path).expect("read preview HTML");

    insta::assert_snapshot!("devto_preview", preview_html);

    let _ = std::fs::remove_file(&preview_path);
}

#[test]
fn test_preview_ghost() {
    let config = typub_config::Config::default();
    let adapter = typub_adapter_ghost::create(&config).expect("create ghost adapter");

    let content = make_test_content();
    let rendered = make_test_rendered();
    let document = make_test_document(&rendered);
    let ctx = make_test_ctx(&content, "ghost", &config);

    let preview_path = adapter
        .build_preview(&content.meta.title, document, &ctx)
        .expect("generate preview");
    let preview_html = std::fs::read_to_string(&preview_path).expect("read preview HTML");

    insta::assert_snapshot!("ghost_preview", preview_html);

    let _ = std::fs::remove_file(&preview_path);
}

/// One Markdown copy-paste profile to cover the format branch wechat doesn't.
#[test]
fn test_preview_csdn() {
    ensure_english_locale();
    let config = typub_config::Config::default();
    let profile = typub_adapter_copypaste::find_profile("csdn").expect("csdn profile should exist");
    let adapter = typub_adapter_copypaste::CopyPasteAdapter::from_profile(profile, &config)
        .expect("create csdn adapter");

    let content = make_test_content();
    let rendered = make_test_rendered();
    let document = make_test_document(&rendered);
    let ctx = make_test_ctx(&content, "csdn", &config);

    let preview_path = adapter
        .build_preview(&content.meta.title, document, &ctx)
        .expect("generate preview");
    let preview_html = std::fs::read_to_string(&preview_path).expect("read preview HTML");

    insta::assert_snapshot!("csdn_preview", preview_html);

    let _ = std::fs::remove_file(&preview_path);
}
