//! Block-level parsing and dispatch.

use anyhow::Result;
use scraper::{ElementRef, Node};
use std::collections::BTreeMap;

use typub_ir::{
    AdmonitionKind, AnchorId, Block, BlockAttrs, Inline, MathPayload, RenderedMath, SvgPayload,
    TableCell, TableCellKind, TableRow, TableSection, TableSectionKind, Url,
};

use super::{
    ParseCtx, class_has_keyword, detect_gfm_alert, is_admonition_wrapper, normalize_text_content,
    parse_block_attrs, parse_footnote_container, parse_header_scope, parse_image_attrs,
    parse_math_source, parse_text_align_from_style,
};
use super::{code, inline, lists, spec};

pub(crate) fn parse_element(
    el: ElementRef,
    out: &mut Vec<Block>,
    ctx: &mut ParseCtx,
) -> Result<()> {
    if parse_footnote_container(el, ctx)? {
        return Ok(());
    }

    let tag = el.value().name();
    let attrs = parse_block_attrs(&el);

    match tag {
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => parse_heading(el, out, attrs, tag, ctx),
        "p" => parse_paragraph(el, out, attrs, ctx),
        "pre" => out.push(code::parse_pre_block(el, attrs)),
        "code" => out.extend(code::parse_standalone_code(el, attrs)),
        "ul" => out.push(lists::parse_list(el, false, attrs, ctx)?),
        "ol" => out.push(lists::parse_list(el, true, attrs, ctx)?),
        "dl" => out.extend(lists::parse_definition_list(el, attrs, ctx)?),
        "table" => out.push(parse_table(el, attrs, ctx)?),
        "img" => parse_standalone_image(el, out, attrs, ctx),
        "svg" => out.push(Block::SvgBlock {
            svg: SvgPayload {
                src: None,
                rendered: Some(RenderedMath::Svg(el.html())),
                id: None,
            },
            attrs,
        }),
        "hr" => out.push(Block::Divider { attrs }),
        "blockquote" => out.push(parse_quote_or_gfm_admonition(el, attrs, ctx)?),
        "details" => out.push(parse_details(el, attrs, ctx)?),
        "figure" => out.push(parse_figure(el, attrs, ctx)?),
        "div" | "section" | "article" | "main" | "header" | "footer" | "body" | "html" => {
            if let Some(block) = parse_svg_block_wrapper(el, attrs.clone()) {
                out.push(block);
            } else if is_admonition_wrapper(el) {
                out.push(parse_class_admonition(el, attrs, ctx)?);
            } else {
                parse_container(el, out, ctx)?;
            }
        }
        _ => out.push(Block::UnknownBlock {
            tag: tag.to_string(),
            attrs,
            children: Vec::new(),
            data: BTreeMap::new(),
            note: Some("unsupported block element".to_string()),
            source: Some(el.html()),
        }),
    }

    Ok(())
}

fn parse_heading(
    el: ElementRef,
    out: &mut Vec<Block>,
    mut attrs: BlockAttrs,
    tag: &str,
    ctx: &mut ParseCtx,
) {
    let level_num = tag
        .strip_prefix('h')
        .and_then(|s| s.parse::<u8>().ok())
        .unwrap_or(1);
    let content = inline::parse_inline_children(el, ctx);
    if !content.is_empty()
        && let Ok(level) = typub_ir::HeadingLevel::new(level_num)
    {
        attrs.passthrough.remove("id");
        out.push(Block::Heading {
            level,
            id: el.value().attr("id").map(|s| AnchorId(s.to_string())),
            content,
            attrs,
        });
    }
}

fn parse_paragraph(el: ElementRef, out: &mut Vec<Block>, attrs: BlockAttrs, ctx: &mut ParseCtx) {
    let content = inline::parse_inline_children(el, ctx);
    if !content.is_empty() {
        out.push(Block::Paragraph { content, attrs });
    }
}

fn parse_standalone_image(
    el: ElementRef,
    out: &mut Vec<Block>,
    attrs: BlockAttrs,
    ctx: &mut ParseCtx,
) {
    let Some(src) = el.value().attr("src") else {
        out.push(Block::UnknownBlock {
            tag: "img".to_string(),
            attrs,
            children: Vec::new(),
            data: BTreeMap::new(),
            note: Some("missing src attribute".to_string()),
            source: Some(el.html()),
        });
        return;
    };

    let width = el.value().attr("width").and_then(|s| s.parse().ok());
    let height = el.value().attr("height").and_then(|s| s.parse().ok());
    let Some(asset) = ctx.register_image(src, width, height) else {
        out.push(Block::UnknownBlock {
            tag: "img".to_string(),
            attrs,
            children: Vec::new(),
            data: BTreeMap::new(),
            note: Some("invalid image source".to_string()),
            source: Some(el.html()),
        });
        return;
    };

    out.push(Block::Paragraph {
        content: vec![Inline::Image {
            asset,
            alt: el.value().attr("alt").unwrap_or_default().to_string(),
            title: el.value().attr("title").map(str::to_string),
            attrs: parse_image_attrs(&el, width, height),
        }],
        attrs: BlockAttrs::default(),
    });
}

fn parse_quote_or_gfm_admonition(
    el: ElementRef,
    attrs: BlockAttrs,
    ctx: &mut ParseCtx,
) -> Result<Block> {
    if let Some((kind, prefix)) = detect_gfm_alert(el.text().collect::<String>().trim()) {
        let mut title = None;
        let mut blocks = Vec::new();
        let mut first_element = true;

        for child in el.children() {
            if let Node::Element(_) = child.value()
                && let Some(child_el) = ElementRef::wrap(child)
            {
                if first_element && child_el.value().name() == "p" {
                    let p_text = child_el.text().collect::<String>();
                    if let Some(rest) = p_text.trim().strip_prefix(prefix) {
                        let trimmed = rest.trim();
                        if !trimmed.is_empty() {
                            let mut lines = trimmed.lines();
                            let first = lines.next().unwrap_or("").trim();
                            if !first.is_empty() {
                                title = Some(vec![Inline::Text(first.to_string())]);
                            }
                            let remaining = lines.collect::<Vec<_>>().join(" ");
                            if !remaining.trim().is_empty() {
                                blocks.push(Block::Paragraph {
                                    content: vec![Inline::Text(remaining.trim().to_string())],
                                    attrs: BlockAttrs::default(),
                                });
                            }
                        }
                        first_element = false;
                        continue;
                    }
                }
                first_element = false;
                parse_element(child_el, &mut blocks, ctx)?;
            }
        }

        if title.is_none() {
            title = Some(vec![Inline::Text(kind.default_title().to_string())]);
        }

        return Ok(Block::Admonition {
            kind,
            title,
            blocks,
            attrs,
        });
    }

    let mut blocks = Vec::new();
    let mut direct_text = String::new();
    for child in el.children() {
        match child.value() {
            Node::Element(_) => {
                if !direct_text.trim().is_empty() {
                    blocks.push(Block::Paragraph {
                        content: vec![Inline::Text(direct_text.trim().to_string())],
                        attrs: BlockAttrs::default(),
                    });
                    direct_text.clear();
                }
                if let Some(child_el) = ElementRef::wrap(child) {
                    parse_element(child_el, &mut blocks, ctx)?;
                }
            }
            Node::Text(t) => direct_text.push_str(t),
            _ => {}
        }
    }
    if !direct_text.trim().is_empty() {
        blocks.push(Block::Paragraph {
            content: vec![Inline::Text(direct_text.trim().to_string())],
            attrs: BlockAttrs::default(),
        });
    }

    Ok(Block::Quote {
        blocks,
        cite: el.value().attr("cite").map(|s| Url(s.to_string())),
        attrs,
    })
}

fn parse_details(el: ElementRef, attrs: BlockAttrs, ctx: &mut ParseCtx) -> Result<Block> {
    let mut summary = None;
    let mut blocks = Vec::new();
    let open = el.value().attr("open").is_some();

    for child in el.children() {
        if let Node::Element(_) = child.value()
            && let Some(child_el) = ElementRef::wrap(child)
        {
            if child_el.value().name() == "summary" && summary.is_none() {
                let s = inline::parse_inline_children(child_el, ctx);
                if !s.is_empty() {
                    summary = Some(s);
                }
            } else {
                parse_element(child_el, &mut blocks, ctx)?;
            }
        }
    }

    Ok(Block::Details {
        summary,
        blocks,
        open,
        attrs,
    })
}

fn parse_figure(el: ElementRef, attrs: BlockAttrs, ctx: &mut ParseCtx) -> Result<Block> {
    let mut content = Vec::new();
    let mut caption = None;

    for child in el.children() {
        if let Node::Element(_) = child.value()
            && let Some(child_el) = ElementRef::wrap(child)
        {
            if child_el.value().name() == "figcaption" {
                let c = parse_element_as_blocks(child_el, ctx)?;
                if !c.is_empty() {
                    caption = Some(c);
                }
            } else {
                parse_element(child_el, &mut content, ctx)?;
            }
        }
    }

    Ok(Block::Figure {
        content,
        caption,
        attrs,
    })
}

fn parse_svg_block_wrapper(el: ElementRef, attrs: BlockAttrs) -> Option<Block> {
    if el.value().name() != "div" {
        return None;
    }
    let class = el.value().attr("class")?;
    if !class.contains("typst-svg-block") || !el.inner_html().contains("<svg") {
        return None;
    }

    let src = parse_math_source(el);
    let math = MathPayload {
        src,
        rendered: Some(RenderedMath::Svg(el.inner_html())),
        id: None,
    };
    Some(Block::MathBlock { math, attrs })
}

fn parse_class_admonition(el: ElementRef, attrs: BlockAttrs, ctx: &mut ParseCtx) -> Result<Block> {
    let class = el.value().attr("class").unwrap_or_default();
    let kind = if class_has_keyword(class, "warning") {
        AdmonitionKind::Warning
    } else if class_has_keyword(class, "danger") || class_has_keyword(class, "error") {
        AdmonitionKind::Danger
    } else if class_has_keyword(class, "tip") {
        AdmonitionKind::Tip
    } else if class_has_keyword(class, "info") {
        AdmonitionKind::Info
    } else {
        AdmonitionKind::Note
    };

    let mut title = None;
    let mut blocks = Vec::new();

    for child in el.children() {
        if let Node::Element(_) = child.value()
            && let Some(child_el) = ElementRef::wrap(child)
        {
            let tag = child_el.value().name();
            let child_class = child_el.value().attr("class").unwrap_or_default();
            let is_title = child_class.contains("admonition-title")
                || child_class.contains("callout-title")
                || matches!(tag, "header" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6");
            if is_title {
                let t = inline::parse_inline_children(child_el, ctx);
                if !t.is_empty() {
                    title = Some(t);
                }
                continue;
            }
            parse_element(child_el, &mut blocks, ctx)?;
        }
    }

    if title.is_none() {
        title = Some(vec![Inline::Text(kind.default_title().to_string())]);
    }

    Ok(Block::Admonition {
        kind,
        title,
        blocks,
        attrs,
    })
}

fn parse_container(el: ElementRef, out: &mut Vec<Block>, ctx: &mut ParseCtx) -> Result<()> {
    parse_flow_children(el, out, ctx)
}

pub(crate) fn parse_element_as_blocks(el: ElementRef, ctx: &mut ParseCtx) -> Result<Vec<Block>> {
    let mut blocks = parse_child_blocks(el, ctx)?;
    if blocks.is_empty() {
        let content = inline::parse_inline_children(el, ctx);
        if !content.is_empty() {
            blocks.push(Block::Paragraph {
                content,
                attrs: BlockAttrs::default(),
            });
        }
    }
    Ok(blocks)
}

pub(crate) fn parse_child_blocks(parent: ElementRef, ctx: &mut ParseCtx) -> Result<Vec<Block>> {
    let mut blocks = Vec::new();
    parse_flow_children(parent, &mut blocks, ctx)?;
    Ok(blocks)
}

fn parse_flow_children(parent: ElementRef, out: &mut Vec<Block>, ctx: &mut ParseCtx) -> Result<()> {
    let mut inline_buf = Vec::new();

    for child in parent.children() {
        match child.value() {
            Node::Text(t) => {
                if let Some(text) = normalize_text_content(t)
                    && !text.is_empty()
                {
                    inline_buf.push(Inline::Text(text));
                }
            }
            Node::Element(e) => {
                if let Some(el) = ElementRef::wrap(child) {
                    if spec::is_phrasing_content_tag(e.name()) {
                        inline_buf.extend(inline::parse_inline_element(el, ctx));
                    } else {
                        flush_inline_paragraph(out, &mut inline_buf);
                        parse_element(el, out, ctx)?;
                    }
                }
            }
            _ => {}
        }
    }

    flush_inline_paragraph(out, &mut inline_buf);
    Ok(())
}

fn flush_inline_paragraph(out: &mut Vec<Block>, inline_buf: &mut Vec<Inline>) {
    if inline_buf.is_empty() {
        return;
    }

    out.push(Block::Paragraph {
        content: std::mem::take(inline_buf),
        attrs: BlockAttrs::default(),
    });
}

fn parse_table(el: ElementRef, attrs: BlockAttrs, ctx: &mut ParseCtx) -> Result<Block> {
    let mut caption = None;
    let mut sections = Vec::new();

    for child in el.children() {
        if let Node::Element(e) = child.value()
            && let Some(child_el) = ElementRef::wrap(child)
        {
            match e.name() {
                "caption" => {
                    let c = parse_element_as_blocks(child_el, ctx)?;
                    if !c.is_empty() {
                        caption = Some(c);
                    }
                }
                "thead" => {
                    sections.push(parse_table_section(child_el, TableSectionKind::Head, ctx)?)
                }
                "tbody" => {
                    sections.push(parse_table_section(child_el, TableSectionKind::Body, ctx)?)
                }
                "tfoot" => {
                    sections.push(parse_table_section(child_el, TableSectionKind::Foot, ctx)?)
                }
                "tr" => {}
                _ => {}
            }
        }
    }

    if sections.is_empty() {
        let mut rows = Vec::new();
        for child in el.children() {
            if let Node::Element(e) = child.value()
                && e.name() == "tr"
                && let Some(tr) = ElementRef::wrap(child)
            {
                let row = parse_table_row(tr, ctx)?;
                if !row.cells.is_empty() {
                    rows.push(row);
                }
            }
        }

        if !rows.is_empty() {
            let head_is_first = rows[0]
                .cells
                .iter()
                .any(|cell| matches!(cell.kind, TableCellKind::Header));
            if head_is_first {
                sections.push(TableSection {
                    kind: TableSectionKind::Head,
                    rows: vec![rows.remove(0)],
                    attrs: BlockAttrs::default(),
                });
            }
            if !rows.is_empty() {
                sections.push(TableSection {
                    kind: TableSectionKind::Body,
                    rows,
                    attrs: BlockAttrs::default(),
                });
            }
        }
    }
    if !sections
        .iter()
        .any(|s| matches!(s.kind, TableSectionKind::Head))
        && let Some(body_idx) = sections
            .iter()
            .position(|s| matches!(s.kind, TableSectionKind::Body) && !s.rows.is_empty())
    {
        let body = &mut sections[body_idx];
        let first_is_head = body.rows[0]
            .cells
            .iter()
            .any(|cell| matches!(cell.kind, TableCellKind::Header));
        if first_is_head {
            let head_row = body.rows.remove(0);
            sections.insert(
                body_idx,
                TableSection {
                    kind: TableSectionKind::Head,
                    rows: vec![head_row],
                    attrs: BlockAttrs::default(),
                },
            );
        }
    }

    Ok(Block::Table {
        caption,
        sections,
        attrs,
    })
}

fn parse_table_section(
    section: ElementRef,
    kind: TableSectionKind,
    ctx: &mut ParseCtx,
) -> Result<TableSection> {
    let mut rows = Vec::new();
    for child in section.children() {
        if let Node::Element(e) = child.value()
            && e.name() == "tr"
            && let Some(tr) = ElementRef::wrap(child)
        {
            let row = parse_table_row(tr, ctx)?;
            if !row.cells.is_empty() {
                rows.push(row);
            }
        }
    }

    Ok(TableSection {
        kind,
        rows,
        attrs: parse_block_attrs(&section),
    })
}

fn parse_table_row(tr: ElementRef, ctx: &mut ParseCtx) -> Result<TableRow> {
    let mut cells = Vec::new();
    for child in tr.children() {
        if let Node::Element(e) = child.value()
            && (e.name() == "th" || e.name() == "td")
            && let Some(cell_el) = ElementRef::wrap(child)
        {
            cells.push(parse_table_cell(cell_el, ctx)?);
        }
    }

    Ok(TableRow {
        cells,
        attrs: parse_block_attrs(&tr),
    })
}

fn parse_table_cell(cell: ElementRef, ctx: &mut ParseCtx) -> Result<TableCell> {
    let kind = if cell.value().name() == "th" {
        TableCellKind::Header
    } else {
        TableCellKind::Data
    };

    let mut blocks = parse_child_blocks(cell, ctx)?;
    if blocks.is_empty() {
        let content = inline::parse_inline_children(cell, ctx);
        if !content.is_empty() {
            blocks.push(Block::Paragraph {
                content,
                attrs: BlockAttrs::default(),
            });
        }
    }

    Ok(TableCell {
        kind,
        blocks,
        colspan: cell
            .value()
            .attr("colspan")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1),
        rowspan: cell
            .value()
            .attr("rowspan")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1),
        scope: cell.value().attr("scope").and_then(parse_header_scope),
        align: cell
            .value()
            .attr("style")
            .and_then(parse_text_align_from_style),
        attrs: parse_block_attrs(&cell),
    })
}
