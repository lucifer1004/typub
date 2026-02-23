//! Inline rendering utilities for typub HTML IR v2.

use comrak::Arena;
use comrak::nodes::{AstNode, NodeValue};
use typub_html::escape_html_attr;
use typub_ir::{
    Asset, AssetRef, AssetSource, Document, ImageAttrs, Inline, MathSource, RenderPayload,
    RenderedArtifact, TextAlign, TextStyle, Url,
};

use super::MarkdownRenderOptions;

pub fn push_text<'a>(arena: &'a Arena<'a>, parent: &'a AstNode<'a>, text: &str) {
    if text.is_empty() {
        return;
    }
    let node = arena.alloc(NodeValue::Text(text.to_string().into()).into());
    parent.append(node);
}

pub fn inline_text(inline: &Inline) -> String {
    match inline {
        Inline::Text(t) => t.clone(),
        Inline::Code(code) => code.clone(),
        Inline::SoftBreak => " ".to_string(),
        Inline::HardBreak => "<br />".to_string(),
        Inline::Styled { content, .. } => inlines_text(content),
        Inline::Link { content, .. } => inlines_text(content),
        Inline::Image { alt, .. } => alt.clone(),
        Inline::FootnoteRef(id) => format!("[{}]", id.0),
        Inline::MathInline { math, .. } | Inline::SvgInline { svg: math, .. } => {
            math_latex_source(math).unwrap_or_default()
        }
        Inline::UnknownInline { content, .. } => inlines_text(content),
        Inline::RawInline { html, .. } => html.clone(),
    }
}

pub fn inlines_text(inlines: &[Inline]) -> String {
    inlines.iter().map(inline_text).collect::<String>()
}

fn build_inline_image_html(src: &str, alt: &str, attrs: &ImageAttrs) -> String {
    let mut html = format!(r#"<img src="{}""#, escape_html_attr(src));
    if !alt.is_empty() {
        html.push_str(&format!(r#" alt="{}""#, escape_html_attr(alt)));
    }
    if let Some(w) = attrs.width {
        html.push_str(&format!(r#" width="{}""#, w));
    }
    if let Some(h) = attrs.height {
        html.push_str(&format!(r#" height="{}""#, h));
    }
    if let Some(align) = attrs.align {
        let align = match align {
            TextAlign::Left => "left",
            TextAlign::Center => "center",
            TextAlign::Right => "right",
        };
        html.push_str(&format!(r#" align="{}""#, align));
    }
    if let Some(style) = attrs.passthrough.get("style") {
        html.push_str(&format!(r#" style="{}""#, escape_html_attr(style)));
    }
    html.push_str(" />");
    html
}

fn push_inline_image<'a>(
    arena: &'a Arena<'a>,
    parent: &'a AstNode<'a>,
    src: &str,
    alt: &str,
    attrs: &ImageAttrs,
    options: &MarkdownRenderOptions<'_>,
) {
    let use_html = options.use_inline_html_for_sized_images
        && (attrs.width.is_some()
            || attrs.height.is_some()
            || attrs.align.is_some()
            || attrs.passthrough.contains_key("style"));

    if use_html {
        let html = build_inline_image_html(src, alt, attrs);
        parent.append(arena.alloc(NodeValue::HtmlInline(html).into()));
        return;
    }

    let image = arena.alloc(
        NodeValue::Image(Box::new(comrak::nodes::NodeLink {
            url: src.to_string(),
            title: String::new(),
        }))
        .into(),
    );
    if !alt.is_empty() {
        push_text(arena, image, alt);
    }
    parent.append(image);
}

fn resolve_asset_url(
    asset: &AssetRef,
    doc: &Document,
    options: &MarkdownRenderOptions<'_>,
) -> Option<String> {
    if let Some(map) = options.asset_urls
        && let Some(url) = map.get(&asset.0)
    {
        return Some(url.0.clone());
    }

    let model = doc.assets.get(&asset.0)?;
    let image = match model {
        Asset::Image(image) => image,
        _ => return None,
    };

    if let Some(original) = image.variants.iter().find(|v| v.name == "original") {
        return Some(original.publish_url.0.clone());
    }
    if let Some(first) = image.variants.first() {
        return Some(first.publish_url.0.clone());
    }

    match &image.source {
        AssetSource::RemoteUrl { url } => Some(url.0.clone()),
        AssetSource::DataUri { uri } => Some(uri.clone()),
        AssetSource::LocalPath { path } => Some(path.as_str().to_string()),
    }
}

fn math_latex_source(payload: &RenderPayload) -> Option<String> {
    match &payload.src {
        Some(MathSource::Latex(latex)) => Some(latex.clone()),
        Some(MathSource::Typst(typst)) => Some(crate::latex::typst_math_to_latex(typst)),
        Some(MathSource::Custom { src, .. }) => Some(src.clone()),
        None => None,
    }
}

fn render_math_inline<'a>(
    arena: &'a Arena<'a>,
    parent: &'a AstNode<'a>,
    payload: &RenderPayload,
    doc: &Document,
    options: &MarkdownRenderOptions<'_>,
) {
    if let Some(latex) = math_latex_source(payload) {
        let formatted = match options.math_delimiters {
            typub_core::MathDelimiters::Dollar => format!("${}$", latex),
            typub_core::MathDelimiters::Brackets
            | typub_core::MathDelimiters::BracketsInlineDollarBlock => {
                format!(r"\\({}\\)", latex)
            }
        };
        parent.append(arena.alloc(NodeValue::HtmlInline(formatted).into()));
        return;
    }

    match &payload.rendered {
        Some(RenderedArtifact::Svg(svg)) | Some(RenderedArtifact::MathMl(svg)) => {
            parent.append(arena.alloc(NodeValue::HtmlInline(svg.clone()).into()));
        }
        Some(RenderedArtifact::Asset { asset, .. }) => {
            if let Some(src) = resolve_asset_url(asset, doc, options) {
                push_inline_image(arena, parent, &src, "", &ImageAttrs::default(), options);
            }
        }
        Some(RenderedArtifact::Custom { .. }) | None => {}
    }
}

fn wrap_with_style<'a>(
    arena: &'a Arena<'a>,
    parent: &'a AstNode<'a>,
    style: TextStyle,
    content: &[Inline],
    doc: &Document,
    options: &MarkdownRenderOptions<'_>,
) {
    fn wrap_html_tag<'a>(
        arena: &'a Arena<'a>,
        parent: &'a AstNode<'a>,
        tag: &str,
        content: &[Inline],
        doc: &Document,
        options: &MarkdownRenderOptions<'_>,
    ) {
        parent.append(arena.alloc(NodeValue::HtmlInline(format!("<{tag}>")).into()));
        push_inline_seq(arena, parent, content, doc, options);
        parent.append(arena.alloc(NodeValue::HtmlInline(format!("</{tag}>")).into()));
    }

    match style {
        TextStyle::Bold => {
            let node = arena.alloc(NodeValue::Strong.into());
            push_inline_seq(arena, node, content, doc, options);
            parent.append(node);
        }
        TextStyle::Italic => {
            let node = arena.alloc(NodeValue::Emph.into());
            push_inline_seq(arena, node, content, doc, options);
            parent.append(node);
        }
        TextStyle::Strikethrough => {
            let node = arena.alloc(NodeValue::Strikethrough.into());
            push_inline_seq(arena, node, content, doc, options);
            parent.append(node);
        }
        TextStyle::Underline => {
            wrap_html_tag(arena, parent, "u", content, doc, options);
        }
        TextStyle::Mark => {
            wrap_html_tag(arena, parent, "mark", content, doc, options);
        }
        TextStyle::Superscript => {
            wrap_html_tag(arena, parent, "sup", content, doc, options);
        }
        TextStyle::Subscript => {
            wrap_html_tag(arena, parent, "sub", content, doc, options);
        }
        TextStyle::Kbd => {
            wrap_html_tag(arena, parent, "kbd", content, doc, options);
        }
    }
}

pub fn push_inline_seq<'a>(
    arena: &'a Arena<'a>,
    parent: &'a AstNode<'a>,
    inlines: &[Inline],
    doc: &Document,
    options: &MarkdownRenderOptions<'_>,
) {
    for inline in inlines {
        match inline {
            Inline::Text(text) => push_text(arena, parent, text),
            Inline::Code(code) => {
                let node = arena.alloc(
                    NodeValue::Code(comrak::nodes::NodeCode {
                        literal: code.clone(),
                        num_backticks: 1,
                    })
                    .into(),
                );
                parent.append(node);
            }
            Inline::SoftBreak => {}
            Inline::HardBreak => {
                parent.append(arena.alloc(NodeValue::HtmlInline("<br />".to_string()).into()));
            }
            Inline::Styled {
                styles, content, ..
            } => {
                let style_list = styles.styles();
                if style_list.is_empty() {
                    push_inline_seq(arena, parent, content, doc, options);
                    continue;
                }

                let first = style_list[0];
                let mut nested_content = content.to_vec();
                for style in style_list.iter().skip(1).rev() {
                    nested_content = vec![Inline::Styled {
                        styles: typub_ir::StyleSet::single(*style),
                        content: nested_content,
                        attrs: Default::default(),
                    }];
                }
                wrap_with_style(arena, parent, first, &nested_content, doc, options);
            }
            Inline::Link { content, href, .. } => {
                let node = arena.alloc(
                    NodeValue::Link(Box::new(comrak::nodes::NodeLink {
                        url: href.0.clone(),
                        title: String::new(),
                    }))
                    .into(),
                );
                push_inline_seq(arena, node, content, doc, options);
                parent.append(node);
            }
            Inline::Image {
                asset, alt, attrs, ..
            } => {
                if let Some(url) = resolve_asset_url(asset, doc, options) {
                    push_inline_image(arena, parent, &url, alt, attrs, options);
                } else {
                    let marker = format!("<code>[[ASSET:{}]]</code>", asset.0.0);
                    parent.append(arena.alloc(NodeValue::HtmlInline(marker).into()));
                }
            }
            Inline::FootnoteRef(id) => {
                let node = arena.alloc(
                    NodeValue::FootnoteReference(Box::new(comrak::nodes::NodeFootnoteReference {
                        name: format!("fn:{}", id.0),
                        texts: Vec::new(),
                        ref_num: 0,
                        ix: 0,
                    }))
                    .into(),
                );
                parent.append(node);
            }
            Inline::MathInline { math, .. } => {
                render_math_inline(arena, parent, math, doc, options)
            }
            Inline::SvgInline { svg, .. } => render_math_inline(arena, parent, svg, doc, options),
            Inline::UnknownInline {
                content, source, ..
            } => {
                if !content.is_empty() {
                    push_inline_seq(arena, parent, content, doc, options);
                } else if let Some(src) = source {
                    parent.append(arena.alloc(NodeValue::HtmlInline(src.clone()).into()));
                }
            }
            Inline::RawInline { html, .. } => {
                parent.append(arena.alloc(NodeValue::HtmlInline(html.clone()).into()));
            }
        }
    }
}

pub fn resolve_rendered_asset_url(
    rendered: &RenderedArtifact,
    doc: &Document,
    options: &MarkdownRenderOptions<'_>,
) -> Option<String> {
    if let RenderedArtifact::Asset { asset, .. } = rendered {
        if let Some(map) = options.asset_urls
            && let Some(Url(url)) = map.get(&asset.0)
        {
            return Some(url.clone());
        }
        let model = doc.assets.get(&asset.0)?;
        let image = match model {
            Asset::Image(image) => image,
            _ => return None,
        };
        if let Some(original) = image.variants.iter().find(|v| v.name == "original") {
            return Some(original.publish_url.0.clone());
        }
        if let Some(v) = image.variants.first() {
            return Some(v.publish_url.0.clone());
        }
        return match &image.source {
            AssetSource::RemoteUrl { url } => Some(url.0.clone()),
            AssetSource::DataUri { uri } => Some(uri.clone()),
            AssetSource::LocalPath { path } => Some(path.as_str().to_string()),
        };
    }
    None
}
