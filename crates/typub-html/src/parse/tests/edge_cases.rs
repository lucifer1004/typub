use typub_ir::{Block, Inline, ListKind};

use super::helpers::{inline_text, parse};

#[test]
fn parse_regular_blockquote_preserves_cite() {
    let html =
        r#"<html><body><blockquote cite="https://example.com"><p>Q</p></blockquote></body></html>"#;
    let doc = parse(html);
    let Block::Quote { cite, .. } = &doc.blocks[0] else {
        panic!("expected quote")
    };
    assert!(matches!(cite, Some(url) if url.0 == "https://example.com"));
}

#[test]
fn parse_gfm_alert_with_cite_still_becomes_admonition() {
    let html = r#"<html><body><blockquote cite="https://example.com"><p>[!NOTE]</p><p>N</p></blockquote></body></html>"#;
    let doc = parse(html);
    assert!(matches!(doc.blocks[0], Block::Admonition { .. }));
    assert!(!matches!(doc.blocks[0], Block::Quote { .. }));
}

#[test]
fn parse_img_without_src_as_unknown_block() {
    let html = r#"<html><body><img alt="x"></body></html>"#;
    let doc = parse(html);
    assert!(matches!(
        doc.blocks[0],
        Block::UnknownBlock { ref tag, .. } if tag == "img"
    ));
}

#[test]
fn parse_unknown_inline_element_as_unknown_inline() {
    let html = r#"<html><body><p>a<custom-inline x="1">b</custom-inline>c</p></body></html>"#;
    let doc = parse(html);
    let Block::Paragraph { content, .. } = &doc.blocks[0] else {
        panic!("expected paragraph")
    };
    assert!(content.iter().any(|i| matches!(
        i,
        Inline::UnknownInline { tag, .. } if tag == "custom-inline"
    )));
    let text = inline_text(content);
    assert!(text.contains('a'));
    assert!(text.contains('b'));
    assert!(text.contains('c'));
}

#[test]
fn parse_ul_without_task_markers_stays_bullet() {
    let html = r#"<html><body><ul><li>a</li><li>b</li></ul></body></html>"#;
    let doc = parse(html);
    let Block::List { list, .. } = &doc.blocks[0] else {
        panic!("expected list")
    };
    assert!(matches!(list.kind, ListKind::Bullet { .. }));
}

#[test]
fn parse_ol_without_start_uses_default_one() {
    let html = r#"<html><body><ol><li>a</li></ol></body></html>"#;
    let doc = parse(html);
    let Block::List { list, .. } = &doc.blocks[0] else {
        panic!("expected list")
    };
    let ListKind::Numbered { start, .. } = list.kind else {
        panic!("expected numbered list")
    };
    assert_eq!(start, 1);
}

#[test]
fn parse_table_invalid_scope_is_none() {
    let html = r#"<html><body><table><tr><th scope="unknown">H</th></tr></table></body></html>"#;
    let doc = parse(html);
    let Block::Table { sections, .. } = &doc.blocks[0] else {
        panic!("expected table")
    };
    let head = sections
        .iter()
        .find(|s| matches!(s.kind, typub_ir::TableSectionKind::Head))
        .expect("head section");
    assert!(head.rows[0].cells[0].scope.is_none());
}

#[test]
fn parse_whitespace_is_normalized_inside_text_nodes() {
    let html = r#"<html><body><p> a
        b   c </p></body></html>"#;
    let doc = parse(html);
    let Block::Paragraph { content, .. } = &doc.blocks[0] else {
        panic!("expected paragraph")
    };
    let text = content
        .iter()
        .filter_map(|i| match i {
            Inline::Text(t) => Some(t.as_str()),
            _ => None,
        })
        .collect::<String>();
    assert!(text.contains("a b c"));
}

#[test]
fn parse_block_id_is_preserved_in_passthrough_for_non_heading() {
    let html = r#"<html><body><p id="p1">x</p></body></html>"#;
    let doc = parse(html);
    let Block::Paragraph { attrs, .. } = &doc.blocks[0] else {
        panic!("expected paragraph")
    };
    assert_eq!(attrs.passthrough.get("id").map(String::as_str), Some("p1"));
}

#[test]
fn parse_footnote_section_into_document_footnotes() {
    let html = r##"<html><body><p>a<sup><a href="#fn-1" id="fnref-1">[1]</a></sup></p><section class="footnotes"><ol><li id="fn-1"><p>note</p></li></ol></section></body></html>"##;
    let doc = parse(html);
    let Block::Paragraph { content, .. } = &doc.blocks[0] else {
        panic!("expected paragraph")
    };
    assert!(
        content
            .iter()
            .any(|inline| matches!(inline, Inline::FootnoteRef(id) if id.0 == 1))
    );
    assert_eq!(doc.footnotes.len(), 1);
    let def = doc
        .footnotes
        .get(&typub_ir::FootnoteId(1))
        .expect("footnote 1");
    assert!(!def.blocks.is_empty());
}

#[test]
fn parse_nested_footnote_container_into_document_footnotes() {
    let html = r##"<html><body><div><section class="footnotes"><ol><li id="fn-2"><p>nested</p></li></ol></section></div></body></html>"##;
    let doc = parse(html);
    assert_eq!(doc.footnotes.len(), 1);
    let def = doc
        .footnotes
        .get(&typub_ir::FootnoteId(2))
        .expect("footnote 2");
    assert!(!def.blocks.is_empty());
    assert!(doc.blocks.is_empty());
}

#[test]
fn parse_footnote_section_without_valid_ids_is_not_swallowed() {
    let html =
        r#"<html><body><section class="footnotes"><p>plain content</p></section></body></html>"#;
    let doc = parse(html);
    assert!(doc.footnotes.is_empty());
    assert_eq!(doc.blocks.len(), 1);
    let Block::Paragraph { content, .. } = &doc.blocks[0] else {
        panic!("expected paragraph")
    };
    assert!(inline_text(content).contains("plain content"));
}

#[test]
fn parse_doc_endnotes_section_into_document_footnotes() {
    let html = r##"<html><body><p>a<a id="loc-1" href="#loc-2" role="doc-noteref"><sup>1</sup></a>b</p><section role="doc-endnotes"><ol style="list-style-type: none"><li id="loc-2"><p><a href="#loc-1" role="doc-backlink"><sup>1</sup></a>note</p></li></ol></section></body></html>"##;
    let doc = parse(html);

    let Block::Paragraph { content, .. } = &doc.blocks[0] else {
        panic!("expected paragraph")
    };
    assert!(
        content
            .iter()
            .any(|inline| matches!(inline, Inline::FootnoteRef(id) if id.0 == 1))
    );

    assert_eq!(doc.footnotes.len(), 1);
    let def = doc
        .footnotes
        .get(&typub_ir::FootnoteId(1))
        .expect("footnote 1");
    assert!(!def.blocks.is_empty());
    assert!(!footnote_blocks_contain_doc_backlink(&def.blocks));
}

fn footnote_blocks_contain_doc_backlink(blocks: &[Block]) -> bool {
    blocks.iter().any(block_contains_doc_backlink)
}

fn block_contains_doc_backlink(block: &Block) -> bool {
    match block {
        Block::Heading { content, .. } | Block::Paragraph { content, .. } => {
            content.iter().any(inline_contains_doc_backlink)
        }
        Block::Quote { blocks, .. }
        | Block::Admonition { blocks, .. }
        | Block::Details { blocks, .. } => blocks.iter().any(block_contains_doc_backlink),
        Block::Figure {
            content, caption, ..
        } => {
            content.iter().any(block_contains_doc_backlink)
                || caption
                    .as_ref()
                    .is_some_and(|blocks| blocks.iter().any(block_contains_doc_backlink))
        }
        Block::List { list, .. } => match &list.kind {
            typub_ir::ListKind::Bullet { items } | typub_ir::ListKind::Numbered { items, .. } => {
                items
                    .iter()
                    .flat_map(|item| item.blocks.iter())
                    .any(block_contains_doc_backlink)
            }
            typub_ir::ListKind::Task { items } => items
                .iter()
                .flat_map(|item| item.blocks.iter())
                .any(block_contains_doc_backlink),
            typub_ir::ListKind::Custom { items, .. } => items
                .iter()
                .flat_map(|item| item.blocks.iter())
                .any(block_contains_doc_backlink),
        },
        Block::DefinitionList { items, .. } => items
            .iter()
            .flat_map(|item| item.terms.iter().chain(item.definitions.iter()))
            .flat_map(|group| group.iter())
            .any(block_contains_doc_backlink),
        Block::Table { sections, .. } => sections
            .iter()
            .flat_map(|section| section.rows.iter())
            .flat_map(|row| row.cells.iter())
            .flat_map(|cell| cell.blocks.iter())
            .any(block_contains_doc_backlink),
        Block::UnknownBlock { children, .. } => children.iter().any(|child| match child {
            typub_ir::UnknownChild::Block(block) => block_contains_doc_backlink(block),
            typub_ir::UnknownChild::Inline(inline) => inline_contains_doc_backlink(inline),
        }),
        Block::CodeBlock { .. }
        | Block::Divider { .. }
        | Block::MathBlock { .. }
        | Block::SvgBlock { .. }
        | Block::RawBlock { .. } => false,
    }
}

fn inline_contains_doc_backlink(inline: &Inline) -> bool {
    match inline {
        Inline::Link { attrs, .. } => attrs
            .passthrough
            .get("role")
            .is_some_and(|role| role == "doc-backlink"),
        Inline::Styled { content, .. } | Inline::UnknownInline { content, .. } => {
            content.iter().any(inline_contains_doc_backlink)
        }
        Inline::Text(_)
        | Inline::Code(_)
        | Inline::SoftBreak
        | Inline::HardBreak
        | Inline::Image { .. }
        | Inline::FootnoteRef(_)
        | Inline::MathInline { .. }
        | Inline::SvgInline { .. }
        | Inline::RawInline { .. } => false,
    }
}
