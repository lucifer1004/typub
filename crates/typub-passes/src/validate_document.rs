//! Validation pass for v2 semantic document IR.

use anyhow::{Result, bail};
use std::collections::BTreeSet;
use typub_ir::{
    AssetId, Block, Document, FootnoteId, Inline, MathPayload, MathSource, RenderedArtifact,
    SvgPayload,
};

use super::walk::{NodePath, VisitorMut, walk_document_mut};
use super::{Diagnostic, Pass, PassCtx};

#[derive(Debug, Default)]
pub struct ValidateDocumentPass;

impl Pass for ValidateDocumentPass {
    fn name(&self) -> &'static str {
        "validate_document"
    }

    fn run(&mut self, doc: &mut Document, ctx: &mut PassCtx) -> Result<()> {
        validate_document_with_ctx(doc, ctx)
    }
}

/// Validate a document and return an error if any issue is found.
pub fn validate_document(doc: &Document) -> Result<()> {
    let mut cloned = doc.clone();
    let mut ctx = PassCtx::default();
    validate_document_with_ctx(&mut cloned, &mut ctx)
}

fn validate_document_with_ctx(doc: &mut Document, ctx: &mut PassCtx) -> Result<()> {
    let asset_ids: BTreeSet<AssetId> = doc.assets.keys().cloned().collect();
    let footnote_ids: BTreeSet<FootnoteId> = doc.footnotes.keys().cloned().collect();

    let mut validator = Validator {
        asset_ids,
        footnote_ids,
        issues: Vec::new(),
    };
    walk_document_mut(doc, &mut validator)?;

    if validator.issues.is_empty() {
        return Ok(());
    }

    for issue in &validator.issues {
        ctx.push_diagnostic(Diagnostic::error(
            "validate_document",
            issue.message.clone(),
            Some(issue.location.clone()),
        ));
    }

    let details = validator
        .issues
        .iter()
        .map(|issue| format!("{}: {}", issue.location, issue.message))
        .collect::<Vec<_>>()
        .join("; ");
    bail!(
        "validation failed with {} issue(s): {}",
        validator.issues.len(),
        details
    );
}

#[derive(Debug)]
struct ValidationIssue {
    location: String,
    message: String,
}

struct Validator {
    asset_ids: BTreeSet<AssetId>,
    footnote_ids: BTreeSet<FootnoteId>,
    issues: Vec<ValidationIssue>,
}

impl Validator {
    fn issue(&mut self, path: &NodePath, message: impl Into<String>) {
        self.issues.push(ValidationIssue {
            location: path.render(),
            message: message.into(),
        });
    }
}

impl VisitorMut for Validator {
    fn visit_block(&mut self, block: &mut Block, path: &NodePath) -> Result<()> {
        match block {
            Block::Heading { level, .. } => {
                if !(1..=6).contains(&level.get()) {
                    self.issue(
                        path,
                        format!("heading level {} out of range 1..=6", level.get()),
                    );
                }
            }
            Block::MathBlock { math, .. } => validate_math_payload(math, path, self),
            Block::SvgBlock { svg, .. } => validate_svg_payload(svg, path, self),
            _ => {}
        }
        Ok(())
    }

    fn visit_inline(&mut self, inline: &mut Inline, path: &NodePath) -> Result<()> {
        match inline {
            Inline::Image { asset, .. } => {
                if !self.asset_ids.contains(&asset.0) {
                    self.issue(
                        path,
                        format!("unresolvable asset reference '{}'", asset.0.0),
                    );
                }
            }
            Inline::FootnoteRef(id) => {
                if !self.footnote_ids.contains(id) {
                    self.issue(path, format!("unresolvable footnote reference '{}'", id.0));
                }
            }
            Inline::MathInline { math, .. } => validate_math_payload(math, path, self),
            Inline::SvgInline { svg, .. } => validate_svg_payload(svg, path, self),
            _ => {}
        }
        Ok(())
    }
}

fn validate_math_payload(math: &MathPayload, path: &NodePath, validator: &mut Validator) {
    if let Some(src) = &math.src {
        match src {
            MathSource::Typst(src) => {
                if src.trim().is_empty() {
                    validator.issue(path, "empty typst math source");
                }
            }
            MathSource::Latex(src) => {
                if src.trim().is_empty() {
                    validator.issue(path, "empty latex math source");
                }
            }
            MathSource::Custom { src, .. } => {
                if src.trim().is_empty() {
                    validator.issue(path, "empty custom math source");
                }
            }
        }
    }

    if !math.has_source_or_rendered() {
        validator.issue(path, "math payload must contain source or rendered payload");
    }

    validate_rendered_artifact(math.rendered.as_ref(), path, validator);
}

fn validate_svg_payload(svg: &SvgPayload, path: &NodePath, validator: &mut Validator) {
    if !svg.has_source_or_rendered() {
        validator.issue(path, "svg payload must contain source or rendered payload");
    }

    validate_rendered_artifact(svg.rendered.as_ref(), path, validator);
}

fn validate_rendered_artifact(
    rendered: Option<&RenderedArtifact>,
    path: &NodePath,
    validator: &mut Validator,
) {
    if let Some(rendered) = rendered {
        match rendered {
            RenderedArtifact::Svg(svg) => {
                if svg.trim().is_empty() {
                    validator.issue(path, "empty rendered svg payload");
                }
            }
            RenderedArtifact::MathMl(mathml) => {
                if mathml.trim().is_empty() {
                    validator.issue(path, "empty rendered mathml payload");
                }
            }
            RenderedArtifact::Asset {
                asset,
                width,
                height,
                ..
            } => {
                if !validator.asset_ids.contains(&asset.0) {
                    validator.issue(
                        path,
                        format!("unresolvable rendered-asset reference '{}'", asset.0.0),
                    );
                }
                if width.is_some_and(|w| w == 0) {
                    validator.issue(path, "rendered asset width must be > 0 when present");
                }
                if height.is_some_and(|h| h == 0) {
                    validator.issue(path, "rendered asset height must be > 0 when present");
                }
            }
            RenderedArtifact::Custom { data, .. } => {
                if data.is_empty() {
                    validator.issue(path, "empty rendered custom payload");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use typub_ir::{
        Asset, AssetRef, AssetSource, BlockAttrs, DocMeta, Document, FootnoteId, ImageAsset,
        ImageAttrs, InlineAttrs, MathPayload, RelativePath,
    };

    fn empty_doc(blocks: Vec<Block>) -> Document {
        Document {
            blocks,
            footnotes: Default::default(),
            assets: Default::default(),
            meta: DocMeta::default(),
        }
    }

    #[test]
    fn validate_document_ok_for_simple_paragraph() {
        let doc = empty_doc(vec![Block::Paragraph {
            content: vec![Inline::Text("ok".to_string())],
            attrs: BlockAttrs::default(),
        }]);
        assert!(validate_document(&doc).is_ok());
    }

    #[test]
    fn validate_document_rejects_missing_asset_ref() {
        let doc = empty_doc(vec![Block::Paragraph {
            content: vec![Inline::Image {
                asset: AssetRef(AssetId("missing".to_string())),
                alt: String::new(),
                title: None,
                attrs: ImageAttrs::default(),
            }],
            attrs: BlockAttrs::default(),
        }]);
        assert!(validate_document(&doc).is_err());
    }

    #[test]
    fn validate_document_accepts_resolved_asset_ref() {
        let mut doc = empty_doc(vec![Block::Paragraph {
            content: vec![Inline::Image {
                asset: AssetRef(AssetId("asset-1".to_string())),
                alt: String::new(),
                title: None,
                attrs: ImageAttrs::default(),
            }],
            attrs: BlockAttrs::default(),
        }]);
        doc.assets.insert(
            AssetId("asset-1".to_string()),
            Asset::Image(ImageAsset {
                source: AssetSource::LocalPath {
                    path: RelativePath::new("img/a.png".to_string()).expect("relative path"),
                },
                meta: None,
                variants: Vec::new(),
            }),
        );
        assert!(validate_document(&doc).is_ok());
    }

    #[test]
    fn validate_document_rejects_empty_math_source() {
        let doc = empty_doc(vec![Block::Paragraph {
            content: vec![Inline::MathInline {
                math: MathPayload {
                    src: Some(MathSource::Latex(" ".to_string())),
                    rendered: None,
                    id: None,
                },
                attrs: InlineAttrs::default(),
            }],
            attrs: BlockAttrs::default(),
        }]);
        assert!(validate_document(&doc).is_err());
    }

    #[test]
    fn validate_document_rejects_missing_footnote_ref() {
        let doc = empty_doc(vec![Block::Paragraph {
            content: vec![Inline::FootnoteRef(FootnoteId("missing".to_string()))],
            attrs: BlockAttrs::default(),
        }]);
        assert!(validate_document(&doc).is_err());
    }
}
