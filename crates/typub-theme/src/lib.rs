//! Theme system for HTML output styling
//!
//! Provides a unified theming system for all HTML-outputting platforms.
//! Themes are CSS files that can be applied with or without inlining.
//!
//! Themes are loaded in two layers:
//! 1. **Builtins** - Embedded at compile time from `templates/themes/`
//! 2. **User themes** - Loaded at runtime from user's `templates/themes/` directory
//!
//! User themes with the same ID override builtins; new IDs extend the registry.

mod builtin_themes;

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// Re-export builtin CSS for external use
pub use builtin_themes::{BUILTIN_BASE_CSS, BUILTIN_PREVIEW_CSS, BUILTIN_THEMES};

/// A theme with its CSS content
#[derive(Debug, Clone)]
pub struct Theme {
    /// Theme identifier (filename without extension)
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Combined CSS (base + theme)
    pub css: String,
}

/// Registry of available themes
pub struct ThemeRegistry {
    themes: HashMap<String, Theme>,
    base_css: String,
}

impl ThemeRegistry {
    /// Create a new registry with embedded builtins, overlaid with user themes.
    ///
    /// 1. Start with builtin themes (embedded at compile time)
    /// 2. Overlay user themes from `templates/themes/` if directory exists
    /// 3. User themes with same ID override builtins; new IDs are added
    pub fn new() -> Result<Self> {
        // Start with builtins
        let mut registry = Self::from_builtins();

        // Overlay user themes if directory exists
        if let Ok(user_dir) = Self::user_themes_directory()
            && user_dir.exists()
        {
            registry.load_user_themes(&user_dir)?;
        }

        Ok(registry)
    }

    /// Create registry from compile-time embedded builtins only.
    fn from_builtins() -> Self {
        let base_css = builtin_themes::BUILTIN_BASE_CSS.to_string();
        let mut themes = HashMap::new();

        for (id, theme_css) in builtin_themes::BUILTIN_THEMES {
            let combined = format!("{}\n\n/* Theme: {} */\n{}", base_css, id, theme_css);
            let name = Self::id_to_name(id);
            themes.insert(
                id.to_string(),
                Theme {
                    id: id.to_string(),
                    name,
                    css: combined,
                },
            );
        }

        // Ensure "minimal" theme exists (base CSS only)
        if !themes.contains_key("minimal") {
            themes.insert(
                "minimal".to_string(),
                Theme {
                    id: "minimal".to_string(),
                    name: "Minimal".to_string(),
                    css: base_css.clone(),
                },
            );
        }

        Self { themes, base_css }
    }

    /// Get the user themes directory path (project-local).
    fn user_themes_directory() -> Result<PathBuf> {
        let cwd = std::env::current_dir().context("Failed to get current directory")?;
        Ok(cwd.join("templates/themes"))
    }

    /// Load user themes from a directory, overlaying/extending existing themes.
    fn load_user_themes(&mut self, dir: &Path) -> Result<()> {
        // Check for user _base.css override
        let base_path = dir.join("_base.css");
        if base_path.exists() {
            self.base_css = std::fs::read_to_string(&base_path).with_context(|| {
                format!("Failed to read user base CSS: {}", base_path.display())
            })?;
        }

        // Load user theme CSS files
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().is_some_and(|e| e == "css") {
                let Some(stem) = path.file_stem() else {
                    continue;
                };
                let filename = stem.to_string_lossy().to_string();

                // Skip underscore-prefixed files (_base.css, _preview-*.css)
                if filename.starts_with('_') {
                    continue;
                }

                let theme_css = std::fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read user theme: {}", path.display()))?;

                // Combine base + theme CSS
                let combined = format!(
                    "{}\n\n/* Theme: {} */\n{}",
                    self.base_css, filename, theme_css
                );

                let name = Self::id_to_name(&filename);
                // Insert or override existing theme
                self.themes.insert(
                    filename.clone(),
                    Theme {
                        id: filename,
                        name,
                        css: combined,
                    },
                );
            }
        }

        Ok(())
    }

    /// Convert theme ID to display name
    fn id_to_name(id: &str) -> String {
        match id {
            "elegant" => "雅致".to_string(),
            "tech" => "技术".to_string(),
            "minimal" => "极简".to_string(),
            "wechat-green" => "微信绿".to_string(),
            other => other
                .split('-')
                .map(|w| {
                    let mut c = w.chars();
                    match c.next() {
                        None => String::new(),
                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" "),
        }
    }

    /// Get a theme by ID
    pub fn get(&self, id: &str) -> Option<&Theme> {
        self.themes.get(id)
    }

    /// Get a theme by ID, falling back to minimal
    pub fn get_or_default(&self, id: &str) -> Result<&Theme> {
        self.themes
            .get(id)
            .or_else(|| self.themes.get("minimal"))
            .or_else(|| self.themes.values().next())
            .ok_or_else(|| anyhow::anyhow!("theme registry has no themes"))
    }

    /// List all available theme IDs
    pub fn list(&self) -> Vec<&str> {
        self.themes.keys().map(|s| s.as_str()).collect()
    }

    /// Get the base CSS
    pub fn base_css(&self) -> &str {
        &self.base_css
    }

    /// Load preview CSS for a specific adapter/platform.
    ///
    /// Preview CSS files are named `_preview-{platform}.css` and provide
    /// styling for the preview page (toolbar, copy button, container, etc.)
    ///
    /// Resolution order:
    /// 1. User's `templates/themes/_preview-{platform}.css` (if exists)
    /// 2. Builtin preview CSS (embedded at compile time)
    pub fn load_preview_css(&self, platform: &str) -> Option<String> {
        // Check user directory first (override)
        if let Ok(user_dir) = Self::user_themes_directory() {
            let preview_path = user_dir.join(format!("_preview-{}.css", platform));
            if preview_path.exists()
                && let Ok(css) = std::fs::read_to_string(&preview_path)
            {
                return Some(css);
            }
        }

        // Fall back to builtin
        builtin_themes::BUILTIN_PREVIEW_CSS
            .iter()
            .find(|(id, _)| *id == platform)
            .map(|(_, css)| css.to_string())
    }
}

/// Resolve theme using 5-level resolution chain (similar to RFC-0005's published resolution):
///
/// 1. `meta.toml[platforms.X].theme` — per-content platform-specific
/// 2. `meta.toml.theme` — per-content default
/// 3. `typub.toml[platforms.X].theme` — global platform-specific
/// 4. `typub.toml.theme` — global default
/// 5. Profile `default_theme` — hardcoded in profiles.toml
///
/// Falls back to the provided `fallback` theme if no match at any layer.
pub fn resolve_theme(
    platform_id: &str,
    content: &typub_core::Content,
    global_config: &typub_config::Config,
    profile_default_theme: Option<&str>,
    registry: &ThemeRegistry,
    fallback: &Theme,
) -> Theme {
    // Layer 1: meta.toml[platforms.X].theme
    let theme_id: Option<String> = content
        .platform_config(platform_id)
        .and_then(|c| c.get_str("theme"))
        // Layer 2: meta.toml.theme
        .or_else(|| content.meta.theme.as_deref().map(String::from))
        // Layer 3: typub.toml[platforms.X].theme
        .or_else(|| {
            global_config
                .platforms
                .get(platform_id)
                .and_then(|p| p.theme.as_deref().map(String::from))
        })
        // Layer 4: typub.toml.theme
        .or_else(|| global_config.theme.as_deref().map(String::from))
        // Layer 5: profile default_theme
        .or_else(|| profile_default_theme.map(|s| s.to_string()));

    // Resolve theme ID to actual theme
    if let Some(id) = theme_id
        && let Some(theme) = registry.get(&id)
    {
        return theme.clone();
    }

    fallback.clone()
}

/// Load a theme by ID from the registry, with fallback.
///
/// This is the second half of theme resolution - after the theme ID has been
/// resolved via `ResolvedConfig`, this function loads the actual Theme object.
///
/// # Arguments
///
/// * `theme_id` - Pre-resolved theme ID (from `ResolvedConfig.theme_id`)
/// * `profile_default` - Adapter-specific default theme ID (layer 5)
/// * `registry` - Theme registry to look up themes
/// * `fallback` - Fallback theme if no match found
pub fn load_theme(
    theme_id: Option<&str>,
    profile_default: Option<&str>,
    registry: &ThemeRegistry,
    fallback: &Theme,
) -> Theme {
    // Try resolved theme_id first (layers 1-4)
    if let Some(id) = theme_id
        && let Some(theme) = registry.get(id)
    {
        return theme.clone();
    }

    // Try profile default (layer 5)
    if let Some(id) = profile_default
        && let Some(theme) = registry.get(id)
    {
        return theme.clone();
    }

    // Final fallback
    fallback.clone()
}

/// Apply theme to HTML body content.
///
/// * `html` - The HTML content (body only, no DOCTYPE)
/// * `theme` - The theme to apply
/// * `inline` - If true, inline all CSS into style attributes (for WeChat/copy-paste)
///
/// Returns themed HTML with CSS either inlined or as `<style>` block.
///
/// Note: No wrapper div is added. Theme CSS uses direct element selectors
/// (e.g., `h1`, `p`) rather than `.content h1` to ensure styles are inlined
/// directly onto elements. This is required for platforms like WeChat that
/// filter out `<div>` tags.
pub fn apply_theme(html: &str, theme: &Theme, inline: bool) -> Result<String> {
    if inline {
        // Inline CSS for platforms that don't support <style> tags
        inline_css(html, &theme.css)
    } else {
        // Add CSS as <style> block
        Ok(format!("<style>\n{}\n</style>\n{}", theme.css, html))
    }
}

/// Apply theme and wrap in full HTML document (for preview pages).
///
/// Unlike `apply_theme`, this produces a complete `<!DOCTYPE html>` document
/// with CSS in `<head>` only — no duplicate `<style>` in `<body>`.
///
/// Note: No wrapper div is added. Theme CSS uses direct element selectors.
pub fn apply_theme_full_document(
    html: &str,
    theme: &Theme,
    title: &str,
    inline: bool,
) -> Result<String> {
    let (style_block, body) = if inline {
        (String::new(), inline_css(html, &theme.css)?)
    } else {
        (
            format!("<style>\n{}\n</style>", theme.css),
            html.to_string(),
        )
    };

    Ok(format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{}</title>
    {}
</head>
<body>
    {}
</body>
</html>"#,
        title, style_block, body
    ))
}

/// Inline CSS into HTML using css-inline crate
fn inline_css(html: &str, css: &str) -> Result<String> {
    // Create a full document for css-inline to process
    let full_html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
<style>{}</style>
</head>
<body>
{}
</body>
</html>"#,
        css, html
    );

    // Use css-inline to inline styles
    let inliner = css_inline::CSSInliner::options()
        .inline_style_tags(true)
        .keep_style_tags(false)
        .build();

    let inlined = inliner.inline(&full_html).context("Failed to inline CSS")?;

    // Extract just the body content
    if let Some(start) = inlined.find("<body>")
        && let Some(end) = inlined.rfind("</body>")
    {
        return Ok(inlined[start + 6..end].trim().to_string());
    }

    Ok(inlined)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn test_id_to_name() {
        assert_eq!(ThemeRegistry::id_to_name("elegant"), "雅致");
        assert_eq!(ThemeRegistry::id_to_name("tech"), "技术");
        assert_eq!(
            ThemeRegistry::id_to_name("my-custom-theme"),
            "My Custom Theme"
        );
    }

    #[test]
    fn test_apply_theme_inline() {
        let theme = Theme {
            id: "test".to_string(),
            name: "Test".to_string(),
            // No .content wrapper - styles apply directly to elements
            css: "p { color: red; }".to_string(),
        };

        let html = "<p>Hello</p>";
        let result = apply_theme(html, &theme, true).expect("apply theme");

        // Should have inlined style
        assert!(result.contains("color"));
        assert!(result.contains("Hello"));
    }

    #[test]
    fn test_apply_theme_external() {
        let theme = Theme {
            id: "test".to_string(),
            name: "Test".to_string(),
            // No .content wrapper - styles apply directly to elements
            css: "p { color: red; }".to_string(),
        };

        let html = "<p>Hello</p>";
        let result = apply_theme(html, &theme, false).expect("apply theme");

        // Should have style block
        assert!(result.contains("<style>"));
        assert!(result.contains("color: red"));
        assert!(result.contains("Hello"));
    }

    #[test]
    fn test_apply_theme_full_document_external() {
        let theme = Theme {
            id: "test".to_string(),
            name: "Test".to_string(),
            css: ".content p { color: blue; }".to_string(),
        };
        let html = "<p>Hello</p>";
        let result = apply_theme_full_document(html, &theme, "My Title", false)
            .expect("apply full document");

        assert!(result.contains("<!DOCTYPE html>"));
        assert!(result.contains("<title>My Title</title>"));
        assert!(result.contains("<style>"));
        assert!(result.contains("color: blue"));
        assert!(result.contains("Hello"));
        assert!(result.contains("</html>"));
    }

    #[test]
    fn test_apply_theme_full_document_inline() {
        let theme = Theme {
            id: "test".to_string(),
            name: "Test".to_string(),
            css: ".content p { color: green; }".to_string(),
        };
        let html = "<p>World</p>";
        let result =
            apply_theme_full_document(html, &theme, "Inline Title", true).expect("apply inline");

        assert!(result.contains("<!DOCTYPE html>"));
        assert!(result.contains("<title>Inline Title</title>"));
        // Inline mode: no <style> in head, CSS inlined into elements
        assert!(result.contains("World"));
    }

    #[test]
    fn test_theme_registry_new_and_get() {
        let registry = ThemeRegistry::new().expect("create registry");
        // Should always have at least one theme
        let list = registry.list();
        assert!(!list.is_empty());
        // base_css should not be empty
        assert!(!registry.base_css().is_empty());
    }

    #[test]
    fn test_theme_registry_get_or_default_known() {
        let registry = ThemeRegistry::new().expect("create registry");
        // "elegant" or "minimal" should exist
        let theme = registry.get_or_default("minimal");
        assert!(theme.is_ok());
    }

    #[test]
    fn test_theme_registry_get_or_default_unknown_falls_back() {
        let registry = ThemeRegistry::new().expect("create registry");
        // Unknown theme should fall back to minimal or first available
        let theme = registry.get_or_default("nonexistent-theme-xyz");
        assert!(theme.is_ok());
    }

    #[test]
    fn test_theme_registry_get_missing_returns_none() {
        let registry = ThemeRegistry::new().expect("create registry");
        assert!(registry.get("nonexistent-theme-xyz").is_none());
    }

    #[test]
    fn test_resolve_theme_no_override() {
        use std::collections::HashMap;
        use std::path::PathBuf;
        use typub_core::{Content, ContentFormat, ContentMeta};

        let fallback_theme = Theme {
            id: "fallback".to_string(),
            name: "Fallback".to_string(),
            css: "body {}".to_string(),
        };

        let content = Content {
            path: PathBuf::from("/tmp/test-post"),
            meta: ContentMeta {
                title: "Test".to_string(),
                created: chrono::NaiveDate::from_ymd_opt(2026, 1, 1).expect("valid date"),
                updated: None,
                tags: vec![],
                categories: vec![],
                published: None,
                theme: None,
                internal_link_target: None,
                preamble: None,
                platforms: HashMap::new(),
            },
            content_file: PathBuf::from("/tmp/test-post/content.typ"),
            source_format: ContentFormat::Typst,
            slides_file: None,
            assets: vec![],
        };

        let global_config = typub_config::Config::default();
        let registry = ThemeRegistry::new().expect("create registry");

        // No overrides at any layer, should return fallback
        let result = resolve_theme(
            "wechat",
            &content,
            &global_config,
            None,
            &registry,
            &fallback_theme,
        );
        assert_eq!(result.id, "fallback");
    }

    #[test]
    fn test_resolve_theme_with_platform_override() {
        use std::collections::HashMap;
        use std::path::PathBuf;
        use typub_core::{Content, ContentFormat, ContentMeta, PostPlatformConfig};

        let fallback_theme = Theme {
            id: "fallback".to_string(),
            name: "Fallback".to_string(),
            css: "body {}".to_string(),
        };

        let mut platforms = HashMap::new();
        let mut extra = HashMap::new();
        // Use "minimal" since that's always present in the registry (layer 1)
        extra.insert(
            "theme".to_string(),
            toml::Value::String("minimal".to_string()),
        );
        platforms.insert(
            "wechat".to_string(),
            PostPlatformConfig {
                published: None,
                internal_link_target: None,
                extra,
            },
        );

        let content = Content {
            path: PathBuf::from("/tmp/test-post"),
            meta: ContentMeta {
                title: "Test".to_string(),
                created: chrono::NaiveDate::from_ymd_opt(2026, 1, 1).expect("valid date"),
                updated: None,
                tags: vec![],
                categories: vec![],
                published: None,
                theme: None,
                internal_link_target: None,
                preamble: None,
                platforms,
            },
            content_file: PathBuf::from("/tmp/test-post/content.typ"),
            source_format: ContentFormat::Typst,
            slides_file: None,
            assets: vec![],
        };

        let global_config = typub_config::Config::default();
        let registry = ThemeRegistry::new().expect("create registry");

        let result = resolve_theme(
            "wechat",
            &content,
            &global_config,
            None,
            &registry,
            &fallback_theme,
        );
        // Should resolve to "minimal" from layer 1
        assert_eq!(result.id, "minimal");
    }

    #[test]
    fn test_resolve_theme_layer_5_profile_default() {
        use std::collections::HashMap;
        use std::path::PathBuf;
        use typub_core::{Content, ContentFormat, ContentMeta};

        let fallback_theme = Theme {
            id: "fallback".to_string(),
            name: "Fallback".to_string(),
            css: "body {}".to_string(),
        };

        let content = Content {
            path: PathBuf::from("/tmp/test-post"),
            meta: ContentMeta {
                title: "Test".to_string(),
                created: chrono::NaiveDate::from_ymd_opt(2026, 1, 1).expect("valid date"),
                updated: None,
                tags: vec![],
                categories: vec![],
                published: None,
                theme: None,
                internal_link_target: None,
                preamble: None,
                platforms: HashMap::new(),
            },
            content_file: PathBuf::from("/tmp/test-post/content.typ"),
            source_format: ContentFormat::Typst,
            slides_file: None,
            assets: vec![],
        };

        let global_config = typub_config::Config::default();
        let registry = ThemeRegistry::new().expect("create registry");

        // Layer 5: profile default_theme = "elegant"
        let result = resolve_theme(
            "wechat",
            &content,
            &global_config,
            Some("elegant"),
            &registry,
            &fallback_theme,
        );
        // Should resolve to "elegant" from layer 5
        assert_eq!(result.id, "elegant");
    }
}
