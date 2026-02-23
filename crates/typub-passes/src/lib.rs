//! v2 semantic document passes.
//!
//! Passes operate on `Document` and may use sidecar context for execution-time
//! metadata that must not leak into conformance IR.

pub mod apply_node_policy;
pub mod apply_resolved_publish_urls;
pub mod flatten_svg;
pub mod rasterize_svg_to_data_uri;
pub mod rasterize_svg_to_local_asset;
pub mod resolve_internal_links;
mod shared;
pub mod validate_document;
pub mod walk;

use anyhow::Result;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;

use typub_ir::Document;

/// Runtime context shared across passes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PassCtx {
    /// Structured diagnostics emitted by passes.
    pub diagnostics: Vec<Diagnostic>,
    /// Execution-only sidecar data. Keep conformance IR out of this map.
    pub sidecar: BTreeMap<String, JsonValue>,
}

impl PassCtx {
    pub fn push_diagnostic(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub pass: &'static str,
    pub level: DiagnosticLevel,
    pub message: String,
    pub location: Option<String>,
}

impl Diagnostic {
    pub fn error(pass: &'static str, message: String, location: Option<String>) -> Self {
        Self {
            pass,
            level: DiagnosticLevel::Error,
            message,
            location,
        }
    }
}

/// A semantic pass over v2 `Document`.
pub trait Pass {
    fn name(&self) -> &'static str;
    fn run(&mut self, doc: &mut Document, ctx: &mut PassCtx) -> Result<()>;
}

pub use apply_node_policy::ApplyNodePolicyPass;
pub use apply_resolved_publish_urls::ApplyResolvedPublishUrlsPass;
pub use flatten_svg::FlattenSvgPass;
pub use rasterize_svg_to_data_uri::RasterizeSvgToDataUriPass;
pub use rasterize_svg_to_local_asset::{
    RasterizeSvgToLocalAssetPass, SIDECAR_GENERATED_RENDER_ASSETS,
};
pub use resolve_internal_links::ResolveInternalLinksPass;
pub use validate_document::ValidateDocumentPass;

/// Run passes in order; stop at first pass error.
pub fn run_passes(
    doc: &mut Document,
    ctx: &mut PassCtx,
    passes: &mut [&mut dyn Pass],
) -> Result<()> {
    for pass in passes {
        pass.run(doc, ctx)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use typub_ir::DocMeta;

    struct MarkPass(&'static str);

    impl Pass for MarkPass {
        fn name(&self) -> &'static str {
            self.0
        }

        fn run(&mut self, _doc: &mut Document, ctx: &mut PassCtx) -> Result<()> {
            ctx.push_diagnostic(Diagnostic {
                pass: self.name(),
                level: DiagnosticLevel::Info,
                message: "ran".to_string(),
                location: None,
            });
            Ok(())
        }
    }

    #[test]
    fn run_passes_executes_in_order() {
        let mut doc = Document {
            blocks: Vec::new(),
            footnotes: Default::default(),
            assets: Default::default(),
            meta: DocMeta::default(),
        };
        let mut ctx = PassCtx::default();
        let mut pass_a = MarkPass("a");
        let mut pass_b = MarkPass("b");
        run_passes(&mut doc, &mut ctx, &mut [&mut pass_a, &mut pass_b]).expect("run passes");

        assert_eq!(ctx.diagnostics.len(), 2);
        assert_eq!(ctx.diagnostics[0].pass, "a");
        assert_eq!(ctx.diagnostics[1].pass, "b");
    }
}
