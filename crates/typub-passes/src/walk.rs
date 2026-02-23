//! Shared document walker for v2 passes.

use anyhow::Result;

use typub_ir::{Asset, AssetId, Block, Document, Inline, List, UnknownChild};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NodePath {
    segments: Vec<String>,
}

impl NodePath {
    pub fn push(&mut self, segment: impl Into<String>) {
        self.segments.push(segment.into());
    }

    pub fn pop(&mut self) {
        let _ = self.segments.pop();
    }

    pub fn render(&self) -> String {
        if self.segments.is_empty() {
            "document".to_string()
        } else {
            format!("document.{}", self.segments.join("."))
        }
    }
}

fn with_path<T, F>(path: &mut NodePath, segment: impl Into<String>, f: F) -> Result<T>
where
    F: FnOnce(&mut NodePath) -> Result<T>,
{
    path.push(segment.into());
    let result = f(path);
    path.pop();
    result
}

pub trait VisitorMut {
    fn visit_asset(
        &mut self,
        _asset_id: &AssetId,
        _asset: &mut Asset,
        _path: &NodePath,
    ) -> Result<()> {
        Ok(())
    }

    fn visit_block(&mut self, _block: &mut Block, _path: &NodePath) -> Result<()> {
        Ok(())
    }

    fn visit_inline(&mut self, _inline: &mut Inline, _path: &NodePath) -> Result<()> {
        Ok(())
    }
}

pub fn walk_document_mut<V: VisitorMut>(doc: &mut Document, visitor: &mut V) -> Result<()> {
    let mut path = NodePath::default();
    walk_blocks_mut(&mut doc.blocks, visitor, &mut path, "blocks")?;

    for (id, def) in &mut doc.footnotes {
        with_path(&mut path, format!("footnotes[{}]", id.0), |path| {
            walk_blocks_mut(&mut def.blocks, visitor, path, "blocks")
        })?;
    }

    for (id, asset) in &mut doc.assets {
        with_path(&mut path, format!("assets[{}]", id.0), |path| {
            visitor.visit_asset(id, asset, path)
        })?;
    }

    Ok(())
}

fn walk_blocks_mut<V: VisitorMut>(
    blocks: &mut [Block],
    visitor: &mut V,
    path: &mut NodePath,
    segment_prefix: &str,
) -> Result<()> {
    for (idx, block) in blocks.iter_mut().enumerate() {
        with_path(path, format!("{segment_prefix}[{idx}]"), |path| {
            visitor.visit_block(block, path)?;
            walk_block_children_mut(block, visitor, path)
        })?;
    }
    Ok(())
}

fn walk_block_children_mut<V: VisitorMut>(
    block: &mut Block,
    visitor: &mut V,
    path: &mut NodePath,
) -> Result<()> {
    match block {
        Block::Heading { content, .. } | Block::Paragraph { content, .. } => {
            walk_inlines_mut(content, visitor, path, "inlines")?
        }
        Block::Quote { blocks, .. }
        | Block::Figure {
            content: blocks, ..
        }
        | Block::Admonition { blocks, .. }
        | Block::Details { blocks, .. } => walk_blocks_mut(blocks, visitor, path, "blocks")?,
        Block::CodeBlock { .. }
        | Block::Divider { .. }
        | Block::MathBlock { .. }
        | Block::SvgBlock { .. } => {}
        Block::List { list, .. } => walk_list_mut(list, visitor, path)?,
        Block::DefinitionList { items, .. } => {
            for (item_idx, item) in items.iter_mut().enumerate() {
                with_path(path, format!("definition_items[{item_idx}]"), |path| {
                    for (term_idx, term_blocks) in item.terms.iter_mut().enumerate() {
                        with_path(path, format!("terms[{term_idx}]"), |path| {
                            walk_blocks_mut(term_blocks, visitor, path, "blocks")
                        })?;
                    }
                    for (def_idx, def_blocks) in item.definitions.iter_mut().enumerate() {
                        with_path(path, format!("definitions[{def_idx}]"), |path| {
                            walk_blocks_mut(def_blocks, visitor, path, "blocks")
                        })?;
                    }
                    Ok(())
                })?;
            }
        }
        Block::Table {
            caption, sections, ..
        } => {
            if let Some(caption_blocks) = caption {
                with_path(path, "caption", |path| {
                    walk_blocks_mut(caption_blocks, visitor, path, "blocks")
                })?;
            }
            for (section_idx, section) in sections.iter_mut().enumerate() {
                with_path(path, format!("sections[{section_idx}]"), |path| {
                    for (row_idx, row) in section.rows.iter_mut().enumerate() {
                        with_path(path, format!("rows[{row_idx}]"), |path| {
                            for (cell_idx, cell) in row.cells.iter_mut().enumerate() {
                                with_path(path, format!("cells[{cell_idx}]"), |path| {
                                    walk_blocks_mut(&mut cell.blocks, visitor, path, "blocks")
                                })?;
                            }
                            Ok(())
                        })?;
                    }
                    Ok(())
                })?;
            }
        }
        Block::UnknownBlock { children, .. } => {
            for (idx, child) in children.iter_mut().enumerate() {
                with_path(path, format!("unknown_children[{idx}]"), |path| {
                    match child {
                        UnknownChild::Block(block) => {
                            visitor.visit_block(block, path)?;
                            walk_block_children_mut(block, visitor, path)?;
                        }
                        UnknownChild::Inline(inline) => {
                            visitor.visit_inline(inline, path)?;
                            walk_inline_children_mut(inline, visitor, path)?;
                        }
                    }
                    Ok(())
                })?;
            }
        }
        Block::RawBlock { .. } => {}
    }
    Ok(())
}

fn walk_list_mut<V: VisitorMut>(
    list: &mut List,
    visitor: &mut V,
    path: &mut NodePath,
) -> Result<()> {
    walk_list_item_blocks_mut(list.kind.item_blocks_mut(), visitor, path)
}

fn walk_list_item_blocks_mut<'a, V>(
    item_blocks: impl Iterator<Item = &'a mut Vec<Block>>,
    visitor: &mut V,
    path: &mut NodePath,
) -> Result<()>
where
    V: VisitorMut,
{
    for (idx, blocks) in item_blocks.enumerate() {
        with_path(path, format!("items[{idx}]"), |path| {
            walk_blocks_mut(blocks, visitor, path, "blocks")
        })?;
    }
    Ok(())
}

fn walk_inlines_mut<V: VisitorMut>(
    inlines: &mut [Inline],
    visitor: &mut V,
    path: &mut NodePath,
    segment_prefix: &str,
) -> Result<()> {
    for (idx, inline) in inlines.iter_mut().enumerate() {
        with_path(path, format!("{segment_prefix}[{idx}]"), |path| {
            visitor.visit_inline(inline, path)?;
            walk_inline_children_mut(inline, visitor, path)
        })?;
    }
    Ok(())
}

fn walk_inline_children_mut<V: VisitorMut>(
    inline: &mut Inline,
    visitor: &mut V,
    path: &mut NodePath,
) -> Result<()> {
    match inline {
        Inline::Styled { content, .. }
        | Inline::Link { content, .. }
        | Inline::UnknownInline { content, .. } => {
            walk_inlines_mut(content, visitor, path, "inlines")?
        }
        Inline::Text(_)
        | Inline::Code(_)
        | Inline::SoftBreak
        | Inline::HardBreak
        | Inline::Image { .. }
        | Inline::FootnoteRef(_)
        | Inline::MathInline { .. }
        | Inline::SvgInline { .. }
        | Inline::RawInline { .. } => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use typub_ir::{BlockAttrs, DocMeta, Inline, InlineAttrs, Url};

    #[derive(Default)]
    struct CountingVisitor {
        blocks: usize,
        inlines: usize,
    }

    impl VisitorMut for CountingVisitor {
        fn visit_block(&mut self, _block: &mut Block, _path: &NodePath) -> Result<()> {
            self.blocks += 1;
            Ok(())
        }

        fn visit_inline(&mut self, _inline: &mut Inline, _path: &NodePath) -> Result<()> {
            self.inlines += 1;
            Ok(())
        }
    }

    #[test]
    fn walk_document_visits_nested_blocks_and_inlines() {
        let mut doc = Document {
            blocks: vec![Block::Paragraph {
                content: vec![Inline::Link {
                    content: vec![Inline::Text("x".to_string())],
                    href: Url("https://example.com".to_string()),
                    title: None,
                    attrs: InlineAttrs::default(),
                }],
                attrs: BlockAttrs::default(),
            }],
            footnotes: Default::default(),
            assets: Default::default(),
            meta: DocMeta::default(),
        };

        let mut visitor = CountingVisitor::default();
        walk_document_mut(&mut doc, &mut visitor).expect("walk document");
        assert!(visitor.blocks >= 1);
        assert!(visitor.inlines >= 2);
    }
}
