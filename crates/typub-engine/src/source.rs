use crate::content::ContentFormat;
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const INSPECTION_LABEL: &str = "<typub-inspection>";

const INSPECTION_HELPERS: &str = r#"#let plain-text(content) = {
  let fields = content.fields()
  if "text" in fields {
    fields.text
  } else if "children" in fields {
    fields.children.map(c => {
      if type(c) == str {
        c
      } else if c.func() == [ ].func() {
        " "
      } else {
        plain-text(c)
      }
    }).join()
  } else if "body" in fields {
    plain-text(fields.body)
  } else if "child" in fields {
    plain-text(fields.child)
  } else {
    ""
  }
}

#let emit-inspection(source-meta) = context {
  let headings = query(heading)
  let title = if headings.len() > 0 {
    plain-text(headings.first().body)
  } else {
    none
  }
  assert(
    source-meta == none or type(source-meta) == dictionary,
    message: "source metadata must be a dictionary",
  )
  let tags-present = source-meta != none and "tags" in source-meta
  let tags = if tags-present { source-meta.at("tags") } else { none }
  [#metadata((
    title: title,
    tags-present: tags-present,
    tags: tags,
  )) <typub-inspection>]
}
"#;

/// Metadata declared in the source document.
///
/// `None` means the field was not declared. `Some(Vec::new())` means the
/// source explicitly declared an empty tag set. The engine reports metadata
/// without merging it into [`typub_core::ContentMeta`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceMetadata {
    /// Tags declared by the source, preserving absent versus explicitly empty.
    pub tags: Option<Vec<String>>,
}

/// Information obtained by evaluating a source document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceInspection {
    /// Plain text of the first rendered heading, when one exists.
    pub title: Option<String>,
    /// Metadata reported by the source without pipeline-level application.
    pub metadata: SourceMetadata,
}

#[derive(Debug, Deserialize)]
struct RawInspection {
    title: Option<String>,
    #[serde(rename = "tags-present")]
    tags_present: bool,
    tags: Value,
}

/// Inspect the first heading and source metadata without applying either one.
///
/// Markdown metadata is read from a leading YAML front matter block. Typst
/// metadata is read from at most one `metadata` element labelled
/// `<typub-meta>`. This function does not mutate the source file.
pub fn inspect_source(
    root: &Path,
    source: &Path,
    format: ContentFormat,
) -> Result<SourceInspection> {
    let root = canonicalize(root, "source root")?;
    let source = canonicalize(source, "source file")?;
    let input = typst_root_path(&root, &source)?;
    let wrapper = match format {
        ContentFormat::Markdown => markdown_wrapper(&input),
        ContentFormat::Typst => typst_wrapper(&input),
    }?;

    let root_str = root
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("source root is not valid UTF-8: {}", root.display()))?;
    let mut child = Command::new("typst")
        .args([
            "query",
            "--root",
            root_str,
            "--field",
            "value",
            "--one",
            "-",
            INSPECTION_LABEL,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| {
            format!(
                "failed to execute typst while inspecting {}",
                source.display()
            )
        })?;

    child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("failed to open typst stdin"))?
        .write_all(wrapper.as_bytes())
        .with_context(|| format!("failed to send {} to typst", source.display()))?;

    let output = child
        .wait_with_output()
        .with_context(|| format!("failed to inspect {} with typst", source.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        anyhow::bail!(
            "typst source inspection failed for {}: {stderr}",
            source.display()
        );
    }

    parse_inspection(&output.stdout)
        .with_context(|| format!("invalid source metadata in {}", source.display()))
}

fn canonicalize(path: &Path, kind: &str) -> Result<PathBuf> {
    path.canonicalize()
        .with_context(|| format!("failed to resolve {kind}: {}", path.display()))
}

fn typst_root_path(root: &Path, source: &Path) -> Result<String> {
    let relative = source.strip_prefix(root).with_context(|| {
        format!(
            "source file {} is outside source root {}",
            source.display(),
            root.display()
        )
    })?;
    let relative = relative
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("source path is not valid UTF-8: {}", source.display()))?
        .replace('\\', "/");
    Ok(format!("/{relative}"))
}

fn markdown_wrapper(input: &str) -> Result<String> {
    let input = typst_string(input)?;
    Ok(format!(
        r#"#import "@preview/cmarker:0.1.8"
{INSPECTION_HELPERS}
#let (source-meta, body) = cmarker.render-with-metadata(
  read({input}),
  scope: (image: (source, alt: none, format: auto) => [],),
  math: (it, block: false) => [],
  metadata-block: "frontmatter-yaml",
)
#body
#emit-inspection(source-meta)
"#
    ))
}

fn typst_wrapper(input: &str) -> Result<String> {
    let input = typst_string(input)?;
    Ok(format!(
        r#"{INSPECTION_HELPERS}
#include {input}
#context {{
  let source-meta = query(<typub-meta>)
  assert(
    source-meta.len() <= 1,
    message: "expected at most one <typub-meta> metadata element",
  )
  emit-inspection(if source-meta.len() == 1 {{
    source-meta.first().value
  }} else {{
    none
  }})
}}
"#
    ))
}

fn typst_string(value: &str) -> Result<String> {
    serde_json::to_string(value).context("failed to encode source path for typst")
}

fn parse_inspection(bytes: &[u8]) -> Result<SourceInspection> {
    let raw: RawInspection =
        serde_json::from_slice(bytes).context("failed to decode typst output")?;
    let title = raw
        .title
        .map(|title| title.trim().to_string())
        .filter(|title| !title.is_empty());
    let tags = if raw.tags_present {
        let values = raw
            .tags
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("metadata field `tags` must be an array of strings"))?;
        Some(
            values
                .iter()
                .map(|value| {
                    value.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                        anyhow::anyhow!("metadata field `tags` must be an array of strings")
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        )
    } else {
        None
    };

    Ok(SourceInspection {
        title,
        metadata: SourceMetadata { tags },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parse_inspection_preserves_absent_and_empty_tags() -> Result<()> {
        let absent = parse_inspection(br#"{"title":"Title","tags-present":false,"tags":null}"#)?;
        assert_eq!(absent.metadata.tags, None);

        let empty = parse_inspection(br#"{"title":"Title","tags-present":true,"tags":[]}"#)?;
        assert_eq!(empty.metadata.tags, Some(Vec::new()));
        Ok(())
    }

    #[test]
    fn parse_inspection_rejects_non_string_tags() -> Result<()> {
        let Err(error) =
            parse_inspection(br#"{"title":"Title","tags-present":true,"tags":["valid",1]}"#)
        else {
            anyhow::bail!("non-string tag unexpectedly parsed successfully");
        };
        assert!(error.to_string().contains("array of strings"));
        Ok(())
    }

    #[test]
    fn wrappers_use_native_metadata_protocols() -> Result<()> {
        let markdown = markdown_wrapper("/notes.md")?;
        assert!(markdown.contains("render-with-metadata"));
        assert!(markdown.contains("frontmatter-yaml"));

        let typst = typst_wrapper("/notes.typ")?;
        assert!(typst.contains("query(<typub-meta>)"));
        assert!(typst.contains("expected at most one <typub-meta>"));
        Ok(())
    }

    #[test]
    fn inspect_markdown_and_typst_sources() -> Result<()> {
        if Command::new("typst").arg("--version").output().is_err() {
            return Ok(());
        }

        let temp = tempfile::tempdir()?;
        let markdown = temp.path().join("note.md");
        fs::write(
            &markdown,
            "---\ntags: [inferlab, platform]\n---\n# Markdown Title\n",
        )?;
        let markdown = inspect_source(temp.path(), &markdown, ContentFormat::Markdown)?;
        assert_eq!(markdown.title.as_deref(), Some("Markdown Title"));
        assert_eq!(
            markdown.metadata.tags,
            Some(vec!["inferlab".to_string(), "platform".to_string()])
        );

        let typst = temp.path().join("note.typ");
        fs::write(
            &typst,
            "#metadata((tags: (\"inferlab\", \"typst\"))) <typub-meta>\n= Typst Title\n",
        )?;
        let typst = inspect_source(temp.path(), &typst, ContentFormat::Typst)?;
        assert_eq!(typst.title.as_deref(), Some("Typst Title"));
        assert_eq!(
            typst.metadata.tags,
            Some(vec!["inferlab".to_string(), "typst".to_string()])
        );
        Ok(())
    }
}
