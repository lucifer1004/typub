use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;

use typub_config::PlatformConfig;
use typub_core::AssetStrategy;
use typub_ir::Document;
use typub_storage::{PendingAssetList, PublishResult};

use crate::context::AdapterContext;
use crate::payload::AdapterPayload;
use crate::types::{ContentInfo, OutputFormat, RenderConfig, ResolvedConfigDefaults};

/// Trait for platform adapters.
#[async_trait(?Send)]
pub trait PlatformAdapter: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn required_format(&self) -> OutputFormat;
    fn asset_strategy(&self) -> AssetStrategy;
    fn validate_config(&self, config: &PlatformConfig) -> Result<()>;

    fn default_config(&self) -> ResolvedConfigDefaults {
        ResolvedConfigDefaults::new(true, None, self.asset_strategy())
    }

    fn render_config(&self, _content_info: &ContentInfo) -> RenderConfig {
        RenderConfig::default()
    }

    fn supports_shared_link_rewrite(&self) -> bool {
        false
    }

    async fn specialize_payload(
        &self,
        document: Document,
        ctx: &dyn AdapterContext,
    ) -> Result<AdapterPayload>;

    async fn provision_target(
        &self,
        payload: AdapterPayload,
        ctx: &dyn AdapterContext,
    ) -> Result<AdapterPayload> {
        let _ = ctx;
        Ok(payload)
    }

    async fn upload_assets(&self, _pending: &PendingAssetList) -> Result<HashMap<usize, String>> {
        Ok(HashMap::new())
    }

    async fn materialize_payload(
        &self,
        payload: AdapterPayload,
        ctx: &dyn AdapterContext,
    ) -> Result<AdapterPayload> {
        let _ = ctx;
        Ok(payload)
    }

    async fn serialize_payload(
        &self,
        payload: AdapterPayload,
        ctx: &dyn AdapterContext,
    ) -> Result<AdapterPayload> {
        let _ = ctx;
        Ok(payload)
    }

    async fn publish_payload(
        &self,
        payload: AdapterPayload,
        ctx: &dyn AdapterContext,
    ) -> Result<PublishResult> {
        let _ = (payload, ctx);
        anyhow::bail!(
            "Adapter '{}' must implement publish_payload() for staged pipeline execution.",
            self.id()
        )
    }

    fn build_preview(
        &self,
        title: &str,
        document: Document,
        ctx: &dyn AdapterContext,
    ) -> Result<PathBuf> {
        let _ = ctx;
        let body = typub_html::document_to_html(&document);
        let html = format!(
            r#"<!DOCTYPE html><html><head><meta charset="utf-8"><title>{} - {}</title></head><body>{}</body></html>"#,
            title,
            self.id(),
            body
        );
        write_preview_file(title, self.id(), &html)
    }

    async fn check_status(&self, slug: &str) -> Result<bool>;
}

pub fn write_preview_file(slug: &str, platform: &str, html: &str) -> Result<PathBuf> {
    let dir = std::env::temp_dir().join("typub-preview");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{slug}-{platform}.html"));
    std::fs::write(&path, html)?;
    Ok(path)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_write_preview_file() {
        let html = "<html><body>Test</body></html>";
        let path = write_preview_file("test-slug", "test-platform", html).expect("write");
        assert!(path.exists());
        let filename = path
            .file_name()
            .expect("filename")
            .to_str()
            .expect("to_str");
        assert!(filename.contains("test-slug"));
        assert!(filename.contains("test-platform"));

        let _ = std::fs::remove_file(&path);
    }
}
