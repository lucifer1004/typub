use crate::parse::parse_html_document;
use typub_ir::{Block, Document, Inline};

pub(super) fn parse(html: &str) -> Document {
    parse_html_document(html).expect("parse html")
}

pub(super) fn inline_text(inlines: &[Inline]) -> String {
    let mut out = String::new();
    for i in inlines {
        match i {
            Inline::Text(t) | Inline::Code(t) => out.push_str(t),
            Inline::SoftBreak | Inline::HardBreak => out.push('\n'),
            Inline::Styled { content, .. } | Inline::Link { content, .. } => {
                out.push_str(&inline_text(content))
            }
            Inline::Image { alt, .. } => out.push_str(alt),
            Inline::FootnoteRef(id) => out.push_str(&format!("[{}]", id.0)),
            Inline::MathInline { .. } => out.push_str("[math]"),
            Inline::SvgInline { .. } => out.push_str("[svg]"),
            Inline::UnknownInline { content, .. } => out.push_str(&inline_text(content)),
            Inline::RawInline { html, .. } => out.push_str(html),
        }
    }
    out
}

pub(super) fn block_text(block: &Block) -> String {
    match block {
        Block::Heading { content, .. } | Block::Paragraph { content, .. } => inline_text(content),
        Block::Quote { blocks, .. } => blocks.iter().map(block_text).collect::<Vec<_>>().join(" "),
        Block::CodeBlock { code, .. } => code.clone(),
        Block::List { list, .. } => match &list.kind {
            typub_ir::ListKind::Bullet { items } | typub_ir::ListKind::Numbered { items, .. } => {
                items
                    .iter()
                    .flat_map(|it| it.blocks.iter())
                    .map(block_text)
                    .collect::<Vec<_>>()
                    .join(" ")
            }
            typub_ir::ListKind::Task { items } => items
                .iter()
                .flat_map(|it| it.blocks.iter())
                .map(block_text)
                .collect::<Vec<_>>()
                .join(" "),
            typub_ir::ListKind::Custom { .. } => String::new(),
        },
        Block::DefinitionList { items, .. } => items
            .iter()
            .flat_map(|it| it.terms.iter().chain(it.definitions.iter()))
            .flat_map(|bs| bs.iter())
            .map(block_text)
            .collect::<Vec<_>>()
            .join(" "),
        Block::Table { sections, .. } => sections
            .iter()
            .flat_map(|s| s.rows.iter())
            .flat_map(|r| r.cells.iter())
            .flat_map(|c| c.blocks.iter())
            .map(block_text)
            .collect::<Vec<_>>()
            .join(" "),
        Block::Figure {
            content, caption, ..
        } => {
            let mut t = content.iter().map(block_text).collect::<Vec<_>>().join(" ");
            if let Some(c) = caption {
                if !t.is_empty() {
                    t.push(' ');
                }
                t.push_str(&c.iter().map(block_text).collect::<Vec<_>>().join(" "));
            }
            t
        }
        Block::Admonition { blocks, .. } | Block::Details { blocks, .. } => {
            blocks.iter().map(block_text).collect::<Vec<_>>().join(" ")
        }
        Block::MathBlock { .. } => "[math]".to_string(),
        Block::SvgBlock { .. } => "[svg]".to_string(),
        Block::UnknownBlock { source, .. } => source.clone().unwrap_or_default(),
        Block::RawBlock { html, .. } => html.clone(),
        Block::Divider { .. } => String::new(),
    }
}
