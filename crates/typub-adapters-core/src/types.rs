use std::collections::HashMap;
use std::path::PathBuf;

use typub_core::{AssetStrategy, MathRendering, ThemeId};

/// Default values provided by the adapter/caller (the "5th layer").
///
/// These are applied when the 4-layer resolution chain returns `None`.
/// Per [[RFC-0005:C-RESOLUTION-ORDER]].
#[derive(Debug, Clone)]
pub struct ResolvedConfigDefaults {
    pub published: bool,
    pub theme: Option<ThemeId>,
    pub asset_strategy: AssetStrategy,
}

impl ResolvedConfigDefaults {
    pub fn new(published: bool, theme: Option<ThemeId>, asset_strategy: AssetStrategy) -> Self {
        Self {
            published,
            theme,
            asset_strategy,
        }
    }
}

impl Default for ResolvedConfigDefaults {
    fn default() -> Self {
        Self {
            published: true,
            theme: None,
            asset_strategy: AssetStrategy::Embed,
        }
    }
}

/// Content metadata packed for adapter use.
#[derive(Debug, Clone)]
pub struct ContentInfo {
    pub title: String,
    pub slug: String,
    pub path: PathBuf,
    pub tags: Vec<String>,
    pub categories: Vec<String>,
    pub assets: Vec<PathBuf>,
    pub platform_extra: HashMap<String, String>,
    pub rendered_paths: Vec<PathBuf>,
}

impl ContentInfo {
    pub fn new(
        title: impl Into<String>,
        slug: impl Into<String>,
        path: impl Into<PathBuf>,
        tags: Vec<String>,
        categories: Vec<String>,
        assets: Vec<PathBuf>,
    ) -> Self {
        Self {
            title: title.into(),
            slug: slug.into(),
            path: path.into(),
            tags,
            categories,
            assets,
            platform_extra: HashMap::new(),
            rendered_paths: Vec::new(),
        }
    }

    pub fn with_platform_extra(
        title: impl Into<String>,
        slug: impl Into<String>,
        path: impl Into<PathBuf>,
        tags: Vec<String>,
        categories: Vec<String>,
        assets: Vec<PathBuf>,
        platform_extra: HashMap<String, String>,
    ) -> Self {
        Self {
            title: title.into(),
            slug: slug.into(),
            path: path.into(),
            tags,
            categories,
            assets,
            platform_extra,
            rendered_paths: Vec::new(),
        }
    }

    pub fn minimal(
        title: impl Into<String>,
        slug: impl Into<String>,
        path: impl Into<PathBuf>,
    ) -> Self {
        Self::new(title, slug, path, Vec::new(), Vec::new(), Vec::new())
    }

    pub fn get_platform_str(&self, key: &str) -> Option<String> {
        self.platform_extra.get(key).cloned()
    }

    pub fn with_rendered_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.rendered_paths = paths;
        self
    }
}

/// Output format for rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Html,
    HtmlFragment,
    Png,
    Pdf,
}

/// Content transformation method
#[derive(Clone, Default, Debug)]
pub enum ContentTransform {
    #[default]
    Default,
    ShowRules(String),
    Custom(String),
}

/// Platform render configuration
#[derive(Clone, Default, Debug)]
pub struct RenderConfig {
    pub imports: Vec<String>,
    pub preamble: String,
    pub template_before: String,
    pub content_transform: ContentTransform,
    pub template_after: String,
    pub image_as_marker: bool,
    pub math_rendering: MathRendering,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_resolved_config_defaults_new() {
        let defaults =
            ResolvedConfigDefaults::new(false, Some("dark".into()), AssetStrategy::Upload);
        assert!(!defaults.published);
        assert_eq!(defaults.theme, Some("dark".into()));
        assert_eq!(defaults.asset_strategy, AssetStrategy::Upload);
    }

    #[test]
    fn test_resolved_config_defaults_default() {
        let defaults = ResolvedConfigDefaults::default();
        assert!(defaults.published);
        assert!(defaults.theme.is_none());
        assert_eq!(defaults.asset_strategy, AssetStrategy::Embed);
    }

    #[test]
    fn test_content_info_new() {
        let info = ContentInfo::new(
            "Title",
            "slug",
            "/path",
            vec!["tag1".into()],
            vec!["cat1".into()],
            vec![PathBuf::from("image.png")],
        );
        assert_eq!(info.title, "Title");
        assert_eq!(info.slug, "slug");
        assert_eq!(info.tags, vec!["tag1"]);
        assert_eq!(info.categories, vec!["cat1"]);
        assert_eq!(info.assets.len(), 1);
        assert!(info.platform_extra.is_empty());
        assert!(info.rendered_paths.is_empty());
    }

    #[test]
    fn test_content_info_with_platform_extra() {
        let mut extra = std::collections::HashMap::new();
        extra.insert("key".into(), "value".into());
        let info = ContentInfo::with_platform_extra(
            "Title",
            "slug",
            "/path",
            vec![],
            vec![],
            vec![],
            extra.clone(),
        );
        assert_eq!(info.get_platform_str("key"), Some("value".into()));
        assert_eq!(info.get_platform_str("missing"), None);
    }

    #[test]
    fn test_content_info_with_rendered_paths() {
        let info = ContentInfo::minimal("T", "s", "/p").with_rendered_paths(vec![
            PathBuf::from("/slide1.png"),
            PathBuf::from("/slide2.png"),
        ]);
        assert_eq!(info.rendered_paths.len(), 2);
    }

    #[test]
    fn test_output_format_variants() {
        assert_eq!(OutputFormat::Html, OutputFormat::Html);
        assert_ne!(OutputFormat::Html, OutputFormat::Png);
    }

    #[test]
    fn test_content_transform_variants() {
        let default = ContentTransform::Default;
        let show_rules = ContentTransform::ShowRules("#show: []".into());
        let custom = ContentTransform::Custom("custom {path}".into());
        assert!(matches!(default, ContentTransform::Default));
        assert!(matches!(show_rules, ContentTransform::ShowRules(_)));
        assert!(matches!(custom, ContentTransform::Custom(_)));
    }

    #[test]
    fn test_render_config_default() {
        let config = RenderConfig::default();
        assert!(config.imports.is_empty());
        assert!(config.preamble.is_empty());
        assert!(!config.image_as_marker);
    }
}
