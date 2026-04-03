use crate::adapters::{PlatformAdapter, PublishContext};
use crate::content::Content;
use crate::renderer::{RenderedOutput, Renderer};
use anyhow::Result;
use std::collections::BTreeMap;
use typub_ir::{DocMeta, Document};
use typub_passes::validate_document::validate_document;

pub async fn render(
    adapter: &dyn PlatformAdapter,
    content: &Content,
    platform_id: &str,
    ctx: &PublishContext,
    renderer: &Renderer<'_>,
    config: &typub_config::Config,
) -> Result<RenderedOutput> {
    let format = adapter.required_format();
    let content_info =
        crate::adapters_impl::content_info_with_platform(content, platform_id, config);
    let mut render_config = adapter.render_config(&content_info);
    let user_preamble = ctx
        .resolved()
        .and_then(|resolved| resolved.render_preamble.as_deref());
    render_config.preamble = merge_render_preamble(&render_config.preamble, user_preamble);
    renderer
        .render_for_platform(content, platform_id, format, &render_config)
        .await
}

fn merge_render_preamble(adapter_preamble: &str, user_preamble: Option<&str>) -> String {
    let Some(user) = user_preamble else {
        return adapter_preamble.to_string();
    };

    if adapter_preamble.is_empty() {
        user.to_string()
    } else {
        format!("{adapter_preamble}\n\n{user}")
    }
}

pub fn parse(rendered: &RenderedOutput) -> Result<Document> {
    if let Some(html) = rendered.html.as_ref() {
        typub_html::parse_html_document(html)
    } else {
        Ok(Document {
            blocks: Vec::new(),
            footnotes: BTreeMap::new(),
            assets: BTreeMap::new(),
            meta: DocMeta::default(),
        })
    }
}

pub fn transform(
    adapter: &dyn PlatformAdapter,
    content: &Content,
    platform_id: &str,
    document: Document,
    ctx: &PublishContext,
) -> Result<Document> {
    let document = super::helpers::apply_shared_transforms(
        content,
        platform_id,
        document,
        ctx,
        adapter.supports_shared_link_rewrite(),
    )?;
    validate_document(&document)?;
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::merge_render_preamble;

    #[test]
    fn test_merge_render_preamble_without_user_override() {
        let merged = merge_render_preamble("#set raw(theme: none)", None);
        assert_eq!(merged, "#set raw(theme: none)");
    }

    #[test]
    fn test_merge_render_preamble_when_adapter_empty() {
        let merged = merge_render_preamble("", Some("#set text(size: 11pt)"));
        assert_eq!(merged, "#set text(size: 11pt)");
    }

    #[test]
    fn test_merge_render_preamble_appends_user_after_adapter() {
        let merged = merge_render_preamble("#set raw(theme: none)", Some("#set text(fill: red)"));
        assert_eq!(merged, "#set raw(theme: none)\n\n#set text(fill: red)");
    }

    #[test]
    fn test_merge_render_preamble_preserves_some_empty_string() {
        let merged = merge_render_preamble("#set text(size: 10pt)", Some(""));
        assert_eq!(merged, "#set text(size: 10pt)\n\n");
    }
}
