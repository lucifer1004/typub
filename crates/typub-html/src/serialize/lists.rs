use super::*;

pub(super) fn serialize_table_cell(ctx: &SerializeCtx<'_>, cell: &TableCell, out: &mut String) {
    let tag = match cell.kind {
        TableCellKind::Header => "th",
        TableCellKind::Data => "td",
    };

    let mut extra = Vec::new();
    if cell.colspan > 1 {
        extra.push(("colspan", cell.colspan.to_string()));
    }
    if cell.rowspan > 1 {
        extra.push(("rowspan", cell.rowspan.to_string()));
    }
    if let Some(scope) = cell.scope {
        let scope_value = match scope {
            TableHeaderScope::Row => "row",
            TableHeaderScope::Col => "col",
            TableHeaderScope::RowGroup => "rowgroup",
            TableHeaderScope::ColGroup => "colgroup",
        };
        extra.push(("scope", scope_value.to_string()));
    }

    let mut attrs = cell.attrs.clone();
    if let Some(align) = cell.align {
        let css = format!("text-align:{}", text_align_css_value(align));
        attrs.style = Some(merge_style(attrs.style.as_deref(), Some(&css)));
    }

    let attr_str = block_attrs_to_html(&attrs, &extra, &[]);
    out.push_str(&format!("<{}{}>", tag, attr_str));
    serialize_blocks(ctx, &cell.blocks, out);
    out.push_str(&format!("</{}>", tag));
}

pub(super) fn serialize_list(
    ctx: &SerializeCtx<'_>,
    list: &List,
    attrs: &BlockAttrs,
    out: &mut String,
) {
    match &list.kind {
        ListKind::Bullet { items } => {
            let attr_str = block_attrs_to_html(attrs, &[], &[]);
            out.push_str(&format!("<ul{}>", attr_str));
            for item in items {
                if ctx.options.sibling_nested_lists {
                    serialize_flow_item_as_sibling(ctx, item, out);
                } else {
                    serialize_flow_item_nested(ctx, item, out);
                }
            }
            out.push_str("</ul>\n");
        }
        ListKind::Numbered {
            start,
            reversed,
            marker,
            items,
        } => {
            let mut extra = Vec::new();
            if *start > 1 {
                extra.push(("start", start.to_string()));
            }
            if *reversed {
                extra.push(("reversed", "reversed".to_string()));
            }
            if let Some(m) = marker {
                extra.push(("type", ordered_marker_value(*m).to_string()));
            }
            let attr_str = block_attrs_to_html(attrs, &extra, &[]);
            out.push_str(&format!("<ol{}>", attr_str));
            for item in items {
                if ctx.options.sibling_nested_lists {
                    serialize_flow_item_as_sibling(ctx, item, out);
                } else {
                    serialize_flow_item_nested(ctx, item, out);
                }
            }
            out.push_str("</ol>\n");
        }
        ListKind::Task { items } => {
            let mut classes = vec!["task-list".to_string()];
            classes.extend(attrs.classes.iter().cloned());
            let attr_str = attrs_to_html(
                &classes,
                attrs.style.as_deref(),
                &attrs.passthrough,
                &[],
                &["class"],
            );
            out.push_str(&format!("<ul{}>", attr_str));
            for item in items {
                if ctx.options.sibling_nested_lists {
                    serialize_task_item_as_sibling(ctx, item, out);
                } else {
                    serialize_task_item_nested(ctx, item, out);
                }
            }
            out.push_str("</ul>\n");
        }
        ListKind::Custom {
            kind,
            items,
            data: _,
        } => {
            let extra = vec![("data-list-kind", kind.as_str().to_string())];
            let attr_str = block_attrs_to_html(attrs, &extra, &[]);
            out.push_str(&format!("<ul{}>", attr_str));
            for item in items {
                out.push_str("<li>");
                serialize_list_item_content(ctx, &item.blocks, out, false);
                out.push_str("</li>");
            }
            out.push_str("</ul>\n");
        }
    }
}

fn serialize_flow_item_nested(ctx: &SerializeCtx<'_>, item: &FlowListItem, out: &mut String) {
    out.push_str(&format!("<li{}>", flow_list_item_attrs(item)));
    serialize_list_item_content(ctx, &item.blocks, out, ctx.options.li_span_wrap);
    out.push_str("</li>");
}

fn serialize_flow_item_as_sibling(ctx: &SerializeCtx<'_>, item: &FlowListItem, out: &mut String) {
    let (nested_lists, others) = split_list_blocks(&item.blocks);

    out.push_str(&format!("<li{}>", flow_list_item_attrs(item)));
    serialize_list_item_content(ctx, &others, out, ctx.options.li_span_wrap);
    out.push_str("</li>");

    for list in nested_lists {
        if let Block::List {
            list: nested_list,
            attrs,
        } = list
        {
            serialize_list(ctx, nested_list, attrs, out);
        }
    }
}

fn flow_list_item_attrs(item: &FlowListItem) -> String {
    match item.marker {
        Some(typub_ir::FlowListItemMarker::Number(n)) => {
            format!(r#" value="{}""#, n)
        }
        _ => String::new(),
    }
}

fn serialize_task_item_nested(ctx: &SerializeCtx<'_>, item: &TaskListItem, out: &mut String) {
    let checked_attr = if item.checked {
        r#" checked="checked""#
    } else {
        ""
    };
    out.push_str(&format!(
        r#"<li class="task-item"><input type="checkbox"{} disabled />"#,
        checked_attr
    ));
    serialize_list_item_content(ctx, &item.blocks, out, ctx.options.li_span_wrap);
    out.push_str("</li>");
}

fn serialize_task_item_as_sibling(ctx: &SerializeCtx<'_>, item: &TaskListItem, out: &mut String) {
    let (nested_lists, others) = split_list_blocks(&item.blocks);

    let checked_attr = if item.checked {
        r#" checked="checked""#
    } else {
        ""
    };
    out.push_str(&format!(
        r#"<li class="task-item"><input type="checkbox"{} disabled />"#,
        checked_attr
    ));
    serialize_list_item_content(ctx, &others, out, ctx.options.li_span_wrap);
    out.push_str("</li>");

    for list in nested_lists {
        if let Block::List {
            list: nested_list,
            attrs,
        } = list
        {
            serialize_list(ctx, nested_list, attrs, out);
        }
    }
}

fn split_list_blocks(blocks: &[Block]) -> (Vec<&Block>, Vec<Block>) {
    let mut nested_lists = Vec::new();
    let mut others = Vec::new();
    for block in blocks {
        if matches!(block, Block::List { .. }) {
            nested_lists.push(block);
        } else {
            others.push(block.clone());
        }
    }
    (nested_lists, others)
}

fn serialize_list_item_content(
    ctx: &SerializeCtx<'_>,
    blocks: &[Block],
    out: &mut String,
    li_span_wrap: bool,
) {
    if blocks.is_empty() {
        return;
    }

    if let Some(content) = single_plain_paragraph_inline(blocks) {
        let html = serialize_inlines(ctx, content);
        if li_span_wrap {
            out.push_str(&format!(r#"<span style="display:inline;">{}</span>"#, html));
        } else {
            out.push_str(&html);
        }
        return;
    }

    for block in blocks {
        serialize_block(ctx, block, out);
    }
}

fn single_plain_paragraph_inline(blocks: &[Block]) -> Option<&[Inline]> {
    if blocks.len() != 1 {
        return None;
    }
    match &blocks[0] {
        Block::Paragraph { content, attrs } if attrs == &BlockAttrs::default() => Some(content),
        _ => None,
    }
}
