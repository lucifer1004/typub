use typub_ir::{Block, Inline, TableSectionKind, TextStyle};

use super::helpers::{block_text, parse};

#[test]
fn parse_table_align_and_span() {
    let html = r#"<html><body><table><tr><th colspan="2">H</th></tr><tr><td style="text-align:right" rowspan="2">C</td></tr></table></body></html>"#;
    let doc = parse(html);
    let Block::Table { sections, .. } = &doc.blocks[0] else {
        panic!("expected table")
    };
    assert!(!sections.is_empty());
    let head = sections
        .iter()
        .find(|s| matches!(s.kind, TableSectionKind::Head))
        .expect("head section");
    assert_eq!(head.rows[0].cells[0].colspan, 2);
    let body = sections
        .iter()
        .find(|s| matches!(s.kind, TableSectionKind::Body))
        .expect("body section");
    assert_eq!(body.rows[0].cells[0].rowspan, 2);
    assert!(
        body.rows[0].cells[0]
            .blocks
            .iter()
            .any(|b| block_text(b).contains("C"))
    );
}

#[test]
fn parse_table_with_explicit_sections_and_caption() {
    let html = r#"<html><body><table><caption>Cap</caption><thead><tr><th scope="col">H</th></tr></thead><tbody><tr><td style="text-align:center">C</td></tr></tbody><tfoot><tr><td>F</td></tr></tfoot></table></body></html>"#;
    let doc = parse(html);
    let Block::Table {
        caption, sections, ..
    } = &doc.blocks[0]
    else {
        panic!("expected table")
    };
    assert!(caption.is_some());
    assert!(
        sections
            .iter()
            .any(|s| matches!(s.kind, TableSectionKind::Head))
    );
    assert!(
        sections
            .iter()
            .any(|s| matches!(s.kind, TableSectionKind::Body))
    );
    assert!(
        sections
            .iter()
            .any(|s| matches!(s.kind, TableSectionKind::Foot))
    );
    assert_eq!(sections.len(), 3);
    let head = sections
        .iter()
        .find(|s| matches!(s.kind, TableSectionKind::Head))
        .expect("head");
    assert!(matches!(
        head.rows[0].cells[0].scope,
        Some(typub_ir::TableHeaderScope::Col)
    ));
    let body = sections
        .iter()
        .find(|s| matches!(s.kind, TableSectionKind::Body))
        .expect("body");
    assert_eq!(
        body.rows[0].cells[0].align,
        Some(typub_ir::TextAlign::Center)
    );
    assert!(
        body.rows[0].cells[0]
            .blocks
            .iter()
            .any(|b| block_text(b).contains("C"))
    );
}

#[test]
fn parse_table_alignment_all_variants() {
    let html = r#"<html><body><table><tr><td style="text-align:left">L</td><td style="text-align:center">C</td><td style="text-align:right">R</td></tr></table></body></html>"#;
    let doc = parse(html);
    let Block::Table { sections, .. } = &doc.blocks[0] else {
        panic!("expected table")
    };
    let body = sections
        .iter()
        .find(|s| matches!(s.kind, TableSectionKind::Body))
        .expect("body");
    assert_eq!(body.rows[0].cells[0].align, Some(typub_ir::TextAlign::Left));
    assert_eq!(
        body.rows[0].cells[1].align,
        Some(typub_ir::TextAlign::Center)
    );
    assert_eq!(
        body.rows[0].cells[2].align,
        Some(typub_ir::TextAlign::Right)
    );
}

#[test]
fn parse_table_cell_inline_formatting_preserved() {
    let html = r#"<html><body><table><tr><td><strong>Bold</strong> <em>Italic</em> <a href="https://example.com">Links</a></td></tr></table></body></html>"#;
    let doc = parse(html);
    let Block::Table { sections, .. } = &doc.blocks[0] else {
        panic!("expected table")
    };

    let body = sections
        .iter()
        .find(|s| matches!(s.kind, TableSectionKind::Body))
        .expect("body");
    let cell = &body.rows[0].cells[0];

    assert!(!cell.blocks.is_empty());
    assert!(
        !cell
            .blocks
            .iter()
            .any(|b| matches!(b, Block::UnknownBlock { .. }))
    );

    let Block::Paragraph { content, .. } = &cell.blocks[0] else {
        panic!("expected paragraph")
    };

    assert!(content.iter().any(|i| matches!(
        i,
        Inline::Styled { styles, .. } if styles.styles().contains(&TextStyle::Bold)
    )));
    assert!(content.iter().any(|i| matches!(
        i,
        Inline::Styled { styles, .. } if styles.styles().contains(&TextStyle::Italic)
    )));
    assert!(content.iter().any(|i| matches!(i, Inline::Link { .. })));
}
