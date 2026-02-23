//! Code block parsing and highlighted HTML reconstruction.

use scraper::{ElementRef, Node, Selector};

use typub_ir::{Block, BlockAttrs, Inline};

const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

pub(crate) fn parse_pre_block(el: ElementRef, attrs: BlockAttrs) -> Block {
    if let Ok(code_sel) = Selector::parse("code")
        && let Some(code_el) = el.select(&code_sel).next()
    {
        let inner_html = code_el.inner_html();
        let code = extract_code_text(code_el);
        let language = code_el
            .value()
            .attr("data-lang")
            .map(str::to_string)
            .or_else(|| {
                code_el.value().attr("class").and_then(|class| {
                    class
                        .split_whitespace()
                        .find_map(|c| c.strip_prefix("language-"))
                        .map(str::to_string)
                })
            });
        let highlighted_html = if inner_html.contains("<span") {
            Some(rebuild_highlighted_html(&code_el))
        } else {
            None
        };

        return Block::CodeBlock {
            code,
            language,
            filename: None,
            highlight_lines: Vec::new(),
            highlighted_html,
            attrs,
        };
    }

    Block::CodeBlock {
        code: extract_code_text(el),
        language: None,
        filename: None,
        highlight_lines: Vec::new(),
        highlighted_html: None,
        attrs,
    }
}

pub(crate) fn parse_standalone_code(el: ElementRef, attrs: BlockAttrs) -> Vec<Block> {
    let text = el.text().collect::<String>();
    vec![Block::Paragraph {
        content: vec![Inline::Code(text)],
        attrs,
    }]
}

fn escape_html_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn extract_code_text(root: ElementRef<'_>) -> String {
    let mut out = String::new();
    extract_code_text_rec(root, &mut out);

    while out.ends_with('\n') {
        out.pop();
    }
    out
}

fn extract_code_text_rec(root: ElementRef<'_>, out: &mut String) {
    for child in root.children() {
        match child.value() {
            Node::Text(text) => out.push_str(&text.text.replace('\u{00A0}', " ")),
            Node::Element(el) if el.name() == "br" => out.push('\n'),
            Node::Element(_) => {
                if let Some(child_el) = ElementRef::wrap(child) {
                    extract_code_text_rec(child_el, out);
                }
            }
            _ => {}
        }
    }
}

fn rebuild_highlighted_html(code: &ElementRef<'_>) -> String {
    let mut out = String::new();
    for child in code.children() {
        match child.value() {
            Node::Text(text) => {
                if !text.text.is_empty() {
                    out.push_str("<span>");
                    out.push_str(&escape_html_text(&text.text).replace(' ', "\u{00A0}"));
                    out.push_str("</span>");
                }
            }
            Node::Element(el) => {
                let tag = el.name();
                out.push('<');
                out.push_str(tag);
                for (name, value) in el.attrs() {
                    out.push_str(&format!(r#" {}="{}""#, name, escape_html_text(value)));
                }
                out.push('>');
                if !VOID_ELEMENTS.contains(&tag) {
                    if let Some(el_ref) = ElementRef::wrap(child) {
                        out.push_str(&rebuild_highlighted_html_inner(&el_ref));
                    }
                    out.push_str("</");
                    out.push_str(tag);
                    out.push('>');
                }
            }
            _ => {}
        }
    }
    out
}

fn rebuild_highlighted_html_inner(element: &ElementRef<'_>) -> String {
    let mut out = String::new();
    for child in element.children() {
        match child.value() {
            Node::Text(text) => out.push_str(&escape_html_text(&text.text)),
            Node::Element(el) => {
                let tag = el.name();
                out.push('<');
                out.push_str(tag);
                for (name, value) in el.attrs() {
                    out.push_str(&format!(r#" {}="{}""#, name, escape_html_text(value)));
                }
                out.push('>');
                if !VOID_ELEMENTS.contains(&tag) {
                    if let Some(el_ref) = ElementRef::wrap(child) {
                        out.push_str(&rebuild_highlighted_html_inner(&el_ref));
                    }
                    out.push_str("</");
                    out.push_str(tag);
                    out.push('>');
                }
            }
            _ => {}
        }
    }
    out
}
