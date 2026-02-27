//! HTML parsing into v2 semantic IR `Document`.
//!
//! Parser emits v2 semantic IR directly.

mod blocks;
mod code;
mod inline;
mod lists;
mod spec;

use anyhow::Result;
use scraper::{ElementRef, Html, Node, Selector};
use std::collections::BTreeMap;

use typub_ir::{
    AdmonitionKind, Asset, AssetId, AssetRef, AssetSource, Block, BlockAttrs, DocMeta, Document,
    FootnoteDef, FootnoteId, ImageAttrs, Inline, InlineAttrs, ListKind, MathSource,
    OrderedListMarker, RelativePath, TableHeaderScope, TextAlign, UnknownChild, Url,
};

#[derive(Default)]
pub(crate) struct ParseCtx {
    assets: BTreeMap<AssetId, Asset>,
    footnotes: BTreeMap<FootnoteId, FootnoteDef>,
    seen_assets: BTreeMap<String, AssetId>,
    next_asset_num: u64,
}

impl ParseCtx {
    pub(crate) fn register_image(
        &mut self,
        src: &str,
        width: Option<u32>,
        height: Option<u32>,
    ) -> Option<AssetRef> {
        let canonical_src = src.trim();
        if canonical_src.starts_with("[[IMG:") && canonical_src.ends_with("]]") {
            return None;
        }

        if let Some(id) = self.seen_assets.get(canonical_src) {
            return Some(AssetRef(id.clone()));
        }

        let source = if canonical_src.starts_with("data:") {
            AssetSource::DataUri {
                uri: canonical_src.to_string(),
            }
        } else if canonical_src.contains("://") || canonical_src.starts_with("//") {
            AssetSource::RemoteUrl {
                url: Url(canonical_src.to_string()),
            }
        } else {
            let path = RelativePath::new(canonical_src.to_string()).ok()?;
            AssetSource::LocalPath { path }
        };

        self.next_asset_num += 1;
        let id = AssetId(format!("asset-{:06}", self.next_asset_num));
        let asset = Asset::Image(typub_ir::ImageAsset {
            source,
            meta: Some(typub_ir::ImageMeta {
                width,
                height,
                format: None,
                sha256: None,
            }),
            variants: Vec::new(),
        });

        self.assets.insert(id.clone(), asset);
        self.seen_assets
            .insert(canonical_src.to_string(), id.clone());
        Some(AssetRef(id))
    }
}

/// Parse HTML into v2 `Document`.
pub fn parse_html_document(html: &str) -> Result<Document> {
    let doc = Html::parse_document(html);
    let body_selector = Selector::parse("body").ok();
    let root = body_selector
        .as_ref()
        .and_then(|s| doc.select(s).next())
        .unwrap_or_else(|| doc.root_element());

    let mut ctx = ParseCtx::default();
    let mut blocks = Vec::new();
    let mut root_text = String::new();
    for child in root.children() {
        match child.value() {
            Node::Element(_) => {
                if let Some(text) = normalize_text_content(&root_text)
                    && !text.trim().is_empty()
                {
                    blocks.push(Block::Paragraph {
                        content: vec![Inline::Text(text)],
                        attrs: BlockAttrs::default(),
                    });
                }
                root_text.clear();

                if let Some(el) = ElementRef::wrap(child) {
                    if parse_footnote_container(el, &mut ctx)? {
                        continue;
                    }
                    blocks::parse_element(el, &mut blocks, &mut ctx)?;
                }
            }
            Node::Text(t) => root_text.push_str(t),
            _ => {}
        }
    }
    if let Some(text) = normalize_text_content(&root_text)
        && !text.trim().is_empty()
    {
        blocks.push(Block::Paragraph {
            content: vec![Inline::Text(text)],
            attrs: BlockAttrs::default(),
        });
    }

    Ok(Document {
        blocks,
        footnotes: ctx.footnotes,
        assets: ctx.assets,
        meta: DocMeta::default(),
    })
}

pub(crate) fn parse_block_attrs(el: &ElementRef<'_>) -> BlockAttrs {
    let mut passthrough = BTreeMap::new();
    let mut classes = Vec::new();
    let mut style = None;

    for (k, v) in el.value().attrs() {
        match k {
            "class" => {
                classes = v
                    .split_whitespace()
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect();
            }
            "style" => style = Some(v.to_string()),
            _ => {
                passthrough.insert(k.to_string(), v.to_string());
            }
        }
    }

    BlockAttrs {
        classes,
        style,
        passthrough,
    }
}

pub(crate) fn parse_image_attrs(
    el: &ElementRef<'_>,
    width: Option<u32>,
    height: Option<u32>,
) -> ImageAttrs {
    let mut passthrough = BTreeMap::new();
    for (k, v) in el.value().attrs() {
        match k {
            "src" | "alt" | "title" | "align" => {}
            _ => {
                passthrough.insert(k.to_string(), v.to_string());
            }
        }
    }

    let align = match el.value().attr("align") {
        Some("left") => Some(TextAlign::Left),
        Some("center") => Some(TextAlign::Center),
        Some("right") => Some(TextAlign::Right),
        _ => el
            .value()
            .attr("style")
            .and_then(parse_text_align_from_style),
    };

    ImageAttrs {
        width,
        height,
        align,
        passthrough,
    }
}

pub(crate) fn parse_inline_attrs(el: &ElementRef<'_>) -> InlineAttrs {
    let mut passthrough = BTreeMap::new();
    let mut classes = Vec::new();
    let mut style = None;

    for (k, v) in el.value().attrs() {
        match k {
            "class" => {
                classes = v
                    .split_whitespace()
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect();
            }
            "style" => style = Some(v.to_string()),
            _ => {
                passthrough.insert(k.to_string(), v.to_string());
            }
        }
    }

    InlineAttrs {
        classes,
        style,
        passthrough,
    }
}

pub(crate) fn parse_math_source(el: ElementRef) -> Option<MathSource> {
    if let Some(latex) = el.value().attr("data-latex-src") {
        Some(MathSource::Latex(latex.to_string()))
    } else {
        el.value()
            .attr("data-typst-src")
            .map(|s| MathSource::Typst(s.to_string()))
    }
}

pub(crate) fn detect_gfm_alert(text: &str) -> Option<(AdmonitionKind, &'static str)> {
    let t = text.trim_start();
    if t.starts_with("[!NOTE]") {
        Some((AdmonitionKind::Note, "[!NOTE]"))
    } else if t.starts_with("[!TIP]") {
        Some((AdmonitionKind::Tip, "[!TIP]"))
    } else if t.starts_with("[!WARNING]") {
        Some((AdmonitionKind::Warning, "[!WARNING]"))
    } else if t.starts_with("[!IMPORTANT]") {
        Some((AdmonitionKind::Info, "[!IMPORTANT]"))
    } else if t.starts_with("[!CAUTION]") {
        Some((AdmonitionKind::Danger, "[!CAUTION]"))
    } else {
        None
    }
}

pub(crate) fn parse_ordered_marker(raw: Option<&str>) -> Option<OrderedListMarker> {
    match raw {
        Some("a") => Some(OrderedListMarker::LowerAlpha),
        Some("A") => Some(OrderedListMarker::UpperAlpha),
        Some("i") => Some(OrderedListMarker::LowerRoman),
        Some("I") => Some(OrderedListMarker::UpperRoman),
        Some("1") => Some(OrderedListMarker::Decimal),
        _ => None,
    }
}

pub(crate) fn parse_header_scope(raw: &str) -> Option<TableHeaderScope> {
    match raw {
        "row" => Some(TableHeaderScope::Row),
        "col" => Some(TableHeaderScope::Col),
        "rowgroup" => Some(TableHeaderScope::RowGroup),
        "colgroup" => Some(TableHeaderScope::ColGroup),
        _ => None,
    }
}

pub(crate) fn parse_text_align_from_style(style: &str) -> Option<TextAlign> {
    let normalized = style.replace(' ', "").to_ascii_lowercase();
    if normalized.contains("text-align:center") {
        Some(TextAlign::Center)
    } else if normalized.contains("text-align:left") {
        Some(TextAlign::Left)
    } else if normalized.contains("text-align:right") {
        Some(TextAlign::Right)
    } else {
        None
    }
}

pub(crate) fn is_admonition_wrapper(el: ElementRef) -> bool {
    if let Some(class) = el.value().attr("class") {
        class_has_keyword(class, "admonition")
            || class_has_keyword(class, "callout")
            || class_has_keyword(class, "notice")
            || class_has_keyword(class, "warning")
            || class_has_keyword(class, "tip")
            || class_has_keyword(class, "note")
            || class_has_keyword(class, "info")
            || class_has_keyword(class, "danger")
    } else {
        false
    }
}

pub(crate) fn class_has_keyword(class_attr: &str, keyword: &str) -> bool {
    class_attr
        .split_whitespace()
        .any(|token| class_token_has_keyword(token, keyword))
}

fn class_token_has_keyword(token: &str, keyword: &str) -> bool {
    token == keyword
        || token
            .split(['-', '_'])
            .any(|segment| !segment.is_empty() && segment == keyword)
}

pub(crate) fn normalize_text_content(text: &str) -> Option<String> {
    if text.is_empty() {
        return None;
    }
    if text.trim().is_empty() {
        return Some(" ".to_string());
    }

    let has_leading_space = text.starts_with(char::is_whitespace);
    let has_trailing_space = text.ends_with(char::is_whitespace);
    let normalized: String = text.split_whitespace().collect::<Vec<_>>().join(" ");

    let mut result = String::new();
    if has_leading_space {
        result.push(' ');
    }
    result.push_str(&normalized);
    if has_trailing_space && !normalized.is_empty() {
        result.push(' ');
    }

    Some(result)
}

pub(crate) fn normalize_footnote_label(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    let unwrapped = trimmed
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(trimmed);
    let normalized = unwrapped.trim();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_string())
    }
}

pub(crate) fn parse_footnote_container(el: ElementRef<'_>, ctx: &mut ParseCtx) -> Result<bool> {
    let tag = el.value().name();
    let class = el.value().attr("class").unwrap_or_default();
    let role = el.value().attr("role").unwrap_or_default();

    let is_doc_endnotes = role == "doc-endnotes";
    let is_footnote_section =
        ((tag == "section" || tag == "div") && class.contains("footnotes")) || is_doc_endnotes;
    if is_footnote_section {
        let mut extracted_any = false;
        if let Ok(li_sel) = Selector::parse("li[id]") {
            for li in el.select(&li_sel) {
                if let Some(id_attr) = li.value().attr("id") {
                    let fallback_id = id_attr.strip_prefix("fn-").unwrap_or(id_attr);
                    if fallback_id.is_empty() {
                        continue;
                    }

                    let mut blocks = blocks::parse_element_as_blocks(li, ctx)?;
                    let footnote_id = if is_doc_endnotes {
                        strip_doc_backlinks(&mut blocks);
                        strip_whitespace_only_paragraphs(&mut blocks);
                        find_doc_backlink_label(li).unwrap_or_else(|| fallback_id.to_string())
                    } else {
                        fallback_id.to_string()
                    };
                    if footnote_id.is_empty() {
                        continue;
                    }
                    let Some(id_num) = footnote_id.parse::<u64>().ok() else {
                        continue;
                    };
                    ctx.footnotes
                        .insert(FootnoteId(id_num), FootnoteDef { blocks });
                    extracted_any = true;
                }
            }
        }
        return Ok(extracted_any);
    }

    let is_single_footnote = tag == "div"
        && class.contains("footnote")
        && el
            .value()
            .attr("id")
            .is_some_and(|id| id.starts_with("fn-"));
    if is_single_footnote
        && let Some(id_attr) = el.value().attr("id")
        && let Some(id) = id_attr.strip_prefix("fn-")
    {
        let blocks = blocks::parse_child_blocks(el, ctx)?;
        let Some(id_num) = id.parse::<u64>().ok() else {
            return Ok(false);
        };
        ctx.footnotes
            .insert(FootnoteId(id_num), FootnoteDef { blocks });
        return Ok(true);
    }

    Ok(false)
}

fn find_doc_backlink_label(li: ElementRef<'_>) -> Option<String> {
    let selector = Selector::parse(r#"a[role="doc-backlink"]"#).ok()?;
    for link in li.select(&selector) {
        let text = link.text().collect::<String>();
        if let Some(label) = normalize_footnote_label(&text) {
            return Some(label);
        }
    }
    None
}

fn strip_doc_backlinks(blocks: &mut [Block]) {
    for block in blocks {
        strip_doc_backlinks_from_block(block);
    }
}

fn strip_doc_backlinks_from_block(block: &mut Block) {
    match block {
        Block::Heading { content, .. } | Block::Paragraph { content, .. } => {
            strip_doc_backlinks_from_inlines(content);
        }
        Block::Quote { blocks, .. }
        | Block::Figure {
            content: blocks, ..
        }
        | Block::Admonition { blocks, .. }
        | Block::Details { blocks, .. } => strip_doc_backlinks(blocks),
        Block::List { list, .. } => match &mut list.kind {
            ListKind::Bullet { items } | ListKind::Numbered { items, .. } => {
                for item in items {
                    strip_doc_backlinks(&mut item.blocks);
                }
            }
            ListKind::Task { items } => {
                for item in items {
                    strip_doc_backlinks(&mut item.blocks);
                }
            }
            ListKind::Custom { items, .. } => {
                for item in items {
                    strip_doc_backlinks(&mut item.blocks);
                }
            }
        },
        Block::DefinitionList { items, .. } => {
            for item in items {
                for group in item.terms.iter_mut().chain(item.definitions.iter_mut()) {
                    strip_doc_backlinks(group);
                }
            }
        }
        Block::Table { sections, .. } => {
            for section in sections {
                for row in &mut section.rows {
                    for cell in &mut row.cells {
                        strip_doc_backlinks(&mut cell.blocks);
                    }
                }
            }
        }
        Block::UnknownBlock { children, .. } => {
            for child in children {
                match child {
                    UnknownChild::Block(block) => strip_doc_backlinks_from_block(block),
                    UnknownChild::Inline(inline) => strip_doc_backlinks_from_inline(inline),
                }
            }
        }
        Block::CodeBlock { .. }
        | Block::Divider { .. }
        | Block::MathBlock { .. }
        | Block::SvgBlock { .. }
        | Block::RawBlock { .. } => {}
    }
}

fn strip_doc_backlinks_from_inline(inline: &mut Inline) {
    match inline {
        Inline::Styled { content, .. } | Inline::UnknownInline { content, .. } => {
            strip_doc_backlinks_from_inlines(content);
        }
        Inline::Text(_)
        | Inline::Code(_)
        | Inline::SoftBreak
        | Inline::HardBreak
        | Inline::Link { .. }
        | Inline::Image { .. }
        | Inline::FootnoteRef(_)
        | Inline::MathInline { .. }
        | Inline::SvgInline { .. }
        | Inline::RawInline { .. } => {}
    }
}

fn strip_doc_backlinks_from_inlines(inlines: &mut Vec<Inline>) {
    let mut kept = Vec::with_capacity(inlines.len());
    for mut inline in std::mem::take(inlines) {
        if is_doc_backlink_link(&inline) {
            continue;
        }
        strip_doc_backlinks_from_inline(&mut inline);
        let keep = match &inline {
            Inline::Styled { content, .. } | Inline::UnknownInline { content, .. } => {
                !content.is_empty()
            }
            _ => true,
        };
        if keep {
            kept.push(inline);
        }
    }
    *inlines = kept;
}

fn is_doc_backlink_link(inline: &Inline) -> bool {
    match inline {
        Inline::Link { attrs, .. } => attrs
            .passthrough
            .get("role")
            .is_some_and(|role| role == "doc-backlink"),
        _ => false,
    }
}

fn strip_whitespace_only_paragraphs(blocks: &mut Vec<Block>) {
    blocks.retain(|block| !is_whitespace_only_paragraph(block));
}

fn is_whitespace_only_paragraph(block: &Block) -> bool {
    match block {
        Block::Paragraph { content, .. } => {
            !content.is_empty() && content.iter().all(inline_is_whitespace_only)
        }
        _ => false,
    }
}

fn inline_is_whitespace_only(inline: &Inline) -> bool {
    match inline {
        Inline::Text(text) => text.trim().is_empty(),
        Inline::SoftBreak | Inline::HardBreak => true,
        Inline::Styled { content, .. } | Inline::UnknownInline { content, .. } => {
            content.iter().all(inline_is_whitespace_only)
        }
        Inline::Code(_)
        | Inline::Link { .. }
        | Inline::Image { .. }
        | Inline::FootnoteRef(_)
        | Inline::MathInline { .. }
        | Inline::SvgInline { .. }
        | Inline::RawInline { .. } => false,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests;
