//! Confluence HTML formatting for semantic IR v2.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use typub_adapters_core::{image_utils, typst_math_to_latex};
use typub_html::{escape_html_attr, escape_html_text};
use typub_ir::{
    AdmonitionKind, Asset, AssetRef, AssetSource, Block, Document, FootnoteDef, FootnoteId,
    ImageAttrs, Inline, ListKind, MathSource, RenderPayload, RenderedArtifact, TableCellKind,
    TableHeaderScope, TableSectionKind, TextAlign, TextStyle, UnknownChild,
};

struct RenderCtx<'a> {
    document: &'a Document,
    app_id: Option<&'a str>,
    env_id: Option<&'a str>,
    marker_url_map: &'a HashMap<String, String>,
}

enum ImageRef {
    External(String),
    Attachment(String),
}

pub fn elements_to_confluence_html(
    document: &Document,
    content_root: &Path,
    asset_map: &HashMap<PathBuf, String>,
    app_id: Option<&str>,
    env_id: Option<&str>,
) -> String {
    let marker_url_map = image_utils::build_image_marker_url_map(content_root, asset_map);
    let ctx = RenderCtx {
        document,
        app_id,
        env_id,
        marker_url_map: &marker_url_map,
    };

    let mut out = String::new();
    render_blocks(&document.blocks, &ctx, &mut out);
    render_footnotes(document, &ctx, &mut out);
    out
}

fn render_blocks(blocks: &[Block], ctx: &RenderCtx<'_>, out: &mut String) {
    for block in blocks {
        render_block(block, ctx, out);
    }
}

fn render_anchor_macro(anchor_id: &str) -> String {
    format!(
        r#"<ac:structured-macro ac:name="anchor" ac:schema-version="1"><ac:parameter ac:name="">{}</ac:parameter></ac:structured-macro>"#,
        anchor_id
    )
}

fn render_block(block: &Block, ctx: &RenderCtx<'_>, out: &mut String) {
    match block {
        Block::Heading {
            level, id, content, ..
        } => {
            if let Some(anchor) = id {
                out.push_str(&render_anchor_macro(&anchor.0));
            }
            out.push_str(&format!(
                "<h{}>{}</h{}>",
                level.get(),
                render_inlines(content, ctx),
                level.get()
            ));
        }
        Block::Paragraph { content, attrs, .. } => {
            if content.len() == 1 {
                match &content[0] {
                    Inline::Image {
                        asset,
                        alt,
                        title,
                        attrs,
                    } => {
                        render_image_block(asset, alt, title.as_deref(), attrs, None, ctx, out);
                        return;
                    }
                    Inline::MathInline { math, .. } | Inline::SvgInline { svg: math, .. } => {
                        if let Some(rendered) = render_math_inline(math, ctx) {
                            out.push_str(&rendered);
                            return;
                        }
                    }
                    _ => {}
                }
            }
            // Handle id attribute from passthrough - render as anchor macro before the paragraph
            if let Some(id) = attrs.passthrough.get("id") {
                out.push_str(&render_anchor_macro(id));
            }
            out.push_str(&format!("<p>{}</p>", render_inlines(content, ctx)));
        }
        Block::Quote { blocks, .. } => {
            out.push_str("<blockquote>");
            render_blocks(blocks, ctx, out);
            out.push_str("</blockquote>");
        }
        Block::CodeBlock { code, language, .. } => {
            let lang_param = if language.as_deref() != Some("plain text") {
                language.as_deref().map_or_else(String::new, |lang| {
                    format!(
                        r#"<ac:parameter ac:name="language">{}</ac:parameter>"#,
                        escape_html_text(lang)
                    )
                })
            } else {
                String::new()
            };
            let escaped = code.replace("]]>", "]]]]><![CDATA[>");
            out.push_str(&format!(
                r#"<ac:structured-macro ac:name="code">{lang_param}<ac:plain-text-body><![CDATA[{escaped}]]></ac:plain-text-body></ac:structured-macro>"#
            ));
        }
        Block::Divider { .. } => out.push_str("<hr />"),
        Block::List { list, .. } => render_list_kind(&list.kind, ctx, out),
        Block::DefinitionList { items, .. } => {
            out.push_str("<table><tbody>");
            for item in items {
                let terms = render_block_groups(&item.terms, ctx);
                let defs = render_block_groups(&item.definitions, ctx);
                out.push_str(&format!(
                    "<tr><td><strong>{}</strong></td><td>{}</td></tr>",
                    terms, defs
                ));
            }
            out.push_str("</tbody></table>");
        }
        Block::Table {
            caption, sections, ..
        } => {
            out.push_str("<table>");
            if let Some(caption) = caption {
                out.push_str("<caption>");
                render_blocks(caption, ctx, out);
                out.push_str("</caption>");
            }
            for section in sections {
                match section.kind {
                    TableSectionKind::Head => out.push_str("<thead>"),
                    TableSectionKind::Body => out.push_str("<tbody>"),
                    TableSectionKind::Foot => out.push_str("<tfoot>"),
                }
                for row in &section.rows {
                    out.push_str("<tr>");
                    for cell in &row.cells {
                        let tag = match cell.kind {
                            TableCellKind::Header => "th",
                            TableCellKind::Data => "td",
                        };
                        let mut attrs = String::new();
                        if cell.colspan > 1 {
                            attrs.push_str(&format!(r#" colspan="{}""#, cell.colspan));
                        }
                        if cell.rowspan > 1 {
                            attrs.push_str(&format!(r#" rowspan="{}""#, cell.rowspan));
                        }
                        if let Some(scope) = cell.scope {
                            let scope = match scope {
                                TableHeaderScope::Row => "row",
                                TableHeaderScope::Col => "col",
                                TableHeaderScope::RowGroup => "rowgroup",
                                TableHeaderScope::ColGroup => "colgroup",
                            };
                            attrs.push_str(&format!(r#" scope="{}""#, scope));
                        }
                        if let Some(align) = cell.align {
                            let align = match align {
                                TextAlign::Left => "left",
                                TextAlign::Center => "center",
                                TextAlign::Right => "right",
                            };
                            attrs.push_str(&format!(r#" style="text-align: {}""#, align));
                        }
                        out.push_str(&format!("<{tag}{attrs}>"));
                        render_blocks(&cell.blocks, ctx, out);
                        out.push_str(&format!("</{tag}>"));
                    }
                    out.push_str("</tr>");
                }
                match section.kind {
                    TableSectionKind::Head => out.push_str("</thead>"),
                    TableSectionKind::Body => out.push_str("</tbody>"),
                    TableSectionKind::Foot => out.push_str("</tfoot>"),
                }
            }
            out.push_str("</table>");
        }
        Block::Figure {
            content,
            caption,
            attrs,
            ..
        } => {
            // Render anchor macro before the figure if it has an id
            if let Some(id) = attrs.passthrough.get("id") {
                out.push_str(&render_anchor_macro(id));
            }

            if content.len() == 1
                && let Block::Paragraph {
                    content: inline_content,
                    ..
                } = &content[0]
                && inline_content.len() == 1
                && let Inline::Image {
                    asset,
                    alt,
                    title,
                    attrs,
                } = &inline_content[0]
            {
                render_image_block(
                    asset,
                    alt,
                    title.as_deref(),
                    attrs,
                    caption.as_deref(),
                    ctx,
                    out,
                );
                return;
            }

            out.push_str("<figure>");
            render_blocks(content, ctx, out);
            if let Some(caption) = caption {
                out.push_str("<figcaption>");
                render_blocks(caption, ctx, out);
                out.push_str("</figcaption>");
            }
            out.push_str("</figure>");
        }
        Block::Admonition {
            kind,
            title,
            blocks,
            ..
        } => {
            let kind_str = match kind {
                AdmonitionKind::Note => "note",
                AdmonitionKind::Tip => "tip",
                AdmonitionKind::Warning => "warning",
                AdmonitionKind::Danger => "caution",
                AdmonitionKind::Info => "info",
                AdmonitionKind::Custom(s) => s.as_str(),
            };
            let macro_name = kind_str.to_lowercase();
            out.push_str(&format!(
                r#"<ac:structured-macro ac:name="{}"><ac:rich-text-body>"#,
                escape_html_attr(&macro_name)
            ));
            if let Some(title) = title {
                out.push_str(&format!(
                    "<p><strong>{}</strong></p>",
                    render_inlines(title, ctx)
                ));
            }
            render_blocks(blocks, ctx, out);
            out.push_str("</ac:rich-text-body></ac:structured-macro>");
        }
        Block::Details {
            summary,
            blocks,
            open,
            ..
        } => {
            out.push_str("<ac:structured-macro ac:name=\"expand\">");
            if *open {
                out.push_str(r#"<ac:parameter ac:name="expanded">true</ac:parameter>"#);
            }
            if let Some(summary) = summary {
                out.push_str(&format!(
                    r#"<ac:parameter ac:name="title">{}</ac:parameter>"#,
                    render_inlines(summary, ctx)
                ));
            }
            out.push_str("<ac:rich-text-body>");
            render_blocks(blocks, ctx, out);
            out.push_str("</ac:rich-text-body></ac:structured-macro>");
        }
        Block::MathBlock { math, attrs, .. }
        | Block::SvgBlock {
            svg: math, attrs, ..
        } => {
            // Render anchor macro before the math block if it has an id
            if let Some(id) = attrs.passthrough.get("id") {
                out.push_str(&render_anchor_macro(id));
            }
            if let Some(rendered) = render_math_block(math, ctx) {
                out.push_str(&rendered);
            }
        }
        Block::UnknownBlock { children, .. } => {
            for child in children {
                match child {
                    UnknownChild::Block(block) => render_block(block, ctx, out),
                    UnknownChild::Inline(inline) => out.push_str(&render_inline(inline, ctx)),
                }
            }
        }
        Block::RawBlock { html, .. } => out.push_str(html),
    }
}

fn render_list_kind(kind: &ListKind, ctx: &RenderCtx<'_>, out: &mut String) {
    match kind {
        ListKind::Bullet { items } => {
            out.push_str("<ul>");
            for item in items {
                out.push_str("<li>");
                render_list_item_blocks(&item.blocks, ctx, out);
                out.push_str("</li>");
            }
            out.push_str("</ul>");
        }
        ListKind::Numbered { items, start, .. } => {
            if *start > 1 {
                out.push_str(&format!(r#"<ol start="{}">"#, start));
            } else {
                out.push_str("<ol>");
            }
            for item in items {
                out.push_str("<li>");
                render_list_item_blocks(&item.blocks, ctx, out);
                out.push_str("</li>");
            }
            out.push_str("</ol>");
        }
        ListKind::Task { items } => {
            out.push_str("<ac:task-list>");
            for item in items {
                out.push_str("<ac:task>");
                out.push_str(&format!(
                    "<ac:task-status>{}</ac:task-status>",
                    if item.checked {
                        "complete"
                    } else {
                        "incomplete"
                    }
                ));
                out.push_str("<ac:task-body>");
                render_list_item_blocks(&item.blocks, ctx, out);
                out.push_str("</ac:task-body>");
                out.push_str("</ac:task>");
            }
            out.push_str("</ac:task-list>");
        }
        ListKind::Custom { items, .. } => {
            out.push_str("<ul>");
            for item in items {
                out.push_str("<li>");
                render_list_item_blocks(&item.blocks, ctx, out);
                out.push_str("</li>");
            }
            out.push_str("</ul>");
        }
    }
}

fn render_list_item_blocks(blocks: &[Block], ctx: &RenderCtx<'_>, out: &mut String) {
    if let Some(first) = blocks.first() {
        match first {
            Block::Paragraph { content, .. } | Block::Heading { content, .. } => {
                out.push_str(&render_inlines(content, ctx));
                render_blocks(&blocks[1..], ctx, out);
            }
            _ => render_blocks(blocks, ctx, out),
        }
    }
}

fn render_block_groups(groups: &[Vec<Block>], ctx: &RenderCtx<'_>) -> String {
    let mut out = String::new();
    for (idx, group) in groups.iter().enumerate() {
        if idx > 0 {
            out.push_str("<br />");
        }
        render_blocks(group, ctx, &mut out);
    }
    out
}

fn render_inlines(inlines: &[Inline], ctx: &RenderCtx<'_>) -> String {
    let mut out = String::new();
    for inline in inlines {
        out.push_str(&render_inline(inline, ctx));
    }
    out
}

fn render_inline(inline: &Inline, ctx: &RenderCtx<'_>) -> String {
    match inline {
        Inline::Text(text) => escape_html_text(text),
        Inline::Code(code) => format!("<code>{}</code>", escape_html_text(code)),
        Inline::SoftBreak => " ".to_string(),
        Inline::HardBreak => "<br />".to_string(),
        Inline::Styled {
            styles, content, ..
        } => render_styled_inline(styles.styles(), content, ctx),
        Inline::Link { content, href, .. } => format!(
            r#"<a href="{}">{}</a>"#,
            escape_html_attr(&href.0),
            render_inlines(content, ctx)
        ),
        Inline::Image {
            asset,
            alt,
            title,
            attrs,
        } => render_image_inline(asset, alt, title.as_deref(), attrs, ctx),
        Inline::FootnoteRef(id) => format!(
            "<sup><a href=\"\\#fn-{}\">[{}]</a></sup>",
            escape_html_attr(&id.0),
            escape_html_text(&id.0)
        ),
        Inline::MathInline { math, .. } | Inline::SvgInline { svg: math, .. } => {
            render_math_inline(math, ctx).unwrap_or_default()
        }
        Inline::UnknownInline { content, .. } => render_inlines(content, ctx),
        Inline::RawInline { html, .. } => html.clone(),
    }
}

fn render_styled_inline(styles: &[TextStyle], content: &[Inline], ctx: &RenderCtx<'_>) -> String {
    let mut open = String::new();
    let mut close = String::new();
    for style in styles {
        let (tag_open, tag_close) = match style {
            TextStyle::Bold => ("<strong>", "</strong>"),
            TextStyle::Italic => ("<em>", "</em>"),
            TextStyle::Strikethrough => (
                r#"<span style="text-decoration: line-through;">"#,
                "</span>",
            ),
            TextStyle::Underline => ("<u>", "</u>"),
            TextStyle::Mark => (r#"<span style="background-color: yellow;">"#, "</span>"),
            TextStyle::Superscript => ("<sup>", "</sup>"),
            TextStyle::Subscript => ("<sub>", "</sub>"),
            TextStyle::Kbd => ("<code>", "</code>"),
        };
        open.push_str(tag_open);
        close.insert_str(0, tag_close);
    }
    format!("{open}{}{close}", render_inlines(content, ctx))
}

fn render_image_inline(
    asset: &AssetRef,
    alt: &str,
    title: Option<&str>,
    attrs: &ImageAttrs,
    ctx: &RenderCtx<'_>,
) -> String {
    match resolve_asset_ref(asset, ctx) {
        Some(ImageRef::External(src)) => {
            let image_attrs = build_confluence_image_attrs(attrs, alt, title);
            format!(
                r#"<span><ac:image{image_attrs}><ri:url ri:value="{}"/></ac:image></span>"#,
                escape_html_attr(&src)
            )
        }
        Some(ImageRef::Attachment(filename)) => {
            let image_attrs = build_confluence_image_attrs(attrs, alt, title);
            format!(
                r#"<span><ac:image{image_attrs}><ri:attachment ri:filename="{}"/></ac:image></span>"#,
                escape_html_attr(&filename)
            )
        }
        None => String::new(),
    }
}

fn render_image_block(
    asset: &AssetRef,
    alt: &str,
    title: Option<&str>,
    attrs: &ImageAttrs,
    caption: Option<&[Block]>,
    ctx: &RenderCtx<'_>,
    out: &mut String,
) {
    let mut caption_html = String::new();
    if let Some(caption_blocks) = caption {
        render_blocks(caption_blocks, ctx, &mut caption_html);
    }
    let has_caption = !caption_html.trim().is_empty();

    match resolve_asset_ref(asset, ctx) {
        Some(ImageRef::External(src)) => {
            let image_attrs = build_confluence_image_attrs(attrs, alt, title);
            out.push_str(&format!(
                r#"<ac:image{image_attrs}><ri:url ri:value="{}"/>"#,
                escape_html_attr(&src)
            ));
            if has_caption {
                out.push_str("<ac:caption>");
                out.push_str(&caption_html);
                out.push_str("</ac:caption>");
            }
            out.push_str("</ac:image>");
        }
        Some(ImageRef::Attachment(filename)) => {
            let image_attrs = build_confluence_image_attrs(attrs, alt, title);
            out.push_str(&format!(
                r#"<ac:image{image_attrs}><ri:attachment ri:filename="{}"/>"#,
                escape_html_attr(&filename)
            ));
            if has_caption {
                out.push_str("<ac:caption>");
                out.push_str(&caption_html);
                out.push_str("</ac:caption>");
            }
            out.push_str("</ac:image>");
        }
        None => {}
    }
}

fn resolve_asset_ref(asset_ref: &AssetRef, ctx: &RenderCtx<'_>) -> Option<ImageRef> {
    let asset = match ctx.document.assets.get(&asset_ref.0) {
        Some(asset) => asset,
        None => return Some(ImageRef::Attachment(asset_ref.0.0.clone())),
    };

    let (source, variants) = match asset {
        Asset::Image(image) => (&image.source, &image.variants),
        Asset::Video(media) | Asset::Audio(media) => (&media.source, &media.variants),
        Asset::File(file) => (&file.source, &file.variants),
        Asset::Custom(custom) => (&custom.source, &custom.variants),
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
        return Some(classify_image_value(&url));
    }

    match source {
        AssetSource::RemoteUrl { url } => Some(ImageRef::External(url.0.clone())),
        AssetSource::DataUri { uri } => Some(ImageRef::External(uri.clone())),
        AssetSource::LocalPath { path } => {
            if let Some(resolved) =
                image_utils::resolve_image_reference_url(path.as_str(), ctx.marker_url_map)
            {
                return Some(classify_image_value(&resolved));
            }
            let filename = Path::new(path.as_str())
                .file_name()
                .and_then(|v| v.to_str())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| path.as_str().to_string());
            Some(ImageRef::Attachment(filename))
        }
    }
}

fn classify_image_value(value: &str) -> ImageRef {
    if is_external_like(value) {
        ImageRef::External(value.to_string())
    } else {
        ImageRef::Attachment(value.to_string())
    }
}

fn is_external_like(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://") || value.starts_with("data:")
}

fn build_confluence_image_attrs(attrs: &ImageAttrs, alt: &str, title: Option<&str>) -> String {
    let mut normalized = BTreeMap::<String, String>::new();
    for key in [
        "align",
        "border",
        "class",
        "title",
        "style",
        "thumbnail",
        "alt",
        "height",
        "width",
        "vspace",
        "hspace",
    ] {
        if let Some(value) = attrs.passthrough.get(key)
            && !value.is_empty()
        {
            normalized.insert(key.to_string(), value.clone());
        }
    }

    if let Some(align) = attrs.align {
        let align = match align {
            TextAlign::Left => "left",
            TextAlign::Center => "center",
            TextAlign::Right => "right",
        };
        normalized.insert("align".to_string(), align.to_string());
    }
    if let Some(width) = attrs.width {
        normalized.insert("width".to_string(), width.to_string());
    }
    if let Some(height) = attrs.height {
        normalized.insert("height".to_string(), height.to_string());
    }
    if let Some(title) = title
        && !title.is_empty()
    {
        normalized.insert("title".to_string(), title.to_string());
    }
    if !alt.is_empty() {
        normalized.insert("alt".to_string(), alt.to_string());
    }

    let mut out = String::new();
    for key in [
        "align",
        "border",
        "class",
        "title",
        "style",
        "thumbnail",
        "alt",
        "height",
        "width",
        "vspace",
        "hspace",
    ] {
        if let Some(value) = normalized.get(key) {
            out.push_str(&format!(r#" ac:{key}="{}""#, escape_html_attr(value)));
        }
    }
    out
}

fn render_math_inline(payload: &RenderPayload, ctx: &RenderCtx<'_>) -> Option<String> {
    if let Some(latex) = payload_to_latex(payload) {
        return Some(render_latex_inline(&latex, ctx.app_id, ctx.env_id));
    }

    match &payload.rendered {
        Some(RenderedArtifact::Svg(svg)) | Some(RenderedArtifact::MathMl(svg)) => Some(svg.clone()),
        Some(RenderedArtifact::Asset {
            asset,
            width,
            height,
            ..
        }) => {
            let size_attrs = build_image_size_attrs(*width, *height);
            match resolve_asset_ref(asset, ctx) {
                Some(ImageRef::External(url)) => Some(format!(
                    r#"<span><ac:image{size_attrs}><ri:url ri:value="{}"/></ac:image></span>"#,
                    escape_html_attr(&url)
                )),
                Some(ImageRef::Attachment(filename)) => Some(format!(
                    r#"<span><ac:image{size_attrs}><ri:attachment ri:filename="{}"/></ac:image></span>"#,
                    escape_html_attr(&filename)
                )),
                None => None,
            }
        }
        Some(RenderedArtifact::Custom { .. }) | None => None,
    }
}

fn render_math_block(payload: &RenderPayload, ctx: &RenderCtx<'_>) -> Option<String> {
    if let Some(latex) = payload_to_latex(payload) {
        return Some(render_latex_block(&latex, ctx.app_id, ctx.env_id));
    }

    match &payload.rendered {
        Some(RenderedArtifact::Svg(svg)) | Some(RenderedArtifact::MathMl(svg)) => {
            Some(format!(r#"<div style="text-align: center;">{svg}</div>"#))
        }
        Some(RenderedArtifact::Asset {
            asset,
            width,
            height,
            ..
        }) => {
            let size_attrs = build_image_size_attrs(*width, *height);
            match resolve_asset_ref(asset, ctx) {
                Some(ImageRef::External(url)) => Some(format!(
                    r#"<ac:image{size_attrs}><ri:url ri:value="{}"/></ac:image>"#,
                    escape_html_attr(&url)
                )),
                Some(ImageRef::Attachment(filename)) => Some(format!(
                    r#"<ac:image{size_attrs}><ri:attachment ri:filename="{}"/></ac:image>"#,
                    escape_html_attr(&filename)
                )),
                None => None,
            }
        }
        Some(RenderedArtifact::Custom { .. }) | None => None,
    }
}

fn payload_to_latex(payload: &RenderPayload) -> Option<String> {
    match &payload.src {
        Some(MathSource::Latex(latex)) => Some(latex.clone()),
        Some(MathSource::Typst(typst)) => Some(typst_math_to_latex(typst)),
        Some(MathSource::Custom { src, .. }) => Some(src.clone()),
        None => None,
    }
}

fn render_footnotes(document: &Document, ctx: &RenderCtx<'_>, out: &mut String) {
    for (id, definition) in &document.footnotes {
        render_footnote(id, definition, ctx, out);
    }
}

fn render_footnote(
    id: &FootnoteId,
    definition: &FootnoteDef,
    ctx: &RenderCtx<'_>,
    out: &mut String,
) {
    let id_attr = escape_html_attr(&id.0);
    let id_text = escape_html_text(&id.0);

    if definition.blocks.is_empty() {
        out.push_str(&format!(r#"<p id="fn-{id_attr}">[{id_text}]</p>"#));
        return;
    }

    if let Some(first) = definition.blocks.first() {
        match first {
            Block::Paragraph { content, .. } | Block::Heading { content, .. } => {
                out.push_str(&format!(
                    r#"<p id="fn-{id_attr}">[{id_text}] {}</p>"#,
                    render_inlines(content, ctx)
                ));
                render_blocks(&definition.blocks[1..], ctx, out);
            }
            _ => {
                out.push_str(&format!(r#"<p id="fn-{id_attr}">[{id_text}]</p>"#));
                render_blocks(&definition.blocks, ctx, out);
            }
        }
    }
}

fn escape_xml_content(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Build image size attributes for Confluence ac:image tag.
fn build_image_size_attrs(width: Option<u32>, height: Option<u32>) -> String {
    let mut attrs = String::new();
    if let Some(w) = width {
        attrs.push_str(&format!(r#" ac:width="{}""#, w));
    }
    if let Some(h) = height {
        attrs.push_str(&format!(r#" ac:height="{}""#, h));
    }
    attrs
}

/// Render LaTeX math as Confluence inline equation using legacy macro format.
/// Uses the LaTeX Math plugin's "mathinline" macro with INLINE output type.
fn render_latex_inline_legacy(latex: &str) -> String {
    // Escape CDATA end sequence for XML
    let escaped = latex.replace("]]>", "]]]]><![CDATA[>");
    format!(
        r#"<ac:structured-macro ac:name="mathinline"><ac:parameter ac:name="body"><![CDATA[{escaped}]]></ac:parameter><ac:parameter ac:name="atlassian-macro-output-type">INLINE</ac:parameter></ac:structured-macro>"#
    )
}

/// Render LaTeX math as Confluence block equation using legacy macro format.
/// Uses the LaTeX Math plugin's "mathblock" macro with BLOCK output type.
fn render_latex_block_legacy(latex: &str) -> String {
    // Escape CDATA end sequence for XML
    let escaped = latex.replace("]]>", "]]]]><![CDATA[>");
    format!(
        r#"<ac:structured-macro ac:name="mathblock"><ac:plain-text-body><![CDATA[{escaped}]]></ac:plain-text-body><ac:parameter ac:name="atlassian-macro-output-type">BLOCK</ac:parameter></ac:structured-macro>"#
    )
}

/// Render LaTeX math as Confluence inline equation.
/// Uses ADF extension format if both app_id and env_id are configured,
/// otherwise falls back to legacy macro format.
fn render_latex_inline(latex: &str, app_id: Option<&str>, env_id: Option<&str>) -> String {
    match (app_id, env_id) {
        (Some(app_id), Some(env_id)) => {
            let escaped = escape_xml_content(latex);
            let extension_key = format!("{app_id}/{env_id}/static/mathinline");
            format!(
                r#"<span><ac:adf-extension><ac:adf-node type="inline-extension"><ac:adf-attribute key="extension-key">{extension_key}</ac:adf-attribute><ac:adf-attribute key="extension-type">com.atlassian.ecosystem</ac:adf-attribute><ac:adf-attribute key="parameters"><ac:adf-parameter key="guest-params"><ac:adf-parameter key="latex">{escaped}</ac:adf-parameter><ac:adf-parameter key="is-large-size" type="boolean">false</ac:adf-parameter><ac:adf-parameter key="data"></ac:adf-parameter></ac:adf-parameter></ac:adf-attribute></ac:adf-node><ac:adf-fallback><ac:adf-node type="inline-extension"><ac:adf-attribute key="extension-key">{extension_key}</ac:adf-attribute><ac:adf-attribute key="extension-type">com.atlassian.ecosystem</ac:adf-attribute><ac:adf-attribute key="parameters"><ac:adf-parameter key="guest-params"><ac:adf-parameter key="latex">{escaped}</ac:adf-parameter><ac:adf-parameter key="is-large-size" type="boolean">false</ac:adf-parameter><ac:adf-parameter key="data"></ac:adf-parameter></ac:adf-parameter></ac:adf-attribute></ac:adf-node></ac:adf-fallback></ac:adf-extension></span>"#
            )
        }
        _ => render_latex_inline_legacy(latex),
    }
}

/// Render LaTeX math as Confluence block equation.
/// Uses ADF extension format if both app_id and env_id are configured,
/// otherwise falls back to legacy macro format.
fn render_latex_block(latex: &str, app_id: Option<&str>, env_id: Option<&str>) -> String {
    match (app_id, env_id) {
        (Some(app_id), Some(env_id)) => {
            let escaped = escape_xml_content(latex);
            let extension_key = format!("{app_id}/{env_id}/static/mathblock");
            format!(
                r#"<ac:adf-extension><ac:adf-node type="extension"><ac:adf-attribute key="extension-key">{extension_key}</ac:adf-attribute><ac:adf-attribute key="extension-type">com.atlassian.ecosystem</ac:adf-attribute><ac:adf-attribute key="parameters"><ac:adf-parameter key="guest-params"><ac:adf-parameter key="latex">{escaped}</ac:adf-parameter><ac:adf-parameter key="is-large-size" type="boolean">false</ac:adf-parameter><ac:adf-parameter key="alignment">center</ac:adf-parameter><ac:adf-parameter key="anchor"></ac:adf-parameter><ac:adf-parameter key="data"></ac:adf-parameter></ac:adf-parameter></ac:adf-attribute></ac:adf-node><ac:adf-fallback><ac:adf-node type="extension"><ac:adf-attribute key="extension-key">{extension_key}</ac:adf-attribute><ac:adf-attribute key="extension-type">com.atlassian.ecosystem</ac:adf-attribute><ac:adf-attribute key="parameters"><ac:adf-parameter key="guest-params"><ac:adf-parameter key="latex">{escaped}</ac:adf-parameter><ac:adf-parameter key="is-large-size" type="boolean">false</ac:adf-parameter><ac:adf-parameter key="alignment">center</ac:adf-parameter><ac:adf-parameter key="anchor"></ac:adf-parameter><ac:adf-parameter key="data"></ac:adf-parameter></ac:adf-parameter></ac:adf-attribute></ac:adf-node></ac:adf-fallback></ac:adf-extension>"#
            )
        }
        _ => render_latex_block_legacy(latex),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use std::collections::BTreeMap;
    use typub_html::{
        asset_variant, bullet_list_text, code_block, document, heading_text, math_block_latex,
        paragraph, paragraph_text,
    };
    use typub_ir::{
        AnchorId, AssetId, AssetSource, BlockAttrs, Document, ExtensionKind, FootnoteDef,
        FootnoteId, HeadingLevel, ImageAsset, Inline, RelativePath,
    };

    const TEST_APP_ID: &str = "test-app-id";
    const TEST_ENV_ID: &str = "test-env-id";

    fn to_html(doc: &Document) -> String {
        elements_to_confluence_html(
            doc,
            Path::new(""),
            &HashMap::new(),
            Some(TEST_APP_ID),
            Some(TEST_ENV_ID),
        )
    }

    fn to_html_no_plugin(doc: &Document) -> String {
        elements_to_confluence_html(doc, Path::new(""), &HashMap::new(), None, None)
    }

    #[test]
    fn heading_block_renders() {
        let doc = document(vec![heading_text(2, "Title")]);
        assert_eq!(to_html(&doc), "<h2>Title</h2>");
    }

    #[test]
    fn bullet_list_renders() {
        let doc = document(vec![bullet_list_text(&["a", "b"])]);
        assert_eq!(to_html(&doc), "<ul><li>a</li><li>b</li></ul>");
    }

    #[test]
    fn block_math_renders_adf_macro() {
        let doc = document(vec![math_block_latex("E = mc^2")]);
        let html = to_html(&doc);
        assert!(html.contains("static/mathblock"));
        assert!(html.contains("E = mc^2"));
    }

    #[test]
    fn block_math_fallback_to_legacy_macro_when_no_plugin_config() {
        let doc = document(vec![math_block_latex("E = mc^2")]);
        let html = to_html_no_plugin(&doc);
        // Should use legacy macro format
        assert!(html.contains(r#"ac:structured-macro ac:name="mathblock""#));
        assert!(html.contains(r#"<ac:plain-text-body><![CDATA[E = mc^2]]></ac:plain-text-body>"#));
        assert!(html.contains(r#"ac:parameter ac:name="atlassian-macro-output-type">BLOCK"#));
        // Should NOT contain ADF extension format
        assert!(!html.contains("adf-extension"));
    }

    #[test]
    fn inline_math_fallback_to_legacy_macro_when_no_plugin_config() {
        // Note: When paragraph contains only MathInline, it's rendered as block.
        // To test inline math, add surrounding text.
        let doc = document(vec![paragraph(vec![
            Inline::Text("The formula is ".to_string()),
            Inline::MathInline {
                math: RenderPayload {
                    src: Some(MathSource::Latex("x^2".to_string())),
                    rendered: None,
                    id: None,
                },
                attrs: Default::default(),
            },
            Inline::Text(".".to_string()),
        ])]);
        let html = to_html_no_plugin(&doc);
        // Should use legacy macro format for inline math
        assert!(html.contains(r#"ac:structured-macro ac:name="mathinline""#));
        assert!(html.contains(r#"<ac:parameter ac:name="body"><![CDATA[x^2]]></ac:parameter>"#));
        assert!(html.contains(r#"ac:parameter ac:name="atlassian-macro-output-type">INLINE"#));
        // Should NOT contain ADF extension format
        assert!(!html.contains("adf-extension"));
    }

    #[test]
    fn block_math_uses_adf_when_both_ids_configured() {
        let doc = document(vec![math_block_latex("E = mc^2")]);
        let html = to_html(&doc);
        // Should use ADF extension format
        assert!(html.contains("adf-extension"));
        assert!(html.contains("test-app-id/test-env-id/static/mathblock"));
        // Should NOT contain legacy macro format
        assert!(!html.contains(r#"ac:structured-macro ac:name="mathblock""#));
    }

    #[test]
    fn inline_math_uses_adf_when_both_ids_configured() {
        // Note: When paragraph contains only MathInline, it's rendered as block.
        // To test inline math, add surrounding text.
        let doc = document(vec![paragraph(vec![
            Inline::Text("The formula is ".to_string()),
            Inline::MathInline {
                math: RenderPayload {
                    src: Some(MathSource::Latex("x^2".to_string())),
                    rendered: None,
                    id: None,
                },
                attrs: Default::default(),
            },
            Inline::Text(".".to_string()),
        ])]);
        let html = to_html(&doc);
        // Should use ADF extension format for inline math
        assert!(html.contains("adf-extension"));
        assert!(html.contains("static/mathinline"));
        // Should NOT contain legacy macro format
        assert!(!html.contains(r#"ac:structured-macro ac:name="mathinline""#));
    }

    #[test]
    fn code_block_preserves_newlines_in_cdata() {
        let doc = document(vec![code_block(
            "fn main() {\n    println!(\"hi\");\n}",
            "rust",
        )]);
        let html = to_html(&doc);
        assert!(html.contains(
            "<ac:plain-text-body><![CDATA[fn main() {\n    println!(\"hi\");\n}]]></ac:plain-text-body>"
        ));
    }

    #[test]
    fn image_with_attachment_variant_uses_ri_attachment() {
        let asset_id = AssetId("img1".to_string());
        let rel = RelativePath::new("assets/photo.png".to_string()).expect("relative path");
        let mut assets = BTreeMap::new();
        assets.insert(
            asset_id.clone(),
            Asset::Image(ImageAsset {
                source: AssetSource::LocalPath { path: rel },
                meta: None,
                variants: vec![asset_variant("original", "photo.png", None, None)],
            }),
        );
        let doc = Document {
            blocks: vec![paragraph(vec![Inline::Image {
                asset: AssetRef(asset_id),
                alt: "pic".to_string(),
                title: None,
                attrs: Default::default(),
            }])],
            footnotes: BTreeMap::new(),
            assets,
            meta: Default::default(),
        };

        let html = to_html(&doc);
        assert!(html.contains(r#"<ri:attachment ri:filename="photo.png"/>"#));
    }

    #[test]
    fn custom_admonition_macro_name_is_lowercased() {
        let kind = ExtensionKind::new("Acme/Panel".to_string()).expect("extension kind");
        let doc = document(vec![Block::Admonition {
            kind: AdmonitionKind::Custom(kind),
            title: None,
            blocks: vec![paragraph_text("hello")],
            attrs: BlockAttrs::default(),
        }]);

        let html = to_html(&doc);
        assert!(html.contains(r#"ac:name="acme/panel""#));
    }

    #[test]
    fn missing_asset_reference_falls_back_to_attachment() {
        let doc = document(vec![paragraph(vec![Inline::Image {
            asset: AssetRef(AssetId("missing-asset".to_string())),
            alt: "missing".to_string(),
            title: None,
            attrs: Default::default(),
        }])]);

        let html = to_html(&doc);
        assert!(html.contains(r#"<ri:attachment ri:filename="missing-asset"/>"#));
        assert!(!html.contains("<ri:url"));
    }

    #[test]
    fn absolute_variant_path_is_treated_as_attachment() {
        let asset_id = AssetId("img2".to_string());
        let rel = RelativePath::new("assets/photo.png".to_string()).expect("relative path");
        let mut assets = BTreeMap::new();
        assets.insert(
            asset_id.clone(),
            Asset::Image(ImageAsset {
                source: AssetSource::LocalPath { path: rel },
                meta: None,
                variants: vec![asset_variant("original", "/assets/photo.png", None, None)],
            }),
        );
        let doc = Document {
            blocks: vec![paragraph(vec![Inline::Image {
                asset: AssetRef(asset_id),
                alt: "pic".to_string(),
                title: None,
                attrs: Default::default(),
            }])],
            footnotes: BTreeMap::new(),
            assets,
            meta: Default::default(),
        };

        let html = to_html(&doc);
        assert!(html.contains(r#"<ri:attachment ri:filename="/assets/photo.png"/>"#));
        assert!(!html.contains("<ri:url"));
    }

    #[test]
    fn figure_with_single_image_maps_caption_to_ac_caption() {
        let asset_id = AssetId("img3".to_string());
        let rel = RelativePath::new("assets/photo.png".to_string()).expect("relative path");
        let mut assets = BTreeMap::new();
        assets.insert(
            asset_id.clone(),
            Asset::Image(ImageAsset {
                source: AssetSource::LocalPath { path: rel },
                meta: None,
                variants: vec![asset_variant("original", "photo.png", None, None)],
            }),
        );
        let doc = Document {
            blocks: vec![Block::Figure {
                content: vec![paragraph(vec![Inline::Image {
                    asset: AssetRef(asset_id),
                    alt: "pic".to_string(),
                    title: None,
                    attrs: Default::default(),
                }])],
                caption: Some(vec![paragraph_text("Figure Caption")]),
                attrs: BlockAttrs::default(),
            }],
            footnotes: BTreeMap::new(),
            assets,
            meta: Default::default(),
        };

        let html = to_html(&doc);
        assert!(html.contains("<ac:image"));
        assert!(html.contains(r#"<ri:attachment ri:filename="photo.png"/>"#));
        assert!(html.contains("<ac:caption><p>Figure Caption</p></ac:caption>"));
        assert!(!html.contains("<figure>"));
        assert!(!html.contains("<figcaption>"));
    }

    #[test]
    fn footnote_does_not_wrap_block_html_inside_paragraph() {
        let mut doc = document(vec![paragraph(vec![Inline::FootnoteRef(FootnoteId(
            "f1".to_string(),
        ))])]);
        doc.footnotes.insert(
            FootnoteId("f1".to_string()),
            FootnoteDef {
                blocks: vec![paragraph_text("first line"), bullet_list_text(&["item"])],
            },
        );

        let html = to_html(&doc);
        assert!(html.contains(r#"<p id="fn-f1">[f1] first line</p>"#));
        assert!(html.contains("<ul><li>item</li></ul>"));
        assert!(!html.contains(r#"<p id="fn-f1">[f1] <p>"#));
    }

    #[test]
    fn heading_with_id_renders_anchor_macro() {
        let doc = document(vec![Block::Heading {
            level: HeadingLevel::new(2).expect("valid heading level"),
            id: Some(AnchorId("sec-intro".to_string())),
            content: vec![Inline::Text("Introduction".to_string())],
            attrs: BlockAttrs::default(),
        }]);

        let html = to_html(&doc);
        // Should render anchor macro before the heading
        assert!(html.contains(r#"<ac:structured-macro ac:name="anchor" ac:schema-version="1"><ac:parameter ac:name="">sec-intro</ac:parameter></ac:structured-macro>"#));
        assert!(html.contains("<h2>Introduction</h2>"));
    }

    #[test]
    fn heading_without_id_renders_without_anchor_macro() {
        let doc = document(vec![Block::Heading {
            level: HeadingLevel::new(2).expect("valid heading level"),
            id: None,
            content: vec![Inline::Text("Introduction".to_string())],
            attrs: BlockAttrs::default(),
        }]);

        let html = to_html(&doc);
        assert!(html.contains("<h2>Introduction</h2>"));
        assert!(!html.contains(r#"<ac:structured-macro ac:name="anchor">"#));
    }

    #[test]
    fn paragraph_with_id_in_passthrough_renders_anchor_macro() {
        let mut passthrough = BTreeMap::new();
        passthrough.insert("id".to_string(), "para-1".to_string());
        let doc = document(vec![Block::Paragraph {
            content: vec![Inline::Text("Some text".to_string())],
            attrs: BlockAttrs {
                classes: vec![],
                style: None,
                passthrough,
            },
        }]);

        let html = to_html(&doc);
        // Should render anchor macro before the paragraph
        assert!(html.contains(r#"<ac:structured-macro ac:name="anchor" ac:schema-version="1"><ac:parameter ac:name="">para-1</ac:parameter></ac:structured-macro>"#));
        assert!(html.contains("<p>Some text</p>"));
    }

    #[test]
    fn figure_with_id_in_passthrough_renders_anchor_macro() {
        let mut passthrough = BTreeMap::new();
        passthrough.insert("id".to_string(), "fig-example".to_string());
        let asset_id = AssetId("test-asset".to_string());
        let mut doc = document(vec![Block::Figure {
            content: vec![paragraph(vec![Inline::Image {
                asset: AssetRef(asset_id.clone()),
                alt: "test image".to_string(),
                title: None,
                attrs: ImageAttrs::default(),
            }])],
            caption: None,
            attrs: BlockAttrs {
                classes: vec![],
                style: None,
                passthrough,
            },
        }]);
        doc.assets.insert(
            asset_id,
            Asset::Image(ImageAsset {
                source: AssetSource::LocalPath {
                    path: RelativePath::new("test.png".to_string()).expect("valid relative path"),
                },
                meta: None,
                variants: vec![],
            }),
        );

        let html = to_html(&doc);
        // Should render anchor macro before the figure (even for single-image figures)
        assert!(html.contains(r#"<ac:structured-macro ac:name="anchor" ac:schema-version="1"><ac:parameter ac:name="">fig-example</ac:parameter></ac:structured-macro>"#));
    }
}
