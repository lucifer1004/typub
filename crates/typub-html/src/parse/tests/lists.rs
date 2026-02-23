use typub_ir::{Block, ListKind};

use super::helpers::{block_text, parse};

#[test]
fn parse_task_list() {
    let html = r#"<html><body><ul><li><input type="checkbox" checked> Done</li><li><input type="checkbox"> Todo</li></ul></body></html>"#;
    let doc = parse(html);
    let Block::List { list, .. } = &doc.blocks[0] else {
        panic!("expected list")
    };
    let ListKind::Task { items } = &list.kind else {
        panic!("expected task list")
    };
    assert_eq!(items.len(), 2);
    assert!(items[0].checked);
    assert!(!items[1].checked);
    assert!(
        items[0]
            .blocks
            .iter()
            .any(|b| block_text(b).contains("Done"))
    );
    assert!(
        items[1]
            .blocks
            .iter()
            .any(|b| block_text(b).contains("Todo"))
    );
    assert!(!matches!(list.kind, ListKind::Bullet { .. }));
}

#[test]
fn parse_nested_list_no_duplication() {
    let html = r#"<html><body><ul><li>Item 1<ul><li>Nested 1</li><li>Nested 2</li></ul></li><li>Item 2</li></ul></body></html>"#;
    let doc = parse(html);
    let Block::List { list, .. } = &doc.blocks[0] else {
        panic!("expected list")
    };
    let ListKind::Bullet { items } = &list.kind else {
        panic!("expected bullet list")
    };
    assert_eq!(items.len(), 2);
    assert!(
        items[0]
            .blocks
            .iter()
            .any(|b| matches!(b, Block::List { .. }))
    );
    assert!(
        items[0]
            .blocks
            .iter()
            .any(|b| block_text(b).contains("Item 1"))
    );
    assert!(
        items[1]
            .blocks
            .iter()
            .any(|b| block_text(b).contains("Item 2"))
    );
}

#[test]
fn parse_definition_list() {
    let html = r#"<html><body><dl><dt>Term</dt><dd>Definition</dd></dl></body></html>"#;
    let doc = parse(html);
    let Block::DefinitionList { items, .. } = &doc.blocks[0] else {
        panic!("expected definition list")
    };
    assert_eq!(items.len(), 1);
    assert!(
        items[0]
            .terms
            .iter()
            .flatten()
            .any(|b| block_text(b).contains("Term"))
    );
    assert!(
        items[0]
            .definitions
            .iter()
            .flatten()
            .any(|b| block_text(b).contains("Definition"))
    );
}

#[test]
fn parse_numbered_list_start_reversed_and_marker() {
    let html = r#"<html><body><ol start="5" reversed type="A"><li>one</li><li value="9">two</li></ol></body></html>"#;
    let doc = parse(html);
    let Block::List { list, .. } = &doc.blocks[0] else {
        panic!("expected list")
    };
    let ListKind::Numbered {
        start,
        reversed,
        marker,
        items,
    } = &list.kind
    else {
        panic!("expected numbered list")
    };
    assert_eq!(*start, 5);
    assert!(*reversed);
    assert!(matches!(
        marker,
        Some(typub_ir::OrderedListMarker::UpperAlpha)
    ));
    assert!(matches!(
        items[1].marker,
        Some(typub_ir::FlowListItemMarker::Number(9))
    ));
    assert!(
        items[0]
            .blocks
            .iter()
            .any(|b| block_text(b).contains("one"))
    );
    assert!(
        items[1]
            .blocks
            .iter()
            .any(|b| block_text(b).contains("two"))
    );
    assert!(!items.is_empty());
}

#[test]
fn parse_nested_task_list_no_duplication() {
    let html = r#"<html><body><ul><li><input type="checkbox" checked> Task 1<ul><li><input type="checkbox"> Sub 1</li></ul></li><li><input type="checkbox"> Task 2</li></ul></body></html>"#;
    let doc = parse(html);
    let Block::List { list, .. } = &doc.blocks[0] else {
        panic!("expected list")
    };
    let ListKind::Task { items } = &list.kind else {
        panic!("expected task list")
    };
    assert_eq!(items.len(), 2);
    assert!(items[0].checked);
    assert!(!items[1].checked);
    assert!(
        items[0]
            .blocks
            .iter()
            .any(|b| matches!(b, Block::List { .. }))
    );
}

#[test]
fn parse_deeply_nested_bullet_list() {
    let html =
        r#"<html><body><ul><li>L1<ul><li>L2<ul><li>L3</li></ul></li></ul></li></ul></body></html>"#;
    let doc = parse(html);
    let Block::List { list, .. } = &doc.blocks[0] else {
        panic!("expected list")
    };
    let ListKind::Bullet { items } = &list.kind else {
        panic!("expected bullet list")
    };
    assert_eq!(items.len(), 1);
    let lvl2 = items[0]
        .blocks
        .iter()
        .find(|b| matches!(b, Block::List { .. }))
        .expect("level2 list");
    let Block::List { list: l2, .. } = lvl2 else {
        panic!("expected list")
    };
    let ListKind::Bullet { items: l2_items } = &l2.kind else {
        panic!("expected bullet list")
    };
    assert_eq!(l2_items.len(), 1);
}

#[test]
fn parse_definition_list_multiple_pairs() {
    let html = r#"<html><body><dl><dt>T1</dt><dd>D1</dd><dt>T2</dt><dd>D2</dd></dl></body></html>"#;
    let doc = parse(html);
    let Block::DefinitionList { items, .. } = &doc.blocks[0] else {
        panic!("expected definition list")
    };
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].terms.len(), 1);
    assert_eq!(items[0].definitions.len(), 1);
}

/// Test that task markers inside <p> elements are properly stripped.
/// This handles HTML from markdown parsers like comrak that wrap
/// task list content in <p> tags.
#[test]
fn parse_task_list_with_paragraph_markers() {
    // HTML generated from markdown like:
    // - [ ] Task item
    //   - [x] Nested task
    let html = r#"<html><body><ul><li><p>[ ] Task item</p><ul><li><input type="checkbox" checked> Nested task</li></ul></li></ul></body></html>"#;
    let doc = parse(html);
    let Block::List { list, .. } = &doc.blocks[0] else {
        panic!("expected list")
    };
    let ListKind::Task { items } = &list.kind else {
        panic!("expected task list")
    };
    assert_eq!(items.len(), 1);
    // Task item should NOT contain "[ ]" in its text
    assert!(!items[0].checked);
    let task_text = block_text(&items[0].blocks[0]);
    assert!(
        !task_text.contains("[ ]"),
        "task marker should be stripped: {}",
        task_text
    );
    assert!(
        task_text.contains("Task item"),
        "should contain task text: {}",
        task_text
    );
    // Should have nested list
    assert!(
        items[0]
            .blocks
            .iter()
            .any(|b| matches!(b, Block::List { .. }))
    );
}

#[test]
fn parse_task_list_with_p_and_checkbox() {
    // Test case with both [x] in <p> and checkbox in nested list
    let html = r#"<html><body><ul><li><p>[x] Completed task</p><ul><li><input type="checkbox"> Sub task</li></ul></li></ul></body></html>"#;
    let doc = parse(html);
    let Block::List { list, .. } = &doc.blocks[0] else {
        panic!("expected list")
    };
    let ListKind::Task { items } = &list.kind else {
        panic!("expected task list")
    };
    assert_eq!(items.len(), 1);
    assert!(items[0].checked, "task should be checked");
    let task_text = block_text(&items[0].blocks[0]);
    assert!(
        !task_text.contains("[x]"),
        "task marker should be stripped: {}",
        task_text
    );
    assert!(
        task_text.contains("Completed task"),
        "should contain task text: {}",
        task_text
    );
}
