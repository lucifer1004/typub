use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

pub const CONFIG_FILE_NAME: &str = "typub.toml";

pub fn find_project_root(start: &Path) -> Result<PathBuf> {
    let start = if start.is_absolute() {
        start.to_path_buf()
    } else {
        std::env::current_dir()?.join(start)
    };

    let mut current = start.as_path();
    loop {
        let config_path = current.join(CONFIG_FILE_NAME);
        if config_path.exists() {
            return Ok(current.to_path_buf());
        }

        match current.parent() {
            Some(parent) => current = parent,
            None => bail!(
                "Could not find {} in any parent directory of {}",
                CONFIG_FILE_NAME,
                start.display()
            ),
        }
    }
}

pub fn normalize_to_relative(path: &Path, project_root: &Path) -> Result<String> {
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_root.join(path)
    };

    let abs_path = abs_path.canonicalize().unwrap_or(abs_path);
    let project_root = project_root.canonicalize().with_context(|| {
        format!(
            "Failed to canonicalize project root: {}",
            project_root.display()
        )
    })?;

    let rel_path = abs_path
        .strip_prefix(&project_root)
        .with_context(|| {
            format!(
                "Asset outside project root: {}\nAssets must be within the project directory.\nConsider moving the asset into the project or creating a symlink.",
                abs_path.display()
            )
        })?;

    let normalized = path_to_forward_slash(rel_path);

    if normalized.contains("..") {
        bail!(
            "Invalid path contains '..' after normalization: {}",
            normalized
        );
    }

    Ok(normalized)
}

pub fn resolve_from_relative(rel_path: &str, project_root: &Path) -> Result<PathBuf> {
    let native_path = forward_slash_to_path(rel_path);
    let abs_path = project_root.join(&native_path);

    validate_within_project(&abs_path, project_root)?;

    Ok(abs_path)
}

pub fn validate_within_project(path: &Path, project_root: &Path) -> Result<()> {
    let abs_path = if path.is_absolute() {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    } else {
        let joined = project_root.join(path);
        joined.canonicalize().unwrap_or(joined)
    };

    let project_root = project_root.canonicalize().with_context(|| {
        format!(
            "Failed to canonicalize project root: {}",
            project_root.display()
        )
    })?;

    if !abs_path.starts_with(&project_root) {
        bail!(
            "Asset outside project root: {}\nAssets must be within the project directory.\nConsider moving the asset into the project or creating a symlink.",
            abs_path.display()
        );
    }

    Ok(())
}

fn path_to_forward_slash(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/")
}

fn forward_slash_to_path(s: &str) -> PathBuf {
    PathBuf::from(s.replace('/', std::path::MAIN_SEPARATOR_STR))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use anyhow::Result;
    use tempfile::TempDir;

    fn setup_project() -> Result<(TempDir, PathBuf)> {
        let dir = TempDir::new()?;
        let project_root = dir.path().to_path_buf();
        std::fs::write(project_root.join(CONFIG_FILE_NAME), "")?;
        Ok((dir, project_root))
    }

    #[test]
    fn test_find_project_root() -> Result<()> {
        let (_dir, project_root) = setup_project()?;

        let nested = project_root.join("posts/my-post");
        std::fs::create_dir_all(&nested)?;

        let found = find_project_root(&nested)?;
        assert_eq!(found.canonicalize()?, project_root.canonicalize()?);
        Ok(())
    }
}
