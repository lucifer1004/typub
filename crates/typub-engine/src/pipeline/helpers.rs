use crate::adapters::{PlatformAdapter, PublishContext, adapter_capability, is_copypaste_platform};
use crate::content::Content;
use crate::internal_links;
use anyhow::Result;
use typub_core::{CapabilityGapBehavior, DraftSupport};
use typub_ir::{Asset, AssetSource, AssetVariant, Document, Url};
use typub_passes::{ApplyNodePolicyPass, PassCtx, run_passes};
use typub_storage::{LifecycleAction, PublishResult, StatusTracker, determine_lifecycle_action};
use typub_ui as ui;

pub fn ensure_node_policy_declared(platform_id: &str) -> Result<()> {
    let Some(capability) = adapter_capability(platform_id) else {
        anyhow::bail!(
            "Missing capability declaration for '{}'. Selected adapter must declare node policy.",
            platform_id
        );
    };
    let _ = capability.node_policy();
    Ok(())
}

pub fn apply_shared_transforms(
    content: &Content,
    platform_id: &str,
    mut document: Document,
    ctx: &PublishContext,
    enable_shared_link_rewrite: bool,
) -> Result<Document> {
    enforce_internal_link_capability(platform_id, &document)?;

    if enable_shared_link_rewrite {
        if is_copypaste_platform(platform_id) {
            let resolved = ctx.resolved().ok_or_else(|| {
                anyhow::anyhow!(
                    "ResolvedConfig not set - pipeline must call set_resolved() before stages"
                )
            })?;
            document = internal_links::rewrite_links_for_copypaste(
                &document,
                content,
                resolved.internal_link_target.as_deref(),
                &ctx.status,
            );
        } else {
            document = internal_links::rewrite_links_in_elements(
                &document,
                platform_id,
                &ctx.status,
                &content.meta.title,
            );
        }
    }

    document = apply_node_policy(platform_id, document, ctx)?;

    Ok(document)
}

fn apply_node_policy(
    platform_id: &str,
    mut document: Document,
    ctx: &PublishContext,
) -> Result<Document> {
    let Some(capability) = adapter_capability(platform_id) else {
        return Ok(document);
    };

    let mut policy = capability.node_policy();
    if let Some(resolved) = ctx.resolved()
        && let Some(override_policy) = resolved.node_policy_override
    {
        if let Some(raw) = override_policy.raw {
            policy.raw = raw;
        }
        if let Some(unknown) = override_policy.unknown {
            policy.unknown = unknown;
        }
    }
    let mut pass = ApplyNodePolicyPass::new(policy.raw, policy.unknown);
    let mut ctx = PassCtx::default();
    run_passes(&mut document, &mut ctx, &mut [&mut pass])?;
    Ok(document)
}

fn enforce_internal_link_capability(platform_id: &str, document: &Document) -> Result<()> {
    let Some(capability) = adapter_capability(platform_id) else {
        return Ok(());
    };
    let Some(behavior) = capability.internal_links_gap_behavior() else {
        return Ok(());
    };

    let targets = internal_links::collect_internal_link_targets(document);
    if targets.is_empty() {
        return Ok(());
    }
    let targets_preview = targets
        .iter()
        .take(3)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    let summary = format!(
        "Found {} unresolved-by-capability internal link target(s) for '{}': {}",
        targets.len(),
        platform_id,
        targets_preview
    );
    match behavior {
        CapabilityGapBehavior::WarnAndDegrade => {
            ui::warn(&format!(
                "{summary}. Links will not be rewritten on this platform."
            ));
            Ok(())
        }
        CapabilityGapBehavior::HardError => {
            anyhow::bail!("{summary}. Platform policy requires hard error.");
        }
    }
}

pub fn resolve_preview_image_paths(document: &mut Document) -> usize {
    let mut resolved = 0usize;

    for asset in document.assets.values_mut() {
        let (source, variants) = match asset {
            Asset::Image(image) => (&image.source, &mut image.variants),
            Asset::Video(media) | Asset::Audio(media) => (&media.source, &mut media.variants),
            Asset::File(file) => (&file.source, &mut file.variants),
            Asset::Custom(custom) => (&custom.source, &mut custom.variants),
        };

        let AssetSource::LocalPath { path } = source else {
            continue;
        };

        let has_preview_url = variants
            .iter()
            .any(|v| v.name == "original" && v.publish_url.0.starts_with("/__asset__/"));
        if has_preview_url {
            continue;
        }

        variants.push(AssetVariant {
            name: "original".to_string(),
            publish_url: Url(format!("/__asset__/{}", path.as_str())),
            width: None,
            height: None,
        });
        resolved += 1;
    }

    resolved
}

pub fn determine_remote_status_from_lifecycle(
    content: &Content,
    platform_id: &str,
    ctx: &PublishContext,
    _adapter: &dyn PlatformAdapter,
) -> Result<Option<String>> {
    if adapter_capability(platform_id).is_some_and(|cap| cap.local_output) {
        return Ok(None);
    }

    let resolved = ctx
        .resolved()
        .ok_or_else(|| anyhow::anyhow!("Pipeline must set resolved config before persist stage"))?;

    let current_status = ctx
        .status
        .load_platform_status_internal(content.slug(), platform_id)?;
    let has_remote_object = current_status
        .as_ref()
        .and_then(|s| s.last_publish.as_ref())
        .and_then(|p| p.platform_id.as_ref())
        .is_some();
    let remote_status = current_status
        .as_ref()
        .and_then(|s| s.remote_status.as_deref());

    let draft_support = adapter_capability(platform_id)
        .map(|cap| cap.draft_support())
        .unwrap_or(DraftSupport::None);

    let action = determine_lifecycle_action(
        has_remote_object,
        remote_status,
        resolved.published,
        draft_support,
    );

    let result_status = match action {
        LifecycleAction::CreatePublished
        | LifecycleAction::UpdatePublished
        | LifecycleAction::TransitionDraftToPublished => "published",
        LifecycleAction::CreateDraft
        | LifecycleAction::UpdateDraft
        | LifecycleAction::TransitionPublishedToDraft => "draft",
        LifecycleAction::WarnCannotUnpublish => remote_status.unwrap_or("published"),
    };

    Ok(Some(result_status.to_string()))
}

pub fn persist_status_or_reconcile(
    tracker: &mut StatusTracker,
    content: &Content,
    platform: &str,
    result: &PublishResult,
    remote_status: Option<&str>,
) -> Result<()> {
    if let Err(err) = tracker.mark_published(content, platform, result, remote_status) {
        let err_text = format!("{:#}", err);
        let reconcile_result = tracker.record_reconcile(
            content.slug(),
            platform,
            result.platform_id.as_deref(),
            result.url.as_deref(),
            &err_text,
        );

        let guidance = match reconcile_result {
            Ok(()) => "A reconcile record was written. Recover local status with remote ID/URL."
                .to_string(),
            Err(reconcile_err) => format!(
                "Failed to write reconcile record: {:#}. Recover local status manually using remote ID/URL.",
                reconcile_err
            ),
        };

        return Err(anyhow::anyhow!(
            "Status persistence failed after remote publish success for '{}' on '{}': {}\n{}",
            content.slug(),
            platform,
            err_text,
            guidance
        )
        .context("publish reconciliation required"));
    }

    Ok(())
}
