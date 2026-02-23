use typub_ir::{Block, ListKind, TableSectionKind};

use super::helpers::parse;

#[test]
fn parse_table_cell_with_nested_list_and_details() {
    let html = r#"<html><body><table><tr><td><ul><li>A</li></ul><details><summary>S</summary><p>D</p></details></td></tr></table></body></html>"#;
    let doc = parse(html);
    let Block::Table { sections, .. } = &doc.blocks[0] else {
        panic!("expected table")
    };
    let body = sections
        .iter()
        .find(|s| matches!(s.kind, TableSectionKind::Body))
        .expect("body");
    let cell_blocks = &body.rows[0].cells[0].blocks;
    assert!(cell_blocks.iter().any(|b| matches!(b, Block::List { .. })));
    assert!(
        cell_blocks
            .iter()
            .any(|b| matches!(b, Block::Details { .. }))
    );
}

#[test]
fn parse_admonition_wrapper_with_nested_blocks() {
    let html = r#"<html><body><div class="admonition note"><p class="admonition-title">T</p><ul><li>I</li></ul><table><tr><td>C</td></tr></table></div></body></html>"#;
    let doc = parse(html);
    let Block::Admonition { blocks, .. } = &doc.blocks[0] else {
        panic!("expected admonition")
    };
    assert!(blocks.iter().any(|b| matches!(b, Block::List { .. })));
    assert!(blocks.iter().any(|b| matches!(b, Block::Table { .. })));
}

#[test]
fn parse_figure_with_mixed_content() {
    let html = r#"<html><body><figure><pre><code data-lang="rs">fn main(){}</code></pre><div class="typst-svg-block" data-latex-src="x"><svg>...</svg></div><figcaption><p>Cap</p></figcaption></figure></body></html>"#;
    let doc = parse(html);
    let Block::Figure {
        content, caption, ..
    } = &doc.blocks[0]
    else {
        panic!("expected figure")
    };
    assert!(content.iter().any(|b| matches!(b, Block::CodeBlock { .. })));
    assert!(content.iter().any(|b| matches!(b, Block::MathBlock { .. })));
    assert!(caption.is_some());
}

#[test]
fn parse_nested_container_mix() {
    let html = r#"<html><body><div><section><blockquote><p>Q</p></blockquote><details><summary>S</summary><ul><li>X</li></ul></details></section></div></body></html>"#;
    let doc = parse(html);
    assert_eq!(doc.blocks.len(), 2);
    assert!(matches!(doc.blocks[0], Block::Quote { .. }));
    assert!(matches!(doc.blocks[1], Block::Details { .. }));
}

#[test]
fn parse_task_list_text_markers() {
    let html = r#"<html><body><ul><li>[x] done</li><li>[ ] todo</li><li>☑ done2</li><li>☐ todo2</li></ul></body></html>"#;
    let doc = parse(html);
    let Block::List { list, .. } = &doc.blocks[0] else {
        panic!("expected list")
    };
    let ListKind::Task { items } = &list.kind else {
        panic!("expected task list")
    };
    assert_eq!(items.len(), 4);
    assert!(items[0].checked);
    assert!(!items[1].checked);
    assert!(items[2].checked);
    assert!(!items[3].checked);
}

#[test]
fn parse_numbered_list_nested_numbered_child() {
    let html = r#"<html><body><ol><li>Top<ol><li>Sub</li></ol></li></ol></body></html>"#;
    let doc = parse(html);
    let Block::List { list, .. } = &doc.blocks[0] else {
        panic!("expected list")
    };
    let ListKind::Numbered { items, .. } = &list.kind else {
        panic!("expected numbered list")
    };
    assert_eq!(items.len(), 1);
    let nested = items[0]
        .blocks
        .iter()
        .find(|b| matches!(b, Block::List { .. }))
        .expect("nested list");
    let Block::List {
        list: nested_list, ..
    } = nested
    else {
        panic!("expected nested list")
    };
    assert!(matches!(nested_list.kind, ListKind::Numbered { .. }));
}
