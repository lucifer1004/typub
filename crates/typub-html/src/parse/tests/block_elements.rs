use typub_ir::{Block, Inline};

use super::helpers::{block_text, parse};

#[test]
fn parse_basic_document() {
    let html =
        r#"<html><body><h1 id="t">Title</h1><p>Hello <strong>world</strong></p></body></html>"#;
    let doc = parse(html);
    assert_eq!(doc.blocks.len(), 2);
    let Block::Heading {
        level, id, content, ..
    } = &doc.blocks[0]
    else {
        panic!("expected heading")
    };
    assert_eq!(level.get(), 1);
    assert_eq!(id.as_ref().map(|a| a.0.as_str()), Some("t"));
    assert_eq!(block_text(&doc.blocks[0]).trim(), "Title");
    assert!(matches!(doc.blocks[1], Block::Paragraph { .. }));
    assert!(matches!(content[0], Inline::Text(ref t) if t.trim() == "Title"));
    assert!(block_text(&doc.blocks[1]).contains("Hello world"));
    assert!(
        !doc.blocks
            .iter()
            .any(|b| matches!(b, Block::UnknownBlock { .. }))
    );
}

#[test]
fn parse_unknown_keeps_unknown_block() {
    let html = r#"<html><body><custom-box x-kind="x:y">z</custom-box></body></html>"#;
    let doc = parse(html);
    let Block::UnknownBlock {
        tag, note, source, ..
    } = &doc.blocks[0]
    else {
        panic!("expected unknown block")
    };
    assert_eq!(tag, "custom-box");
    assert!(note.as_deref().unwrap_or_default().contains("unsupported"));
    assert!(source.as_deref().unwrap_or_default().contains("custom-box"));
}

#[test]
fn parse_no_body_tag() {
    let html = r#"<h1>Hello</h1><p>World</p>"#;
    let doc = parse(html);
    assert_eq!(doc.blocks.len(), 2);
    assert!(matches!(doc.blocks[0], Block::Heading { .. }));
    assert!(matches!(doc.blocks[1], Block::Paragraph { .. }));
    assert!(
        !doc.blocks
            .iter()
            .any(|b| matches!(b, Block::UnknownBlock { .. }))
    );
}

#[test]
fn parse_nested_div_containers() {
    let html = r#"<html><body><div><section><article><p>nested</p></article></section></div></body></html>"#;
    let doc = parse(html);
    assert_eq!(doc.blocks.len(), 1);
    assert!(matches!(doc.blocks[0], Block::Paragraph { .. }));
}

#[test]
fn parse_container_text_node_preserved_as_paragraph() {
    let html = r#"<html><body><div>prefix <p>x</p> suffix</div></body></html>"#;
    let doc = parse(html);
    assert_eq!(doc.blocks.len(), 3);
    assert!(block_text(&doc.blocks[0]).contains("prefix"));
    assert!(block_text(&doc.blocks[1]).contains("x"));
    assert!(block_text(&doc.blocks[2]).contains("suffix"));
}

#[test]
fn parse_root_text_node_preserved_as_paragraph() {
    let html = r#"hello <p>world</p>"#;
    let doc = parse(html);
    assert_eq!(doc.blocks.len(), 2);
    assert!(matches!(&doc.blocks[0], Block::Paragraph { .. }));
    assert!(block_text(&doc.blocks[0]).contains("hello"));
}

#[test]
fn parse_figure_with_caption() {
    let html = r#"<html><body><figure><img src="assets/f.png" alt="f"><figcaption>Cap</figcaption></figure></body></html>"#;
    let doc = parse(html);
    let Block::Figure {
        content, caption, ..
    } = &doc.blocks[0]
    else {
        panic!("expected figure")
    };
    assert_eq!(content.len(), 1);
    let cap = caption.as_ref().expect("caption");
    assert!(block_text(&cap[0]).contains("Cap"));
    assert!(block_text(&content[0]).contains("f"));
}

#[test]
fn parse_figure_with_image_width_not_on_wrapper_paragraph() {
    // Bug: width was being applied to both the wrapper <p> and the <img>,
    // leading to incorrect sizing.
    use typub_ir::Inline;
    let html = r#"<html><body><figure><img src="assets/f.png" alt="f" width="300"></figure></body></html>"#;
    let doc = parse(html);
    let Block::Figure { content, .. } = &doc.blocks[0] else {
        panic!("expected figure")
    };
    // The content should be a Paragraph wrapping an Image
    let Block::Paragraph {
        content: inline_content,
        attrs,
    } = &content[0]
    else {
        panic!("expected paragraph")
    };
    // The paragraph should NOT have width in its passthrough attrs
    assert!(
        !attrs.passthrough.contains_key("width"),
        "width should not be on wrapper paragraph passthrough"
    );
    // The image should have width
    let Inline::Image {
        attrs: img_attrs, ..
    } = &inline_content[0]
    else {
        panic!("expected image")
    };
    assert_eq!(img_attrs.width, Some(300), "width should be on image");
}
