//! Apply node policy over semantic IR.

use crate::Pass;
use anyhow::Result;
use typub_core::NodePolicyAction;
use typub_ir::{Block, BlockAttrs, Document, Inline, UnknownChild};

pub struct ApplyNodePolicyPass {
    raw: NodePolicyAction,
    unknown: NodePolicyAction,
}

impl ApplyNodePolicyPass {
    pub fn new(raw: NodePolicyAction, unknown: NodePolicyAction) -> Self {
        Self { raw, unknown }
    }

    fn transform_blocks(
        &self,
        blocks: Vec<Block>,
        path: &str,
        parent_action: Option<NodePolicyAction>,
    ) -> Result<Vec<Block>> {
        let mut out = Vec::with_capacity(blocks.len());
        for (idx, block) in blocks.into_iter().enumerate() {
            let block_path = format!("{path}.blocks[{idx}]");
            let mut transformed = self.transform_block(block, &block_path, parent_action)?;
            out.append(&mut transformed);
        }
        Ok(out)
    }

    fn transform_block(
        &self,
        block: Block,
        path: &str,
        parent_action: Option<NodePolicyAction>,
    ) -> Result<Vec<Block>> {
        match block {
            Block::Heading {
                level,
                id,
                content,
                attrs,
            } => Ok(vec![Block::Heading {
                level,
                id,
                content: self.transform_inlines(
                    content,
                    &format!("{path}.inlines"),
                    parent_action,
                )?,
                attrs,
            }]),
            Block::Paragraph { content, attrs } => Ok(vec![Block::Paragraph {
                content: self.transform_inlines(
                    content,
                    &format!("{path}.inlines"),
                    parent_action,
                )?,
                attrs,
            }]),
            Block::Quote {
                blocks,
                cite,
                attrs,
            } => Ok(vec![Block::Quote {
                blocks: self.transform_blocks(blocks, path, parent_action)?,
                cite,
                attrs,
            }]),
            Block::CodeBlock { .. }
            | Block::Divider { .. }
            | Block::MathBlock { .. }
            | Block::SvgBlock { .. } => Ok(vec![block]),
            Block::List { mut list, attrs } => {
                match &mut list.kind {
                    typub_ir::ListKind::Bullet { items }
                    | typub_ir::ListKind::Numbered { items, .. } => {
                        for (item_idx, item) in items.iter_mut().enumerate() {
                            let blocks = std::mem::take(&mut item.blocks);
                            item.blocks = self.transform_blocks(
                                blocks,
                                &format!("{path}.items[{item_idx}]"),
                                parent_action,
                            )?;
                        }
                    }
                    typub_ir::ListKind::Task { items } => {
                        for (item_idx, item) in items.iter_mut().enumerate() {
                            let blocks = std::mem::take(&mut item.blocks);
                            item.blocks = self.transform_blocks(
                                blocks,
                                &format!("{path}.items[{item_idx}]"),
                                parent_action,
                            )?;
                        }
                    }
                    typub_ir::ListKind::Custom { items, .. } => {
                        for (item_idx, item) in items.iter_mut().enumerate() {
                            let blocks = std::mem::take(&mut item.blocks);
                            item.blocks = self.transform_blocks(
                                blocks,
                                &format!("{path}.items[{item_idx}]"),
                                parent_action,
                            )?;
                        }
                    }
                }
                Ok(vec![Block::List { list, attrs }])
            }
            Block::DefinitionList { mut items, attrs } => {
                for (item_idx, item) in items.iter_mut().enumerate() {
                    for (term_idx, term) in item.terms.iter_mut().enumerate() {
                        let blocks = std::mem::take(term);
                        *term = self.transform_blocks(
                            blocks,
                            &format!("{path}.definition_items[{item_idx}].terms[{term_idx}]"),
                            parent_action,
                        )?;
                    }
                    for (def_idx, defn) in item.definitions.iter_mut().enumerate() {
                        let blocks = std::mem::take(defn);
                        *defn = self.transform_blocks(
                            blocks,
                            &format!("{path}.definition_items[{item_idx}].definitions[{def_idx}]"),
                            parent_action,
                        )?;
                    }
                }
                Ok(vec![Block::DefinitionList { items, attrs }])
            }
            Block::Table {
                mut caption,
                mut sections,
                attrs,
            } => {
                if let Some(caption_blocks) = caption.as_mut() {
                    let blocks = std::mem::take(caption_blocks);
                    *caption_blocks =
                        self.transform_blocks(blocks, &format!("{path}.caption"), parent_action)?;
                }
                for (section_idx, section) in sections.iter_mut().enumerate() {
                    for (row_idx, row) in section.rows.iter_mut().enumerate() {
                        for (cell_idx, cell) in row.cells.iter_mut().enumerate() {
                            let blocks = std::mem::take(&mut cell.blocks);
                            cell.blocks = self.transform_blocks(
                                blocks,
                                &format!(
                                    "{path}.sections[{section_idx}].rows[{row_idx}].cells[{cell_idx}]"
                                ),
                                parent_action,
                            )?;
                        }
                    }
                }
                Ok(vec![Block::Table {
                    caption,
                    sections,
                    attrs,
                }])
            }
            Block::Figure {
                content,
                mut caption,
                attrs,
            } => {
                let content =
                    self.transform_blocks(content, &format!("{path}.content"), parent_action)?;
                if let Some(caption_blocks) = caption.as_mut() {
                    let blocks = std::mem::take(caption_blocks);
                    *caption_blocks =
                        self.transform_blocks(blocks, &format!("{path}.caption"), parent_action)?;
                }
                Ok(vec![Block::Figure {
                    content,
                    caption,
                    attrs,
                }])
            }
            Block::Admonition {
                kind,
                title,
                blocks,
                attrs,
            } => Ok(vec![Block::Admonition {
                kind,
                title: title
                    .map(|inlines| {
                        self.transform_inlines(inlines, &format!("{path}.title"), parent_action)
                    })
                    .transpose()?,
                blocks: self.transform_blocks(blocks, path, parent_action)?,
                attrs,
            }]),
            Block::Details {
                summary,
                blocks,
                open,
                attrs,
            } => Ok(vec![Block::Details {
                summary: summary
                    .map(|inlines| {
                        self.transform_inlines(inlines, &format!("{path}.summary"), parent_action)
                    })
                    .transpose()?,
                blocks: self.transform_blocks(blocks, path, parent_action)?,
                open,
                attrs,
            }]),
            Block::RawBlock {
                html,
                origin,
                trust,
                attrs,
            } => {
                let action = parent_action.unwrap_or(self.raw);
                match action {
                    NodePolicyAction::Pass => Ok(vec![Block::RawBlock {
                        html,
                        origin,
                        trust,
                        attrs,
                    }]),
                    NodePolicyAction::Sanitize => Ok(if html.trim().is_empty() {
                        Vec::new()
                    } else {
                        vec![Block::Paragraph {
                            content: vec![Inline::Text(html)],
                            attrs: BlockAttrs::default(),
                        }]
                    }),
                    NodePolicyAction::Drop => Ok(Vec::new()),
                    NodePolicyAction::Error => anyhow::bail!(
                        "Raw node encountered at {path}, but adapter policy is 'error'"
                    ),
                }
            }
            Block::UnknownBlock {
                tag,
                attrs,
                children,
                data,
                note,
                source,
            } => {
                let action = parent_action.unwrap_or(self.unknown);
                match action {
                    NodePolicyAction::Pass => Ok(vec![Block::UnknownBlock {
                        tag,
                        attrs,
                        children: self.transform_unknown_children(children, path, Some(action))?,
                        data,
                        note,
                        source,
                    }]),
                    NodePolicyAction::Sanitize => {
                        let children =
                            self.transform_unknown_children(children, path, Some(action))?;
                        Ok(self.unknown_children_to_blocks(children))
                    }
                    NodePolicyAction::Drop => Ok(Vec::new()),
                    NodePolicyAction::Error => anyhow::bail!(
                        "Unknown node encountered at {path}, but adapter policy is 'error'"
                    ),
                }
            }
        }
    }

    fn transform_inlines(
        &self,
        inlines: Vec<Inline>,
        path: &str,
        parent_action: Option<NodePolicyAction>,
    ) -> Result<Vec<Inline>> {
        let mut out = Vec::with_capacity(inlines.len());
        for (idx, inline) in inlines.into_iter().enumerate() {
            let inline_path = format!("{path}.inlines[{idx}]");
            let mut transformed = self.transform_inline(inline, &inline_path, parent_action)?;
            out.append(&mut transformed);
        }
        Ok(out)
    }

    fn transform_inline(
        &self,
        inline: Inline,
        path: &str,
        parent_action: Option<NodePolicyAction>,
    ) -> Result<Vec<Inline>> {
        match inline {
            Inline::Text(_)
            | Inline::Code(_)
            | Inline::SoftBreak
            | Inline::HardBreak
            | Inline::Image { .. }
            | Inline::FootnoteRef(_)
            | Inline::MathInline { .. }
            | Inline::SvgInline { .. } => Ok(vec![inline]),
            Inline::Styled {
                styles,
                content,
                attrs,
            } => Ok(vec![Inline::Styled {
                styles,
                content: self.transform_inlines(content, path, parent_action)?,
                attrs,
            }]),
            Inline::Link {
                content,
                href,
                title,
                attrs,
            } => Ok(vec![Inline::Link {
                content: self.transform_inlines(content, path, parent_action)?,
                href,
                title,
                attrs,
            }]),
            Inline::RawInline {
                html,
                origin,
                trust,
                attrs,
            } => {
                let action = parent_action.unwrap_or(self.raw);
                match action {
                    NodePolicyAction::Pass => Ok(vec![Inline::RawInline {
                        html,
                        origin,
                        trust,
                        attrs,
                    }]),
                    NodePolicyAction::Sanitize => Ok(if html.is_empty() {
                        Vec::new()
                    } else {
                        vec![Inline::Text(html)]
                    }),
                    NodePolicyAction::Drop => Ok(Vec::new()),
                    NodePolicyAction::Error => anyhow::bail!(
                        "Raw inline encountered at {path}, but adapter policy is 'error'"
                    ),
                }
            }
            Inline::UnknownInline {
                tag,
                attrs,
                content,
                data,
                note,
                source,
            } => {
                let action = parent_action.unwrap_or(self.unknown);
                match action {
                    NodePolicyAction::Pass => Ok(vec![Inline::UnknownInline {
                        tag,
                        attrs,
                        content: self.transform_inlines(content, path, Some(action))?,
                        data,
                        note,
                        source,
                    }]),
                    NodePolicyAction::Sanitize => {
                        self.transform_inlines(content, path, Some(action))
                    }
                    NodePolicyAction::Drop => Ok(Vec::new()),
                    NodePolicyAction::Error => anyhow::bail!(
                        "Unknown inline encountered at {path}, but adapter policy is 'error'"
                    ),
                }
            }
        }
    }

    fn transform_unknown_children(
        &self,
        children: Vec<UnknownChild>,
        path: &str,
        parent_action: Option<NodePolicyAction>,
    ) -> Result<Vec<UnknownChild>> {
        let mut out = Vec::with_capacity(children.len());
        for (idx, child) in children.into_iter().enumerate() {
            let child_path = format!("{path}.unknown_children[{idx}]");
            match child {
                UnknownChild::Block(block) => {
                    for block in self.transform_block(block, &child_path, parent_action)? {
                        out.push(UnknownChild::Block(block));
                    }
                }
                UnknownChild::Inline(inline) => {
                    for inline in self.transform_inline(inline, &child_path, parent_action)? {
                        out.push(UnknownChild::Inline(inline));
                    }
                }
            }
        }
        Ok(out)
    }

    fn unknown_children_to_blocks(&self, children: Vec<UnknownChild>) -> Vec<Block> {
        let mut out = Vec::new();
        let mut pending_inline = Vec::new();
        for child in children {
            match child {
                UnknownChild::Block(block) => {
                    if !pending_inline.is_empty() {
                        out.push(Block::Paragraph {
                            content: std::mem::take(&mut pending_inline),
                            attrs: BlockAttrs::default(),
                        });
                    }
                    out.push(block);
                }
                UnknownChild::Inline(inline) => {
                    pending_inline.push(inline);
                }
            }
        }
        if !pending_inline.is_empty() {
            out.push(Block::Paragraph {
                content: pending_inline,
                attrs: BlockAttrs::default(),
            });
        }
        out
    }
}

impl Pass for ApplyNodePolicyPass {
    fn name(&self) -> &'static str {
        "apply_node_policy"
    }

    fn run(&mut self, doc: &mut Document, _ctx: &mut crate::PassCtx) -> Result<()> {
        let blocks = std::mem::take(&mut doc.blocks);
        doc.blocks = self.transform_blocks(blocks, "document", None)?;

        for (id, def) in &mut doc.footnotes {
            let blocks = std::mem::take(&mut def.blocks);
            def.blocks =
                self.transform_blocks(blocks, &format!("document.footnotes[{}]", id.0), None)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use typub_ir::{DocMeta, InlineAttrs, RawOrigin, RawTrust};

    fn doc_with(blocks: Vec<Block>) -> Document {
        Document {
            blocks,
            footnotes: Default::default(),
            assets: Default::default(),
            meta: DocMeta::default(),
        }
    }

    #[test]
    fn pass_action_keeps_raw_and_unknown() {
        let mut doc = doc_with(vec![
            Block::RawBlock {
                html: "<x/>".to_string(),
                origin: RawOrigin::Markdown,
                trust: RawTrust::Trusted,
                attrs: BlockAttrs::default(),
            },
            Block::Paragraph {
                content: vec![Inline::UnknownInline {
                    tag: "foo".to_string(),
                    attrs: InlineAttrs::default(),
                    content: vec![Inline::Text("ok".to_string())],
                    data: Default::default(),
                    note: None,
                    source: None,
                }],
                attrs: BlockAttrs::default(),
            },
        ]);

        let mut pass = ApplyNodePolicyPass::new(NodePolicyAction::Pass, NodePolicyAction::Pass);
        pass.run(&mut doc, &mut crate::PassCtx::default())
            .expect("run pass");

        assert!(matches!(doc.blocks[0], Block::RawBlock { .. }));
        match &doc.blocks[1] {
            Block::Paragraph { content, .. } => {
                assert!(matches!(content[0], Inline::UnknownInline { .. }));
            }
            _ => panic!("expected paragraph"),
        }
    }

    #[test]
    fn sanitize_action_neutralizes_raw_and_unknown() {
        let mut doc = doc_with(vec![
            Block::RawBlock {
                html: "<script>alert(1)</script>".to_string(),
                origin: RawOrigin::Markdown,
                trust: RawTrust::Untrusted,
                attrs: BlockAttrs::default(),
            },
            Block::UnknownBlock {
                tag: "x-card".to_string(),
                attrs: BlockAttrs::default(),
                children: vec![
                    UnknownChild::Inline(Inline::Text("hello".to_string())),
                    UnknownChild::Block(Block::Paragraph {
                        content: vec![Inline::Text("world".to_string())],
                        attrs: BlockAttrs::default(),
                    }),
                ],
                data: Default::default(),
                note: None,
                source: None,
            },
        ]);

        let mut pass =
            ApplyNodePolicyPass::new(NodePolicyAction::Sanitize, NodePolicyAction::Sanitize);
        pass.run(&mut doc, &mut crate::PassCtx::default())
            .expect("run pass");

        match &doc.blocks[0] {
            Block::Paragraph { content, .. } => {
                assert!(matches!(content[0], Inline::Text(_)));
            }
            _ => panic!("expected paragraph for sanitized raw"),
        }

        assert!(matches!(doc.blocks[1], Block::Paragraph { .. }));
        assert!(matches!(doc.blocks[2], Block::Paragraph { .. }));
    }

    #[test]
    fn drop_action_removes_raw_and_unknown() {
        let mut doc = doc_with(vec![
            Block::Paragraph {
                content: vec![
                    Inline::Text("a".to_string()),
                    Inline::RawInline {
                        html: "<b>x</b>".to_string(),
                        origin: RawOrigin::Markdown,
                        trust: RawTrust::Untrusted,
                        attrs: InlineAttrs::default(),
                    },
                    Inline::UnknownInline {
                        tag: "foo".to_string(),
                        attrs: InlineAttrs::default(),
                        content: vec![Inline::Text("b".to_string())],
                        data: Default::default(),
                        note: None,
                        source: None,
                    },
                ],
                attrs: BlockAttrs::default(),
            },
            Block::UnknownBlock {
                tag: "x".to_string(),
                attrs: BlockAttrs::default(),
                children: vec![],
                data: Default::default(),
                note: None,
                source: None,
            },
        ]);

        let mut pass = ApplyNodePolicyPass::new(NodePolicyAction::Drop, NodePolicyAction::Drop);
        pass.run(&mut doc, &mut crate::PassCtx::default())
            .expect("run pass");

        assert_eq!(doc.blocks.len(), 1);
        match &doc.blocks[0] {
            Block::Paragraph { content, .. } => {
                assert_eq!(content.len(), 1);
                assert!(matches!(content[0], Inline::Text(_)));
            }
            _ => panic!("expected paragraph"),
        }
    }

    #[test]
    fn error_action_fails_on_raw_unknown() {
        let mut doc = doc_with(vec![Block::RawBlock {
            html: "<x/>".to_string(),
            origin: RawOrigin::Markdown,
            trust: RawTrust::Untrusted,
            attrs: BlockAttrs::default(),
        }]);
        let mut pass = ApplyNodePolicyPass::new(NodePolicyAction::Error, NodePolicyAction::Pass);
        let err = pass
            .run(&mut doc, &mut crate::PassCtx::default())
            .expect_err("raw should error");
        assert!(err.to_string().contains("Raw node encountered"));
    }
}
