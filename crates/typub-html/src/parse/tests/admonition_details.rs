use typub_ir::{AdmonitionKind, Block, Inline};

use super::helpers::{block_text, inline_text, parse};

#[test]
fn parse_gfm_note_alert() {
    let html = r#"<html><body><blockquote><p>[!NOTE]</p><p>This is a note alert.</p></blockquote></body></html>"#;
    let doc = parse(html);
    let Block::Admonition {
        kind,
        title,
        blocks,
        ..
    } = &doc.blocks[0]
    else {
        panic!("expected admonition")
    };
    assert!(matches!(kind, AdmonitionKind::Note));
    assert_eq!(blocks.len(), 1);
    assert_eq!(inline_text(title.as_ref().expect("title")).trim(), "Note");
    assert!(block_text(&blocks[0]).contains("note alert"));
    assert!(!block_text(&blocks[0]).contains("[!NOTE]"));
}

#[test]
fn parse_gfm_alert_same_line_title() {
    let html = r#"<html><body><blockquote><p>[!WARNING] Watch config edits.</p></blockquote></body></html>"#;
    let doc = parse(html);
    let Block::Admonition {
        kind,
        title,
        blocks,
        ..
    } = &doc.blocks[0]
    else {
        panic!("expected admonition")
    };
    let title_text = title.as_ref().expect("title");
    assert!(matches!(title_text[0], Inline::Text(ref t) if t.contains("Watch config edits.")));
    assert!(blocks.is_empty());
    assert!(matches!(kind, AdmonitionKind::Warning));
}

#[test]
fn parse_details_with_summary() {
    let html = r#"<html><body><details open><summary>Click</summary><p>Hidden content</p></details></body></html>"#;
    let doc = parse(html);
    let Block::Details {
        summary,
        blocks,
        open,
        ..
    } = &doc.blocks[0]
    else {
        panic!("expected details")
    };
    assert!(*open);
    assert!(summary.is_some());
    assert_eq!(blocks.len(), 1);
    assert_eq!(
        inline_text(summary.as_ref().expect("summary")).trim(),
        "Click"
    );
    assert!(block_text(&blocks[0]).contains("Hidden content"));
}

#[test]
fn parse_regular_blockquote_not_converted() {
    let html = r#"<html><body><blockquote><p>Regular quote</p></blockquote></body></html>"#;
    let doc = parse(html);
    let Block::Quote { blocks, .. } = &doc.blocks[0] else {
        panic!("expected quote")
    };
    assert_eq!(blocks.len(), 1);
    assert!(block_text(&blocks[0]).contains("Regular quote"));
}

#[test]
fn parse_gfm_all_alert_types() {
    let cases = [
        ("[!NOTE]", AdmonitionKind::Note),
        ("[!TIP]", AdmonitionKind::Tip),
        ("[!IMPORTANT]", AdmonitionKind::Info),
        ("[!WARNING]", AdmonitionKind::Warning),
        ("[!CAUTION]", AdmonitionKind::Danger),
    ];
    for (marker, expected) in cases {
        let html = format!(
            r#"<html><body><blockquote><p>{}</p><p>X</p></blockquote></body></html>"#,
            marker
        );
        let doc = parse(&html);
        let Block::Admonition { kind, .. } = &doc.blocks[0] else {
            panic!("expected admonition")
        };
        assert_eq!(*kind, expected);
    }
}

#[test]
fn parse_admonition_wrapper_class() {
    let html = r#"<html><body><div class="admonition warning"><p class="admonition-title">Heads up</p><p>Body</p></div></body></html>"#;
    let doc = parse(html);
    let Block::Admonition {
        kind,
        title,
        blocks,
        ..
    } = &doc.blocks[0]
    else {
        panic!("expected admonition")
    };
    assert!(matches!(kind, AdmonitionKind::Warning));
    assert_eq!(
        inline_text(title.as_ref().expect("title")).trim(),
        "Heads up"
    );
    assert_eq!(blocks.len(), 1);
    assert!(block_text(&blocks[0]).contains("Body"));
}

#[test]
fn parse_details_without_summary_and_closed() {
    let html = r#"<html><body><details><p>Only body</p></details></body></html>"#;
    let doc = parse(html);
    let Block::Details {
        summary,
        open,
        blocks,
        ..
    } = &doc.blocks[0]
    else {
        panic!("expected details")
    };
    assert!(summary.is_none());
    assert!(!open);
    assert_eq!(blocks.len(), 1);
    assert!(matches!(blocks[0], Block::Paragraph { .. }));
}

#[test]
fn parse_details_with_nested_content() {
    let html = r#"<html><body><details><summary>S</summary><p>P</p><ul><li>I</li></ul><blockquote><p>Q</p></blockquote></details></body></html>"#;
    let doc = parse(html);
    let Block::Details { blocks, .. } = &doc.blocks[0] else {
        panic!("expected details")
    };
    assert_eq!(blocks.len(), 3);
    assert!(matches!(blocks[0], Block::Paragraph { .. }));
    assert!(matches!(blocks[1], Block::List { .. }));
    assert!(matches!(blocks[2], Block::Quote { .. }));
}
