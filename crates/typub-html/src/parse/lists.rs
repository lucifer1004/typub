//! List parsing (bullet/numbered/task + definition list).

use anyhow::Result;
use scraper::{ElementRef, Node};

use typub_ir::{
    Block, BlockAttrs, DefinitionItem, FlowListItem, FlowListItemMarker, Inline, List, ListKind,
    TaskListItem,
};

use super::{ParseCtx, normalize_text_content, parse_ordered_marker};
use super::{blocks, inline, spec};

pub(crate) fn parse_list(
    el: ElementRef,
    ordered: bool,
    attrs: BlockAttrs,
    ctx: &mut ParseCtx,
) -> Result<Block> {
    if !ordered && is_task_list(el) {
        let mut items = Vec::new();
        for child in el.children() {
            if let Node::Element(e) = child.value()
                && e.name() == "li"
                && let Some(li) = ElementRef::wrap(child)
                && let Some(item) = parse_task_list_item(li, ctx)?
            {
                items.push(item);
            }
        }

        return Ok(Block::List {
            list: List {
                kind: ListKind::Task { items },
            },
            attrs,
        });
    }

    let mut items = Vec::new();
    for child in el.children() {
        if let Node::Element(e) = child.value()
            && e.name() == "li"
            && let Some(li) = ElementRef::wrap(child)
            && let Some(item) = parse_flow_list_item(li, ordered, ctx)?
        {
            items.push(item);
        }
    }

    let list = if ordered {
        let start = el
            .value()
            .attr("start")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1);
        let reversed = el.value().attr("reversed").is_some();
        let marker = parse_ordered_marker(el.value().attr("type"));
        List {
            kind: ListKind::Numbered {
                start,
                reversed,
                marker,
                items,
            },
        }
    } else {
        List {
            kind: ListKind::Bullet { items },
        }
    };

    Ok(Block::List { list, attrs })
}

pub(crate) fn parse_definition_list(
    el: ElementRef,
    attrs: BlockAttrs,
    ctx: &mut ParseCtx,
) -> Result<Vec<Block>> {
    let mut dts = Vec::new();
    let mut dds = Vec::new();

    for child in el.children() {
        if let Node::Element(e) = child.value() {
            if e.name() == "dt" {
                if let Some(dt) = ElementRef::wrap(child) {
                    dts.push(dt);
                }
            } else if e.name() == "dd"
                && let Some(dd) = ElementRef::wrap(child)
            {
                dds.push(dd);
            }
        }
    }

    let mut items = Vec::new();
    let pairs = dts.len().max(dds.len());
    for i in 0..pairs {
        let mut terms = Vec::new();
        let mut definitions = Vec::new();

        if let Some(dt) = dts.get(i) {
            let term_blocks = blocks::parse_element_as_blocks(*dt, ctx)?;
            if !term_blocks.is_empty() {
                terms.push(term_blocks);
            }
        }

        if let Some(dd) = dds.get(i) {
            let def_blocks = blocks::parse_element_as_blocks(*dd, ctx)?;
            if !def_blocks.is_empty() {
                definitions.push(def_blocks);
            }
        }

        if !terms.is_empty() || !definitions.is_empty() {
            items.push(DefinitionItem { terms, definitions });
        }
    }

    if items.is_empty() {
        Ok(Vec::new())
    } else {
        Ok(vec![Block::DefinitionList { items, attrs }])
    }
}

fn parse_flow_list_item(
    li: ElementRef,
    ordered: bool,
    ctx: &mut ParseCtx,
) -> Result<Option<FlowListItem>> {
    let blocks = parse_list_item_blocks(li, ctx, false, &mut false)?;
    if blocks.is_empty() {
        return Ok(None);
    }

    let marker = if ordered {
        li.value()
            .attr("value")
            .and_then(|s| s.parse::<u32>().ok())
            .map(FlowListItemMarker::Number)
    } else {
        None
    };

    Ok(Some(FlowListItem { marker, blocks }))
}

fn parse_task_list_item(li: ElementRef, ctx: &mut ParseCtx) -> Result<Option<TaskListItem>> {
    let mut checked = false;
    let blocks = parse_list_item_blocks(li, ctx, true, &mut checked)?;
    if blocks.is_empty() {
        return Ok(None);
    }
    Ok(Some(TaskListItem { checked, blocks }))
}

fn parse_list_item_blocks(
    li: ElementRef,
    ctx: &mut ParseCtx,
    task_mode: bool,
    checked: &mut bool,
) -> Result<Vec<Block>> {
    let mut blocks = Vec::new();
    let mut inline_buf = Vec::new();
    let mut marker_consumed = false;

    for child in li.children() {
        match child.value() {
            Node::Text(text) => {
                let mut text_content = text.text.to_string();
                if task_mode
                    && !marker_consumed
                    && let Some((is_checked, rest)) = strip_task_text_marker(&text_content)
                {
                    *checked = is_checked;
                    text_content = rest.to_string();
                    marker_consumed = true;
                }
                if let Some(t) = normalize_text_content(&text_content)
                    && !t.trim().is_empty()
                {
                    inline_buf.push(Inline::Text(t));
                }
            }
            Node::Element(el) => {
                let tag = el.name();

                if task_mode && tag == "input" {
                    if el.attr("type") == Some("checkbox") {
                        *checked = el.attr("checked").is_some();
                        marker_consumed = true;
                    }
                    continue;
                }

                if !spec::is_phrasing_content_tag(tag) {
                    flush_inline_as_paragraph(&mut blocks, &mut inline_buf);

                    // Special handling for <p> in task mode: strip task markers from paragraph text
                    if task_mode
                        && tag == "p"
                        && !marker_consumed
                        && let Some(p_el) = ElementRef::wrap(child)
                        && let Some(stripped_blocks) = parse_task_paragraph(p_el, ctx, checked)?
                    {
                        blocks.extend(stripped_blocks);
                        marker_consumed = true;
                        continue;
                    }

                    if let Some(nested) = ElementRef::wrap(child) {
                        blocks::parse_element(nested, &mut blocks, ctx)?;
                    }
                    continue;
                }

                if let Some(el_ref) = ElementRef::wrap(child) {
                    inline_buf.extend(inline::parse_inline_element(el_ref, ctx));
                }
            }
            _ => {}
        }
    }

    flush_inline_as_paragraph(&mut blocks, &mut inline_buf);
    Ok(blocks)
}

/// Parse a <p> element in task mode, stripping task markers from its text content
fn parse_task_paragraph(
    el: ElementRef,
    ctx: &mut ParseCtx,
    checked: &mut bool,
) -> Result<Option<Vec<Block>>> {
    let mut inline_buf = Vec::new();

    for child in el.children() {
        match child.value() {
            Node::Text(text) => {
                let mut text_content = text.text.to_string();
                if let Some((is_checked, rest)) = strip_task_text_marker(&text_content) {
                    *checked = is_checked;
                    text_content = rest.to_string();
                }
                if let Some(t) = normalize_text_content(&text_content)
                    && !t.trim().is_empty()
                {
                    inline_buf.push(Inline::Text(t));
                }
            }
            Node::Element(el_ref) => {
                let tag = el_ref.name();
                if spec::is_phrasing_content_tag(tag)
                    && let Some(el_wrap) = ElementRef::wrap(child)
                {
                    inline_buf.extend(inline::parse_inline_element(el_wrap, ctx));
                }
            }
            _ => {}
        }
    }

    if inline_buf.is_empty() {
        return Ok(None);
    }

    // Check if there's actual non-whitespace content
    if inline_buf
        .iter()
        .any(|n| !matches!(n, Inline::Text(t) if t.trim().is_empty()))
    {
        Ok(Some(vec![Block::Paragraph {
            content: inline_buf,
            attrs: BlockAttrs::default(),
        }]))
    } else {
        Ok(None)
    }
}

fn flush_inline_as_paragraph(blocks: &mut Vec<Block>, inline_buf: &mut Vec<Inline>) {
    if inline_buf.is_empty() {
        return;
    }
    let content = std::mem::take(inline_buf);
    if content
        .iter()
        .any(|n| !matches!(n, Inline::Text(t) if t.trim().is_empty()))
    {
        blocks.push(Block::Paragraph {
            content,
            attrs: BlockAttrs::default(),
        });
    }
}

fn is_task_list(el: ElementRef) -> bool {
    for child in el.children() {
        if let Node::Element(e) = child.value()
            && e.name() == "li"
            && let Some(li) = ElementRef::wrap(child)
        {
            let text = li.text().collect::<String>();
            if strip_task_text_marker(&text).is_some() {
                return true;
            }
            for li_child in li.children() {
                if let Node::Element(input_el) = li_child.value()
                    && input_el.name() == "input"
                    && input_el.attr("type") == Some("checkbox")
                {
                    return true;
                }
            }
        }
    }
    false
}

fn strip_task_text_marker(text: &str) -> Option<(bool, &str)> {
    let trimmed = text.trim_start();
    if let Some(rest) = trimmed.strip_prefix("[x]") {
        return Some((true, rest));
    }
    if let Some(rest) = trimmed.strip_prefix("[X]") {
        return Some((true, rest));
    }
    if let Some(rest) = trimmed.strip_prefix("[ ]") {
        return Some((false, rest));
    }
    if let Some(rest) = trimmed.strip_prefix('☑') {
        return Some((true, rest));
    }
    if let Some(rest) = trimmed.strip_prefix('☒') {
        return Some((true, rest));
    }
    if let Some(rest) = trimmed.strip_prefix('☐') {
        return Some((false, rest));
    }
    None
}
