//! SVG flatten pass for v2 semantic IR.
//!
//! Flattens `RenderedArtifact::Svg` payloads in `Math*`/`Svg*` nodes by
//! resolving `<use>` references through `usvg` while preserving width/height
//! style semantics from Typst-generated SVG.

use anyhow::Result;
use typub_ir::{Block, Document, Inline, RenderPayload, RenderedArtifact};

use super::walk::{NodePath, VisitorMut, walk_document_mut};
use super::{Pass, PassCtx};

#[derive(Debug, Default)]
pub struct FlattenSvgPass;

impl Pass for FlattenSvgPass {
    fn name(&self) -> &'static str {
        "flatten_svg"
    }

    fn run(&mut self, doc: &mut Document, _ctx: &mut PassCtx) -> Result<()> {
        let mut visitor = FlattenVisitor;
        walk_document_mut(doc, &mut visitor)
    }
}

struct FlattenVisitor;

impl FlattenVisitor {
    fn flatten_payload(&mut self, payload: &mut RenderPayload, _path: &NodePath) {
        let Some(RenderedArtifact::Svg(svg)) = payload.rendered.as_ref() else {
            return;
        };
        payload.rendered = Some(RenderedArtifact::Svg(flatten_svg(svg)));
    }
}

impl VisitorMut for FlattenVisitor {
    fn visit_block(&mut self, block: &mut Block, path: &NodePath) -> Result<()> {
        match block {
            Block::MathBlock { math, .. } => self.flatten_payload(math, path),
            Block::SvgBlock { svg, .. } => self.flatten_payload(svg, path),
            _ => {}
        }
        Ok(())
    }

    fn visit_inline(&mut self, inline: &mut Inline, path: &NodePath) -> Result<()> {
        match inline {
            Inline::MathInline { math, .. } => self.flatten_payload(math, path),
            Inline::SvgInline { svg, .. } => self.flatten_payload(svg, path),
            _ => {}
        }
        Ok(())
    }
}

fn flatten_svg(svg: &str) -> String {
    use scraper::{Html, Selector};

    let orig_style_width_height = {
        let fragment = Html::parse_fragment(svg);
        if let Ok(sel) = Selector::parse("svg") {
            fragment
                .select(&sel)
                .next()
                .and_then(|elem| elem.value().attr("style"))
                .and_then(extract_width_height_from_style)
        } else {
            None
        }
    };

    let options = usvg::Options::default();
    match usvg::Tree::from_str(svg, &options) {
        Ok(tree) => {
            let flattened = tree.to_string(&usvg::WriteOptions::default());
            restore_svg_dimensions(&flattened, orig_style_width_height)
        }
        Err(_) => svg.to_string(),
    }
}

fn restore_svg_dimensions(svg: &str, orig_style_width_height: Option<String>) -> String {
    use scraper::{Html, Selector};

    if orig_style_width_height.is_none() {
        return svg.to_string();
    }

    let fragment = Html::parse_fragment(svg);
    let selector = match Selector::parse("svg") {
        Ok(v) => v,
        Err(_) => return svg.to_string(),
    };

    let Some(svg_element) = fragment.select(&selector).next() else {
        return svg.to_string();
    };

    let mut attrs = String::new();
    let mut has_style = false;
    for (name, value) in svg_element.value().attrs() {
        match name {
            "width" | "height" => {}
            "style" => {
                has_style = true;
                let merged = merge_styles(orig_style_width_height.as_deref(), value);
                attrs.push_str(&format!(r#" style="{}""#, merged));
            }
            _ => attrs.push_str(&format!(r#" {}="{}""#, name, value)),
        }
    }

    if !has_style && let Some(style) = orig_style_width_height {
        attrs.push_str(&format!(r#" style="{}""#, style));
    }

    format!("<svg{}>{}</svg>", attrs, svg_element.inner_html())
}

fn extract_width_height_from_style(style: &str) -> Option<String> {
    let mut parts = Vec::new();
    for decl in style.split(';') {
        if let Some((key, value)) = decl.split_once(':') {
            let key = key.trim().to_lowercase();
            if key == "width" || key == "height" {
                parts.push(format!("{}:{}", key, value.trim()));
            }
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(";"))
    }
}

fn merge_styles(orig_width_height: Option<&str>, usvg_style: &str) -> String {
    use std::collections::HashMap;

    let mut style_map: HashMap<&str, &str> = HashMap::new();
    for decl in usvg_style.split(';') {
        let decl = decl.trim();
        if let Some((key, val)) = decl.split_once(':') {
            style_map.insert(key.trim(), val.trim());
        }
    }
    if let Some(orig) = orig_width_height {
        for decl in orig.split(';') {
            let decl = decl.trim();
            if let Some((key, val)) = decl.split_once(':') {
                style_map.insert(key.trim(), val.trim());
            }
        }
    }

    let mut result: Vec<String> = style_map
        .iter()
        .map(|(k, v)| format!("{}:{}", k, v))
        .collect();
    result.sort();
    result.join(";")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use std::collections::BTreeMap;
    use typub_ir::{BlockAttrs, DocMeta, InlineAttrs, RenderPayload};

    fn doc_with_payload(svg: &str) -> Document {
        Document {
            blocks: vec![
                Block::Paragraph {
                    content: vec![Inline::MathInline {
                        math: RenderPayload {
                            src: None,
                            rendered: Some(RenderedArtifact::Svg(svg.to_string())),
                            id: None,
                        },
                        attrs: InlineAttrs::default(),
                    }],
                    attrs: BlockAttrs::default(),
                },
                Block::SvgBlock {
                    svg: RenderPayload {
                        src: None,
                        rendered: Some(RenderedArtifact::Svg(svg.to_string())),
                        id: None,
                    },
                    attrs: BlockAttrs::default(),
                },
            ],
            footnotes: BTreeMap::new(),
            assets: BTreeMap::new(),
            meta: DocMeta::default(),
        }
    }

    #[test]
    fn flatten_svg_pass_flattens_use_in_math_and_svg_nodes() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><defs><path id="p" d="M0 0 L10 10"/></defs><use href="#p"/></svg>"##;
        let mut doc = doc_with_payload(svg);
        let mut pass = FlattenSvgPass;
        pass.run(&mut doc, &mut PassCtx::default())
            .expect("run flatten pass");

        let Block::Paragraph { content, .. } = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        let Inline::MathInline { math, .. } = &content[0] else {
            panic!("expected math inline");
        };
        let Some(RenderedArtifact::Svg(inline_svg)) = math.rendered.as_ref() else {
            panic!("expected svg rendered payload");
        };
        assert!(
            !inline_svg.contains("<use"),
            "flattened inline svg should not contain <use>"
        );

        let Block::SvgBlock { svg, .. } = &doc.blocks[1] else {
            panic!("expected svg block");
        };
        let Some(RenderedArtifact::Svg(block_svg)) = svg.rendered.as_ref() else {
            panic!("expected svg rendered payload");
        };
        assert!(
            !block_svg.contains("<use"),
            "flattened block svg should not contain <use>"
        );
    }

    #[test]
    fn flatten_svg_pass_preserves_style_width_height() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" style="width:3.8em;height:1.5em"><rect width="10" height="10" fill="red"/></svg>"#;
        let mut doc = doc_with_payload(svg);
        let mut pass = FlattenSvgPass;
        pass.run(&mut doc, &mut PassCtx::default())
            .expect("run flatten pass");

        let Block::SvgBlock { svg, .. } = &doc.blocks[1] else {
            panic!("expected svg block");
        };
        let Some(RenderedArtifact::Svg(flattened)) = svg.rendered.as_ref() else {
            panic!("expected svg rendered payload");
        };

        assert!(
            flattened.contains("style=\""),
            "flattened svg should keep style"
        );
        assert!(
            flattened.contains("width:3.8em"),
            "flattened svg should preserve width from style: {}",
            flattened
        );
        assert!(
            flattened.contains("height:1.5em"),
            "flattened svg should preserve height from style: {}",
            flattened
        );
    }

    #[test]
    fn flatten_svg_pass_keeps_invalid_svg_unchanged() {
        let svg = "<svg";
        let mut doc = doc_with_payload(svg);
        let mut pass = FlattenSvgPass;
        pass.run(&mut doc, &mut PassCtx::default())
            .expect("run flatten pass");

        let Block::Paragraph { content, .. } = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        let Inline::MathInline { math, .. } = &content[0] else {
            panic!("expected math inline");
        };
        let Some(RenderedArtifact::Svg(flattened)) = math.rendered.as_ref() else {
            panic!("expected svg rendered payload");
        };
        assert_eq!(flattened, svg);
    }
}
