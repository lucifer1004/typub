use crate::adapters::{ContentTransform, RenderConfig};
use crate::content::{Content, ContentFormat};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use typub_adapters_core::OutputFormat;
use typub_config::Config;

const MATH_TO_STRING_TYP: &str = include_str!("../typst-scripts/math-to-string.typ");
const CMARKER_CONFIG_TYP: &str = include_str!("../typst-scripts/cmarker-config.typ");

#[derive(Debug)]
pub struct RenderedOutput {
    pub format: OutputFormat,
    pub paths: Vec<PathBuf>,
    pub html: Option<String>,
}

impl RenderedOutput {
    pub fn html(&self) -> anyhow::Result<&str> {
        self.html
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("No HTML content available"))
    }
}

pub struct Renderer<'a> {
    config: &'a Config,
    project_root: PathBuf,
}

impl<'a> Renderer<'a> {
    pub fn new(config: &'a Config) -> Self {
        let project_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            config,
            project_root,
        }
    }

    pub fn new_with_root(config: &'a Config, project_root: PathBuf) -> Self {
        Self {
            config,
            project_root,
        }
    }

    fn project_root(&self) -> &std::path::Path {
        &self.project_root
    }

    pub async fn render_for_platform(
        &self,
        content: &Content,
        platform_id: &str,
        format: OutputFormat,
        config: &RenderConfig,
    ) -> Result<RenderedOutput> {
        let output_dir = self
            .config
            .output_dir
            .join(content.slug())
            .join(platform_id);
        std::fs::create_dir_all(&output_dir)?;

        let wrapper_file = self.generate_wrapper(content, &output_dir, format, config)?;

        match format {
            OutputFormat::Html | OutputFormat::HtmlFragment => {
                self.compile_html(&wrapper_file, &output_dir, format).await
            }
            OutputFormat::Png => self.compile_png(&wrapper_file, &output_dir).await,
            OutputFormat::Pdf => self.compile_pdf(&wrapper_file, &output_dir).await,
        }
    }

    fn generate_wrapper(
        &self,
        content: &Content,
        output_dir: &Path,
        format: OutputFormat,
        config: &RenderConfig,
    ) -> Result<PathBuf> {
        let wrapper_path = output_dir.join(".wrapper.typ");
        let content_path = self.get_relative_content_path(content)?;

        let imports = config.imports.join("\n");
        let content_include = self.build_content_include(content, &content_path, config);

        let html_math_rule: String = match format {
            OutputFormat::Html | OutputFormat::HtmlFragment => {
                use typub_core::MathRendering;
                match config.math_rendering {
                    MathRendering::Latex => {
                        format!(
                            r#"{math_to_string}

#show math.equation: it => {{
  let src = math-to-string(it.body)
  if it.block {{
    html.elem("div", attrs: (class: "typst-svg-block", "data-typst-src": src), html.frame(it))
  }} else {{
    html.elem("span", attrs: (class: "typst-svg-inline", "data-typst-src": src), html.frame(it))
  }}
}}"#,
                            math_to_string = MATH_TO_STRING_TYP,
                        )
                    }
                    MathRendering::Svg | MathRendering::Png => r#"#show math.equation: it => {
  if it.block {
    html.elem("div", attrs: (class: "typst-svg-block"), html.frame(it))
  } else {
    html.elem("span", attrs: (class: "typst-svg-inline"), html.frame(it))
  }
}"#
                    .to_string(),
                }
            }
            _ => String::new(),
        };

        // For deferred upload strategies, emit <img src="path"> markers instead of embedding images.
        // This allows the asset pipeline to handle them as LocalPath assets.
        let html_image_rule: String = match format {
            OutputFormat::Html | OutputFormat::HtmlFragment if config.image_as_marker => {
                r#"#show image: it => {
  let attrs = (:)
  if it.width != auto {
    let width-str = repr(it.width)
    attrs.insert("width", width-str.slice(0, width-str.position("%") + 1))
  }
  if it.height != auto {
    let height-str = repr(it.height)
    attrs.insert("height", height-str.slice(0, height-str.position("%") + 1))
  }
  if it.alt != none {
    attrs.insert("alt", it.alt)
  }
  attrs.insert("src", it.source)
  return html.elem("img", attrs: attrs)
}"#
                .to_string()
            }
            _ => String::new(),
        };

        let wrapper = format!(
            r#"
{imports}

{html_math_rule}

{html_image_rule}

{preamble}

{template_before}

{content_include}

{template_after}
"#,
            imports = imports,
            html_math_rule = html_math_rule,
            preamble = config.preamble,
            template_before = config.template_before,
            content_include = content_include,
            template_after = config.template_after,
        );

        std::fs::write(&wrapper_path, wrapper)?;
        Ok(wrapper_path)
    }

    fn build_content_include(
        &self,
        content: &Content,
        content_path: &str,
        config: &RenderConfig,
    ) -> String {
        use typub_core::MathRendering;

        let content_dir = content
            .path
            .strip_prefix(self.project_root())
            .unwrap_or(&content.path)
            .to_str()
            .unwrap_or("")
            .replace('\\', "/");

        let math_mode = match config.math_rendering {
            MathRendering::Latex => "latex",
            MathRendering::Svg | MathRendering::Png => "svg",
        };

        match &config.content_transform {
            ContentTransform::Default => match content.source_format {
                ContentFormat::Typst => {
                    format!(r#"#include "/{path}""#, path = content_path)
                }
                ContentFormat::Markdown => {
                    format!(
                        r##"{cmarker_config}

#render-md(
  "/{path}",
  content-dir: "{content_dir}",
  math-mode: "{math_mode}",
  image-as-marker: {image_as_marker},
)"##,
                        cmarker_config = CMARKER_CONFIG_TYP,
                        path = content_path,
                        content_dir = content_dir,
                        math_mode = math_mode,
                        image_as_marker = config.image_as_marker
                    )
                }
            },
            ContentTransform::ShowRules(rules) => match content.source_format {
                ContentFormat::Typst => {
                    format!(
                        r#"#{{
{rules}
  include "/{path}"
}}"#,
                        rules = rules,
                        path = content_path
                    )
                }
                ContentFormat::Markdown => {
                    format!(
                        r##"{cmarker_config}

{rules}

#render-md(
  "/{path}",
  content-dir: "{content_dir}",
  math-mode: "{math_mode}",
  image-as-marker: {image_as_marker},
)"##,
                        cmarker_config = CMARKER_CONFIG_TYP,
                        path = content_path,
                        content_dir = content_dir,
                        math_mode = math_mode,
                        image_as_marker = config.image_as_marker,
                        rules = rules
                    )
                }
            },
            ContentTransform::Custom(template) => template.replace("{path}", content_path),
        }
    }

    fn get_relative_content_path(&self, content: &Content) -> Result<String> {
        let path = content
            .content_file
            .strip_prefix(self.project_root())
            .unwrap_or(&content.content_file);
        Ok(path.display().to_string().replace('\\', "/"))
    }

    async fn compile_html(
        &self,
        wrapper_file: &Path,
        output_dir: &Path,
        format: OutputFormat,
    ) -> Result<RenderedOutput> {
        let output_path = output_dir.join("content.html");
        let root = self.project_root();

        let root_str = root
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("project root path is not valid UTF-8"))?;
        let wrapper_str = wrapper_file
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("wrapper file path is not valid UTF-8"))?;
        let output_str = output_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("output path is not valid UTF-8"))?;

        let output = Command::new("typst")
            .args([
                "compile",
                "--root",
                root_str,
                "--format",
                "html",
                "--features",
                "html",
                wrapper_str,
                output_str,
            ])
            .output()
            .context("Failed to execute typst")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("typst compile failed: {}", stderr);
        }

        let html = std::fs::read_to_string(&output_path)?;
        let html = if format == OutputFormat::HtmlFragment {
            extract_body_html(&html)
        } else {
            html
        };

        Ok(RenderedOutput {
            format,
            paths: vec![output_path],
            html: Some(html),
        })
    }

    async fn compile_png(&self, wrapper_file: &Path, output_dir: &Path) -> Result<RenderedOutput> {
        // Clear old slide images to handle cases where slide count decreases
        for entry in std::fs::read_dir(output_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "png") {
                std::fs::remove_file(&path)?;
            }
        }

        let output_pattern = output_dir.join("slide-{n}.png");
        let root = self.project_root();

        let root_str = root
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("project root path is not valid UTF-8"))?;
        let wrapper_str = wrapper_file
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("wrapper file path is not valid UTF-8"))?;
        let pattern_str = output_pattern
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("output pattern path is not valid UTF-8"))?;

        let output = Command::new("typst")
            .args(["compile", "--root", root_str, wrapper_str, pattern_str])
            .output()
            .context("Failed to execute typst")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("typst compile failed: {}", stderr);
        }

        let mut paths = Vec::new();
        for entry in std::fs::read_dir(output_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "png") {
                paths.push(path);
            }
        }
        paths.sort();

        Ok(RenderedOutput {
            format: OutputFormat::Png,
            paths,
            html: None,
        })
    }

    async fn compile_pdf(&self, wrapper_file: &Path, output_dir: &Path) -> Result<RenderedOutput> {
        let output_path = output_dir.join("content.pdf");
        let root = self.project_root();

        let root_str = root
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("project root path is not valid UTF-8"))?;
        let wrapper_str = wrapper_file
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("wrapper file path is not valid UTF-8"))?;
        let output_str = output_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("output path is not valid UTF-8"))?;

        let output = Command::new("typst")
            .args([
                "compile",
                "--root",
                root_str,
                "--format",
                "pdf",
                wrapper_str,
                output_str,
            ])
            .output()
            .context("Failed to execute typst")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("typst compile failed: {}", stderr);
        }

        Ok(RenderedOutput {
            format: OutputFormat::Pdf,
            paths: vec![output_path],
            html: None,
        })
    }
}

pub(crate) fn extract_body_html(html: &str) -> String {
    if let Some(start) = html.find("<body>")
        && let Some(end) = html.find("</body>")
    {
        return html[start + 6..end].trim().to_string();
    }
    html.to_string()
}

#[cfg(test)]
mod tests {
    use super::{Renderer, extract_body_html};
    use crate::content::{Content, ContentFormat, ContentMeta};
    use chrono::NaiveDate;
    use std::collections::HashMap;
    use std::fs;
    use typub_adapters_core::{OutputFormat, RenderConfig};
    use typub_config::Config;

    async fn render_markdown(source: &str) -> anyhow::Result<String> {
        let temp = tempfile::tempdir()?;
        let post_dir = temp.path().join("posts/note");
        fs::create_dir_all(&post_dir)?;
        let content_file = post_dir.join("content.md");
        fs::write(&content_file, source)?;

        let config = Config {
            content_dir: temp.path().join("posts"),
            output_dir: temp.path().join("output"),
            ..Config::default()
        };
        let content = Content {
            path: post_dir,
            meta: ContentMeta {
                title: "Visible Title".to_string(),
                created: NaiveDate::from_ymd_opt(2026, 7, 10)
                    .ok_or_else(|| anyhow::anyhow!("invalid test date"))?,
                updated: None,
                tags: Vec::new(),
                categories: Vec::new(),
                published: None,
                theme: None,
                internal_link_target: None,
                preamble: None,
                platforms: HashMap::new(),
            },
            content_file: content_file.clone(),
            source_format: ContentFormat::Markdown,
            slides_file: None,
            assets: Vec::new(),
        };
        let renderer = Renderer::new_with_root(&config, temp.path().to_path_buf());
        let rendered = renderer
            .render_for_platform(
                &content,
                "test",
                OutputFormat::HtmlFragment,
                &RenderConfig::default(),
            )
            .await?;
        let html = rendered.html()?.to_string();

        assert_eq!(fs::read_to_string(content_file)?, source);
        Ok(html)
    }

    #[test]
    fn test_extract_body() {
        let html = r#"<!DOCTYPE html>
<html>
<head><title>Test</title></head>
<body>
<h1>Hello</h1>
<p>World</p>
</body>
</html>"#;
        let body = extract_body_html(html);
        assert!(body.contains("<h1>Hello</h1>"));
        assert!(!body.contains("DOCTYPE"));
    }

    #[tokio::test]
    async fn markdown_frontmatter_is_not_rendered() -> anyhow::Result<()> {
        let html = render_markdown(
            "---\ntags: [frontmatter-only-marker]\n---\n# Visible Title\n\nVisible body.\n",
        )
        .await?;

        assert!(html.contains("Visible Title"));
        assert!(html.contains("Visible body."));
        assert!(!html.contains("frontmatter-only-marker"));
        Ok(())
    }

    #[tokio::test]
    async fn markdown_dotted_frontmatter_is_not_rendered() -> anyhow::Result<()> {
        let html = render_markdown(
            "---\ntags: [dotted-frontmatter-marker]\n...\n# Dotted Title\n\nVisible body.\n",
        )
        .await?;

        assert!(html.contains("Dotted Title"));
        assert!(html.contains("Visible body."));
        assert!(!html.contains("dotted-frontmatter-marker"));
        Ok(())
    }

    #[tokio::test]
    async fn markdown_thematic_break_is_not_treated_as_frontmatter() -> anyhow::Result<()> {
        let html = render_markdown("---\n\nVisible body.\n\n---\n\nAfter break.\n").await?;

        assert!(html.contains("Visible body."));
        assert!(html.contains("After break."));
        Ok(())
    }
}
