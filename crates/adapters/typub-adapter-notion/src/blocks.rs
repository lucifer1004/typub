//! Notion block conversion (v2 prototype) — pure functions, no I/O.
//!
//! Converts v2 semantic `Document` into Notion API block JSON values.

use serde_json::{Map, Value, json};
use std::collections::HashMap;
use typub_adapters_core::typst_math_to_latex;
use typub_html::inlines_text;
use typub_ir::{
    AdmonitionKind, Asset, AssetRef, AssetSource, Block, Document, FlowListItem, Inline, ListKind,
    MathSource, RenderPayload, TableCell, TableSection, TableSectionKind, TaskListItem, TextStyle,
};

use crate::spec::{InlinePlacement, inline_capability};

type Annotations = Map<String, Value>;

fn apply_style(style: TextStyle, annotations: &mut Annotations) {
    match style {
        TextStyle::Bold => {
            annotations.insert("bold".to_string(), json!(true));
        }
        TextStyle::Italic => {
            annotations.insert("italic".to_string(), json!(true));
        }
        TextStyle::Strikethrough => {
            annotations.insert("strikethrough".to_string(), json!(true));
        }
        TextStyle::Underline => {
            annotations.insert("underline".to_string(), json!(true));
        }
        TextStyle::Mark => {
            annotations.insert("color".to_string(), json!("yellow_background"));
        }
        TextStyle::Superscript | TextStyle::Subscript | TextStyle::Kbd => {
            annotations.insert("code".to_string(), json!(true));
        }
    }
}

fn apply_styles(styles: &[TextStyle], annotations: &mut Annotations) {
    for style in styles {
        apply_style(*style, annotations);
    }
}

fn text_rich_text(content: &str, annotations: &Annotations, link: Option<&str>) -> Value {
    let text = if let Some(href) = link.filter(|href| is_supported_rich_text_link(href)) {
        json!({
            "content": content,
            "link": { "url": href }
        })
    } else {
        json!({
            "content": content
        })
    };

    if annotations.is_empty() {
        json!({
            "type": "text",
            "text": text
        })
    } else {
        json!({
            "type": "text",
            "text": text,
            "annotations": annotations
        })
    }
}

fn math_payload_to_latex(payload: &RenderPayload) -> Option<String> {
    match &payload.src {
        Some(MathSource::Latex(latex)) => Some(latex.clone()),
        Some(MathSource::Typst(typst)) => Some(typst_math_to_latex(typst)),
        Some(MathSource::Custom { src, .. }) => Some(src.clone()),
        None => None,
    }
}

fn resolve_asset_value(
    asset_ref: &AssetRef,
    document: &Document,
    asset_map: &HashMap<String, String>,
) -> Option<String> {
    let asset = document.assets.get(&asset_ref.0)?;

    let variants = match asset {
        Asset::Image(image) => &image.variants,
        Asset::Video(media) | Asset::Audio(media) => &media.variants,
        Asset::File(file) => &file.variants,
        Asset::Custom(custom) => &custom.variants,
    };

    if let Some(url) = variants
        .iter()
        .find(|variant| variant.name == "original")
        .map(|variant| variant.publish_url.0.clone())
        .or_else(|| {
            variants
                .first()
                .map(|variant| variant.publish_url.0.clone())
        })
    {
        return Some(url);
    }

    let source = match asset {
        Asset::Image(image) => &image.source,
        Asset::Video(media) | Asset::Audio(media) => &media.source,
        Asset::File(file) => &file.source,
        Asset::Custom(custom) => &custom.source,
    };

    match source {
        AssetSource::RemoteUrl { url } => Some(url.0.clone()),
        AssetSource::DataUri { uri } => Some(uri.clone()),
        AssetSource::LocalPath { path } => asset_map
            .get(path.as_str())
            .cloned()
            .or_else(|| Some(path.as_str().to_string())),
    }
}

fn is_external_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn is_supported_rich_text_link(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://") || value.starts_with("mailto:")
}

fn collect_inline_rich_text(
    inline: &Inline,
    _document: &Document,
    _asset_map: &HashMap<String, String>,
    annotations: &Annotations,
    active_link: Option<&str>,
    out: &mut Vec<Value>,
) {
    match inline {
        Inline::Text(text) => {
            out.push(text_rich_text(text, annotations, active_link));
        }
        Inline::Code(code) => {
            let mut code_annotations = annotations.clone();
            code_annotations.insert("code".to_string(), json!(true));
            out.push(text_rich_text(code, &code_annotations, active_link));
        }
        Inline::SoftBreak => {
            out.push(text_rich_text(" ", annotations, active_link));
        }
        Inline::HardBreak => {
            out.push(text_rich_text("\n", annotations, active_link));
        }
        Inline::Styled {
            styles, content, ..
        } => {
            let mut merged = annotations.clone();
            apply_styles(styles.styles(), &mut merged);
            for child in content {
                collect_inline_rich_text(child, _document, _asset_map, &merged, active_link, out);
            }
        }
        Inline::Link { content, href, .. } => {
            for child in content {
                collect_inline_rich_text(
                    child,
                    _document,
                    _asset_map,
                    annotations,
                    Some(&href.0),
                    out,
                );
            }
        }
        Inline::Image { alt, .. } => {
            if !alt.is_empty() {
                out.push(text_rich_text(alt, annotations, active_link));
            }
        }
        Inline::FootnoteRef(id) => {
            out.push(text_rich_text(
                &format!("[^{}]", id.0),
                annotations,
                active_link,
            ));
        }
        Inline::MathInline { math, .. } | Inline::SvgInline { svg: math, .. } => {
            if let Some(latex) = math_payload_to_latex(math) {
                out.push(json!({
                    "type": "equation",
                    "equation": { "expression": latex },
                    "plain_text": latex,
                }));
            }
        }
        Inline::UnknownInline { content, .. } => {
            for child in content {
                collect_inline_rich_text(
                    child,
                    _document,
                    _asset_map,
                    annotations,
                    active_link,
                    out,
                );
            }
        }
        Inline::RawInline { html, .. } => {
            if !html.is_empty() {
                out.push(text_rich_text(html, annotations, active_link));
            }
        }
    }
}

fn inlines_to_rich_text(
    inlines: &[Inline],
    document: &Document,
    asset_map: &HashMap<String, String>,
) -> Vec<Value> {
    let mut result = Vec::new();
    let annotations = Annotations::new();
    for inline in inlines {
        collect_inline_rich_text(inline, document, asset_map, &annotations, None, &mut result);
    }
    result
}

fn blocks_plain_text(blocks: &[Block]) -> String {
    let mut out = String::new();
    for block in blocks {
        match block {
            Block::Heading { content, .. } | Block::Paragraph { content, .. } => {
                out.push_str(&inlines_text(content));
            }
            Block::CodeBlock { code, .. } => out.push_str(code),
            Block::Quote { blocks, .. }
            | Block::Figure {
                content: blocks, ..
            }
            | Block::Admonition { blocks, .. }
            | Block::Details { blocks, .. } => out.push_str(&blocks_plain_text(blocks)),
            Block::RawBlock { html, .. } => out.push_str(html),
            _ => {}
        }
    }
    out
}

fn blocks_to_rich_text(
    blocks: &[Block],
    document: &Document,
    asset_map: &HashMap<String, String>,
) -> Vec<Value> {
    if let Some(Block::Paragraph { content, .. } | Block::Heading { content, .. }) = blocks.first()
    {
        return inlines_to_rich_text(content, document, asset_map);
    }

    let text = blocks_plain_text(blocks);
    if text.is_empty() {
        Vec::new()
    } else {
        vec![text_rich_text(&text, &Annotations::new(), None)]
    }
}

fn render_single_image_block(
    inline: &Inline,
    document: &Document,
    asset_map: &HashMap<String, String>,
    caption: Option<Vec<Value>>,
) -> Option<Value> {
    let Inline::Image { asset, alt, .. } = inline else {
        return None;
    };

    let src = resolve_asset_value(asset, document, asset_map)?;
    let mut image = if is_external_url(&src) {
        json!({
            "type": "image",
            "image": {
                "type": "external",
                "external": { "url": src }
            }
        })
    } else {
        json!({
            "type": "image",
            "image": {
                "type": "file_upload",
                "file_upload": { "id": src }
            }
        })
    };

    if let Some(caption_rich_text) = caption
        .filter(|rich| rich_text_has_visible_content(rich))
        .or_else(|| {
            let trimmed = alt.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(vec![text_rich_text(trimmed, &Annotations::new(), None)])
            }
        })
    {
        image["image"]["caption"] = json!(caption_rich_text);
    }

    Some(image)
}

fn render_single_math_block(inline: &Inline) -> Option<Value> {
    let payload = match inline {
        Inline::MathInline { math, .. } => math,
        Inline::SvgInline { svg, .. } => svg,
        _ => return None,
    };

    let latex = math_payload_to_latex(payload)?;
    Some(json!({
        "type": "equation",
        "equation": { "expression": latex }
    }))
}

fn paragraph_block_with_rich_text(rich_text: Vec<Value>) -> Value {
    json!({
        "type": "paragraph",
        "paragraph": { "rich_text": rich_text }
    })
}

fn rich_text_has_visible_content(rich_text: &[Value]) -> bool {
    rich_text
        .iter()
        .any(|value| match value.get("type").and_then(Value::as_str) {
            Some("text") => value
                .get("text")
                .and_then(|text| text.get("content"))
                .and_then(Value::as_str)
                .is_none_or(|content| !content.trim().is_empty()),
            Some(_) => true,
            None => true,
        })
}

fn flush_inline_run_as_paragraph(
    run: &mut Vec<Inline>,
    document: &Document,
    asset_map: &HashMap<String, String>,
    out: &mut Vec<Value>,
) {
    if run.is_empty() {
        return;
    }

    let rich_text = inlines_to_rich_text(run, document, asset_map);
    run.clear();

    if rich_text_has_visible_content(&rich_text) {
        out.push(paragraph_block_with_rich_text(rich_text));
    }
}

fn paragraph_blocks_by_spec(
    content: &[Inline],
    document: &Document,
    asset_map: &HashMap<String, String>,
) -> Vec<Value> {
    let mut out = Vec::new();
    let mut inline_run = Vec::new();

    for inline in content {
        match inline_capability(inline).placement {
            InlinePlacement::RichText => inline_run.push(inline.clone()),
            InlinePlacement::BlockOnly => {
                flush_inline_run_as_paragraph(&mut inline_run, document, asset_map, &mut out);

                if let Some(image) = render_single_image_block(inline, document, asset_map, None) {
                    out.push(image);
                } else {
                    // Keep rendering resilient when a block-only inline cannot be materialized.
                    // Fallback to rich_text conversion path.
                    inline_run.push(inline.clone());
                }
            }
        }
    }

    flush_inline_run_as_paragraph(&mut inline_run, document, asset_map, &mut out);

    if out.is_empty() {
        out.push(paragraph_block_with_rich_text(inlines_to_rich_text(
            content, document, asset_map,
        )));
    }

    out
}

fn admonition_emoji(kind: &AdmonitionKind) -> &'static str {
    match kind {
        AdmonitionKind::Note => "📝",
        AdmonitionKind::Tip => "💡",
        AdmonitionKind::Warning => "⚠️",
        AdmonitionKind::Danger => "🚨",
        AdmonitionKind::Info => "ℹ️",
        AdmonitionKind::Custom(_) => "📌",
    }
}

fn table_cell_rich_text(
    cell: &TableCell,
    document: &Document,
    asset_map: &HashMap<String, String>,
) -> Value {
    json!(blocks_to_rich_text(&cell.blocks, document, asset_map))
}

fn table_to_notion(
    sections: &[TableSection],
    document: &Document,
    asset_map: &HashMap<String, String>,
) -> Value {
    let mut header_cells: Vec<&TableCell> = Vec::new();
    let mut body_rows = Vec::new();

    for section in sections {
        match section.kind {
            TableSectionKind::Head => {
                if let Some(first_row) = section.rows.first() {
                    if header_cells.is_empty() {
                        header_cells.extend(first_row.cells.iter());
                    } else {
                        body_rows.push(first_row);
                    }
                }
                for row in section.rows.iter().skip(1) {
                    body_rows.push(row);
                }
            }
            TableSectionKind::Body | TableSectionKind::Foot => {
                for row in &section.rows {
                    body_rows.push(row);
                }
            }
        }
    }

    let width = header_cells
        .len()
        .max(body_rows.first().map_or(0, |row| row.cells.len()))
        .max(1);

    let mut rows = Vec::new();

    if !header_cells.is_empty() {
        let cells: Vec<Value> = (0..width)
            .map(|idx| {
                header_cells.get(idx).map_or_else(
                    || json!([]),
                    |cell| table_cell_rich_text(cell, document, asset_map),
                )
            })
            .collect();
        rows.push(json!({ "type": "table_row", "table_row": { "cells": cells } }));
    }

    for row in body_rows {
        let cells: Vec<Value> = (0..width)
            .map(|idx| {
                row.cells.get(idx).map_or_else(
                    || json!([]),
                    |cell| table_cell_rich_text(cell, document, asset_map),
                )
            })
            .collect();
        rows.push(json!({ "type": "table_row", "table_row": { "cells": cells } }));
    }

    json!({
        "type": "table",
        "table": {
            "table_width": width,
            "has_column_header": !header_cells.is_empty(),
            "has_row_header": false,
            "children": rows
        }
    })
}

fn block_to_notion(
    block: &Block,
    document: &Document,
    asset_map: &HashMap<String, String>,
) -> Option<Value> {
    match block {
        Block::Heading { level, content, .. } => {
            let block_type = match level.get() {
                1 => "heading_1",
                2 => "heading_2",
                _ => "heading_3",
            };
            let mut payload = Map::new();
            payload.insert(
                "rich_text".to_string(),
                json!(inlines_to_rich_text(content, document, asset_map)),
            );
            let mut root = Map::new();
            root.insert("type".to_string(), json!(block_type));
            root.insert(block_type.to_string(), Value::Object(payload));
            Some(Value::Object(root))
        }
        Block::Paragraph { content, .. } => {
            if content.len() == 1 {
                if let Some(image) =
                    render_single_image_block(&content[0], document, asset_map, None)
                {
                    return Some(image);
                }
                if let Some(math) = render_single_math_block(&content[0]) {
                    return Some(math);
                }
            }

            Some(json!({
                "type": "paragraph",
                "paragraph": {
                    "rich_text": inlines_to_rich_text(content, document, asset_map)
                }
            }))
        }
        Block::CodeBlock { code, language, .. } => {
            let lang = crate::spec::normalize_language(language.as_deref().unwrap_or(""));
            Some(json!({
                "type": "code",
                "code": {
                    "language": lang,
                    "rich_text": [{ "type": "text", "text": { "content": code } }]
                }
            }))
        }
        Block::Table { sections, .. } => Some(table_to_notion(sections, document, asset_map)),
        Block::Divider { .. } => Some(json!({ "type": "divider", "divider": {} })),
        Block::Quote { blocks, .. } => {
            let mut rich_text = Vec::new();
            let mut children = Vec::new();

            for child in blocks {
                match child {
                    Block::Paragraph { content, .. } if rich_text.is_empty() => {
                        rich_text = inlines_to_rich_text(content, document, asset_map);
                    }
                    other => {
                        children.extend(blocks_to_notion(
                            std::slice::from_ref(other),
                            document,
                            asset_map,
                        ));
                    }
                }
            }

            if rich_text.is_empty() {
                rich_text.push(json!({ "type": "text", "text": { "content": "" } }));
            }

            let mut quote = json!({
                "type": "quote",
                "quote": { "rich_text": rich_text }
            });
            if !children.is_empty() {
                quote["quote"]["children"] = json!(children);
            }
            Some(quote)
        }
        Block::Admonition {
            kind,
            title,
            blocks,
            ..
        } => {
            let mut rich_text = Vec::new();
            let mut children = Vec::new();

            if let Some(title_content) = title {
                let mut title_rt = inlines_to_rich_text(title_content, document, asset_map);
                for value in &mut title_rt {
                    if let Some(obj) = value.as_object_mut() {
                        let annotations = obj
                            .entry("annotations")
                            .or_insert_with(|| Value::Object(Map::new()));
                        if let Some(map) = annotations.as_object_mut() {
                            map.insert("bold".to_string(), json!(true));
                        }
                    }
                }
                rich_text.extend(title_rt);
            }

            for child in blocks {
                match child {
                    Block::Paragraph { content, .. } => {
                        if !rich_text.is_empty() {
                            rich_text.push(json!({ "type": "text", "text": { "content": "\n" } }));
                        }
                        rich_text.extend(inlines_to_rich_text(content, document, asset_map));
                    }
                    other => {
                        children.extend(blocks_to_notion(
                            std::slice::from_ref(other),
                            document,
                            asset_map,
                        ));
                    }
                }
            }

            let mut callout = json!({
                "type": "callout",
                "callout": {
                    "rich_text": rich_text,
                    "icon": { "type": "emoji", "emoji": admonition_emoji(kind) }
                }
            });

            if !children.is_empty() {
                callout["callout"]["children"] = json!(children);
            }

            Some(callout)
        }
        Block::Details {
            summary,
            blocks,
            open,
            ..
        } => {
            let mut rich_text = if let Some(summary_content) = summary {
                inlines_to_rich_text(summary_content, document, asset_map)
            } else {
                vec![json!({ "type": "text", "text": { "content": "Details" } })]
            };

            if rich_text.is_empty() {
                rich_text.push(json!({ "type": "text", "text": { "content": "Details" } }));
            }

            let children = blocks_to_notion(blocks, document, asset_map);

            let mut callout = json!({
                "type": "callout",
                "callout": {
                    "rich_text": rich_text,
                    "icon": { "type": "emoji", "emoji": if *open { "📂" } else { "📁" } }
                }
            });
            if !children.is_empty() {
                callout["callout"]["children"] = json!(children);
            }
            Some(callout)
        }
        Block::MathBlock { math, .. } | Block::SvgBlock { svg: math, .. } => {
            math_payload_to_latex(math).map(|latex| {
                json!({
                    "type": "equation",
                    "equation": { "expression": latex }
                })
            })
        }
        Block::RawBlock { .. }
        | Block::List { .. }
        | Block::DefinitionList { .. }
        | Block::Figure { .. }
        | Block::UnknownBlock { .. } => None,
    }
}

fn flow_list_item_to_notion(
    item: &FlowListItem,
    block_type: &str,
    document: &Document,
    asset_map: &HashMap<String, String>,
) -> Value {
    let rich_text = blocks_to_rich_text(&item.blocks, document, asset_map);

    let nested = if let Some(first) = item.blocks.first() {
        match first {
            Block::Paragraph { .. } | Block::Heading { .. } => {
                blocks_to_notion(&item.blocks[1..], document, asset_map)
            }
            _ => blocks_to_notion(&item.blocks, document, asset_map),
        }
    } else {
        Vec::new()
    };

    let mut payload = Map::new();
    payload.insert(
        "rich_text".to_string(),
        json!(if rich_text.is_empty() {
            vec![json!({ "type": "text", "text": { "content": "" } })]
        } else {
            rich_text
        }),
    );
    if !nested.is_empty() {
        payload.insert("children".to_string(), json!(nested));
    }

    let mut root = Map::new();
    root.insert("type".to_string(), json!(block_type));
    root.insert(block_type.to_string(), Value::Object(payload));
    Value::Object(root)
}

fn task_item_to_notion(
    item: &TaskListItem,
    document: &Document,
    asset_map: &HashMap<String, String>,
) -> Value {
    let rich_text = blocks_to_rich_text(&item.blocks, document, asset_map);

    let nested = if let Some(first) = item.blocks.first() {
        match first {
            Block::Paragraph { .. } | Block::Heading { .. } => {
                blocks_to_notion(&item.blocks[1..], document, asset_map)
            }
            _ => blocks_to_notion(&item.blocks, document, asset_map),
        }
    } else {
        Vec::new()
    };

    let mut payload = Map::new();
    payload.insert(
        "rich_text".to_string(),
        json!(if rich_text.is_empty() {
            vec![json!({ "type": "text", "text": { "content": "" } })]
        } else {
            rich_text
        }),
    );
    payload.insert("checked".to_string(), json!(item.checked));
    if !nested.is_empty() {
        payload.insert("children".to_string(), json!(nested));
    }

    let mut root = Map::new();
    root.insert("type".to_string(), json!("to_do"));
    root.insert("to_do".to_string(), Value::Object(payload));
    Value::Object(root)
}

fn definition_list_to_blocks(
    blocks: &mut Vec<Value>,
    items: &[typub_ir::DefinitionItem],
    document: &Document,
    asset_map: &HashMap<String, String>,
) {
    for item in items {
        let empty: &[Block] = &[];
        let term_blocks = item.terms.first().map(Vec::as_slice).unwrap_or(empty);
        let def_blocks = item.definitions.first().map(Vec::as_slice).unwrap_or(empty);

        let mut term_rich = blocks_to_rich_text(term_blocks, document, asset_map);
        for value in &mut term_rich {
            if let Some(obj) = value.as_object_mut() {
                let annotations = obj
                    .entry("annotations")
                    .or_insert_with(|| Value::Object(Map::new()));
                if let Some(map) = annotations.as_object_mut() {
                    map.insert("bold".to_string(), json!(true));
                }
            }
        }

        let mut rich_text = term_rich;
        if !rich_text.is_empty() {
            rich_text.push(json!({ "type": "text", "text": { "content": ": " } }));
        }
        rich_text.extend(blocks_to_rich_text(def_blocks, document, asset_map));

        if !rich_text.is_empty() {
            blocks.push(json!({
                "type": "paragraph",
                "paragraph": { "rich_text": rich_text }
            }));
        }
    }
}

fn blocks_to_notion(
    blocks: &[Block],
    document: &Document,
    asset_map: &HashMap<String, String>,
) -> Vec<Value> {
    let mut out = Vec::new();

    for block in blocks {
        match block {
            Block::Paragraph { content, .. } => {
                out.extend(paragraph_blocks_by_spec(content, document, asset_map));
            }
            Block::Figure {
                content, caption, ..
            } => {
                if content.len() == 1
                    && let Block::Paragraph {
                        content: inline_content,
                        ..
                    } = &content[0]
                    && inline_content.len() == 1
                    && matches!(&inline_content[0], Inline::Image { .. })
                {
                    let caption_rich = caption.as_ref().map(|caption_blocks| {
                        blocks_to_rich_text(caption_blocks, document, asset_map)
                    });
                    if let Some(image) = render_single_image_block(
                        &inline_content[0],
                        document,
                        asset_map,
                        caption_rich,
                    ) {
                        out.push(image);
                        continue;
                    }
                }

                out.extend(blocks_to_notion(content, document, asset_map));
                if let Some(caption_blocks) = caption {
                    let rich_text = blocks_to_rich_text(caption_blocks, document, asset_map);
                    if rich_text_has_visible_content(&rich_text) {
                        out.push(paragraph_block_with_rich_text(rich_text));
                    }
                }
            }
            Block::List { list, .. } => match &list.kind {
                ListKind::Bullet { items } => {
                    out.extend(items.iter().map(|item| {
                        flow_list_item_to_notion(item, "bulleted_list_item", document, asset_map)
                    }));
                }
                ListKind::Numbered { items, .. } => {
                    out.extend(items.iter().map(|item| {
                        flow_list_item_to_notion(item, "numbered_list_item", document, asset_map)
                    }));
                }
                ListKind::Task { items } => {
                    out.extend(
                        items
                            .iter()
                            .map(|item| task_item_to_notion(item, document, asset_map)),
                    );
                }
                ListKind::Custom { items, .. } => {
                    for item in items {
                        let rich_text = blocks_to_rich_text(&item.blocks, document, asset_map);
                        if !rich_text.is_empty() {
                            out.push(json!({
                                "type": "paragraph",
                                "paragraph": { "rich_text": rich_text }
                            }));
                        }
                    }
                }
            },
            Block::DefinitionList { items, .. } => {
                definition_list_to_blocks(&mut out, items, document, asset_map);
            }
            other => {
                if let Some(json) = block_to_notion(other, document, asset_map) {
                    out.push(json);
                }
            }
        }
    }

    out
}

/// Convert semantic document to Notion blocks (no I/O).
///
/// `asset_map` is optional fallback mapping for unresolved local-path assets.
pub fn document_to_blocks(document: &Document, asset_map: &HashMap<String, String>) -> Vec<Value> {
    let mut blocks = blocks_to_notion(&document.blocks, document, asset_map);

    for (id, def) in &document.footnotes {
        let mut rich_text = vec![text_rich_text(
            &format!("[^{}]: ", id.0),
            &Annotations::new(),
            None,
        )];
        rich_text.extend(blocks_to_rich_text(&def.blocks, document, asset_map));
        blocks.push(json!({
            "type": "paragraph",
            "paragraph": { "rich_text": rich_text }
        }));
    }

    blocks
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::document_to_blocks;
    use std::collections::{BTreeMap, HashMap};
    use typub_ir::{
        Asset, AssetId, AssetRef, AssetSource, Block, BlockAttrs, DocMeta, Document, ImageAsset,
        ImageAttrs, Inline, InlineAttrs, MathSource, RelativePath, RenderPayload,
    };

    fn image_asset_doc(content: Vec<Inline>) -> Document {
        let mut assets = BTreeMap::new();
        assets.insert(
            AssetId("asset-000001".to_string()),
            Asset::Image(ImageAsset {
                source: AssetSource::LocalPath {
                    path: RelativePath::new("assets/test-image.jpg".to_string())
                        .expect("valid relative path"),
                },
                meta: None,
                variants: Vec::new(),
            }),
        );

        Document {
            blocks: vec![Block::Paragraph {
                content,
                attrs: BlockAttrs::default(),
            }],
            footnotes: BTreeMap::new(),
            assets,
            meta: DocMeta::default(),
        }
    }

    fn figure_with_image_doc(image_alt: &str, figcaption_text: &str) -> Document {
        let mut assets = BTreeMap::new();
        assets.insert(
            AssetId("asset-000001".to_string()),
            Asset::Image(ImageAsset {
                source: AssetSource::LocalPath {
                    path: RelativePath::new("assets/test-image.jpg".to_string())
                        .expect("valid relative path"),
                },
                meta: None,
                variants: Vec::new(),
            }),
        );

        Document {
            blocks: vec![Block::Figure {
                content: vec![Block::Paragraph {
                    content: vec![image_inline(image_alt)],
                    attrs: BlockAttrs::default(),
                }],
                caption: Some(vec![Block::Paragraph {
                    content: vec![Inline::Text(figcaption_text.to_string())],
                    attrs: BlockAttrs::default(),
                }]),
                attrs: BlockAttrs::default(),
            }],
            footnotes: BTreeMap::new(),
            assets,
            meta: DocMeta::default(),
        }
    }

    fn image_inline(alt: &str) -> Inline {
        Inline::Image {
            asset: AssetRef(AssetId("asset-000001".to_string())),
            alt: alt.to_string(),
            title: None,
            attrs: ImageAttrs::default(),
        }
    }

    fn inline_math(latex: &str) -> Inline {
        Inline::MathInline {
            math: RenderPayload {
                src: Some(MathSource::Latex(latex.to_string())),
                rendered: None,
                id: None,
            },
            attrs: InlineAttrs::default(),
        }
    }

    #[test]
    fn paragraph_with_multiple_images_renders_multiple_image_blocks() {
        let doc = image_asset_doc(vec![
            image_inline(""),
            Inline::Text(" ".to_string()),
            image_inline(""),
        ]);
        let mut asset_map = HashMap::new();
        asset_map.insert(
            "assets/test-image.jpg".to_string(),
            "upload-id-1".to_string(),
        );

        let blocks = document_to_blocks(&doc, &asset_map);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0]["type"], "image");
        assert_eq!(blocks[0]["image"]["type"], "file_upload");
        assert_eq!(blocks[0]["image"]["file_upload"]["id"], "upload-id-1");
        assert!(blocks[0]["image"].get("alt").is_none());
        assert!(blocks[0]["image"].get("caption").is_none());
        assert_eq!(blocks[1]["type"], "image");
        assert_eq!(blocks[1]["image"]["type"], "file_upload");
        assert_eq!(blocks[1]["image"]["file_upload"]["id"], "upload-id-1");
        assert!(blocks[1]["image"].get("alt").is_none());
        assert!(blocks[1]["image"].get("caption").is_none());
    }

    #[test]
    fn paragraph_with_mixed_text_and_image_splits_by_spec() {
        let doc = image_asset_doc(vec![
            Inline::Text("Before".to_string()),
            Inline::Text(" ".to_string()),
            image_inline("Hero"),
            Inline::Text(" ".to_string()),
            Inline::Text("After".to_string()),
        ]);
        let mut asset_map = HashMap::new();
        asset_map.insert(
            "assets/test-image.jpg".to_string(),
            "upload-id-1".to_string(),
        );

        let blocks = document_to_blocks(&doc, &asset_map);
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0]["type"], "paragraph");
        let before_text = blocks[0]["paragraph"]["rich_text"]
            .as_array()
            .expect("before rich_text should be array")
            .iter()
            .filter_map(|value| value["text"]["content"].as_str())
            .collect::<String>();
        assert_eq!(before_text.trim(), "Before");
        assert_eq!(blocks[1]["type"], "image");
        assert!(blocks[1]["image"].get("alt").is_none());
        assert_eq!(blocks[1]["image"]["caption"][0]["text"]["content"], "Hero");
        assert_eq!(blocks[2]["type"], "paragraph");
        let after_text = blocks[2]["paragraph"]["rich_text"]
            .as_array()
            .expect("after rich_text should be array")
            .iter()
            .filter_map(|value| value["text"]["content"].as_str())
            .collect::<String>();
        assert_eq!(after_text.trim(), "After");
    }

    #[test]
    fn paragraph_with_multiple_inline_equations_stays_paragraph() {
        let doc = image_asset_doc(vec![
            inline_math("x"),
            Inline::Text(" ".to_string()),
            inline_math("y"),
        ]);
        let asset_map = HashMap::new();

        let blocks = document_to_blocks(&doc, &asset_map);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["type"], "paragraph");
        let rich_text = blocks[0]["paragraph"]["rich_text"]
            .as_array()
            .expect("paragraph rich_text should be array");
        assert!(
            rich_text
                .iter()
                .filter(|item| item["type"] == "equation")
                .count()
                >= 2
        );
    }

    #[test]
    fn figure_caption_maps_to_notion_caption_and_ignores_alt_field() {
        let doc = figure_with_image_doc("Hero Alt", "Figure Caption");
        let mut asset_map = HashMap::new();
        asset_map.insert(
            "assets/test-image.jpg".to_string(),
            "upload-id-1".to_string(),
        );

        let blocks = document_to_blocks(&doc, &asset_map);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["type"], "image");
        assert!(blocks[0]["image"].get("alt").is_none());
        assert_eq!(
            blocks[0]["image"]["caption"][0]["text"]["content"],
            "Figure Caption"
        );
    }

    #[test]
    fn inline_anchor_link_is_downgraded_to_plain_text() {
        let doc = image_asset_doc(vec![Inline::Link {
            content: vec![Inline::Text("backlink".to_string())],
            href: typub_ir::Url("#fn1".to_string()),
            title: None,
            attrs: InlineAttrs::default(),
        }]);
        let blocks = document_to_blocks(&doc, &HashMap::new());
        let rich_text = blocks[0]["paragraph"]["rich_text"]
            .as_array()
            .expect("paragraph rich_text should be array");
        assert_eq!(rich_text[0]["text"]["content"], "backlink");
        assert!(rich_text[0]["text"].get("link").is_none());
    }

    #[test]
    fn relative_link_is_downgraded_to_plain_text() {
        let doc = image_asset_doc(vec![Inline::Link {
            content: vec![Inline::Text("relative".to_string())],
            href: typub_ir::Url("../other-post/".to_string()),
            title: None,
            attrs: InlineAttrs::default(),
        }]);
        let blocks = document_to_blocks(&doc, &HashMap::new());
        let rich_text = blocks[0]["paragraph"]["rich_text"]
            .as_array()
            .expect("paragraph rich_text should be array");
        assert_eq!(rich_text[0]["text"]["content"], "relative");
        assert!(rich_text[0]["text"].get("link").is_none());
    }
}
