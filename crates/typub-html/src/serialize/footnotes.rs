use super::*;
use typub_ir::Url;

pub(super) fn serialize_footnotes(
    ctx: &SerializeCtx<'_>,
    footnotes: &BTreeMap<FootnoteId, FootnoteDef>,
    out: &mut String,
) {
    if footnotes.is_empty() {
        return;
    }

    out.push_str("<section class=\"footnotes\"><ol>");
    for (id, def) in footnotes {
        let id_str = id.0.to_string();
        out.push_str(&format!("<li id=\"fn-{}\">", id_str));
        if has_footnote_backlink(def, id) {
            serialize_blocks(ctx, &def.blocks, out);
        } else if let Some(blocks) = append_backlink_to_last_paragraph(def, id) {
            serialize_blocks(ctx, &blocks, out);
        } else {
            serialize_blocks(ctx, &def.blocks, out);
            out.push_str(&format!("<a href=\"#fnref-{}\">↩</a>", id_str));
        }
        out.push_str("</li>");
    }
    out.push_str("</ol></section>\n");
}

fn append_backlink_to_last_paragraph(def: &FootnoteDef, id: &FootnoteId) -> Option<Vec<Block>> {
    let mut blocks = def.blocks.clone();
    for block in blocks.iter_mut().rev() {
        if let Block::Paragraph { content, .. } = block {
            content.push(Inline::Link {
                content: vec![Inline::Text("↩".to_string())],
                href: Url(format!("#fnref-{}", id.0)),
                title: None,
                attrs: InlineAttrs::default(),
            });
            return Some(blocks);
        }
    }
    None
}

fn has_footnote_backlink(def: &FootnoteDef, id: &FootnoteId) -> bool {
    let target = format!("#fnref-{}", id.0);
    def.blocks
        .iter()
        .any(|block| block_has_footnote_backlink(block, &target))
}

fn block_has_footnote_backlink(block: &Block, target: &str) -> bool {
    match block {
        Block::Heading { content, .. } | Block::Paragraph { content, .. } => content
            .iter()
            .any(|inline| inline_has_footnote_backlink(inline, target)),
        Block::Quote { blocks, .. }
        | Block::Figure {
            content: blocks, ..
        }
        | Block::Admonition { blocks, .. }
        | Block::Details { blocks, .. } => blocks
            .iter()
            .any(|child| block_has_footnote_backlink(child, target)),
        Block::List { list, .. } => match &list.kind {
            ListKind::Bullet { items } | ListKind::Numbered { items, .. } => items
                .iter()
                .flat_map(|item| item.blocks.iter())
                .any(|child| block_has_footnote_backlink(child, target)),
            ListKind::Task { items } => items
                .iter()
                .flat_map(|item| item.blocks.iter())
                .any(|child| block_has_footnote_backlink(child, target)),
            ListKind::Custom { items, .. } => items
                .iter()
                .flat_map(|item| item.blocks.iter())
                .any(|child| block_has_footnote_backlink(child, target)),
        },
        Block::DefinitionList { items, .. } => items
            .iter()
            .flat_map(|item| item.terms.iter().chain(item.definitions.iter()))
            .flat_map(|group| group.iter())
            .any(|child| block_has_footnote_backlink(child, target)),
        Block::Table { sections, .. } => sections
            .iter()
            .flat_map(|section| section.rows.iter())
            .flat_map(|row| row.cells.iter())
            .flat_map(|cell| cell.blocks.iter())
            .any(|child| block_has_footnote_backlink(child, target)),
        Block::UnknownBlock { children, .. } => children.iter().any(|child| match child {
            UnknownChild::Block(block) => block_has_footnote_backlink(block, target),
            UnknownChild::Inline(inline) => inline_has_footnote_backlink(inline, target),
        }),
        Block::CodeBlock { .. }
        | Block::Divider { .. }
        | Block::MathBlock { .. }
        | Block::SvgBlock { .. }
        | Block::RawBlock { .. } => false,
    }
}

fn inline_has_footnote_backlink(inline: &Inline, target: &str) -> bool {
    match inline {
        Inline::Link { href, .. } => href.0 == target,
        Inline::Styled { content, .. } | Inline::UnknownInline { content, .. } => content
            .iter()
            .any(|child| inline_has_footnote_backlink(child, target)),
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
