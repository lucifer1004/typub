use typub_ir::{Block, Inline};

use super::helpers::{block_text, parse};

#[test]
fn parse_code_block_highlighted_html() {
    let html = r#"<html><body><pre><code data-lang="rust"><span style="color:red">fn</span> main()</code></pre></body></html>"#;
    let doc = parse(html);
    let Block::CodeBlock {
        highlighted_html,
        language,
        ..
    } = &doc.blocks[0]
    else {
        panic!("expected code block")
    };
    assert_eq!(language.as_deref(), Some("rust"));
    let highlighted = highlighted_html.as_ref().expect("highlighted html");
    assert!(highlighted.contains("<span>"));
    assert!(highlighted.contains('\u{00A0}'));
}

#[test]
fn parse_code_block_preserves_line_breaks_from_br() {
    let html = r#"<html><body><pre><code data-lang="rust"><span>fn</span> main() {<br>    println!("hi");<br>}<br></code></pre></body></html>"#;
    let doc = parse(html);
    let Block::CodeBlock { code, .. } = &doc.blocks[0] else {
        panic!("expected code block")
    };
    assert_eq!(code, "fn main() {\n    println!(\"hi\");\n}");
}

#[test]
fn parse_code_block_preserves_literal_newlines() {
    let html = "<html><body><pre><code>line1\nline2\n</code></pre></body></html>";
    let doc = parse(html);
    let Block::CodeBlock { code, .. } = &doc.blocks[0] else {
        panic!("expected code block")
    };
    assert_eq!(code, "line1\nline2");
}

#[test]
fn parse_code_block_keeps_literal_br_text() {
    let html =
        "<html><body><pre><code>&lt;br&gt;\n&lt;div&gt;x&lt;/div&gt;\n</code></pre></body></html>";
    let doc = parse(html);
    let Block::CodeBlock { code, .. } = &doc.blocks[0] else {
        panic!("expected code block")
    };
    assert_eq!(code, "<br>\n<div>x</div>");
}

#[test]
fn parse_legacy_image_marker_code_remains_inline_code() {
    let html = r#"<html><body><p><code>[[IMG:assets/photo.jpg]]</code></p></body></html>"#;
    let doc = parse(html);
    assert_eq!(doc.assets.len(), 0);
    let Block::Paragraph { content, .. } = &doc.blocks[0] else {
        panic!("expected paragraph")
    };
    assert!(
        matches!(content.as_slice(), [Inline::Code(code)] if code == "[[IMG:assets/photo.jpg]]")
    );
}

#[test]
fn parse_registers_assets() {
    let html = r#"<html><body><p><img src="assets/a.png" alt="a"></p><img src="https://example.com/b.png"></body></html>"#;
    let doc = parse(html);
    assert_eq!(doc.assets.len(), 2);
    let sources = doc
        .assets
        .values()
        .filter_map(|a| match a {
            typub_ir::Asset::Image(i) => Some(&i.source),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        sources
            .iter()
            .any(|s| matches!(s, typub_ir::AssetSource::LocalPath { .. }))
    );
    assert!(
        sources
            .iter()
            .any(|s| matches!(s, typub_ir::AssetSource::RemoteUrl { .. }))
    );
    assert!(
        !doc.blocks
            .iter()
            .any(|b| matches!(b, Block::UnknownBlock { .. }))
    );
}

#[test]
fn parse_deduplicates_same_asset_source() {
    let html = r#"<html><body><p><img src="assets/a.png"></p><p><img src="assets/a.png"></p></body></html>"#;
    let doc = parse(html);
    assert_eq!(doc.assets.len(), 1);
}

#[test]
fn parse_img_src_legacy_marker_is_unknown_block() {
    let html = r#"<html><body><img src="[[IMG:assets/photo.jpg]]"></body></html>"#;
    let doc = parse(html);
    assert_eq!(doc.assets.len(), 0);
    assert!(matches!(doc.blocks[0], Block::UnknownBlock { .. }));
}

#[test]
fn parse_plain_standalone_code_preserved_as_inline_code_paragraph() {
    let html = r#"<html><body><code>plain</code><p>x</p></body></html>"#;
    let doc = parse(html);
    assert_eq!(doc.blocks.len(), 2);
    let Block::Paragraph { content, .. } = &doc.blocks[0] else {
        panic!("expected first block paragraph")
    };
    assert!(matches!(content.as_slice(), [Inline::Code(code)] if code == "plain"));
    assert!(block_text(&doc.blocks[1]).contains("x"));
}

#[test]
fn parse_image_passthrough_attrs_are_preserved() {
    let html = r#"<html><body><p><img src="assets/a.png" alt="a" loading="lazy" data-x="1"></p></body></html>"#;
    let doc = parse(html);
    let Block::Paragraph { content, .. } = &doc.blocks[0] else {
        panic!("expected paragraph")
    };
    let Inline::Image { attrs, .. } = &content[0] else {
        panic!("expected inline image")
    };
    assert_eq!(
        attrs.passthrough.get("loading").map(String::as_str),
        Some("lazy")
    );
    assert_eq!(
        attrs.passthrough.get("data-x").map(String::as_str),
        Some("1")
    );
}
