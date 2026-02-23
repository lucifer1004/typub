//! Internal link pass for v2 semantic document IR.

use anyhow::Result;
use std::collections::BTreeSet;
use typub_core::LinkResolution;
use typub_html::inlines_text;
use typub_ir::{Block, Document, Inline, ListKind, UnknownChild, Url};
use typub_storage::StatusTracker;

use super::walk::{NodePath, VisitorMut, walk_document_mut};
use super::{Diagnostic, DiagnosticLevel, Pass, PassCtx};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedInternalHref {
    slug: String,
    suffix: String,
}

/// Behavior for unresolved internal links.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UnresolvedInternalLinkBehavior {
    /// Keep the original link href unchanged.
    KeepLink,
    /// Replace link node with plain text extracted from link content.
    #[default]
    ReplaceWithText,
}

/// Resolve internal slugs to published URLs.
pub trait LinkResolver {
    /// Resolve `slug` for a preferred platform, or any platform when `preferred_platform` is `None`.
    fn resolve_slug(&self, slug: &str, preferred_platform: Option<&str>) -> Result<Option<String>>;
}

/// `LinkResolver` backed by `StatusTracker`.
pub struct StatusTrackerResolver<'a> {
    tracker: &'a StatusTracker,
}

impl<'a> StatusTrackerResolver<'a> {
    pub fn new(tracker: &'a StatusTracker) -> Self {
        Self { tracker }
    }
}

impl LinkResolver for StatusTrackerResolver<'_> {
    fn resolve_slug(&self, slug: &str, preferred_platform: Option<&str>) -> Result<Option<String>> {
        if let Some(platform) = preferred_platform {
            return self.tracker.get_published_url(slug, platform);
        }
        self.tracker
            .get_first_published_url(slug)
            .map(|v| v.map(|(_, url)| url))
    }
}

/// Link rewrite pass over `Inline::Link`.
pub struct ResolveInternalLinksPass<'a> {
    resolver: &'a dyn LinkResolver,
    preferred_platform: Option<&'a str>,
    unresolved_behavior: UnresolvedInternalLinkBehavior,
}

impl<'a> ResolveInternalLinksPass<'a> {
    pub fn new(resolver: &'a dyn LinkResolver, preferred_platform: Option<&'a str>) -> Self {
        Self {
            resolver,
            preferred_platform,
            unresolved_behavior: UnresolvedInternalLinkBehavior::default(),
        }
    }

    pub fn with_unresolved_behavior(mut self, behavior: UnresolvedInternalLinkBehavior) -> Self {
        self.unresolved_behavior = behavior;
        self
    }
}

impl Pass for ResolveInternalLinksPass<'_> {
    fn name(&self) -> &'static str {
        "resolve_internal_links"
    }

    fn run(&mut self, doc: &mut Document, ctx: &mut PassCtx) -> Result<()> {
        let mut rewriter = LinkRewriter {
            resolver: self.resolver,
            preferred_platform: self.preferred_platform,
            unresolved_behavior: self.unresolved_behavior,
            diagnostics: Vec::new(),
        };
        walk_document_mut(doc, &mut rewriter)?;
        ctx.diagnostics.append(&mut rewriter.diagnostics);
        Ok(())
    }
}

struct LinkRewriter<'a> {
    resolver: &'a dyn LinkResolver,
    preferred_platform: Option<&'a str>,
    unresolved_behavior: UnresolvedInternalLinkBehavior,
    diagnostics: Vec<Diagnostic>,
}

impl LinkRewriter<'_> {
    fn warning(&mut self, message: String, path: &NodePath) {
        self.diagnostics.push(Diagnostic {
            pass: "resolve_internal_links",
            level: DiagnosticLevel::Warning,
            message,
            location: Some(path.render()),
        });
    }
}

impl VisitorMut for LinkRewriter<'_> {
    fn visit_inline(&mut self, inline: &mut Inline, path: &NodePath) -> Result<()> {
        let (href, text_fallback) = match inline {
            Inline::Link { content, href, .. } => (href.0.clone(), inlines_text(content)),
            _ => return Ok(()),
        };

        match resolve_href_with(self.resolver, &href, self.preferred_platform) {
            Ok(LinkResolution::NonInternal) => {}
            Ok(LinkResolution::InternalResolved { url, .. }) => {
                if let Inline::Link { href, .. } = inline {
                    *href = Url(url);
                }
            }
            Ok(LinkResolution::InternalUnresolved { slug }) => {
                self.warning(
                    format!("internal link target '{}' not found; link degraded", slug),
                    path,
                );
                if self.unresolved_behavior == UnresolvedInternalLinkBehavior::ReplaceWithText {
                    *inline = Inline::Text(text_fallback);
                }
            }
            Err(err) => {
                self.warning(
                    format!(
                        "failed to resolve internal link '{}': {}; link degraded",
                        href, err
                    ),
                    path,
                );
                if self.unresolved_behavior == UnresolvedInternalLinkBehavior::ReplaceWithText {
                    *inline = Inline::Text(text_fallback);
                }
            }
        }

        Ok(())
    }
}

fn looks_external(href: &str) -> bool {
    let h = href.trim().to_ascii_lowercase();
    let looks_like_scheme = h
        .find(':')
        .is_some_and(|idx| !h[..idx].contains('/') && !h[..idx].is_empty());
    h.starts_with("http://")
        || h.starts_with("https://")
        || h.starts_with("mailto:")
        || h.starts_with("tel:")
        || h.starts_with("file:")
        || h.starts_with("data:")
        || h.starts_with('#')
        || h.starts_with("//")
        || looks_like_scheme
}

fn split_path_and_suffix(href: &str) -> Option<(&str, &str)> {
    let trimmed = href.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(i) = trimmed.find(['?', '#']) {
        Some((&trimmed[..i], &trimmed[i..]))
    } else {
        Some((trimmed, ""))
    }
}

fn parse_internal_href(href: &str) -> Option<ParsedInternalHref> {
    if looks_external(href) {
        return None;
    }
    let (base, suffix) = split_path_and_suffix(href)?;
    if base.is_empty() {
        return None;
    }
    let trimmed = base.trim_start_matches('/').trim_end_matches('/');
    let parts: Vec<&str> = trimmed.split('/').filter(|s| !s.is_empty()).collect();
    let mut segment = parts.last().copied()?.trim();
    if segment.is_empty() {
        return None;
    }
    if let Some(no_ext) = segment.strip_suffix(".html") {
        segment = no_ext;
    }
    if segment == "index"
        && let Some(prev) = parts.iter().rev().nth(1)
        && !prev.trim().is_empty()
    {
        segment = prev.trim();
    }
    if segment.is_empty() {
        return None;
    }
    Some(ParsedInternalHref {
        slug: segment.to_string(),
        suffix: suffix.to_string(),
    })
}

fn apply_url_suffix(base_url: &str, suffix: &str) -> String {
    if suffix.is_empty() {
        return base_url.to_string();
    }
    format!("{base_url}{suffix}")
}

/// Resolve href using a custom resolver.
pub fn resolve_href_with(
    resolver: &dyn LinkResolver,
    href: &str,
    preferred_platform: Option<&str>,
) -> Result<LinkResolution> {
    let Some(parsed) = parse_internal_href(href) else {
        return Ok(LinkResolution::NonInternal);
    };

    if let Some(preferred) = preferred_platform
        && let Some(url) = resolver.resolve_slug(&parsed.slug, Some(preferred))?
    {
        return Ok(LinkResolution::InternalResolved {
            slug: parsed.slug,
            url: apply_url_suffix(&url, &parsed.suffix),
        });
    }

    if let Some(url) = resolver.resolve_slug(&parsed.slug, None)? {
        return Ok(LinkResolution::InternalResolved {
            slug: parsed.slug,
            url: apply_url_suffix(&url, &parsed.suffix),
        });
    }

    Ok(LinkResolution::InternalUnresolved { slug: parsed.slug })
}

/// Resolve href with a `StatusTracker`.
pub fn resolve_href(
    href: &str,
    preferred_platform: Option<&str>,
    tracker: &StatusTracker,
) -> Result<LinkResolution> {
    let resolver = StatusTrackerResolver::new(tracker);
    resolve_href_with(&resolver, href, preferred_platform)
}

/// Extract the target slug from a relative/internal href.
pub fn extract_slug_from_relative_href(href: &str) -> Option<String> {
    parse_internal_href(href).map(|v| v.slug)
}

/// Collect unique internal link targets from a semantic document.
pub fn collect_internal_link_targets(doc: &Document) -> Vec<String> {
    let mut slugs = BTreeSet::new();
    collect_from_blocks(&doc.blocks, &mut slugs);
    for def in doc.footnotes.values() {
        collect_from_blocks(&def.blocks, &mut slugs);
    }
    slugs.into_iter().collect()
}

fn collect_from_blocks(blocks: &[Block], out: &mut BTreeSet<String>) {
    for block in blocks {
        collect_from_block(block, out);
    }
}

fn collect_from_block(block: &Block, out: &mut BTreeSet<String>) {
    match block {
        Block::Heading { content, .. } | Block::Paragraph { content, .. } => {
            collect_from_inlines(content, out);
        }
        Block::Quote { blocks, .. }
        | Block::Figure {
            content: blocks, ..
        }
        | Block::Admonition { blocks, .. }
        | Block::Details { blocks, .. } => collect_from_blocks(blocks, out),
        Block::CodeBlock { .. }
        | Block::Divider { .. }
        | Block::MathBlock { .. }
        | Block::SvgBlock { .. } => {}
        Block::List { list, .. } => collect_from_list_kind(&list.kind, out),
        Block::DefinitionList { items, .. } => {
            for item in items {
                for term in &item.terms {
                    collect_from_blocks(term, out);
                }
                for definition in &item.definitions {
                    collect_from_blocks(definition, out);
                }
            }
        }
        Block::Table {
            caption, sections, ..
        } => {
            if let Some(caption) = caption {
                collect_from_blocks(caption, out);
            }
            for section in sections {
                for row in &section.rows {
                    for cell in &row.cells {
                        collect_from_blocks(&cell.blocks, out);
                    }
                }
            }
        }
        Block::UnknownBlock { children, .. } => {
            for child in children {
                match child {
                    UnknownChild::Block(block) => collect_from_block(block, out),
                    UnknownChild::Inline(inline) => collect_from_inline(inline, out),
                }
            }
        }
        Block::RawBlock { .. } => {}
    }
}

fn collect_from_list_kind(kind: &ListKind, out: &mut BTreeSet<String>) {
    match kind {
        ListKind::Bullet { items } | ListKind::Numbered { items, .. } => {
            for item in items {
                collect_from_blocks(&item.blocks, out);
            }
        }
        ListKind::Task { items } => {
            for item in items {
                collect_from_blocks(&item.blocks, out);
            }
        }
        ListKind::Custom { items, .. } => {
            for item in items {
                collect_from_blocks(&item.blocks, out);
            }
        }
    }
}

fn collect_from_inlines(inlines: &[Inline], out: &mut BTreeSet<String>) {
    for inline in inlines {
        collect_from_inline(inline, out);
    }
}

fn collect_from_inline(inline: &Inline, out: &mut BTreeSet<String>) {
    match inline {
        Inline::Link { content, href, .. } => {
            if let Some(parsed) = parse_internal_href(&href.0) {
                out.insert(parsed.slug);
            }
            collect_from_inlines(content, out);
        }
        Inline::Styled { content, .. } | Inline::UnknownInline { content, .. } => {
            collect_from_inlines(content, out)
        }
        Inline::Text(_)
        | Inline::Code(_)
        | Inline::SoftBreak
        | Inline::HardBreak
        | Inline::Image { .. }
        | Inline::FootnoteRef(_)
        | Inline::MathInline { .. }
        | Inline::SvgInline { .. }
        | Inline::RawInline { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use std::collections::BTreeMap;
    use typub_ir::{BlockAttrs, DocMeta, Document, InlineAttrs};

    #[derive(Default)]
    struct MockResolver {
        by_platform: BTreeMap<(String, String), String>,
        any_platform: BTreeMap<String, String>,
        fail_slug: Option<String>,
    }

    impl LinkResolver for MockResolver {
        fn resolve_slug(
            &self,
            slug: &str,
            preferred_platform: Option<&str>,
        ) -> Result<Option<String>> {
            if self.fail_slug.as_deref() == Some(slug) {
                anyhow::bail!("resolver failure for slug '{}'", slug);
            }
            if let Some(platform) = preferred_platform
                && let Some(url) = self
                    .by_platform
                    .get(&(slug.to_string(), platform.to_string()))
            {
                return Ok(Some(url.clone()));
            }
            Ok(self.any_platform.get(slug).cloned())
        }
    }

    fn simple_doc_with_link(href: &str, text: &str) -> Document {
        Document {
            blocks: vec![Block::Paragraph {
                content: vec![Inline::Link {
                    content: vec![Inline::Text(text.to_string())],
                    href: Url(href.to_string()),
                    title: None,
                    attrs: InlineAttrs::default(),
                }],
                attrs: BlockAttrs::default(),
            }],
            footnotes: BTreeMap::new(),
            assets: BTreeMap::new(),
            meta: DocMeta::default(),
        }
    }

    #[test]
    fn extract_slug_from_relative_href_handles_common_paths() {
        assert_eq!(
            extract_slug_from_relative_href("../2026-02-09-hello-world/"),
            Some("2026-02-09-hello-world".to_string())
        );
        assert_eq!(
            extract_slug_from_relative_href("./2026-02-09-hello-world/index.html"),
            Some("2026-02-09-hello-world".to_string())
        );
        assert_eq!(
            extract_slug_from_relative_href("https://example.com/post"),
            None
        );
        assert_eq!(extract_slug_from_relative_href("#section"), None);
        assert_eq!(
            extract_slug_from_relative_href("/absolute/path"),
            Some("path".to_string())
        );
    }

    #[test]
    fn collect_internal_link_targets_is_deduplicated_and_sorted() {
        let doc = Document {
            blocks: vec![
                Block::Paragraph {
                    content: vec![
                        Inline::Link {
                            content: vec![Inline::Text("a".to_string())],
                            href: Url("/b-post".to_string()),
                            title: None,
                            attrs: InlineAttrs::default(),
                        },
                        Inline::Link {
                            content: vec![Inline::Text("a2".to_string())],
                            href: Url("/a-post".to_string()),
                            title: None,
                            attrs: InlineAttrs::default(),
                        },
                        Inline::Link {
                            content: vec![Inline::Text("external".to_string())],
                            href: Url("https://example.com".to_string()),
                            title: None,
                            attrs: InlineAttrs::default(),
                        },
                    ],
                    attrs: BlockAttrs::default(),
                },
                Block::UnknownBlock {
                    tag: "x".to_string(),
                    attrs: BlockAttrs::default(),
                    children: vec![UnknownChild::Inline(Inline::Link {
                        content: vec![Inline::Text("dup".to_string())],
                        href: Url("/a-post".to_string()),
                        title: None,
                        attrs: InlineAttrs::default(),
                    })],
                    data: BTreeMap::new(),
                    note: None,
                    source: None,
                },
            ],
            footnotes: BTreeMap::new(),
            assets: BTreeMap::new(),
            meta: DocMeta::default(),
        };

        let targets = collect_internal_link_targets(&doc);
        assert_eq!(targets, vec!["a-post".to_string(), "b-post".to_string()]);
    }

    #[test]
    fn link_pass_rewrites_internal_links_when_resolved() {
        let mut doc = simple_doc_with_link("/hello/index.html?x=1#y", "Hello");
        let mut resolver = MockResolver::default();
        resolver.any_platform.insert(
            "hello".to_string(),
            "https://example.com/posts/hello".to_string(),
        );

        let mut pass = ResolveInternalLinksPass::new(&resolver, Some("ghost"));
        let mut ctx = PassCtx::default();
        pass.run(&mut doc, &mut ctx).expect("run link pass");

        match &doc.blocks[0] {
            Block::Paragraph { content, .. } => {
                assert!(
                    matches!(
                        &content[0],
                        Inline::Link { href, .. } if href.0 == "https://example.com/posts/hello?x=1#y"
                    ),
                    "link should be rewritten with suffix preserved"
                );
            }
            other => panic!("unexpected block: {other:?}"),
        }
        assert!(ctx.diagnostics.is_empty());
    }

    #[test]
    fn link_pass_degrades_unresolved_links_to_text_by_default() {
        let mut doc = simple_doc_with_link("/missing", "Missing target");
        let resolver = MockResolver::default();
        let mut pass = ResolveInternalLinksPass::new(&resolver, Some("ghost"));
        let mut ctx = PassCtx::default();
        pass.run(&mut doc, &mut ctx).expect("run link pass");

        match &doc.blocks[0] {
            Block::Paragraph { content, .. } => {
                assert!(matches!(&content[0], Inline::Text(t) if t == "Missing target"));
            }
            other => panic!("unexpected block: {other:?}"),
        }
        assert_eq!(ctx.diagnostics.len(), 1);
        assert_eq!(ctx.diagnostics[0].level, DiagnosticLevel::Warning);
    }

    #[test]
    fn link_pass_can_keep_unresolved_link() {
        let mut doc = simple_doc_with_link("/missing", "Missing target");
        let resolver = MockResolver::default();
        let mut pass = ResolveInternalLinksPass::new(&resolver, None)
            .with_unresolved_behavior(UnresolvedInternalLinkBehavior::KeepLink);
        let mut ctx = PassCtx::default();
        pass.run(&mut doc, &mut ctx).expect("run link pass");

        match &doc.blocks[0] {
            Block::Paragraph { content, .. } => {
                assert!(matches!(&content[0], Inline::Link { href, .. } if href.0 == "/missing"));
            }
            other => panic!("unexpected block: {other:?}"),
        }
        assert_eq!(ctx.diagnostics.len(), 1);
    }

    #[test]
    fn link_pass_leaves_non_internal_links_unchanged() {
        let mut doc = simple_doc_with_link("https://example.com/path", "External");
        let resolver = MockResolver::default();
        let mut pass = ResolveInternalLinksPass::new(&resolver, Some("ghost"));
        let mut ctx = PassCtx::default();
        pass.run(&mut doc, &mut ctx).expect("run link pass");

        match &doc.blocks[0] {
            Block::Paragraph { content, .. } => {
                assert!(matches!(
                    &content[0],
                    Inline::Link { href, .. } if href.0 == "https://example.com/path"
                ));
            }
            other => panic!("unexpected block: {other:?}"),
        }
        assert!(ctx.diagnostics.is_empty());
    }
}
