//! Rasterize SVG rendered payloads into data-URI image assets.

use anyhow::{Result, bail};

use typub_ir::Document;

use crate::shared::{
    DEFAULT_ASSET_ID_PREFIX, DEFAULT_PNG_SCALE, RasterizeConfig, RasterizeTarget,
    rasterize_document,
};
use crate::{Pass, PassCtx};

#[derive(Debug, Clone)]
pub struct RasterizeSvgToDataUriPass {
    scale: f32,
    asset_id_prefix: String,
}

impl Default for RasterizeSvgToDataUriPass {
    fn default() -> Self {
        Self {
            scale: DEFAULT_PNG_SCALE,
            asset_id_prefix: DEFAULT_ASSET_ID_PREFIX.to_string(),
        }
    }
}

impl RasterizeSvgToDataUriPass {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    pub fn with_asset_id_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.asset_id_prefix = prefix.into();
        self
    }
}

impl Pass for RasterizeSvgToDataUriPass {
    fn name(&self) -> &'static str {
        "rasterize_svg_to_data_uri"
    }

    fn run(&mut self, doc: &mut Document, ctx: &mut PassCtx) -> Result<()> {
        if self.scale <= 0.0 {
            bail!("rasterize_svg_to_data_uri scale must be > 0");
        }
        rasterize_document(
            doc,
            ctx,
            RasterizeConfig {
                pass_name: self.name(),
                scale: self.scale,
                asset_id_prefix: &self.asset_id_prefix,
                target: RasterizeTarget::DataUri,
                sidecar_key: None,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::collections::BTreeMap;

    use typub_ir::{
        Asset, AssetId, AssetSource, Block, BlockAttrs, DocMeta, Document, ImageAsset, Inline,
        InlineAttrs, MathSource, RenderPayload, RenderedArtifact,
    };

    use crate::shared::test_fixtures::fixture_doc_for_render_pass;
    use crate::{DiagnosticLevel, Pass, PassCtx};

    use super::RasterizeSvgToDataUriPass;

    #[test]
    fn rasterize_svg_to_data_uri_pass_converts_math_and_svg_nodes() {
        let mut doc = fixture_doc_for_render_pass();
        let mut pass = RasterizeSvgToDataUriPass::new();
        let mut ctx = PassCtx::default();

        pass.run(&mut doc, &mut ctx).expect("run data-uri pass");

        assert_eq!(doc.assets.len(), 3);
        assert!(ctx.diagnostics.is_empty());

        for asset in doc.assets.values() {
            let Asset::Image(image) = asset else {
                panic!("expected image asset");
            };
            match &image.source {
                AssetSource::DataUri { uri } => assert!(uri.starts_with("data:image/png;base64,")),
                other => panic!("expected data-uri source, got: {other:?}"),
            }
            assert_eq!(
                image.meta.as_ref().and_then(|meta| meta.format.as_deref()),
                Some("png")
            );
            assert_eq!(image.meta.as_ref().and_then(|meta| meta.width), Some(10));
            assert_eq!(image.meta.as_ref().and_then(|meta| meta.height), Some(12));
        }

        let Block::Paragraph { content, .. } = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        let Inline::MathInline { math, .. } = &content[0] else {
            panic!("expected math inline");
        };
        assert!(
            matches!(
                math.rendered,
                Some(RenderedArtifact::Asset {
                    width: Some(10),
                    height: Some(12),
                    ..
                })
            ),
            "math inline should be rewritten to rendered asset"
        );

        let Block::SvgBlock { svg, .. } = &doc.blocks[1] else {
            panic!("expected svg block");
        };
        assert!(
            matches!(
                svg.rendered,
                Some(RenderedArtifact::Asset {
                    width: Some(10),
                    height: Some(12),
                    ..
                })
            ),
            "svg block should be rewritten to rendered asset"
        );
    }

    #[test]
    fn rasterize_svg_to_data_uri_pass_skips_non_svg_rendered_payload() {
        let mut doc = Document {
            blocks: vec![Block::Paragraph {
                content: vec![Inline::MathInline {
                    math: RenderPayload {
                        src: Some(MathSource::Typst("$x$".to_string())),
                        rendered: Some(RenderedArtifact::MathMl("<math/>".to_string())),
                        id: None,
                    },
                    attrs: InlineAttrs::default(),
                }],
                attrs: BlockAttrs::default(),
            }],
            footnotes: BTreeMap::new(),
            assets: BTreeMap::new(),
            meta: DocMeta::default(),
        };
        let mut pass = RasterizeSvgToDataUriPass::new();
        let mut ctx = PassCtx::default();
        pass.run(&mut doc, &mut ctx).expect("run data-uri pass");

        assert!(doc.assets.is_empty());
        let Block::Paragraph { content, .. } = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        let Inline::MathInline { math, .. } = &content[0] else {
            panic!("expected math inline");
        };
        assert!(
            matches!(math.rendered, Some(RenderedArtifact::MathMl(_))),
            "non-svg rendered payload should stay unchanged"
        );
    }

    #[test]
    fn rasterize_svg_to_data_uri_pass_skips_existing_asset_id() {
        let mut doc = fixture_doc_for_render_pass();
        doc.assets.insert(
            AssetId("render-000001".to_string()),
            Asset::Image(ImageAsset {
                source: AssetSource::DataUri {
                    uri: "data:image/png;base64,AAA=".to_string(),
                },
                meta: None,
                variants: Vec::new(),
            }),
        );

        let mut pass = RasterizeSvgToDataUriPass::new();
        let mut ctx = PassCtx::default();
        pass.run(&mut doc, &mut ctx).expect("run data-uri pass");

        assert_eq!(doc.assets.len(), 4);
        assert!(
            doc.assets
                .contains_key(&AssetId("render-000001".to_string()))
        );
        assert!(
            doc.assets
                .contains_key(&AssetId("render-000002".to_string()))
        );
    }

    #[test]
    fn rasterize_svg_pass_reports_invalid_svg() {
        let mut doc = Document {
            blocks: vec![Block::MathBlock {
                math: RenderPayload {
                    src: None,
                    rendered: Some(RenderedArtifact::Svg("<svg".to_string())),
                    id: None,
                },
                attrs: BlockAttrs::default(),
            }],
            footnotes: BTreeMap::new(),
            assets: BTreeMap::new(),
            meta: DocMeta::default(),
        };
        let mut pass = RasterizeSvgToDataUriPass::new();
        let mut ctx = PassCtx::default();

        let err = pass
            .run(&mut doc, &mut ctx)
            .expect_err("invalid svg should fail");
        assert!(err.to_string().contains("failed to rasterize svg"));
        assert_eq!(ctx.diagnostics.len(), 1);
        assert_eq!(ctx.diagnostics[0].level, DiagnosticLevel::Error);
    }
}
