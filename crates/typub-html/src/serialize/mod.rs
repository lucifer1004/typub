//! HTML serialization for typub HTML IR v2.
//!
//! Converts semantic `Document`/`Block`/`Inline` structures to HTML.

use scraper::{Html, Selector};
use std::collections::BTreeMap;

use typub_ir::{
    AdmonitionKind, Asset, AssetId, AssetRef, AssetSource, Block, BlockAttrs, DefinitionItem,
    Document, FlowListItem, FootnoteDef, FootnoteId, ImageAttrs, Inline, InlineAttrs, List,
    ListKind, MathPayload, MathSource, OrderedListMarker, RenderedArtifact, TableCell,
    TableCellKind, TableHeaderScope, TableSectionKind, TaskListItem, TextAlign, TextStyle,
    UnknownChild,
};

mod attrs;
mod footnotes;
mod lists;
mod math;
#[cfg(test)]
mod tests;

use attrs::*;
use footnotes::serialize_footnotes;
use lists::{serialize_list, serialize_table_cell};
use math::*;

/// Options for platform-specific HTML serialization.
#[derive(Debug, Clone, Default)]
pub struct SerializeOptions {
    /// Wrap `<li>` content in `<span style="display:inline;">`.
    pub li_span_wrap: bool,
    /// Use syntax-highlighted HTML in code blocks when available.
    pub use_code_highlight: bool,
    /// Use `<blockquote>` instead of `<div>` for admonitions.
    pub blockquote_for_admonition: bool,
    /// Emit nested list blocks as siblings of `<li>` for editor compatibility.
    pub sibling_nested_lists: bool,
    /// Convert definition lists to paragraph fallback.
    pub definition_list_to_paragraph: bool,
}

struct SerializeCtx<'a> {
    assets: &'a BTreeMap<AssetId, Asset>,
    options: &'a SerializeOptions,
}

/// Escape text content for safe HTML embedding.
pub fn escape_html_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Escape attribute values for safe HTML embedding.
pub fn escape_html_attr(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Extract plain text from inlines.
pub fn inlines_text(inlines: &[Inline]) -> String {
    let mut out = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(t) | Inline::Code(t) => out.push_str(t),
            Inline::SoftBreak | Inline::HardBreak => out.push(' '),
            Inline::Styled { content, .. } => out.push_str(&inlines_text(content)),
            Inline::Link { content, .. } => out.push_str(&inlines_text(content)),
            Inline::Image { alt, .. } => out.push_str(alt),
            Inline::FootnoteRef(id) => {
                out.push('[');
                out.push_str(&id.0.to_string());
                out.push(']');
            }
            Inline::MathInline { math, .. } => out.push_str(&math_source_text(&math.src)),
            Inline::SvgInline { svg, .. } => out.push_str(&math_source_text(&svg.src)),
            Inline::UnknownInline { content, .. } => out.push_str(&inlines_text(content)),
            Inline::RawInline { .. } => {}
        }
    }
    out
}

/// Serialize a full document to HTML using default options.
pub fn document_to_html(doc: &Document) -> String {
    document_to_html_with_options(doc, &SerializeOptions::default())
}

/// Serialize a full document to HTML using custom options.
pub fn document_to_html_with_options(doc: &Document, options: &SerializeOptions) -> String {
    let ctx = SerializeCtx {
        assets: &doc.assets,
        options,
    };

    let mut out = String::new();
    serialize_blocks(&ctx, &doc.blocks, &mut out);
    serialize_footnotes(&ctx, &doc.footnotes, &mut out);
    out
}

/// Serialize inlines to HTML with document asset context and default options.
pub fn inlines_to_html(inlines: &[Inline], assets: &BTreeMap<AssetId, Asset>) -> String {
    inlines_to_html_with_options(inlines, assets, &SerializeOptions::default())
}

/// Serialize inlines to HTML with document asset context and custom options.
pub fn inlines_to_html_with_options(
    inlines: &[Inline],
    assets: &BTreeMap<AssetId, Asset>,
    options: &SerializeOptions,
) -> String {
    let ctx = SerializeCtx { assets, options };
    serialize_inlines(&ctx, inlines)
}

fn serialize_blocks(ctx: &SerializeCtx<'_>, blocks: &[Block], out: &mut String) {
    for block in blocks {
        serialize_block(ctx, block, out);
    }
}

fn serialize_block(ctx: &SerializeCtx<'_>, block: &Block, out: &mut String) {
    match block {
        Block::Heading {
            level,
            id,
            content,
            attrs,
        } => {
            let mut extra = Vec::new();
            if let Some(anchor) = id {
                extra.push(("id", anchor.0.clone()));
            }
            let attr_str = block_attrs_to_html(attrs, &extra, &[]);
            out.push_str(&format!(
                "<h{}{}>{}</h{}>\n",
                level.get(),
                attr_str,
                serialize_inlines(ctx, content),
                level.get()
            ));
        }
        Block::Paragraph { content, attrs } => {
            let attr_str = block_attrs_to_html(attrs, &[], &[]);
            out.push_str(&format!(
                "<p{}>{}</p>\n",
                attr_str,
                serialize_inlines(ctx, content)
            ));
        }
        Block::Quote {
            blocks,
            cite,
            attrs,
        } => {
            let mut extra = Vec::new();
            if let Some(url) = cite {
                extra.push(("cite", url.0.clone()));
            }
            let attr_str = block_attrs_to_html(attrs, &extra, &[]);
            let mut content = String::new();
            serialize_blocks(ctx, blocks, &mut content);
            out.push_str(&format!(
                "<blockquote{}>{}</blockquote>\n",
                attr_str,
                content.trim_end()
            ));
        }
        Block::CodeBlock {
            code,
            language,
            filename,
            highlight_lines,
            highlighted_html,
            attrs,
        } => {
            let mut extra = Vec::new();
            if let Some(name) = filename {
                extra.push(("data-filename", name.clone()));
            }
            if !highlight_lines.is_empty() {
                let lines = highlight_lines
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                extra.push(("data-highlight-lines", lines));
            }
            let pre_attr = block_attrs_to_html(attrs, &extra, &[]);

            let mut code_extra = Vec::new();
            if let Some(lang) = language {
                code_extra.push(("data-lang", lang.clone()));
                code_extra.push(("class", format!("hljs language-{}", lang)));
            }
            let code_attr = extra_attrs_to_html(&code_extra);
            let code_content = if ctx.options.use_code_highlight {
                highlighted_html.as_deref().unwrap_or(code)
            } else {
                code
            };
            let code_body = if ctx.options.use_code_highlight && highlighted_html.is_some() {
                code_content.to_string()
            } else {
                escape_html_text(code_content)
            };

            out.push_str(&format!(
                "<pre{}><code{}>{}</code></pre>\n",
                pre_attr, code_attr, code_body
            ));
        }
        Block::Divider { attrs } => {
            let attr_str = block_attrs_to_html(attrs, &[], &[]);
            out.push_str(&format!("<hr{}>\n", attr_str));
        }
        Block::List { list, attrs } => serialize_list(ctx, list, attrs, out),
        Block::DefinitionList { items, attrs } => serialize_definition_list(ctx, items, attrs, out),
        Block::Table {
            caption,
            sections,
            attrs,
        } => {
            let attr_str = block_attrs_to_html(attrs, &[], &[]);
            out.push_str(&format!("<table{}>", attr_str));

            if let Some(caption_blocks) = caption {
                out.push_str("<caption>");
                serialize_blocks(ctx, caption_blocks, out);
                out.push_str("</caption>");
            }

            for section in sections {
                let section_tag = match section.kind {
                    TableSectionKind::Head => "thead",
                    TableSectionKind::Body => "tbody",
                    TableSectionKind::Foot => "tfoot",
                };
                let section_attr = block_attrs_to_html(&section.attrs, &[], &[]);
                out.push_str(&format!("<{}{}>", section_tag, section_attr));
                for row in &section.rows {
                    let row_attr = block_attrs_to_html(&row.attrs, &[], &[]);
                    out.push_str(&format!("<tr{}>", row_attr));
                    for cell in &row.cells {
                        serialize_table_cell(ctx, cell, out);
                    }
                    out.push_str("</tr>");
                }
                out.push_str(&format!("</{}>", section_tag));
            }

            out.push_str("</table>\n");
        }
        Block::Figure {
            content,
            caption,
            attrs,
        } => {
            let attr_str = block_attrs_to_html(attrs, &[], &[]);
            out.push_str(&format!("<figure{}>", attr_str));
            serialize_blocks(ctx, content, out);
            if let Some(caption_blocks) = caption {
                out.push_str("<figcaption>");
                serialize_blocks(ctx, caption_blocks, out);
                out.push_str("</figcaption>");
            }
            out.push_str("</figure>\n");
        }
        Block::Admonition {
            kind,
            title,
            blocks,
            attrs,
        } => {
            let wrapper_tag = if ctx.options.blockquote_for_admonition {
                "blockquote"
            } else {
                "div"
            };
            let mut classes = vec!["admonition".to_string(), admonition_kind_class(kind)];
            classes.extend(attrs.classes.iter().cloned());
            let attr_str = attrs_to_html(
                &classes,
                attrs.style.as_deref(),
                &attrs.passthrough,
                &[],
                &["class"],
            );

            out.push_str(&format!("<{}{}>", wrapper_tag, attr_str));
            if let Some(t) = title {
                out.push_str(&format!(
                    "<p class=\"admonition-title\"><strong>{}</strong></p>",
                    serialize_inlines(ctx, t)
                ));
            }
            serialize_blocks(ctx, blocks, out);
            out.push_str(&format!("</{}>\n", wrapper_tag));
        }
        Block::Details {
            summary,
            blocks,
            open,
            attrs,
        } => {
            let mut extra = Vec::new();
            if *open {
                extra.push(("open", "open".to_string()));
            }
            let attr_str = block_attrs_to_html(attrs, &extra, &[]);
            out.push_str(&format!("<details{}>", attr_str));
            if let Some(sum) = summary {
                out.push_str(&format!(
                    "<summary>{}</summary>",
                    serialize_inlines(ctx, sum)
                ));
            }
            serialize_blocks(ctx, blocks, out);
            out.push_str("</details>\n");
        }
        Block::MathBlock { math, attrs } => {
            out.push_str(&serialize_math_block(ctx, math, attrs));
            out.push('\n');
        }
        Block::SvgBlock { svg, attrs } => {
            out.push_str(&serialize_svg_block(ctx, svg, attrs));
            out.push('\n');
        }
        Block::UnknownBlock {
            tag,
            attrs,
            children,
            data: _,
            note,
            source,
        } => {
            let mut extra = vec![("data-unknown-block", tag.clone())];
            if let Some(n) = note {
                extra.push(("data-unknown-note", n.clone()));
            }
            let attr_str = block_attrs_to_html(attrs, &extra, &[]);
            out.push_str(&format!("<div{}>", attr_str));
            if let Some(src) = source {
                out.push_str(&format!(
                    "<pre data-unknown-source=\"true\">{}</pre>",
                    escape_html_text(src)
                ));
            }
            serialize_unknown_children(ctx, children, out);
            out.push_str("</div>\n");
        }
        Block::RawBlock {
            html,
            origin: _,
            trust: _,
            attrs: _,
        } => {
            out.push_str(html);
            if !html.ends_with('\n') {
                out.push('\n');
            }
        }
    }
}

fn serialize_unknown_children(ctx: &SerializeCtx<'_>, children: &[UnknownChild], out: &mut String) {
    for child in children {
        match child {
            UnknownChild::Block(block) => serialize_block(ctx, block, out),
            UnknownChild::Inline(inline) => out.push_str(&serialize_inline(ctx, inline)),
        }
    }
}

fn serialize_definition_list(
    ctx: &SerializeCtx<'_>,
    items: &[DefinitionItem],
    attrs: &BlockAttrs,
    out: &mut String,
) {
    if ctx.options.definition_list_to_paragraph {
        for item in items {
            let term_html = item
                .terms
                .iter()
                .map(|blocks| blocks_inline_fallback_html(ctx, blocks))
                .filter(|s| !s.trim().is_empty())
                .collect::<Vec<_>>()
                .join(" / ");
            let def_html = item
                .definitions
                .iter()
                .map(|blocks| blocks_inline_fallback_html(ctx, blocks))
                .filter(|s| !s.trim().is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            out.push_str(&format!(
                "<p><strong>{}</strong>: {}</p>\n",
                term_html, def_html
            ));
        }
        return;
    }

    let attr_str = block_attrs_to_html(attrs, &[], &[]);
    out.push_str(&format!("<dl{}>", attr_str));
    for item in items {
        for terms in &item.terms {
            out.push_str("<dt>");
            serialize_blocks(ctx, terms, out);
            out.push_str("</dt>");
        }
        for defs in &item.definitions {
            out.push_str("<dd>");
            serialize_blocks(ctx, defs, out);
            out.push_str("</dd>");
        }
    }
    out.push_str("</dl>\n");
}

fn blocks_inline_fallback_html(ctx: &SerializeCtx<'_>, blocks: &[Block]) -> String {
    let mut parts = Vec::new();
    for block in blocks {
        match block {
            Block::Paragraph { content, .. } | Block::Heading { content, .. } => {
                parts.push(serialize_inlines(ctx, content));
            }
            Block::CodeBlock { code, .. } => {
                parts.push(format!("<code>{}</code>", escape_html_text(code)));
            }
            Block::MathBlock { math, .. } => {
                parts.push(escape_html_text(&math_source_text(&math.src)));
            }
            Block::SvgBlock { svg, .. } => {
                parts.push(escape_html_text(&math_source_text(&svg.src)));
            }
            _ => parts.push(escape_html_text(&block_inline_fallback_text(block))),
        }
    }
    parts.join(" ")
}

fn block_inline_fallback_text(block: &Block) -> String {
    match block {
        Block::Heading { content, .. } | Block::Paragraph { content, .. } => inlines_text(content),
        Block::Quote { blocks, .. }
        | Block::Figure {
            content: blocks, ..
        }
        | Block::Admonition { blocks, .. }
        | Block::Details { blocks, .. } => blocks
            .iter()
            .map(block_inline_fallback_text)
            .collect::<Vec<_>>()
            .join(" "),
        Block::CodeBlock { code, .. } => code.clone(),
        Block::List { list, .. } => match &list.kind {
            ListKind::Bullet { items } | ListKind::Numbered { items, .. } => items
                .iter()
                .flat_map(|i| i.blocks.iter())
                .map(block_inline_fallback_text)
                .collect::<Vec<_>>()
                .join(" "),
            ListKind::Task { items } => items
                .iter()
                .flat_map(|i| i.blocks.iter())
                .map(block_inline_fallback_text)
                .collect::<Vec<_>>()
                .join(" "),
            ListKind::Custom { items, .. } => items
                .iter()
                .flat_map(|i| i.blocks.iter())
                .map(block_inline_fallback_text)
                .collect::<Vec<_>>()
                .join(" "),
        },
        Block::DefinitionList { items, .. } => items
            .iter()
            .flat_map(|item| item.terms.iter().chain(item.definitions.iter()))
            .flat_map(|group| group.iter())
            .map(block_inline_fallback_text)
            .collect::<Vec<_>>()
            .join(" "),
        Block::Table { sections, .. } => sections
            .iter()
            .flat_map(|s| s.rows.iter())
            .flat_map(|r| r.cells.iter())
            .flat_map(|c| c.blocks.iter())
            .map(block_inline_fallback_text)
            .collect::<Vec<_>>()
            .join(" "),
        Block::MathBlock { math, .. } => math_source_text(&math.src),
        Block::SvgBlock { svg, .. } => math_source_text(&svg.src),
        Block::UnknownBlock { note, .. } => note.clone().unwrap_or_default(),
        Block::RawBlock { .. } | Block::Divider { .. } => String::new(),
    }
}

fn serialize_inlines(ctx: &SerializeCtx<'_>, inlines: &[Inline]) -> String {
    let mut out = String::new();
    for inline in inlines {
        out.push_str(&serialize_inline(ctx, inline));
    }
    out
}

fn serialize_inline(ctx: &SerializeCtx<'_>, inline: &Inline) -> String {
    match inline {
        Inline::Text(text) => escape_html_text(text),
        Inline::Code(code) => format!("<code>{}</code>", escape_html_text(code)),
        Inline::SoftBreak => " ".to_string(),
        Inline::HardBreak => "<br>".to_string(),
        Inline::Styled {
            styles,
            content,
            attrs,
        } => serialize_styled_inline(ctx, styles.styles(), content, attrs),
        Inline::Link {
            content,
            href,
            title,
            attrs,
        } => {
            let mut extra = vec![("href", href.0.clone())];
            if let Some(t) = title {
                extra.push(("title", t.clone()));
            }
            let attr_str = inline_attrs_to_html(attrs, &extra, &["href", "title"]);
            format!("<a{}>{}</a>", attr_str, serialize_inlines(ctx, content))
        }
        Inline::Image {
            asset,
            alt,
            title,
            attrs,
        } => serialize_image_inline(ctx, asset, alt, title.as_deref(), attrs),
        Inline::FootnoteRef(id) => {
            let id_str = id.0.to_string();
            format!(
                "<sup><a href=\"#fn-{}\" id=\"fnref-{}\">[{}]</a></sup>",
                id_str, id_str, id_str
            )
        }
        Inline::MathInline { math, attrs } => serialize_math_inline(ctx, math, attrs),
        Inline::SvgInline { svg, attrs } => serialize_svg_inline(ctx, svg, attrs),
        Inline::UnknownInline {
            tag,
            attrs,
            content,
            data: _,
            note,
            source,
        } => {
            let mut extra = vec![("data-unknown-inline", tag.clone())];
            if let Some(n) = note {
                extra.push(("data-unknown-note", n.clone()));
            }
            let attr_str = inline_attrs_to_html(attrs, &extra, &[]);
            let mut html = String::new();
            html.push_str(&format!("<span{}>", attr_str));
            html.push_str(&serialize_inlines(ctx, content));
            if let Some(src) = source {
                html.push_str(&format!(
                    "<code data-unknown-source=\"true\">{}</code>",
                    escape_html_text(src)
                ));
            }
            html.push_str("</span>");
            html
        }
        Inline::RawInline {
            html,
            origin: _,
            trust: _,
            attrs: _,
        } => html.clone(),
    }
}

fn serialize_styled_inline(
    ctx: &SerializeCtx<'_>,
    styles: &[TextStyle],
    content: &[Inline],
    attrs: &InlineAttrs,
) -> String {
    let mut html = serialize_inlines(ctx, content);
    if attrs != &InlineAttrs::default() {
        let attr_str = inline_attrs_to_html(attrs, &[], &[]);
        html = format!("<span{}>{}</span>", attr_str, html);
    }

    for style in styles.iter().rev() {
        let (open, close) = text_style_tag(*style);
        html = format!("<{}>{}</{}>", open, html, close);
    }

    html
}

fn text_style_tag(style: TextStyle) -> (&'static str, &'static str) {
    match style {
        TextStyle::Bold => ("strong", "strong"),
        TextStyle::Italic => ("em", "em"),
        TextStyle::Strikethrough => ("s", "s"),
        TextStyle::Underline => ("u", "u"),
        TextStyle::Mark => ("mark", "mark"),
        TextStyle::Superscript => ("sup", "sup"),
        TextStyle::Subscript => ("sub", "sub"),
        TextStyle::Kbd => ("kbd", "kbd"),
    }
}

fn serialize_image_inline(
    ctx: &SerializeCtx<'_>,
    asset_ref: &AssetRef,
    alt: &str,
    title: Option<&str>,
    attrs: &ImageAttrs,
) -> String {
    let mut extra = vec![("alt", alt.to_string())];

    if let Some(src) = resolve_asset_src(asset_ref, ctx.assets) {
        extra.push(("src", src));
    } else {
        extra.push(("src", "".to_string()));
        extra.push(("data-missing-asset", asset_ref.0.0.clone()));
    }

    if let Some(t) = title {
        extra.push(("title", t.to_string()));
    }
    if let Some(width) = attrs.width {
        extra.push(("width", width.to_string()));
    }
    if let Some(height) = attrs.height {
        extra.push(("height", height.to_string()));
    }
    if let Some(align) = attrs.align {
        extra.push(("data-align", text_align_css_value(align).to_string()));
    }

    let attr_str = attrs_to_html(
        &[],
        None,
        &attrs.passthrough,
        &extra,
        &["src", "alt", "title", "width", "height", "data-align"],
    );
    format!("<img{}>", attr_str)
}

fn resolve_asset_src(asset_ref: &AssetRef, assets: &BTreeMap<AssetId, Asset>) -> Option<String> {
    let asset = assets.get(&asset_ref.0)?;
    let (source, variants) = match asset {
        Asset::Image(a) => (&a.source, &a.variants),
        Asset::Video(a) | Asset::Audio(a) => (&a.source, &a.variants),
        Asset::File(a) => (&a.source, &a.variants),
        Asset::Custom(a) => (&a.source, &a.variants),
    };

    if let Some(url) = variants
        .iter()
        .find(|v| v.name == "original")
        .map(|v| v.publish_url.0.clone())
        .or_else(|| variants.first().map(|v| v.publish_url.0.clone()))
    {
        return Some(url);
    }

    match source {
        AssetSource::RemoteUrl { url } => Some(url.0.clone()),
        AssetSource::DataUri { uri } => Some(uri.clone()),
        AssetSource::LocalPath { path } => Some(path.as_str().to_string()),
    }
}
