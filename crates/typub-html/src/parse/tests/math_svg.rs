use typub_ir::{Block, Inline};

use super::helpers::parse;

#[test]
fn parse_inline_svg_math() {
    let html = r#"<html><body><p>A <span class="typst-svg-inline" data-latex-src="E = mc^2"><svg>...</svg></span> B</p></body></html>"#;
    let doc = parse(html);
    let Block::Paragraph { content, .. } = &doc.blocks[0] else {
        panic!("expected paragraph")
    };
    let math = content
        .iter()
        .find_map(|i| match i {
            Inline::MathInline { math, .. } => Some(math),
            _ => None,
        })
        .expect("inline math");
    assert!(matches!(
        math.src,
        Some(typub_ir::MathSource::Latex(ref s)) if s == "E = mc^2"
    ));
    assert!(matches!(
        math.rendered,
        Some(typub_ir::RenderedMath::Svg(ref s)) if s.contains("<svg")
    ));
    assert!(
        !content
            .iter()
            .any(|i| matches!(i, Inline::UnknownInline { .. }))
    );
}

#[test]
fn parse_block_svg_math() {
    let html = r#"<html><body><div class="typst-svg-block" data-latex-src="\int_0^1"><svg>...</svg></div></body></html>"#;
    let doc = parse(html);
    let Block::MathBlock { math, .. } = &doc.blocks[0] else {
        panic!("expected math block")
    };
    assert!(matches!(
        math.src,
        Some(typub_ir::MathSource::Latex(ref s)) if s == r"\int_0^1"
    ));
    assert!(matches!(
        math.rendered,
        Some(typub_ir::RenderedMath::Svg(ref s)) if s.contains("<svg")
    ));
}

#[test]
fn parse_inline_svg_with_typst_source() {
    let html = r#"<html><body><p><span class="typst-svg-inline" data-typst-src="$x$"><svg>...</svg></span></p></body></html>"#;
    let doc = parse(html);
    let Block::Paragraph { content, .. } = &doc.blocks[0] else {
        panic!("expected paragraph")
    };
    let math = content
        .iter()
        .find_map(|i| match i {
            Inline::MathInline { math, .. } => Some(math),
            _ => None,
        })
        .expect("inline math");
    assert!(matches!(
        math.src,
        Some(typub_ir::MathSource::Typst(ref s)) if s == "$x$"
    ));
}

#[test]
fn parse_block_svg_with_typst_source() {
    let html = r#"<html><body><div class="typst-svg-block" data-typst-src="$x$"><svg>...</svg></div></body></html>"#;
    let doc = parse(html);
    let Block::MathBlock { math, .. } = &doc.blocks[0] else {
        panic!("expected math block")
    };
    assert!(matches!(
        math.src,
        Some(typub_ir::MathSource::Typst(ref s)) if s == "$x$"
    ));
}

#[test]
fn parse_inline_svg_math_without_source_keeps_math_node() {
    let html =
        r#"<html><body><p><span class="typst-svg-inline"><svg>...</svg></span></p></body></html>"#;
    let doc = parse(html);
    let Block::Paragraph { content, .. } = &doc.blocks[0] else {
        panic!("expected paragraph")
    };
    let math = content
        .iter()
        .find_map(|i| match i {
            Inline::MathInline { math, .. } => Some(math),
            _ => None,
        })
        .expect("inline math");
    assert!(matches!(
        math.rendered,
        Some(typub_ir::RenderedMath::Svg(ref s)) if s.contains("<svg")
    ));
}

#[test]
fn parse_block_svg_math_without_source_keeps_math_node() {
    let html = r#"<html><body><div class="typst-svg-block"><svg>...</svg></div></body></html>"#;
    let doc = parse(html);
    let Block::MathBlock { math, .. } = &doc.blocks[0] else {
        panic!("expected math block")
    };
    assert!(matches!(
        math.rendered,
        Some(typub_ir::RenderedMath::Svg(ref s)) if s.contains("<svg")
    ));
}
