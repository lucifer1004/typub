use crate::adapters::{PlatformAdapter, PublishContext};
use crate::content::Content;
use crate::renderer::Renderer;
use crate::resolved_config::ResolvedConfig;
use anyhow::Result;
use std::path::PathBuf;
use typub_storage::PublishResult;
use typub_ui as ui;

mod helpers;
mod stages;

/// Mode of pipeline execution.
/// Allows stages to behave differently for preview vs publish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineMode {
    /// Preview mode: no remote side effects, local output only
    Preview,
    /// Publish mode: full pipeline with remote operations
    Publish,
}

/// Pipeline stage enumeration for debugging.
/// Per [[RFC-0002:C-PIPELINE-STAGES]].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineStage {
    Resolve,
    Render,
    Parse,
    Transform,
    Specialize,
    Provision,
    Materialize,
    Serialize,
    Publish,
    Persist,
}

impl std::str::FromStr for PipelineStage {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "1" | "resolve" => Ok(Self::Resolve),
            "2" | "render" => Ok(Self::Render),
            "3" | "parse" => Ok(Self::Parse),
            "4" | "transform" => Ok(Self::Transform),
            "5" | "specialize" => Ok(Self::Specialize),
            "6" | "provision" => Ok(Self::Provision),
            "7" | "materialize" => Ok(Self::Materialize),
            "8" | "serialize" => Ok(Self::Serialize),
            "9" | "publish" => Ok(Self::Publish),
            "10" | "persist" => Ok(Self::Persist),
            _ => Err(format!("Invalid stage: {}", s)),
        }
    }
}

impl PipelineStage {
    /// Get the stage number (1-based).
    pub fn number(&self) -> u8 {
        match self {
            Self::Resolve => 1,
            Self::Render => 2,
            Self::Parse => 3,
            Self::Transform => 4,
            Self::Specialize => 5,
            Self::Provision => 6,
            Self::Materialize => 7,
            Self::Serialize => 8,
            Self::Publish => 9,
            Self::Persist => 10,
        }
    }

    /// Get the stage name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Resolve => "Resolve",
            Self::Render => "Render",
            Self::Parse => "Parse",
            Self::Transform => "Transform",
            Self::Specialize => "Specialize",
            Self::Provision => "Provision",
            Self::Materialize => "Materialize",
            Self::Serialize => "Serialize",
            Self::Publish => "Publish",
            Self::Persist => "Persist",
        }
    }
}

/// Check if we should dump at the given stage.
fn should_dump(debug_stage: Option<PipelineStage>, stage: PipelineStage) -> bool {
    debug_stage == Some(stage)
}

/// Dump stage output to stderr.
fn dump_stage<T: std::fmt::Debug>(stage_num: u8, stage_name: &str, value: &T) {
    eprintln!("[DEBUG] Stage {} ({})", stage_num, stage_name);
    eprintln!("{:#?}", value);
}

/// Execute publish pipeline stages 1-10 for one platform.
///
/// Per [[RFC-0002:C-PIPELINE-STAGES]], the 10-stage pipeline is:
/// 1. Resolve, 2. Render, 3. Parse, 4. Transform, 5. Specialize,
/// 6. Provision, 7. Materialize, 8. Serialize, 9. Publish, 10. Persist
pub async fn publish_single_platform(
    adapter: &dyn PlatformAdapter,
    platform_id: &str,
    content: &Content,
    renderer: &Renderer<'_>,
    ctx: &mut PublishContext,
    config: &typub_config::Config,
    debug_stage: Option<PipelineStage>,
) -> Result<PublishResult> {
    // Stage 1 (Resolve): compute ResolvedConfig for this (content, platform) pair
    let resolved = ResolvedConfig::resolve(content, platform_id, config, adapter.default_config())?;
    if should_dump(debug_stage, PipelineStage::Resolve) {
        dump_stage(1, "Resolve", &resolved);
    }
    ctx.set_resolved(resolved);
    helpers::ensure_node_policy_declared(platform_id)?;

    // Stage 2 (Render)
    let rendered = stages::render(adapter, content, platform_id, ctx, renderer, config).await?;
    if should_dump(debug_stage, PipelineStage::Render) {
        dump_stage(2, "Render", &rendered);
    }

    // Set content info with rendered paths for adapter context
    let content_info =
        crate::adapters_impl::content_info_with_platform(content, platform_id, config)
            .with_rendered_paths(rendered.paths.clone());
    ctx.set_content_info(content_info);

    // Stage 3 (Parse)
    let document = stages::parse(&rendered)?;
    if should_dump(debug_stage, PipelineStage::Parse) {
        dump_stage(3, "Parse", &document);
    }

    // Stage 4 (Transform)
    let document = stages::transform(adapter, content, platform_id, document, ctx)?;
    if should_dump(debug_stage, PipelineStage::Transform) {
        dump_stage(4, "Transform", &document);
    }

    // Stage 5 (Specialize): adapter-specific payload construction.
    // IR is moved into AdapterPayload here.
    let payload = adapter.specialize_payload(document, ctx).await?;
    if should_dump(debug_stage, PipelineStage::Specialize) {
        dump_stage(5, "Specialize", &payload);
    }

    // Stage 6 (Provision, optional): find/create remote target identity.
    let payload = adapter.provision_target(payload, ctx).await?;
    if should_dump(debug_stage, PipelineStage::Provision) {
        dump_stage(6, "Provision", &payload);
    }

    // Stage 7 (Materialize, optional): resolve remote asset references / modify IR.
    let payload = adapter.materialize_payload(payload, ctx).await?;
    if should_dump(debug_stage, PipelineStage::Materialize) {
        dump_stage(7, "Materialize", &payload);
    }

    // Stage 8 (Serialize): document IR -> target format.
    let payload = adapter.serialize_payload(payload, ctx).await?;
    if should_dump(debug_stage, PipelineStage::Serialize) {
        dump_stage(8, "Serialize", &payload);
    }

    // Stage 9 (Publish): adapter publish execution.
    let result = adapter.publish_payload(payload, ctx).await?;
    if should_dump(debug_stage, PipelineStage::Publish) {
        dump_stage(9, "Publish", &result);
    }

    // Stage 10 (Persist): status persistence with reconciliation on failure.
    // Per [[RFC-0005:C-STATUS-TRACKING]], API-based platforms store remote_status.
    // Determine remote_status from lifecycle action.
    let remote_status =
        helpers::determine_remote_status_from_lifecycle(content, platform_id, ctx, adapter)?;
    helpers::persist_status_or_reconcile(
        &mut ctx.status,
        content,
        platform_id,
        &result,
        remote_status.as_deref(),
    )?;
    if should_dump(debug_stage, PipelineStage::Persist) {
        eprintln!("[DEBUG] Stage 10 (Persist): status saved");
    }

    Ok(result)
}

/// Execute pipeline stages 1-8 for one platform (dry-run mode).
///
/// This runs the full local processing pipeline but skips:
/// - Stage 7 (Materialize): no asset uploads (validates files exist but doesn't upload)
/// - Stage 9 (Publish): no remote API calls
/// - Stage 10 (Persist): no status changes
///
/// Useful for validating the entire pipeline without side effects.
pub async fn dry_run_single_platform(
    adapter: &dyn PlatformAdapter,
    platform_id: &str,
    content: &Content,
    renderer: &Renderer<'_>,
    ctx: &mut PublishContext,
    config: &typub_config::Config,
    debug_stage: Option<PipelineStage>,
) -> Result<()> {
    // Stage 1 (Resolve): compute ResolvedConfig for this (content, platform) pair
    let resolved = ResolvedConfig::resolve(content, platform_id, config, adapter.default_config())?;
    if should_dump(debug_stage, PipelineStage::Resolve) {
        dump_stage(1, "Resolve", &resolved);
    }
    ctx.set_resolved(resolved);
    helpers::ensure_node_policy_declared(platform_id)?;

    // Stage 2 (Render)
    let rendered = stages::render(adapter, content, platform_id, ctx, renderer, config).await?;
    if should_dump(debug_stage, PipelineStage::Render) {
        dump_stage(2, "Render", &rendered);
    }

    // Set content info with rendered paths for adapter context
    let content_info =
        crate::adapters_impl::content_info_with_platform(content, platform_id, config)
            .with_rendered_paths(rendered.paths.clone());
    ctx.set_content_info(content_info);

    // Stage 3 (Parse)
    let document = stages::parse(&rendered)?;
    if should_dump(debug_stage, PipelineStage::Parse) {
        dump_stage(3, "Parse", &document);
    }

    // Stage 4 (Transform)
    let document = stages::transform(adapter, content, platform_id, document, ctx)?;
    if should_dump(debug_stage, PipelineStage::Transform) {
        dump_stage(4, "Transform", &document);
    }

    // Stage 5 (Specialize): adapter-specific payload construction.
    let payload = adapter.specialize_payload(document, ctx).await?;
    if should_dump(debug_stage, PipelineStage::Specialize) {
        dump_stage(5, "Specialize", &payload);
    }

    // Stage 6 (Provision): Execute normally.
    // Most adapters' provision_target is no-op or read-only (e.g., Notion schema fetch).
    // Note: Confluence's provision_target may create a page — this is a known limitation.
    // TODO: Add dry_run flag to provision_target for side-effect-free mode.
    let payload = adapter.provision_target(payload, ctx).await?;
    if should_dump(debug_stage, PipelineStage::Provision) {
        dump_stage(6, "Provision", &payload);
    }

    // Stage 7 (Materialize): Call adapter's materialize_payload.
    // In dry-run mode (ctx.is_dry_run() = true), adapters should mock
    // asset uploads by copying to temp dir instead of uploading to remote.
    let payload = adapter.materialize_payload(payload, ctx).await?;
    if should_dump(debug_stage, PipelineStage::Materialize) {
        dump_stage(7, "Materialize", &payload);
    }

    // Stage 8 (Serialize): document IR -> target format.
    // This is local-only, safe to run in dry-run.
    let payload = adapter.serialize_payload(payload, ctx).await?;
    if should_dump(debug_stage, PipelineStage::Serialize) {
        dump_stage(8, "Serialize", &payload);
    }

    // Debug: output serialized payload
    ui::debug(&format!("Serialized payload: \n{:#?}", payload));

    // Stage 9 (Publish): SKIPPED in dry-run
    // Stage 10 (Persist): SKIPPED in dry-run

    Ok(())
}

/// Execute preview pipeline stages 1-5 (or 1-7 for External assets) for one platform,
/// then build preview output.
///
/// All adapters now go through Stage 5 (Specialize) for consistent AST processing
/// (e.g., SVG flattening, SVG→PNG conversion). This ensures preview output matches
/// publish output.
///
/// Per [[RFC-0002:C-PIPELINE-STAGES]], stages used:
/// 1. Resolve,
/// 2. Render,
/// 3. Parse,
/// 4. Transform,
/// 5. Specialize (all adapters),
/// 6. (External assets only) Provision,
/// 7. (External assets only) Materialize,
/// 8. Then: adapter-specific preview building (no remote side effects)
///
/// For Upload strategy, asset local paths are resolved to preview URLs
/// since preview cannot upload attachments to the platform.
pub async fn preview_single_platform(
    adapter: &dyn PlatformAdapter,
    platform_id: &str,
    content: &Content,
    renderer: &Renderer<'_>,
    ctx: &mut PublishContext,
    config: &typub_config::Config,
    debug_stage: Option<PipelineStage>,
) -> Result<PathBuf> {
    use crate::assets::AssetStrategy;

    // Stage 1 (Resolve): compute ResolvedConfig for this (content, platform) pair
    let resolved = ResolvedConfig::resolve(content, platform_id, config, adapter.default_config())?;
    if should_dump(debug_stage, PipelineStage::Resolve) {
        dump_stage(1, "Resolve", &resolved);
    }
    ctx.set_resolved(resolved);
    helpers::ensure_node_policy_declared(platform_id)?;

    // Stage 2 (Render)
    let rendered = stages::render(adapter, content, platform_id, ctx, renderer, config).await?;
    if should_dump(debug_stage, PipelineStage::Render) {
        dump_stage(2, "Render", &rendered);
    }

    // Set content info with rendered paths for adapter context
    let content_info =
        crate::adapters_impl::content_info_with_platform(content, platform_id, config)
            .with_rendered_paths(rendered.paths.clone());
    ctx.set_content_info(content_info);

    // Stage 3 (Parse)
    let document = stages::parse(&rendered)?;
    if should_dump(debug_stage, PipelineStage::Parse) {
        dump_stage(3, "Parse", &document);
    }

    // Stage 4 (Transform)
    let document = stages::transform(adapter, content, platform_id, document, ctx)?;
    if should_dump(debug_stage, PipelineStage::Transform) {
        dump_stage(4, "Transform", &document);
    }

    // Stage 5 (Specialize): adapter-specific IR transformations.
    // All adapters now go through specialize_payload for consistent processing
    // (e.g., SVG flattening, SVG→PNG conversion for CopyPaste).
    let mut payload = adapter.specialize_payload(document, ctx).await?;
    if should_dump(debug_stage, PipelineStage::Specialize) {
        dump_stage(5, "Specialize", &payload);
    }

    match adapter.asset_strategy() {
        AssetStrategy::External => {
            // For External asset strategy, run stages 6-7 to upload and resolve assets.
            // Per [[RFC-0004:C-EXTERNAL-STRATEGY]], external assets must be materialized
            // before preview to populate asset variants with actual URLs.
            // Stage 6 (Provision, optional): no-op for copypaste adapters
            let payload = adapter.provision_target(payload, ctx).await?;
            if should_dump(debug_stage, PipelineStage::Provision) {
                dump_stage(6, "Provision", &payload);
            }

            // Stage 7 (Materialize): upload assets and resolve URLs in AST
            let payload = adapter.materialize_payload(payload, ctx).await?;
            if should_dump(debug_stage, PipelineStage::Materialize) {
                dump_stage(7, "Materialize", &payload);
            }

            // Build preview with resolved document
            adapter.build_preview(&content.meta.title, payload.document, ctx)
        }
        AssetStrategy::Upload => {
            // For Upload strategy, resolve asset local paths to preview URLs.
            // Preview cannot upload attachments to the platform, so we serve local files.
            // Per [[WI-2026-02-20-001]], use IR-level transformation instead of file:// URLs.
            let resolved = helpers::resolve_preview_image_paths(&mut payload.document);
            if resolved > 0 {
                ui::debug(&format!(
                    "Preview: resolved {} image path(s) for preview serving",
                    resolved
                ));
            }
            adapter.build_preview(&content.meta.title, payload.document, ctx)
        }
        _ => {
            // Embed/Copy strategy: specialize already handled asset resolution
            adapter.build_preview(&content.meta.title, payload.document, ctx)
        }
    }
}
