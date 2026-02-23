use anyhow::Result;
use typub_core::{Content, LinkResolution};
use typub_ir::Document;
use typub_passes::resolve_internal_links::{
    StatusTrackerResolver, UnresolvedInternalLinkBehavior,
    collect_internal_link_targets as collect_targets,
    extract_slug_from_relative_href as extract_slug, resolve_href as resolve_href_with_tracker,
};
use typub_passes::{PassCtx, ResolveInternalLinksPass, run_passes};
use typub_storage::StatusTracker;

pub fn extract_slug_from_relative_href(href: &str) -> Option<String> {
    extract_slug(href)
}

pub fn resolve_href(href: &str, platform: &str, tracker: &StatusTracker) -> Result<LinkResolution> {
    resolve_href_with_tracker(href, Some(platform), tracker)
}

pub fn resolve_href_for_copypaste(
    href: &str,
    preferred_platform: Option<&str>,
    tracker: &StatusTracker,
) -> Result<LinkResolution> {
    resolve_href_with_tracker(href, preferred_platform, tracker)
}

pub fn rewrite_links_in_elements(
    document: &Document,
    platform: &str,
    tracker: &StatusTracker,
    _source_title: &str,
) -> Document {
    let mut doc = document.clone();
    let resolver = StatusTrackerResolver::new(tracker);
    let mut pass = ResolveInternalLinksPass::new(&resolver, Some(platform))
        .with_unresolved_behavior(UnresolvedInternalLinkBehavior::ReplaceWithText);
    let mut ctx = PassCtx::default();
    let _ = run_passes(&mut doc, &mut ctx, &mut [&mut pass]);
    doc
}

pub fn rewrite_links_for_copypaste(
    document: &Document,
    _content: &Content,
    internal_link_target: Option<&str>,
    tracker: &StatusTracker,
) -> Document {
    let mut doc = document.clone();
    let resolver = StatusTrackerResolver::new(tracker);
    let mut pass = ResolveInternalLinksPass::new(&resolver, internal_link_target)
        .with_unresolved_behavior(UnresolvedInternalLinkBehavior::ReplaceWithText);
    let mut ctx = PassCtx::default();
    let _ = run_passes(&mut doc, &mut ctx, &mut [&mut pass]);
    doc
}

pub fn collect_internal_link_targets(document: &Document) -> Vec<String> {
    collect_targets(document)
}

pub fn collect_internal_link_targets_from_html(html: &str) -> Result<Vec<String>> {
    let doc = typub_html::parse_html_document(html)?;
    Ok(collect_targets(&doc))
}
