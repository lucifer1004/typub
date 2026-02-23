use std::any::Any;
use std::collections::BTreeMap;
use std::fmt::Debug;

use anyhow::Result;

use typub_config::StorageConfig;
use typub_ir::{DocMeta, Document};
use typub_storage::DeferredAssets;

use crate::types::ContentInfo;

/// Trait for platform-specific payloads.
pub trait PayloadInner: Send + Debug {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

impl<T: Any + Send + Debug> PayloadInner for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

/// Adapter payload carrying AST, platform-specific data, and deferred assets.
pub struct AdapterPayload {
    inner: Box<dyn PayloadInner>,
    pub content_info: ContentInfo,
    pub assets: DeferredAssets,
    pub document: Document,
    pub storage_config: Option<StorageConfig>,
    pub theme_id: Option<String>,
    pub published: Option<bool>,
}

impl AdapterPayload {
    pub fn new<T: Any + Send + Debug>(
        inner: T,
        content_info: ContentInfo,
        assets: DeferredAssets,
        document: Document,
    ) -> Self {
        Self {
            inner: Box::new(inner),
            content_info,
            assets,
            document,
            storage_config: None,
            theme_id: None,
            published: None,
        }
    }

    pub fn simple<T: Any + Send + Debug>(inner: T, slug: &str) -> Self {
        Self::new(
            inner,
            ContentInfo::minimal("", slug, ""),
            DeferredAssets::empty(),
            empty_document(),
        )
    }

    pub fn with_storage(mut self, config: StorageConfig) -> Self {
        self.storage_config = Some(config);
        self
    }

    pub fn with_theme(mut self, theme_id: impl Into<String>) -> Self {
        self.theme_id = Some(theme_id.into());
        self
    }

    pub fn with_published(mut self, published: bool) -> Self {
        self.published = Some(published);
        self
    }

    pub fn downcast<T: 'static>(self, adapter_name: &str) -> Result<T> {
        self.inner
            .into_any()
            .downcast::<T>()
            .map(|b| *b)
            .map_err(|_| anyhow::anyhow!("Invalid {} publish payload type", adapter_name))
    }

    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        (*self.inner).as_any().downcast_ref()
    }

    pub fn downcast_mut<T: 'static>(&mut self) -> Option<&mut T> {
        (*self.inner).as_any_mut().downcast_mut()
    }

    pub fn inner_type_id(&self) -> std::any::TypeId {
        (*self.inner).as_any().type_id()
    }

    pub fn map_inner<T: 'static + Send + Debug, F>(self, adapter_name: &str, f: F) -> Result<Self>
    where
        F: FnOnce(T) -> T,
    {
        let content_info = self.content_info;
        let assets = self.assets;
        let document = self.document;
        let storage_config = self.storage_config;
        let theme_id = self.theme_id;
        let published = self.published;
        let inner = self
            .inner
            .into_any()
            .downcast::<T>()
            .map(|b| *b)
            .map_err(|_| anyhow::anyhow!("Invalid {} publish payload type", adapter_name))?;
        Ok(Self {
            inner: Box::new(f(inner)),
            content_info,
            assets,
            document,
            storage_config,
            theme_id,
            published,
        })
    }

    pub async fn map_inner_async<T: 'static + Send + Debug, F, Fut>(
        self,
        adapter_name: &str,
        f: F,
    ) -> Result<Self>
    where
        F: FnOnce(T) -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let content_info = self.content_info;
        let assets = self.assets;
        let document = self.document;
        let storage_config = self.storage_config;
        let theme_id = self.theme_id;
        let published = self.published;
        let inner = self
            .inner
            .into_any()
            .downcast::<T>()
            .map(|b| *b)
            .map_err(|_| anyhow::anyhow!("Invalid {} publish payload type", adapter_name))?;
        let transformed = f(inner).await?;
        Ok(Self {
            inner: Box::new(transformed),
            content_info,
            assets,
            document,
            storage_config,
            theme_id,
            published,
        })
    }
}

impl Debug for AdapterPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdapterPayload")
            .field("inner", &self.inner)
            .field("content_info", &self.content_info)
            .field("assets", &self.assets)
            .field("document", &self.document)
            .field("storage_config", &self.storage_config.is_some())
            .field("theme_id", &self.theme_id)
            .field("published", &self.published)
            .finish()
    }
}

/// Downcast a type-erased adapter payload to a concrete type.
pub fn downcast_payload<T: 'static>(payload: AdapterPayload, adapter_name: &str) -> Result<T> {
    payload.downcast(adapter_name)
}

fn empty_document() -> Document {
    Document {
        blocks: Vec::new(),
        footnotes: BTreeMap::new(),
        assets: BTreeMap::new(),
        meta: DocMeta::default(),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::types::ContentInfo;
    use typub_ir::{Block, BlockAttrs, Inline};
    use typub_storage::DeferredAssets;

    #[test]
    fn test_payload_simple() {
        let payload = AdapterPayload::simple(42u32, "test-slug");
        assert_eq!(payload.content_info.slug, "test-slug");
        assert!(payload.document.blocks.is_empty());
        assert!(payload.storage_config.is_none());
        assert!(payload.theme_id.is_none());
        assert!(payload.published.is_none());
    }

    #[test]
    fn test_payload_new() {
        let content_info = ContentInfo::minimal("Title", "slug", "/path");
        let deferred = DeferredAssets::empty();
        let mut document = empty_document();
        document.blocks.push(Block::Paragraph {
            content: vec![Inline::Text("Hello".to_string())],
            attrs: BlockAttrs::default(),
        });

        let payload = AdapterPayload::new(42u32, content_info.clone(), deferred, document);
        assert_eq!(payload.content_info.title, "Title");
        assert_eq!(payload.document.blocks.len(), 1);
    }

    #[test]
    fn test_payload_downcast_ref() {
        let payload = AdapterPayload::simple(42u32, "slug");
        let value = payload.downcast_ref::<u32>();
        assert_eq!(value, Some(&42));
        let wrong = payload.downcast_ref::<String>();
        assert!(wrong.is_none());
    }

    #[test]
    fn test_payload_downcast_mut() {
        let mut payload = AdapterPayload::simple(42u32, "slug");
        if let Some(value) = payload.downcast_mut::<u32>() {
            *value = 100;
        }
        let result: u32 = payload.downcast("test").expect("downcast");
        assert_eq!(result, 100);
    }

    #[test]
    fn test_payload_inner_type_id() {
        let payload = AdapterPayload::simple(42u32, "slug");
        let type_id = payload.inner_type_id();
        assert_eq!(type_id, std::any::TypeId::of::<u32>());
    }

    #[test]
    fn test_payload_with_storage() {
        let mut storage = typub_config::StorageConfig::default();
        storage.bucket = Some("my-bucket".into());
        let payload = AdapterPayload::simple(42u32, "slug").with_storage(storage);
        assert!(payload.storage_config.is_some());
    }
}
