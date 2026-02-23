//! Apply resolved publish URLs to `Document.assets`.

use anyhow::Result;
use std::collections::BTreeMap;
use typub_ir::{Asset, AssetId, AssetVariant, Document, Url};

use crate::{Diagnostic, DiagnosticLevel, Pass, PassCtx};

const DEFAULT_VARIANT_NAME: &str = "original";

#[derive(Debug)]
pub struct ApplyResolvedPublishUrlsPass<'a> {
    resolved_urls: &'a BTreeMap<AssetId, Url>,
    variant_name: String,
}

impl<'a> ApplyResolvedPublishUrlsPass<'a> {
    pub fn new(resolved_urls: &'a BTreeMap<AssetId, Url>) -> Self {
        Self {
            resolved_urls,
            variant_name: DEFAULT_VARIANT_NAME.to_string(),
        }
    }

    pub fn with_variant_name(mut self, variant_name: impl Into<String>) -> Self {
        self.variant_name = variant_name.into();
        self
    }
}

impl Pass for ApplyResolvedPublishUrlsPass<'_> {
    fn name(&self) -> &'static str {
        "apply_resolved_publish_urls"
    }

    fn run(&mut self, doc: &mut Document, ctx: &mut PassCtx) -> Result<()> {
        for (asset_id, publish_url) in self.resolved_urls {
            let Some(asset) = doc.assets.get_mut(asset_id) else {
                ctx.push_diagnostic(Diagnostic {
                    pass: self.name(),
                    level: DiagnosticLevel::Warning,
                    message: format!(
                        "resolved publish URL provided for unknown asset id '{}'",
                        asset_id.0
                    ),
                    location: Some(format!("document.assets[{}]", asset_id.0)),
                });
                continue;
            };

            upsert_asset_variant(asset, &self.variant_name, publish_url.clone());
        }

        Ok(())
    }
}

fn upsert_asset_variant(asset: &mut Asset, variant_name: &str, publish_url: Url) {
    let (variants, default_width, default_height) = match asset {
        Asset::Image(image) => (
            &mut image.variants,
            image.meta.as_ref().and_then(|m| m.width),
            image.meta.as_ref().and_then(|m| m.height),
        ),
        Asset::Video(media) | Asset::Audio(media) => {
            (&mut media.variants, media.width, media.height)
        }
        Asset::File(file) => (&mut file.variants, None, None),
        Asset::Custom(custom) => (&mut custom.variants, None, None),
    };

    if let Some(existing) = variants.iter_mut().find(|v| v.name == variant_name) {
        existing.publish_url = publish_url;
        return;
    }

    variants.push(AssetVariant {
        name: variant_name.to_string(),
        publish_url,
        width: default_width,
        height: default_height,
    });
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use typub_ir::{AssetSource, DocMeta, ImageAsset, ImageMeta, RelativePath};

    fn mk_image_asset() -> Asset {
        Asset::Image(ImageAsset {
            source: AssetSource::LocalPath {
                path: RelativePath::new("images/a.png".to_string()).expect("relative path"),
            },
            meta: Some(ImageMeta {
                width: Some(320),
                height: Some(240),
                format: Some("png".to_string()),
                sha256: None,
            }),
            variants: vec![],
        })
    }

    #[test]
    fn apply_adds_original_variant_when_missing() {
        let asset_id = AssetId("asset-1".to_string());
        let mut doc = Document {
            blocks: vec![],
            footnotes: Default::default(),
            assets: BTreeMap::from([(asset_id.clone(), mk_image_asset())]),
            meta: DocMeta::default(),
        };
        let resolved = BTreeMap::from([(
            asset_id.clone(),
            Url("https://cdn.example.com/a.png".to_string()),
        )]);

        let mut pass = ApplyResolvedPublishUrlsPass::new(&resolved);
        pass.run(&mut doc, &mut PassCtx::default())
            .expect("run pass");

        let Some(Asset::Image(image)) = doc.assets.get(&asset_id) else {
            panic!("expected image asset");
        };
        assert_eq!(image.variants.len(), 1);
        assert_eq!(image.variants[0].name, "original");
        assert_eq!(
            image.variants[0].publish_url.0,
            "https://cdn.example.com/a.png"
        );
        assert_eq!(image.variants[0].width, Some(320));
        assert_eq!(image.variants[0].height, Some(240));
    }

    #[test]
    fn apply_updates_existing_original_variant() {
        let asset_id = AssetId("asset-1".to_string());
        let mut doc = Document {
            blocks: vec![],
            footnotes: Default::default(),
            assets: BTreeMap::from([(
                asset_id.clone(),
                Asset::Image(ImageAsset {
                    source: AssetSource::RemoteUrl {
                        url: Url("https://old.example.com/a.png".to_string()),
                    },
                    meta: None,
                    variants: vec![AssetVariant {
                        name: "original".to_string(),
                        publish_url: Url("https://old.example.com/a.png".to_string()),
                        width: None,
                        height: None,
                    }],
                }),
            )]),
            meta: DocMeta::default(),
        };
        let resolved = BTreeMap::from([(
            asset_id.clone(),
            Url("https://new.example.com/a.png".to_string()),
        )]);

        let mut pass = ApplyResolvedPublishUrlsPass::new(&resolved);
        pass.run(&mut doc, &mut PassCtx::default())
            .expect("run pass");

        let Some(Asset::Image(image)) = doc.assets.get(&asset_id) else {
            panic!("expected image asset");
        };
        assert_eq!(image.variants.len(), 1);
        assert_eq!(
            image.variants[0].publish_url.0,
            "https://new.example.com/a.png"
        );
    }

    #[test]
    fn apply_warns_for_unknown_asset_id() {
        let mut doc = Document {
            blocks: vec![],
            footnotes: Default::default(),
            assets: BTreeMap::new(),
            meta: DocMeta::default(),
        };
        let resolved = BTreeMap::from([(
            AssetId("missing".to_string()),
            Url("https://cdn.example.com/missing.png".to_string()),
        )]);
        let mut ctx = PassCtx::default();

        let mut pass = ApplyResolvedPublishUrlsPass::new(&resolved);
        pass.run(&mut doc, &mut ctx).expect("run pass");

        assert_eq!(ctx.diagnostics.len(), 1);
        assert_eq!(ctx.diagnostics[0].level, DiagnosticLevel::Warning);
        assert!(ctx.diagnostics[0].message.contains("unknown asset id"));
    }

    #[test]
    fn apply_supports_custom_variant_name() {
        let asset_id = AssetId("asset-1".to_string());
        let mut doc = Document {
            blocks: vec![],
            footnotes: Default::default(),
            assets: BTreeMap::from([(asset_id.clone(), mk_image_asset())]),
            meta: DocMeta::default(),
        };
        let resolved = BTreeMap::from([(
            asset_id.clone(),
            Url("https://cdn.example.com/a.webp".to_string()),
        )]);

        let mut pass = ApplyResolvedPublishUrlsPass::new(&resolved).with_variant_name("webp-2x");
        pass.run(&mut doc, &mut PassCtx::default())
            .expect("run pass");

        let Some(Asset::Image(image)) = doc.assets.get(&asset_id) else {
            panic!("expected image asset");
        };
        assert_eq!(image.variants.len(), 1);
        assert_eq!(image.variants[0].name, "webp-2x");
    }
}
