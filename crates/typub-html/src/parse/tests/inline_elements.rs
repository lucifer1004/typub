use typub_ir::{Block, Inline, TextStyle};

use super::helpers::parse;

#[test]
fn parse_inline_styles_full_set() {
    let html = r#"<html><body><p><strong>b</strong><em>i</em><del>d</del><u>u</u><mark>m</mark><sup>s</sup><sub>sb</sub><kbd>k</kbd></p></body></html>"#;
    let doc = parse(html);
    let Block::Paragraph { content, .. } = &doc.blocks[0] else {
        panic!("expected paragraph")
    };

    let expected = [
        TextStyle::Bold,
        TextStyle::Italic,
        TextStyle::Strikethrough,
        TextStyle::Underline,
        TextStyle::Mark,
        TextStyle::Superscript,
        TextStyle::Subscript,
        TextStyle::Kbd,
    ];
    for style in expected {
        assert!(content.iter().any(|i| {
            matches!(i, Inline::Styled { styles, .. } if styles.styles().contains(&style))
        }));
    }
}

#[test]
fn parse_line_break_and_link() {
    let html =
        r#"<html><body><p>A<br><a href="https://example.com" title="t">B</a></p></body></html>"#;
    let doc = parse(html);
    let Block::Paragraph { content, .. } = &doc.blocks[0] else {
        panic!("expected paragraph")
    };
    assert!(content.iter().any(|i| matches!(i, Inline::HardBreak)));
    assert!(content.iter().any(|i| matches!(
        i,
        Inline::Link { href, title, .. } if href.0 == "https://example.com" && title.as_deref() == Some("t")
    )));
}

#[test]
fn parse_anchor_without_href_falls_back_to_content() {
    let html = r#"<html><body><p><a>plain</a></p></body></html>"#;
    let doc = parse(html);
    let Block::Paragraph { content, .. } = &doc.blocks[0] else {
        panic!("expected paragraph")
    };
    assert!(matches!(content[0], Inline::Text(ref t) if t.trim() == "plain"));
}

#[test]
fn parse_inline_image_missing_src_as_unknown_inline() {
    let html = r#"<html><body><p><img alt="x"></p></body></html>"#;
    let doc = parse(html);
    let Block::Paragraph { content, .. } = &doc.blocks[0] else {
        panic!("expected paragraph")
    };
    assert!(matches!(
        content[0],
        Inline::UnknownInline { ref tag, .. } if tag == "img"
    ));
}

#[test]
fn parse_sup_without_footnote_href_stays_superscript_style() {
    let html = r##"<html><body><p>a<sup><a href="#other">x</a></sup>b</p></body></html>"##;
    let doc = parse(html);
    let Block::Paragraph { content, .. } = &doc.blocks[0] else {
        panic!("expected paragraph")
    };
    assert!(content.iter().any(|inline| matches!(
        inline,
        Inline::Styled { styles, .. } if styles.styles().contains(&TextStyle::Superscript)
    )));
    assert!(
        !content
            .iter()
            .any(|inline| matches!(inline, Inline::FootnoteRef(_)))
    );
}

#[test]
fn parse_sup_with_mixed_content_does_not_become_footnote_ref() {
    let html = r##"<html><body><p>a<sup><a href="#fn-1">1</a>x</sup>b</p></body></html>"##;
    let doc = parse(html);
    let Block::Paragraph { content, .. } = &doc.blocks[0] else {
        panic!("expected paragraph")
    };
    assert!(content.iter().any(|inline| matches!(
        inline,
        Inline::Styled { styles, .. } if styles.styles().contains(&TextStyle::Superscript)
    )));
    assert!(
        !content
            .iter()
            .any(|inline| matches!(inline, Inline::FootnoteRef(_)))
    );
}

#[test]
fn parse_anchor_doc_noteref_becomes_footnote_ref() {
    let html = r##"<html><body><p>a<a id="loc-1" href="#loc-2" role="doc-noteref"><sup>1</sup></a>b</p></body></html>"##;
    let doc = parse(html);
    let Block::Paragraph { content, .. } = &doc.blocks[0] else {
        panic!("expected paragraph")
    };
    assert!(
        content
            .iter()
            .any(|inline| matches!(inline, Inline::FootnoteRef(id) if id.0 == "1"))
    );
}
