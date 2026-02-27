//! Inline parsing.

use scraper::{ElementRef, Node};
use std::collections::BTreeMap;

use typub_ir::{
    FootnoteId, Inline, InlineSeq, MathPayload, RenderedMath, StyleSet, SvgPayload, TextStyle, Url,
};

use super::{
    ParseCtx, normalize_footnote_label, normalize_text_content, parse_image_attrs,
    parse_inline_attrs, parse_math_source,
};

pub(crate) fn parse_inline_children(parent: ElementRef, ctx: &mut ParseCtx) -> InlineSeq {
    let mut out = Vec::new();
    for child in parent.children() {
        match child.value() {
            Node::Text(text) => {
                if let Some(t) = normalize_text_content(text)
                    && !t.is_empty()
                {
                    out.push(Inline::Text(t));
                }
            }
            Node::Element(_) => {
                if let Some(el) = ElementRef::wrap(child) {
                    out.extend(parse_inline_element(el, ctx));
                }
            }
            _ => {}
        }
    }
    out
}

pub(crate) fn parse_inline_element(el: ElementRef, ctx: &mut ParseCtx) -> InlineSeq {
    let tag = el.value().name();

    if tag == "span"
        && let Some(class) = el.value().attr("class")
        && class.contains("typst-svg-inline")
        && el.inner_html().contains("<svg")
    {
        let src = parse_math_source(el);
        return vec![Inline::MathInline {
            math: MathPayload {
                src,
                rendered: Some(RenderedMath::Svg(el.inner_html())),
                id: None,
            },
            attrs: parse_inline_attrs(&el),
        }];
    }

    match tag {
        "br" => vec![Inline::HardBreak],
        "svg" => vec![Inline::SvgInline {
            svg: SvgPayload {
                src: None,
                rendered: Some(RenderedMath::Svg(el.html())),
                id: None,
            },
            attrs: parse_inline_attrs(&el),
        }],
        "code" => {
            let text = el.text().collect::<String>();
            vec![Inline::Code(text)]
        }
        "a" => {
            if let Some(id) = parse_footnote_ref_from_anchor(el) {
                return vec![Inline::FootnoteRef(id)];
            }
            let content = parse_inline_children(el, ctx);
            if let Some(href) = el.value().attr("href") {
                vec![Inline::Link {
                    content,
                    href: Url(href.to_string()),
                    title: el.value().attr("title").map(str::to_string),
                    attrs: parse_inline_attrs(&el),
                }]
            } else {
                content
            }
        }
        "img" => {
            let Some(src) = el.value().attr("src") else {
                return vec![Inline::UnknownInline {
                    tag: "img".to_string(),
                    attrs: parse_inline_attrs(&el),
                    content: Vec::new(),
                    data: BTreeMap::new(),
                    note: Some("missing src attribute".to_string()),
                    source: Some(el.html()),
                }];
            };

            let width = el.value().attr("width").and_then(|s| s.parse().ok());
            let height = el.value().attr("height").and_then(|s| s.parse().ok());
            let Some(asset) = ctx.register_image(src, width, height) else {
                return vec![Inline::UnknownInline {
                    tag: "img".to_string(),
                    attrs: parse_inline_attrs(&el),
                    content: Vec::new(),
                    data: BTreeMap::new(),
                    note: Some("invalid image source".to_string()),
                    source: Some(el.html()),
                }];
            };

            vec![Inline::Image {
                asset,
                alt: el.value().attr("alt").unwrap_or_default().to_string(),
                title: el.value().attr("title").map(str::to_string),
                attrs: parse_image_attrs(&el, width, height),
            }]
        }
        "strong" | "b" => parse_styled(el, ctx, TextStyle::Bold),
        "em" | "i" => parse_styled(el, ctx, TextStyle::Italic),
        "del" | "s" | "strike" => parse_styled(el, ctx, TextStyle::Strikethrough),
        "u" => parse_styled(el, ctx, TextStyle::Underline),
        "mark" => parse_styled(el, ctx, TextStyle::Mark),
        "sup" => {
            if let Some(id) = parse_footnote_ref_from_sup(el) {
                vec![Inline::FootnoteRef(id)]
            } else {
                parse_styled(el, ctx, TextStyle::Superscript)
            }
        }
        "sub" => parse_styled(el, ctx, TextStyle::Subscript),
        "kbd" => parse_styled(el, ctx, TextStyle::Kbd),
        "span" => parse_inline_children(el, ctx),
        _ => vec![Inline::UnknownInline {
            tag: tag.to_string(),
            attrs: parse_inline_attrs(&el),
            content: parse_inline_children(el, ctx),
            data: BTreeMap::new(),
            note: Some("unsupported inline element".to_string()),
            source: Some(el.html()),
        }],
    }
}

fn parse_styled(el: ElementRef, ctx: &mut ParseCtx, style: TextStyle) -> InlineSeq {
    let content = parse_inline_children(el, ctx);
    if content.is_empty() {
        return Vec::new();
    }

    vec![Inline::Styled {
        styles: StyleSet::single(style),
        content,
        attrs: parse_inline_attrs(&el),
    }]
}

fn parse_footnote_id_str(s: &str) -> Option<FootnoteId> {
    let num = s.parse::<u64>().ok()?;
    Some(FootnoteId(num))
}

fn parse_footnote_ref_from_sup(el: ElementRef<'_>) -> Option<FootnoteId> {
    let mut footnote_id: Option<FootnoteId> = None;

    for child in el.children() {
        match child.value() {
            Node::Text(text) => {
                if !text.text.trim().is_empty() {
                    return None;
                }
            }
            Node::Element(node) => {
                if node.name() != "a" {
                    return None;
                }
                if footnote_id.is_some() {
                    return None;
                }
                let a = ElementRef::wrap(child)?;
                if let Some(id) = parse_footnote_ref_from_anchor(a) {
                    footnote_id = Some(id);
                    continue;
                }
                let href = a.value().attr("href")?;
                let id = href.strip_prefix("#fn-")?;
                if id.is_empty() {
                    return None;
                }
                footnote_id = parse_footnote_id_str(id);
            }
            _ => return None,
        }
    }

    footnote_id
}

fn parse_footnote_ref_from_anchor(el: ElementRef<'_>) -> Option<FootnoteId> {
    let role = el.value().attr("role")?;
    if role != "doc-noteref" {
        return None;
    }

    let text = el.text().collect::<String>();
    if let Some(label) = normalize_footnote_label(&text) {
        return parse_footnote_id_str(&label);
    }

    let href = el.value().attr("href")?;
    if let Some(id) = href.strip_prefix("#fn-") {
        if id.is_empty() {
            None
        } else {
            parse_footnote_id_str(id)
        }
    } else {
        None
    }
}
