use typub_ir::{Block, Inline, ListKind};

use super::helpers::{block_text, parse};

#[test]
fn parse_extreme_deep_mixed_nesting() {
    let html = r#"<html><body>
    <ul><li>L1
      <ol><li>L2
        <details open><summary>S</summary>
          <table><tr><td><blockquote><p>Q</p></blockquote></td></tr></table>
        </details>
      </li></ol>
    </li></ul>
    </body></html>"#;
    let doc = parse(html);
    let Block::List { list, .. } = &doc.blocks[0] else {
        panic!("expected top list")
    };
    let ListKind::Bullet { items } = &list.kind else {
        panic!("expected bullet list")
    };
    assert_eq!(items.len(), 1);
    assert!(
        items[0]
            .blocks
            .iter()
            .any(|b| matches!(b, Block::List { .. }))
    );
    assert!(items[0].blocks.iter().any(|b| block_text(b).contains("L1")));
}

#[test]
fn parse_known_and_unknown_interleaving() {
    let html = r#"<html><body>
    <p>A <custom-inline>x</custom-inline> B</p>
    <custom-block><p>inside</p></custom-block>
    <p>C</p>
    </body></html>"#;
    let doc = parse(html);
    assert_eq!(doc.blocks.len(), 3);
    let Block::Paragraph { content, .. } = &doc.blocks[0] else {
        panic!("expected first paragraph")
    };
    assert!(
        content
            .iter()
            .any(|i| matches!(i, Inline::UnknownInline { .. }))
    );
    assert!(matches!(doc.blocks[1], Block::UnknownBlock { .. }));
    assert!(matches!(doc.blocks[2], Block::Paragraph { .. }));
    assert!(block_text(&doc.blocks[2]).contains("C"));
    assert_eq!(
        doc.blocks
            .iter()
            .filter(|b| matches!(b, Block::UnknownBlock { .. }))
            .count(),
        1
    );
}

#[test]
fn parse_asset_dedup_ignores_legacy_marker_code() {
    let html = r#"<html><body>
    <p><img src="assets/x.png"></p>
    <img src="assets/x.png">
    <p><code>[[IMG:assets/x.png]]</code></p>
    </body></html>"#;
    let doc = parse(html);
    assert_eq!(doc.assets.len(), 1);
    assert!(
        doc.blocks
            .iter()
            .any(|b| matches!(b, Block::Paragraph { content, .. } if matches!(content.as_slice(), [Inline::Code(code)] if code.contains("[[IMG:assets/x.png]]"))))
    );
    assert!(
        doc.blocks
            .iter()
            .filter(|b| matches!(b, Block::Paragraph { .. }))
            .count()
            >= 2
    );
}

#[test]
fn parse_large_mixed_document_shape() {
    let html = r#"<html><body>
    <h1 id="t">Title</h1>
    <p>Intro <a href="https://e.com">link</a></p>
    <div class="admonition warning"><p class="admonition-title">W</p><p>Body</p></div>
    <figure><img src="assets/f.png"><figcaption>Cap</figcaption></figure>
    <details><summary>S</summary><ul><li>A</li><li>B</li></ul></details>
    <table><tr><th>H</th></tr><tr><td>C</td></tr></table>
    <blockquote><p>[!TIP]</p><p>T</p></blockquote>
    <div class="typst-svg-block" data-latex-src="x"><svg>...</svg></div>
    </body></html>"#;
    let doc = parse(html);
    assert_eq!(doc.blocks.len(), 8);
    assert!(
        doc.blocks
            .iter()
            .any(|b| matches!(b, Block::Heading { .. }))
    );
    assert!(
        doc.blocks
            .iter()
            .any(|b| matches!(b, Block::Admonition { .. }))
    );
    assert!(doc.blocks.iter().any(|b| matches!(b, Block::Figure { .. })));
    assert!(
        doc.blocks
            .iter()
            .any(|b| matches!(b, Block::Details { .. }))
    );
    assert!(doc.blocks.iter().any(|b| matches!(b, Block::Table { .. })));
    assert!(
        doc.blocks
            .iter()
            .any(|b| matches!(b, Block::MathBlock { .. }))
    );
}

#[test]
fn parse_multiple_math_nodes_both_inline_and_block() {
    let html = r#"<html><body>
    <p>X <span class="typst-svg-inline" data-typst-src="$x$"><svg>...</svg></span> Y</p>
    <div class="typst-svg-block" data-typst-src="$y$"><svg>...</svg></div>
    <p>Z <span class="typst-svg-inline" data-latex-src="z"><svg>...</svg></span></p>
    </body></html>"#;
    let doc = parse(html);
    let inline_count = doc
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::Paragraph { content, .. } => Some(
                content
                    .iter()
                    .filter(|i| matches!(i, Inline::MathInline { .. }))
                    .count(),
            ),
            _ => None,
        })
        .sum::<usize>();
    let block_count = doc
        .blocks
        .iter()
        .filter(|b| matches!(b, Block::MathBlock { .. }))
        .count();
    assert_eq!(inline_count, 2);
    assert_eq!(block_count, 1);
}

#[test]
fn parse_task_detection_boundary_does_not_false_positive() {
    let html = r#"<html><body><ul><li>[abc] not task</li><li>normal</li></ul></body></html>"#;
    let doc = parse(html);
    let Block::List { list, .. } = &doc.blocks[0] else {
        panic!("expected list")
    };
    assert!(matches!(list.kind, ListKind::Bullet { .. }));
    assert!(!matches!(list.kind, ListKind::Task { .. }));
}
