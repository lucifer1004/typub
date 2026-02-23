use std::path::Path;
use typub_ui as ui;

pub use typub_core::{
    Content, ContentFormat, ContentMeta, DiscoverResult, PostInfo, PostPlatformConfig,
};

pub fn discover_all_with_logging(content_dir: &Path) -> anyhow::Result<Vec<Content>> {
    let result = Content::discover_all(content_dir)?;

    for (path, err) in &result.errors {
        ui::debug(&format!("Skipping {}: {}", path.display(), err));
    }

    Ok(result.contents)
}
