//! Shared utilities for rasterization passes.

use anyhow::{Context, Result, bail};
use serde_json::{Value as JsonValue, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use typub_ir::{
    Asset, AssetId, AssetRef, AssetSource, Block, Document, ImageAsset, ImageMeta, Inline,
    RelativePath, RenderPayload, RenderedArtifact,
};

use crate::walk::{NodePath, VisitorMut, walk_document_mut};
use crate::{Diagnostic, DiagnosticLevel, PassCtx};

const DEFAULT_PNG_PADDING: f32 = 1.0;

pub(crate) const DEFAULT_PNG_SCALE: f32 = 2.0;
pub(crate) const DEFAULT_ASSET_ID_PREFIX: &str = "render";
pub(crate) const DEFAULT_ASSET_SUBDIR: &str = "assets";

#[derive(Debug, Clone)]
pub(crate) enum RasterizeTarget {
    DataUri,
    LocalFile {
        output_dir: PathBuf,
        slug: String,
        assets_subdir: String,
    },
}

pub(crate) struct RasterizeConfig<'a> {
    pub(crate) pass_name: &'static str,
    pub(crate) scale: f32,
    pub(crate) asset_id_prefix: &'a str,
    pub(crate) target: RasterizeTarget,
    pub(crate) sidecar_key: Option<&'a str>,
}

pub(crate) fn sanitize_slug(slug: &str) -> String {
    let mut out = String::with_capacity(slug.len());
    for c in slug.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            out.push(c);
        } else {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "render".to_string()
    } else {
        trimmed.to_string()
    }
}

pub(crate) fn rasterize_document(
    doc: &mut Document,
    ctx: &mut PassCtx,
    config: RasterizeConfig<'_>,
) -> Result<()> {
    if let RasterizeTarget::LocalFile { output_dir, .. } = &config.target {
        std::fs::create_dir_all(output_dir).with_context(|| {
            format!(
                "failed to create render output directory '{}'",
                output_dir.display()
            )
        })?;
    }

    let mut visitor = RenderVisitor::new(
        config.pass_name,
        config.scale,
        AssetIdAllocator::new(&doc.assets, config.asset_id_prefix),
        config.target,
    );
    let walk_result = walk_document_mut(doc, &mut visitor);
    ctx.diagnostics.append(&mut visitor.diagnostics);
    walk_result?;

    if let Some(sidecar_key) = config.sidecar_key {
        let generated_entries = visitor
            .generated_assets
            .iter()
            .filter_map(|asset| {
                asset
                    .file_path
                    .as_ref()
                    .zip(asset.logical_path.as_ref())
                    .map(|(file_path, logical_path)| {
                        json!({
                            "asset_id": asset.id.0,
                            "file_path": file_path.to_string_lossy(),
                            "logical_path": logical_path,
                        })
                    })
            })
            .collect::<Vec<_>>();
        append_sidecar_entries(ctx, sidecar_key, generated_entries);
    }

    for generated in visitor.generated_assets {
        doc.assets.insert(generated.id, generated.asset);
    }

    Ok(())
}

fn append_sidecar_entries(ctx: &mut PassCtx, key: &str, new_entries: Vec<JsonValue>) {
    if new_entries.is_empty() {
        return;
    }

    let mut entries = ctx
        .sidecar
        .get(key)
        .and_then(JsonValue::as_array)
        .cloned()
        .unwrap_or_default();
    entries.extend(new_entries);
    ctx.sidecar
        .insert(key.to_string(), JsonValue::Array(entries));
}

#[derive(Debug)]
struct GeneratedAsset {
    id: AssetId,
    asset: Asset,
    file_path: Option<PathBuf>,
    logical_path: Option<String>,
}

struct RenderVisitor {
    pass_name: &'static str,
    scale: f32,
    id_alloc: AssetIdAllocator,
    target: RasterizeTarget,
    diagnostics: Vec<Diagnostic>,
    generated_assets: Vec<GeneratedAsset>,
    local_file_counter: u32,
}

impl RenderVisitor {
    fn new(
        pass_name: &'static str,
        scale: f32,
        id_alloc: AssetIdAllocator,
        target: RasterizeTarget,
    ) -> Self {
        Self {
            pass_name,
            scale,
            id_alloc,
            target,
            diagnostics: Vec::new(),
            generated_assets: Vec::new(),
            local_file_counter: 0,
        }
    }

    fn record_error(&mut self, path: &NodePath, message: String) {
        self.diagnostics.push(Diagnostic {
            pass: self.pass_name,
            level: DiagnosticLevel::Error,
            message,
            location: Some(path.render()),
        });
    }

    fn convert_payload(&mut self, payload: &mut RenderPayload, path: &NodePath) -> Result<()> {
        let Some(RenderedArtifact::Svg(svg_xml)) = payload.rendered.clone() else {
            return Ok(());
        };

        let converted = self
            .build_asset_from_svg(&svg_xml)
            .with_context(|| format!("failed to rasterize svg at {}", path.render()));

        let converted = match converted {
            Ok(v) => v,
            Err(err) => {
                self.record_error(path, err.to_string());
                return Err(err);
            }
        };

        payload.rendered = Some(RenderedArtifact::Asset {
            asset: AssetRef(converted.id.clone()),
            mime: Some("image/png".to_string()),
            width: converted.asset_width,
            height: converted.asset_height,
        });

        self.generated_assets.push(GeneratedAsset {
            id: converted.id,
            asset: converted.asset,
            file_path: converted.file_path,
            logical_path: converted.logical_path,
        });
        Ok(())
    }

    fn build_asset_from_svg(&mut self, svg: &str) -> Result<ConvertedAsset> {
        match &self.target {
            RasterizeTarget::DataUri => {
                let (uri, width, height) = svg_to_png_data_url(svg, self.scale)?;
                let id = self.id_alloc.next_id();
                Ok(ConvertedAsset {
                    id,
                    asset: Asset::Image(ImageAsset {
                        source: AssetSource::DataUri { uri },
                        meta: Some(ImageMeta {
                            width: Some(width),
                            height: Some(height),
                            format: Some("png".to_string()),
                            sha256: None,
                        }),
                        variants: Vec::new(),
                    }),
                    asset_width: Some(width),
                    asset_height: Some(height),
                    file_path: None,
                    logical_path: None,
                })
            }
            RasterizeTarget::LocalFile {
                output_dir,
                slug,
                assets_subdir,
            } => {
                let (bytes, width, height) = svg_to_png(svg, self.scale)?;
                let id = self.id_alloc.next_id();
                self.local_file_counter += 1;
                let filename = format!("{}-render-{:06}.png", slug, self.local_file_counter);
                // Use same logic as build_logical_path to ensure consistency
                let subdir = assets_subdir.trim().trim_matches('/');
                let output_path = if subdir.is_empty() {
                    output_dir.join(&filename)
                } else {
                    output_dir.join(subdir).join(&filename)
                };
                // Ensure the directory exists before writing
                if let Some(parent) = output_path.parent() {
                    std::fs::create_dir_all(parent).with_context(|| {
                        format!("failed to create directory '{}'", parent.display())
                    })?;
                }
                std::fs::write(&output_path, bytes).with_context(|| {
                    format!("failed to write rendered PNG '{}'", output_path.display())
                })?;

                let logical_path = build_logical_path(assets_subdir, &filename);
                let relative = RelativePath::new(logical_path.clone())
                    .map_err(anyhow::Error::msg)
                    .with_context(|| {
                        format!(
                            "invalid logical asset path generated for rendered PNG: '{}'",
                            logical_path
                        )
                    })?;

                Ok(ConvertedAsset {
                    id,
                    asset: Asset::Image(ImageAsset {
                        source: AssetSource::LocalPath { path: relative },
                        meta: Some(ImageMeta {
                            width: Some(width),
                            height: Some(height),
                            format: Some("png".to_string()),
                            sha256: None,
                        }),
                        variants: Vec::new(),
                    }),
                    asset_width: Some(width),
                    asset_height: Some(height),
                    file_path: Some(output_path),
                    logical_path: Some(logical_path),
                })
            }
        }
    }
}

impl VisitorMut for RenderVisitor {
    fn visit_block(&mut self, block: &mut Block, path: &NodePath) -> Result<()> {
        match block {
            Block::MathBlock { math, .. } => self.convert_payload(math, path)?,
            Block::SvgBlock { svg, .. } => self.convert_payload(svg, path)?,
            _ => {}
        }
        Ok(())
    }

    fn visit_inline(&mut self, inline: &mut Inline, path: &NodePath) -> Result<()> {
        match inline {
            Inline::MathInline { math, .. } => self.convert_payload(math, path)?,
            Inline::SvgInline { svg, .. } => self.convert_payload(svg, path)?,
            _ => {}
        }
        Ok(())
    }
}

#[derive(Debug)]
struct ConvertedAsset {
    id: AssetId,
    asset: Asset,
    asset_width: Option<u32>,
    asset_height: Option<u32>,
    file_path: Option<PathBuf>,
    logical_path: Option<String>,
}

#[derive(Debug)]
struct AssetIdAllocator {
    used: BTreeSet<AssetId>,
    prefix: String,
    next: u64,
}

impl AssetIdAllocator {
    fn new(existing: &BTreeMap<AssetId, Asset>, prefix: &str) -> Self {
        Self {
            used: existing.keys().cloned().collect(),
            prefix: if prefix.trim().is_empty() {
                DEFAULT_ASSET_ID_PREFIX.to_string()
            } else {
                prefix.to_string()
            },
            next: 1,
        }
    }

    fn next_id(&mut self) -> AssetId {
        loop {
            let candidate = AssetId(format!("{}-{:06}", self.prefix, self.next));
            self.next += 1;
            if self.used.insert(candidate.clone()) {
                return candidate;
            }
        }
    }
}

fn build_logical_path(assets_subdir: &str, filename: &str) -> String {
    let subdir = assets_subdir.trim().trim_matches('/');
    if subdir.is_empty() {
        filename.to_string()
    } else {
        format!("{subdir}/{filename}")
    }
}

fn svg_to_png(svg: &str, scale: f32) -> Result<(Vec<u8>, u32, u32)> {
    let options = usvg::Options::default();
    let tree =
        usvg::Tree::from_str(svg, &options).map_err(|e| anyhow::anyhow!("SVG parse error: {e}"))?;
    let bbox = tree.root().abs_layer_bounding_box();

    let size = tree.size();
    let width = (size.width() * scale) as u32;
    let height = (size.height() * scale) as u32;
    if width == 0 || height == 0 {
        bail!("SVG has zero dimensions");
    }

    let w = (bbox.width() * scale + DEFAULT_PNG_PADDING * 2.0).ceil() as u32;
    let h = (bbox.height() * scale + DEFAULT_PNG_PADDING * 2.0).ceil() as u32;
    let mut pixmap = tiny_skia::Pixmap::new(w, h)
        .ok_or_else(|| anyhow::anyhow!("failed to create pixmap for rendered size {w}x{h}"))?;

    let transform = tiny_skia::Transform::from_translate(DEFAULT_PNG_PADDING, DEFAULT_PNG_PADDING)
        .pre_scale(scale, scale)
        .pre_translate(-bbox.left(), -bbox.top());
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    let png = pixmap
        .encode_png()
        .map_err(|e| anyhow::anyhow!("PNG encode error: {e}"))?;
    Ok((png, bbox.width().ceil() as u32, bbox.height().ceil() as u32))
}

fn svg_to_png_data_url(svg: &str, scale: f32) -> Result<(String, u32, u32)> {
    let (png_bytes, width, height) = svg_to_png(svg, scale)?;
    let base64 = base64_encode(&png_bytes);
    Ok((format!("data:image/png;base64,{base64}"), width, height))
}

fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;

        out.push(ALPHABET[b0 >> 2] as char);
        out.push(ALPHABET[((b0 & 0x03) << 4) | (b1 >> 4)] as char);

        if chunk.len() > 1 {
            out.push(ALPHABET[((b1 & 0x0f) << 2) | (b2 >> 6)] as char);
        } else {
            out.push('=');
        }

        if chunk.len() > 2 {
            out.push(ALPHABET[b2 & 0x3f] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
pub(crate) mod test_fixtures {
    use std::collections::BTreeMap;
    use typub_ir::{
        Block, BlockAttrs, DocMeta, Document, FootnoteDef, FootnoteId, Inline, InlineAttrs,
        MathSource, RenderPayload, RenderedArtifact,
    };

    pub(crate) fn fixture_svg() -> String {
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="12"><rect width="10" height="12" fill="red"/></svg>"#.to_string()
    }

    pub(crate) fn fixture_doc_for_render_pass() -> Document {
        let svg = fixture_svg();
        let mut footnotes = BTreeMap::new();
        footnotes.insert(
            FootnoteId(1),
            FootnoteDef {
                blocks: vec![Block::Paragraph {
                    content: vec![Inline::SvgInline {
                        svg: RenderPayload {
                            src: None,
                            rendered: Some(RenderedArtifact::Svg(svg.clone())),
                            id: None,
                        },
                        attrs: InlineAttrs::default(),
                    }],
                    attrs: BlockAttrs::default(),
                }],
            },
        );

        Document {
            blocks: vec![
                Block::Paragraph {
                    content: vec![Inline::MathInline {
                        math: RenderPayload {
                            src: Some(MathSource::Latex("E=mc^2".to_string())),
                            rendered: Some(RenderedArtifact::Svg(svg.clone())),
                            id: Some("m1".to_string()),
                        },
                        attrs: InlineAttrs::default(),
                    }],
                    attrs: BlockAttrs::default(),
                },
                Block::SvgBlock {
                    svg: RenderPayload {
                        src: None,
                        rendered: Some(RenderedArtifact::Svg(svg)),
                        id: None,
                    },
                    attrs: BlockAttrs::default(),
                },
            ],
            footnotes,
            assets: BTreeMap::new(),
            meta: DocMeta::default(),
        }
    }
}
