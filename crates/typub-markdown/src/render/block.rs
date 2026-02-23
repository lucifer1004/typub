//! Block rendering utilities for typub HTML IR v2 `Document`.

use anyhow::Result;
use comrak::nodes::{
    AlertType, AstNode, NodeAlert, NodeCodeBlock, NodeFootnoteDefinition, NodeHeading, NodeList,
    NodeValue,
};
use comrak::{Arena, Options, format_commonmark};
use typub_html::{escape_html_attr, escape_html_text};
use typub_ir::{
    AdmonitionKind, Block, Document, FlowListItem, FlowListItemMarker, FootnoteDef, FootnoteId,
    ListKind, OrderedListMarker, RenderPayload, RenderedArtifact, TableCellKind, TableSectionKind,
    TextAlign, UnknownChild,
};

use super::MarkdownRenderOptions;
use super::inline::{inlines_text, push_inline_seq, push_text, resolve_rendered_asset_url};

fn blocks_to_text(blocks: &[Block]) -> String {
    blocks
        .iter()
        .map(block_to_text)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn block_to_text(block: &Block) -> String {
    match block {
        Block::Heading { content, .. } | Block::Paragraph { content, .. } => inlines_text(content),
        Block::Quote { blocks, .. }
        | Block::Admonition { blocks, .. }
        | Block::Details { blocks, .. } => blocks_to_text(blocks),
        Block::CodeBlock { code, .. } => code.clone(),
        Block::Divider { .. } => "---".to_string(),
        Block::List { list, .. } => match &list.kind {
            ListKind::Bullet { items } | ListKind::Numbered { items, .. } => items
                .iter()
                .map(|item| blocks_to_text(&item.blocks))
                .collect::<Vec<_>>()
                .join("\n"),
            ListKind::Task { items } => items
                .iter()
                .map(|item| blocks_to_text(&item.blocks))
                .collect::<Vec<_>>()
                .join("\n"),
            ListKind::Custom { items, .. } => items
                .iter()
                .map(|item| blocks_to_text(&item.blocks))
                .collect::<Vec<_>>()
                .join("\n"),
        },
        Block::DefinitionList { items, .. } => items
            .iter()
            .flat_map(|item| item.terms.iter().chain(item.definitions.iter()))
            .map(|group| blocks_to_text(group))
            .collect::<Vec<_>>()
            .join("\n"),
        Block::Table { .. } => "[table]".to_string(),
        Block::Figure {
            content, caption, ..
        } => {
            let mut text = blocks_to_text(content);
            if let Some(cap) = caption {
                let c = blocks_to_text(cap);
                if !c.is_empty() {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(&c);
                }
            }
            text
        }
        Block::MathBlock { math, .. } | Block::SvgBlock { svg: math, .. } => {
            payload_to_latex(math).unwrap_or_else(|| "[math]".to_string())
        }
        Block::UnknownBlock { source, note, .. } => {
            source.clone().or_else(|| note.clone()).unwrap_or_default()
        }
        Block::RawBlock { html, .. } => html.clone(),
    }
}

fn payload_to_latex(payload: &RenderPayload) -> Option<String> {
    match &payload.src {
        Some(typub_ir::MathSource::Latex(src)) => Some(src.clone()),
        Some(typub_ir::MathSource::Typst(src)) => Some(crate::latex::typst_math_to_latex(src)),
        Some(typub_ir::MathSource::Custom { src, .. }) => Some(src.clone()),
        None => None,
    }
}

fn render_payload_block<'a>(
    arena: &'a Arena<'a>,
    payload: &RenderPayload,
    doc: &Document,
    options: &MarkdownRenderOptions<'_>,
) -> Option<&'a AstNode<'a>> {
    if let Some(latex) = payload_to_latex(payload) {
        let formatted = match options.math_delimiters {
            typub_core::MathDelimiters::Dollar => format!("$${}$$", latex),
            typub_core::MathDelimiters::Brackets
            | typub_core::MathDelimiters::BracketsInlineDollarBlock => {
                format!(r"\\[{}\\]", latex)
            }
        };
        return Some(
            arena.alloc(
                NodeValue::HtmlBlock(comrak::nodes::NodeHtmlBlock {
                    block_type: 6,
                    literal: formatted,
                })
                .into(),
            ),
        );
    }

    match &payload.rendered {
        Some(RenderedArtifact::Svg(svg)) | Some(RenderedArtifact::MathMl(svg)) => Some(
            arena.alloc(
                NodeValue::HtmlBlock(comrak::nodes::NodeHtmlBlock {
                    block_type: 6,
                    literal: svg.clone(),
                })
                .into(),
            ),
        ),
        Some(rendered @ RenderedArtifact::Asset { .. }) => {
            let src = resolve_rendered_asset_url(rendered, doc, options)?;
            let html = format!(r#"<img src="{}" />"#, escape_html_attr(&src));
            Some(
                arena.alloc(
                    NodeValue::HtmlBlock(comrak::nodes::NodeHtmlBlock {
                        block_type: 6,
                        literal: html,
                    })
                    .into(),
                ),
            )
        }
        Some(RenderedArtifact::Custom { data, .. }) => {
            let html = data.get("html").and_then(|v| v.as_str())?;
            let node: &'a AstNode<'a> = arena.alloc(
                NodeValue::HtmlBlock(comrak::nodes::NodeHtmlBlock {
                    block_type: 6,
                    literal: html.to_string(),
                })
                .into(),
            );
            Some(node)
        }
        None => None,
    }
}

fn append_block_children<'a>(
    arena: &'a Arena<'a>,
    parent: &'a AstNode<'a>,
    blocks: &[Block],
    doc: &Document,
    options: &MarkdownRenderOptions<'_>,
) {
    for block in blocks {
        if let Some(node) = block_to_ast(arena, block, doc, options) {
            parent.append(node);
        }
    }
}

fn append_list_item_children<'a>(
    arena: &'a Arena<'a>,
    parent: &'a AstNode<'a>,
    blocks: &[Block],
    doc: &Document,
    options: &MarkdownRenderOptions<'_>,
) {
    append_block_children(arena, parent, blocks, doc, options);
}

fn render_flow_list_item<'a>(
    arena: &'a Arena<'a>,
    item: &FlowListItem,
    doc: &Document,
    options: &MarkdownRenderOptions<'_>,
    list_type: comrak::nodes::ListType,
) -> &'a AstNode<'a> {
    let (padding, bullet_char) = match list_type {
        comrak::nodes::ListType::Bullet => (2, b'-'),
        comrak::nodes::ListType::Ordered => (3, b'.'),
    };
    let node = arena.alloc(
        NodeValue::Item(NodeList {
            list_type,
            marker_offset: 0,
            padding,
            start: 1,
            tight: options.tight_lists,
            delimiter: comrak::nodes::ListDelimType::Period,
            bullet_char,
            is_task_list: false,
        })
        .into(),
    );
    append_list_item_children(arena, node, &item.blocks, doc, options);
    node
}

fn render_table_html(table: &Block) -> Option<String> {
    let Block::Table {
        caption, sections, ..
    } = table
    else {
        return None;
    };

    let mut html = String::from("<table>\n");
    if let Some(c) = caption {
        html.push_str("<caption>");
        html.push_str(&escape_html_text(&blocks_to_text(c)));
        html.push_str("</caption>\n");
    }

    for section in sections {
        let tag = match section.kind {
            TableSectionKind::Head => "thead",
            TableSectionKind::Body => "tbody",
            TableSectionKind::Foot => "tfoot",
        };
        html.push('<');
        html.push_str(tag);
        html.push_str(">\n");
        for row in &section.rows {
            html.push_str("<tr>\n");
            for cell in &row.cells {
                let cell_tag = match cell.kind {
                    TableCellKind::Header => "th",
                    TableCellKind::Data => "td",
                };
                html.push('<');
                html.push_str(cell_tag);
                if cell.colspan > 1 {
                    html.push_str(&format!(r#" colspan="{}""#, cell.colspan));
                }
                if cell.rowspan > 1 {
                    html.push_str(&format!(r#" rowspan="{}""#, cell.rowspan));
                }
                if let Some(align) = cell.align {
                    let align = match align {
                        TextAlign::Left => "left",
                        TextAlign::Center => "center",
                        TextAlign::Right => "right",
                    };
                    html.push_str(&format!(r#" align="{}""#, align));
                }
                html.push('>');
                html.push_str(&escape_html_text(&blocks_to_text(&cell.blocks)));
                html.push_str("</");
                html.push_str(cell_tag);
                html.push_str(">\n");
            }
            html.push_str("</tr>\n");
        }
        html.push_str("</");
        html.push_str(tag);
        html.push_str(">\n");
    }

    html.push_str("</table>");
    Some(html)
}

fn simple_table_cell_text(cell: &typub_ir::TableCell) -> Option<String> {
    if cell.colspan != 1 || cell.rowspan != 1 {
        return None;
    }
    if !cell
        .blocks
        .iter()
        .all(|block| matches!(block, Block::Paragraph { .. }))
    {
        return None;
    }
    let text = blocks_to_text(&cell.blocks).trim().replace('\n', "<br>");
    Some(text.replace('|', r"\|"))
}

fn table_alignment(align: Option<TextAlign>) -> comrak::nodes::TableAlignment {
    match align {
        Some(TextAlign::Left) => comrak::nodes::TableAlignment::Left,
        Some(TextAlign::Center) => comrak::nodes::TableAlignment::Center,
        Some(TextAlign::Right) => comrak::nodes::TableAlignment::Right,
        None => comrak::nodes::TableAlignment::None,
    }
}

struct SimpleTable {
    header: Vec<String>,
    alignments: Vec<comrak::nodes::TableAlignment>,
    body: Vec<Vec<String>>,
}

fn extract_simple_table(table: &Block) -> Option<SimpleTable> {
    let Block::Table {
        caption, sections, ..
    } = table
    else {
        return None;
    };

    if caption.is_some() {
        return None;
    }

    let mut head_rows = Vec::new();
    let mut body_rows = Vec::new();
    for section in sections {
        match section.kind {
            TableSectionKind::Head => head_rows.extend(section.rows.iter()),
            TableSectionKind::Body => body_rows.extend(section.rows.iter()),
            TableSectionKind::Foot => return None,
        }
    }

    if head_rows.len() > 1 {
        return None;
    }

    let (header_row, body_rows) = if let Some(row) = head_rows.first().copied() {
        (row, body_rows)
    } else if let Some((first, rest)) = body_rows.split_first() {
        (*first, rest.to_vec())
    } else {
        return None;
    };

    let col_count = header_row.cells.len();
    if col_count == 0 {
        return None;
    }

    if !body_rows.iter().all(|row| row.cells.len() == col_count)
        || header_row.cells.len() != col_count
    {
        return None;
    }

    let header = header_row
        .cells
        .iter()
        .map(simple_table_cell_text)
        .collect::<Option<Vec<_>>>()?;
    let alignments = header_row
        .cells
        .iter()
        .map(|cell| table_alignment(cell.align))
        .collect::<Vec<_>>();

    let mut body = Vec::new();
    for row in body_rows {
        let cells = row
            .cells
            .iter()
            .map(simple_table_cell_text)
            .collect::<Option<Vec<_>>>()?;
        body.push(cells);
    }

    Some(SimpleTable {
        header,
        alignments,
        body,
    })
}

fn ordered_list_type_attr(marker: &OrderedListMarker) -> Option<&'static str> {
    match marker {
        OrderedListMarker::Decimal => None,
        OrderedListMarker::LowerAlpha => Some("a"),
        OrderedListMarker::UpperAlpha => Some("A"),
        OrderedListMarker::LowerRoman => Some("i"),
        OrderedListMarker::UpperRoman => Some("I"),
    }
}

fn numbered_item_semantics_supported(start: u32, items: &[FlowListItem]) -> bool {
    items
        .iter()
        .enumerate()
        .all(|(idx, item)| match item.marker {
            None => true,
            Some(FlowListItemMarker::Number(n)) => n == start.saturating_add(idx as u32),
            Some(FlowListItemMarker::Bullet) => false,
        })
}

fn render_numbered_list_html(
    start: u32,
    reversed: bool,
    marker: Option<&OrderedListMarker>,
    items: &[FlowListItem],
) -> String {
    let mut html = String::from("<ol");
    if start != 1 {
        html.push_str(&format!(r#" start="{}""#, start));
    }
    if reversed {
        html.push_str(" reversed");
    }
    if let Some(marker) = marker.and_then(ordered_list_type_attr) {
        html.push_str(&format!(r#" type="{}""#, marker));
    }
    html.push_str(">\n");

    for item in items {
        html.push_str("<li");
        if let Some(FlowListItemMarker::Number(value)) = item.marker {
            html.push_str(&format!(r#" value="{}""#, value));
        }
        html.push('>');
        html.push_str(&escape_html_text(&blocks_to_text(&item.blocks)));
        html.push_str("</li>\n");
    }
    html.push_str("</ol>");
    html
}

fn block_to_ast<'a>(
    arena: &'a Arena<'a>,
    block: &Block,
    doc: &Document,
    options: &MarkdownRenderOptions<'_>,
) -> Option<&'a AstNode<'a>> {
    match block {
        Block::Heading { level, content, .. } => {
            let node = arena.alloc(
                NodeValue::Heading(NodeHeading {
                    level: level.get().clamp(1, 6),
                    setext: false,
                    closed: false,
                })
                .into(),
            );
            push_inline_seq(arena, node, content, doc, options);
            Some(node)
        }
        Block::Paragraph { content, .. } => {
            let node = arena.alloc(NodeValue::Paragraph.into());
            push_inline_seq(arena, node, content, doc, options);
            Some(node)
        }
        Block::Quote { blocks, .. } => {
            let node = arena.alloc(NodeValue::BlockQuote.into());
            append_block_children(arena, node, blocks, doc, options);
            Some(node)
        }
        Block::CodeBlock { code, language, .. } => Some(
            arena.alloc(
                NodeValue::CodeBlock(Box::new(NodeCodeBlock {
                    fenced: true,
                    fence_char: b'`',
                    fence_length: 3,
                    fence_offset: 0,
                    info: language.clone().unwrap_or_default(),
                    literal: code.clone(),
                    closed: true,
                }))
                .into(),
            ),
        ),
        Block::Divider { .. } => Some(arena.alloc(NodeValue::ThematicBreak.into())),
        Block::List { list, .. } => match &list.kind {
            ListKind::Bullet { items } => {
                let node = arena.alloc(
                    NodeValue::List(NodeList {
                        list_type: comrak::nodes::ListType::Bullet,
                        marker_offset: 0,
                        padding: 2,
                        start: 1,
                        tight: options.tight_lists,
                        delimiter: comrak::nodes::ListDelimType::Period,
                        bullet_char: b'-',
                        is_task_list: false,
                    })
                    .into(),
                );
                for item in items {
                    node.append(render_flow_list_item(
                        arena,
                        item,
                        doc,
                        options,
                        comrak::nodes::ListType::Bullet,
                    ));
                }
                Some(node)
            }
            ListKind::Numbered {
                start,
                reversed,
                marker,
                items,
            } => {
                let requires_html = *reversed
                    || marker.is_some_and(|m| !matches!(m, OrderedListMarker::Decimal))
                    || !numbered_item_semantics_supported(*start, items);
                if requires_html {
                    let html = render_numbered_list_html(*start, *reversed, marker.as_ref(), items);
                    return Some(
                        arena.alloc(
                            NodeValue::HtmlBlock(comrak::nodes::NodeHtmlBlock {
                                block_type: 6,
                                literal: html,
                            })
                            .into(),
                        ),
                    );
                }
                let node = arena.alloc(
                    NodeValue::List(NodeList {
                        list_type: comrak::nodes::ListType::Ordered,
                        marker_offset: 0,
                        padding: 3,
                        start: *start as usize,
                        tight: options.tight_lists,
                        delimiter: comrak::nodes::ListDelimType::Period,
                        bullet_char: b'.',
                        is_task_list: false,
                    })
                    .into(),
                );
                for item in items {
                    node.append(render_flow_list_item(
                        arena,
                        item,
                        doc,
                        options,
                        comrak::nodes::ListType::Ordered,
                    ));
                }
                Some(node)
            }
            ListKind::Task { items } => {
                let node = arena.alloc(
                    NodeValue::List(NodeList {
                        list_type: comrak::nodes::ListType::Bullet,
                        marker_offset: 0,
                        padding: 2,
                        start: 1,
                        tight: options.tight_lists,
                        delimiter: comrak::nodes::ListDelimType::Period,
                        bullet_char: b'-',
                        is_task_list: true,
                    })
                    .into(),
                );
                for item in items {
                    let task = arena.alloc(
                        NodeValue::TaskItem(comrak::nodes::NodeTaskItem {
                            symbol: if item.checked { Some('x') } else { Some(' ') },
                            symbol_sourcepos: (0, 0, 0, 0).into(),
                        })
                        .into(),
                    );
                    append_list_item_children(arena, task, &item.blocks, doc, options);
                    node.append(task);
                }
                Some(node)
            }
            ListKind::Custom { items, .. } => {
                let node = arena.alloc(
                    NodeValue::List(NodeList {
                        list_type: comrak::nodes::ListType::Bullet,
                        marker_offset: 0,
                        padding: 2,
                        start: 1,
                        tight: options.tight_lists,
                        delimiter: comrak::nodes::ListDelimType::Period,
                        bullet_char: b'-',
                        is_task_list: false,
                    })
                    .into(),
                );
                for item in items {
                    let li = arena.alloc(
                        NodeValue::Item(NodeList {
                            list_type: comrak::nodes::ListType::Bullet,
                            marker_offset: 0,
                            padding: 2,
                            start: 1,
                            tight: options.tight_lists,
                            delimiter: comrak::nodes::ListDelimType::Period,
                            bullet_char: b'-',
                            is_task_list: false,
                        })
                        .into(),
                    );
                    append_list_item_children(arena, li, &item.blocks, doc, options);
                    node.append(li);
                }
                Some(node)
            }
        },
        Block::DefinitionList { items, .. } => {
            let mut html = String::from("<dl>\n");
            for item in items {
                for term in &item.terms {
                    html.push_str("<dt>");
                    html.push_str(&escape_html_text(&blocks_to_text(term)));
                    html.push_str("</dt>\n");
                }
                for def in &item.definitions {
                    html.push_str("<dd>");
                    html.push_str(&escape_html_text(&blocks_to_text(def)));
                    html.push_str("</dd>\n");
                }
            }
            html.push_str("</dl>");
            Some(
                arena.alloc(
                    NodeValue::HtmlBlock(comrak::nodes::NodeHtmlBlock {
                        block_type: 6,
                        literal: html,
                    })
                    .into(),
                ),
            )
        }
        Block::Table { .. } => {
            if let Some(table) = extract_simple_table(block) {
                let mut nonempty = table.header.iter().filter(|c| !c.is_empty()).count();
                nonempty += table
                    .body
                    .iter()
                    .flat_map(|row| row.iter())
                    .filter(|cell| !cell.is_empty())
                    .count();

                let table_node = arena.alloc(
                    NodeValue::Table(Box::new(comrak::nodes::NodeTable {
                        alignments: table.alignments.clone(),
                        num_columns: table.header.len(),
                        num_rows: 1 + table.body.len(),
                        num_nonempty_cells: nonempty,
                    }))
                    .into(),
                );

                let header_row = arena.alloc(NodeValue::TableRow(true).into());
                for text in &table.header {
                    let cell = arena.alloc(NodeValue::TableCell.into());
                    push_text(arena, cell, text);
                    header_row.append(cell);
                }
                table_node.append(header_row);

                for row in &table.body {
                    let body_row = arena.alloc(NodeValue::TableRow(false).into());
                    for text in row {
                        let cell = arena.alloc(NodeValue::TableCell.into());
                        push_text(arena, cell, text);
                        body_row.append(cell);
                    }
                    table_node.append(body_row);
                }

                return Some(table_node);
            }

            let html = render_table_html(block)?;
            let node: &'a AstNode<'a> = arena.alloc(
                NodeValue::HtmlBlock(comrak::nodes::NodeHtmlBlock {
                    block_type: 6,
                    literal: html,
                })
                .into(),
            );
            Some(node)
        }
        Block::Figure {
            content, caption, ..
        } => {
            let mut html = String::from("<figure>\n");
            let content_text = blocks_to_text(content);
            if !content_text.is_empty() {
                html.push_str(&escape_html_text(&content_text));
                html.push('\n');
            }
            if let Some(cap) = caption {
                html.push_str("<figcaption>");
                html.push_str(&escape_html_text(&blocks_to_text(cap)));
                html.push_str("</figcaption>\n");
            }
            html.push_str("</figure>");
            Some(
                arena.alloc(
                    NodeValue::HtmlBlock(comrak::nodes::NodeHtmlBlock {
                        block_type: 6,
                        literal: html,
                    })
                    .into(),
                ),
            )
        }
        Block::Admonition {
            kind,
            title,
            blocks,
            ..
        } => {
            let alert_type = match kind {
                AdmonitionKind::Note => AlertType::Note,
                AdmonitionKind::Tip => AlertType::Tip,
                AdmonitionKind::Warning => AlertType::Warning,
                AdmonitionKind::Danger => AlertType::Caution,
                AdmonitionKind::Info => AlertType::Important,
                AdmonitionKind::Custom(_) => AlertType::Note,
            };
            let title_text = title.as_ref().map(|t| inlines_text(t));
            let output_title = title_text.and_then(|t| {
                if t == kind.default_title() {
                    None
                } else {
                    Some(t)
                }
            });
            let node = arena.alloc(
                NodeValue::Alert(Box::new(NodeAlert {
                    alert_type,
                    title: output_title,
                    multiline: false,
                    fence_length: 3,
                    fence_offset: 0,
                }))
                .into(),
            );
            append_block_children(arena, node, blocks, doc, options);
            Some(node)
        }
        Block::Details {
            summary,
            blocks,
            open,
            ..
        } => {
            let open_attr = if *open { " open" } else { "" };
            let mut html = format!("<details{}>\n", open_attr);
            if let Some(s) = summary {
                html.push_str("<summary>");
                html.push_str(&escape_html_text(&inlines_text(s)));
                html.push_str("</summary>\n");
            }
            html.push_str(&escape_html_text(&blocks_to_text(blocks)));
            html.push_str("\n</details>");
            Some(
                arena.alloc(
                    NodeValue::HtmlBlock(comrak::nodes::NodeHtmlBlock {
                        block_type: 6,
                        literal: html,
                    })
                    .into(),
                ),
            )
        }
        Block::MathBlock { math, .. } | Block::SvgBlock { svg: math, .. } => {
            render_payload_block(arena, math, doc, options)
        }
        Block::UnknownBlock {
            source,
            note,
            children,
            ..
        } => {
            if let Some(src) = source {
                return Some(
                    arena.alloc(
                        NodeValue::HtmlBlock(comrak::nodes::NodeHtmlBlock {
                            block_type: 6,
                            literal: src.clone(),
                        })
                        .into(),
                    ),
                );
            }
            if children.is_empty() {
                let note = note.as_ref()?;
                let node: &'a AstNode<'a> = arena.alloc(NodeValue::Paragraph.into());
                push_text(arena, node, note);
                return Some(node);
            }
            let node = arena.alloc(NodeValue::BlockQuote.into());
            for child in children {
                match child {
                    UnknownChild::Block(b) => {
                        if let Some(child_node) = block_to_ast(arena, b, doc, options) {
                            node.append(child_node);
                        }
                    }
                    UnknownChild::Inline(i) => {
                        let para = arena.alloc(NodeValue::Paragraph.into());
                        let one = std::slice::from_ref(i);
                        push_inline_seq(arena, para, one, doc, options);
                        node.append(para);
                    }
                }
            }
            Some(node)
        }
        Block::RawBlock { html, .. } => Some(
            arena.alloc(
                NodeValue::HtmlBlock(comrak::nodes::NodeHtmlBlock {
                    block_type: 6,
                    literal: html.clone(),
                })
                .into(),
            ),
        ),
    }
}

fn render_footnote_def<'a>(
    arena: &'a Arena<'a>,
    id: &FootnoteId,
    def: &FootnoteDef,
    doc: &Document,
    options: &MarkdownRenderOptions<'_>,
) -> &'a AstNode<'a> {
    let node = arena.alloc(
        NodeValue::FootnoteDefinition(NodeFootnoteDefinition {
            name: format!("fn:{}", id.0),
            total_references: 0,
        })
        .into(),
    );
    append_block_children(arena, node, &def.blocks, doc, options);
    node
}

pub fn document_to_markdown(doc: &Document) -> Result<String> {
    document_to_markdown_with_options(doc, &MarkdownRenderOptions::default())
}

pub fn document_to_markdown_with_options(
    doc: &Document,
    options: &MarkdownRenderOptions<'_>,
) -> Result<String> {
    let arena = Arena::new();
    let root = arena.alloc(NodeValue::Document.into());

    for block in &doc.blocks {
        if let Some(node) = block_to_ast(&arena, block, doc, options) {
            root.append(node);
        }
    }

    for (id, def) in &doc.footnotes {
        root.append(render_footnote_def(&arena, id, def, doc, options));
    }

    let mut fmt_options = Options::default();
    fmt_options.extension.strikethrough = true;
    fmt_options.extension.table = true;
    fmt_options.extension.tasklist = true;
    fmt_options.extension.footnotes = true;
    fmt_options.extension.alerts = true;
    let mut out = String::new();
    format_commonmark(root, &fmt_options, &mut out)?;
    Ok(out.trim().to_string())
}
