use std::path::{Path, PathBuf};

use anyhow::Result;

use typub_config::PlatformConfig;
use typub_core::{AssetStrategy, MathDelimiters, MathRendering};
use typub_ir::Document;
use typub_passes::{
    PassCtx, RasterizeSvgToDataUriPass, RasterizeSvgToLocalAssetPass,
    SIDECAR_GENERATED_RENDER_ASSETS, run_passes,
};
use typub_storage::{
    DeferredAssets, PendingAssetList, build_resolved_url_map, materialize_external_assets,
    materialize_external_assets_with_status,
};

use crate::capability::AdapterCapability;
use crate::context::AdapterContext;
use crate::payload::AdapterPayload;
use crate::registrar::{AdapterFactory, AdapterRegistrar};
use crate::types::RenderConfig;

/// Resolve asset strategy from platform configuration.
pub fn resolve_asset_strategy_from_config(
    platform_config: Option<&PlatformConfig>,
    capability: &AdapterCapability,
) -> Result<AssetStrategy> {
    if let Some(cfg) = platform_config
        && !cfg.enabled
    {
        return Ok(capability.default_asset_strategy());
    }

    let configured = platform_config.and_then(|c| c.asset_strategy.as_deref());

    let strategy = match configured {
        Some(raw) => AssetStrategy::parse(raw).ok_or_else(|| {
            anyhow::anyhow!("Invalid asset strategy '{}' for {}", raw, capability.id)
        })?,
        None => return Ok(capability.default_asset_strategy()),
    };

    if !capability.supported_asset_strategies().contains(&strategy) {
        anyhow::bail!(
            "{} does not support asset strategy '{:?}'. Supported: {:?}",
            capability.id,
            strategy,
            capability.supported_asset_strategies()
        );
    }

    Ok(strategy)
}

/// Resolve asset strategy with policy validation.
///
/// This is the centralized helper for resolving asset strategy with support for
/// custom policy validation. Used by both typub-engine and adapter crates.
/// Per [[ADR-0005]].
pub fn resolve_asset_strategy_with_policy(
    platform_id: &str,
    platform_config: Option<&PlatformConfig>,
    default: AssetStrategy,
    supported: &[AssetStrategy],
) -> Result<AssetStrategy> {
    let configured = platform_config.and_then(|c| c.asset_strategy.as_deref());

    let strategy = match configured {
        Some(raw) => AssetStrategy::parse(raw).ok_or_else(|| {
            anyhow::anyhow!("Invalid asset strategy '{}' for {}", raw, platform_id)
        })?,
        None => return Ok(default),
    };

    if !supported.contains(&strategy) {
        anyhow::bail!(
            "asset_strategy='{}' is not supported for '{}'. Supported strategies: {}.",
            strategy_name(strategy),
            platform_id,
            supported_names(supported)
        );
    }

    Ok(strategy)
}

fn strategy_name(strategy: AssetStrategy) -> &'static str {
    match strategy {
        AssetStrategy::Copy => "copy",
        AssetStrategy::Embed => "embed",
        AssetStrategy::Upload => "upload",
        AssetStrategy::External => "external",
    }
}

fn supported_names(supported: &[AssetStrategy]) -> String {
    supported
        .iter()
        .map(|s| strategy_name(*s))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Resolve math rendering strategy from platform configuration.
/// Per [[WI-2026-02-17-002]].
pub fn resolve_math_rendering_from_config(
    platform_config: Option<&PlatformConfig>,
    capability: &AdapterCapability,
) -> Result<MathRendering> {
    if let Some(cfg) = platform_config
        && !cfg.enabled
    {
        return Ok(capability.default_math_rendering());
    }

    let configured = platform_config.and_then(|c| c.math_rendering.as_deref());

    let rendering = match configured {
        Some(raw) => parse_math_rendering(raw).ok_or_else(|| {
            anyhow::anyhow!(
                "Invalid math_rendering '{}' for {}. Expected: svg, latex, or png",
                raw,
                capability.id
            )
        })?,
        None => return Ok(capability.default_math_rendering()),
    };

    if !capability.supports_math_rendering(rendering) {
        anyhow::bail!(
            "{} does not support math_rendering '{:?}'. Supported: {:?}",
            capability.id,
            rendering,
            capability.supported_math_renderings()
        );
    }

    Ok(rendering)
}

/// Resolve math delimiter syntax from platform configuration.
/// Per [[WI-2026-02-17-002]].
pub fn resolve_math_delimiters_from_config(
    platform_config: Option<&PlatformConfig>,
    capability: &AdapterCapability,
) -> Result<MathDelimiters> {
    if let Some(cfg) = platform_config
        && !cfg.enabled
    {
        return Ok(capability.default_math_delimiter());
    }

    let configured = platform_config.and_then(|c| c.math_delimiters.as_deref());

    let delimiters = match configured {
        Some(raw) => parse_math_delimiters(raw).ok_or_else(|| {
            anyhow::anyhow!(
                "Invalid math_delimiters '{}' for {}. Expected: dollar or brackets",
                raw,
                capability.id
            )
        })?,
        None => return Ok(capability.default_math_delimiter()),
    };

    if !capability.supports_math_delimiter(delimiters) {
        anyhow::bail!(
            "{} does not support math_delimiters '{:?}'. Supported: {:?}",
            capability.id,
            delimiters,
            capability.supported_math_delimiters()
        );
    }

    Ok(delimiters)
}

/// Parse math rendering strategy from string.
fn parse_math_rendering(s: &str) -> Option<MathRendering> {
    match s.to_lowercase().as_str() {
        "svg" => Some(MathRendering::Svg),
        "latex" => Some(MathRendering::Latex),
        "png" => Some(MathRendering::Png),
        _ => None,
    }
}

/// Parse math delimiter syntax from string.
fn parse_math_delimiters(s: &str) -> Option<MathDelimiters> {
    match s.to_lowercase().as_str() {
        "dollar" | "$" => Some(MathDelimiters::Dollar),
        "brackets" | "[]" => Some(MathDelimiters::Brackets),
        _ => None,
    }
}

/// Build default RenderConfig from asset strategy and capability.
pub fn default_render_config_for(
    strategy: AssetStrategy,
    capability: &AdapterCapability,
) -> RenderConfig {
    RenderConfig {
        image_as_marker: strategy.requires_deferred_upload(),
        math_rendering: capability.default_math_rendering(),
        ..RenderConfig::default()
    }
}

/// Build RenderConfig for PNG math support.
/// Returns SVG for rendering (PNG conversion happens later in specialize_payload).
/// Per [[WI-2026-02-17-002]].
pub fn render_config_for_png_math(
    strategy: AssetStrategy,
    math_rendering: MathRendering,
) -> RenderConfig {
    let math_rendering = match math_rendering {
        MathRendering::Png => MathRendering::Svg, // Render as SVG, convert later
        other => other,
    };
    RenderConfig {
        image_as_marker: strategy.requires_deferred_upload(),
        math_rendering,
        ..RenderConfig::default()
    }
}

/// Convert SVG payloads to local PNG assets if math_rendering is Png.
/// Uses content's assets folder for generated PNG files.
/// Returns the converted document and a list of generated file paths.
/// Per [[WI-2026-02-17-002]].
pub fn convert_svg_to_png_markers_if_configured(
    document: &Document,
    math_rendering: MathRendering,
    content_path: &Path,
    slug: &str,
) -> Result<(Document, Vec<PathBuf>)> {
    if math_rendering != MathRendering::Png {
        return Ok((document.clone(), Vec::new()));
    }

    let mut converted = document.clone();
    let mut ctx = PassCtx::default();
    let mut pass = RasterizeSvgToLocalAssetPass::new(content_path.to_path_buf(), slug)
        .with_assets_subdir("assets");
    run_passes(&mut converted, &mut ctx, &mut [&mut pass])?;

    let generated = ctx
        .sidecar
        .get(SIDECAR_GENERATED_RENDER_ASSETS)
        .and_then(serde_json::Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| {
                    entry
                        .get("file_path")
                        .and_then(serde_json::Value::as_str)
                        .map(PathBuf::from)
                })
                .collect()
        })
        .unwrap_or_default();

    Ok((converted, generated))
}

/// Convert SVG payloads to PNG data-uri assets if math_rendering is Png.
/// For use with Embed asset strategy.
/// Per [[WI-2026-02-17-002]].
pub fn convert_svg_to_png_inline_if_configured(
    document: &Document,
    math_rendering: MathRendering,
) -> Result<Document> {
    if math_rendering != MathRendering::Png {
        return Ok(document.clone());
    }

    let mut converted = document.clone();
    let mut ctx = PassCtx::default();
    let mut pass = RasterizeSvgToDataUriPass::new();
    run_passes(&mut converted, &mut ctx, &mut [&mut pass])?;
    Ok(converted)
}

/// Convert PNG math based on asset strategy (unified helper).
///
/// This helper centralizes the strategy-based PNG math conversion:
/// - For deferred strategies (Upload/External), converts SVG to local PNG assets
/// - For immediate strategies (Copy/Embed), converts SVG to data-uri PNG assets
///
/// Returns the converted document and any generated file paths (for deferred upload).
/// Generated PNG files are placed in content's assets folder.
/// Per [[WI-2026-02-17-002]].
pub fn convert_png_math_for_strategy(
    document: Document,
    asset_strategy: AssetStrategy,
    math_rendering: MathRendering,
    content_path: &Path,
    slug: &str,
) -> Result<(Document, Vec<PathBuf>)> {
    if math_rendering != MathRendering::Png {
        return Ok((document, Vec::new()));
    }

    if asset_strategy.requires_deferred_upload() {
        convert_svg_to_png_markers_if_configured(&document, math_rendering, content_path, slug)
    } else {
        let converted = convert_svg_to_png_inline_if_configured(&document, math_rendering)?;
        Ok((converted, Vec::new()))
    }
}

/// Register an adapter factory and capability with the registrar.
pub fn register_adapter(
    registrar: &mut AdapterRegistrar,
    capability: &AdapterCapability,
    factory: AdapterFactory,
) -> Result<()> {
    registrar.register_factory(capability.id, factory)?;
    registrar.register_capability(capability.id, *capability)?;
    Ok(())
}

// ============================================================================
// Asset Strategy Helpers
// ============================================================================

/// Create DeferredAssets based on strategy (for specialize_payload).
///
/// This helper centralizes the logic for creating DeferredAssets:
/// - For deferred strategies (Upload/External), builds pending asset list from document assets
/// - For immediate strategies (Copy/Embed), returns empty DeferredAssets
///
/// Per [[RFC-0004:C-PIPELINE-INTEGRATION]].
pub fn prepare_deferred_assets(
    strategy: AssetStrategy,
    document: &Document,
    content_path: &Path,
) -> DeferredAssets {
    let pending = if strategy.requires_deferred_upload() {
        crate::build_pending_asset_list_from_document(document, content_path)
    } else {
        PendingAssetList::new()
    };
    DeferredAssets::new(pending, strategy)
}

/// Materialize external assets and resolve URLs in-place (for materialize_payload).
///
/// This helper handles:
/// 1. Checking if materialization is needed
/// 2. Uploading assets to external storage (S3) for External strategy
/// 3. Resolving URLs in the semantic document assets
///
/// Per [[RFC-0004:C-PIPELINE-INTEGRATION]].
pub async fn materialize_and_resolve_urls(
    payload: &mut AdapterPayload,
    ctx: &dyn AdapterContext,
) -> Result<()> {
    if !payload.assets.needs_materialize() {
        return Ok(());
    }

    if payload.assets.strategy == AssetStrategy::External {
        let storage_config = ctx.storage_config().ok_or_else(|| {
            anyhow::anyhow!(
                "External asset strategy requires [storage] configuration. See RFC-0004."
            )
        })?;

        // Use cached version if status tracker is available
        if let Some(tracker) = ctx.status_tracker() {
            materialize_external_assets_with_status(&mut payload.assets, storage_config, tracker)
                .await?;
        } else {
            materialize_external_assets(&mut payload.assets, storage_config).await?;
        }
    }

    if !payload.assets.resolved.is_empty() {
        let url_map = build_resolved_url_map(&payload.assets, &payload.content_info.path);
        crate::resolve_asset_urls(&mut payload.document, &url_map);
    }

    Ok(())
}

// ============================================================================
// Mock Materialization for Dry-Run Mode
// ============================================================================

/// Mock asset materialization for dry-run mode.
///
/// This generates mock URLs without file I/O, useful for testing the pipeline.
/// Returns AssetAnalysis for UI logging.
///
/// Mock URL format: https://mock.typub.dev/{hash}/{filename}
///
/// This function:
/// 1. Analyzes assets (hash + cache check) using typub_storage
/// 2. Generates generic mock URLs
/// 3. Resolves URLs in `Document.assets`
pub fn mock_materialize_and_resolve_urls(
    payload: &mut AdapterPayload,
    ctx: &dyn AdapterContext,
) -> Result<typub_storage::AssetAnalysis> {
    use typub_storage::analyze_assets;

    if !payload.assets.needs_materialize() {
        return Ok(typub_storage::AssetAnalysis::empty());
    }

    let storage_config = ctx.storage_config();
    let tracker = ctx.status_tracker();
    let storage_id = storage_config
        .map(|c| c.config_id())
        .unwrap_or_else(|| "default".to_string());

    // 1. Analyze assets (compute hashes, check cache)
    let analysis = analyze_assets(&payload.assets, &storage_id, tracker)?;

    // 2. Generate generic mock URLs
    // Format: https://mock.typub.dev/{hash}/{filename}
    for info in &analysis.assets {
        let filename = info
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "asset".to_string());

        let mock_url = format!(
            "https://mock.typub.dev/{}/{}",
            &info.content_hash[..8],
            filename
        );
        payload.assets.resolved.insert(info.index, mock_url);
    }

    // 3. Resolve URLs in elements
    let url_map = build_resolved_url_map(&payload.assets, &payload.content_info.path);
    crate::resolve_asset_urls(&mut payload.document, &url_map);

    // Log summary to UI using table format
    if analysis.total_count > 0 {
        typub_ui::log_asset_analysis(
            "[DRY RUN] Asset Analysis",
            analysis.total_count,
            analysis.new_count,
            analysis.new_size_bytes,
            analysis.cached_count,
            analysis.cached_size_bytes,
        );
    }

    Ok(analysis)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::collections::HashMap;
    use typub_config::Config;
    use typub_core::{
        CapabilityGapBehavior, CapabilitySupport, DraftSupport, MathDelimiters, MathRendering,
        NodePolicyAction,
    };
    use typub_ir::{DocMeta, Document};

    use crate::{NodePolicy, TaxonomyCapability};

    const TEST_CAP: AdapterCapability = AdapterCapability {
        id: "test",
        name: "Test",
        short_code: "ts",
        local_output: false,
        requires_config: true,
        taxonomy: TaxonomyCapability::new(
            CapabilitySupport::Supported,
            CapabilitySupport::Supported,
            CapabilitySupport::Unsupported(CapabilityGapBehavior::HardError),
            DraftSupport::None,
        ),
        asset_strategies: &[AssetStrategy::Upload, AssetStrategy::Embed],
        math_renderings: &[MathRendering::Svg, MathRendering::Png],
        math_delimiters: &[MathDelimiters::Dollar, MathDelimiters::Brackets],
        code_highlight: true,
        notes: "",
        node_policy: NodePolicy {
            raw: NodePolicyAction::Pass,
            unknown: NodePolicyAction::Pass,
        },
    };

    fn make_config() -> PlatformConfig {
        PlatformConfig {
            enabled: true,
            asset_strategy: None,
            published: None,
            theme: None,
            internal_link_target: None,
            math_rendering: None,
            math_delimiters: None,
            extra: HashMap::new(),
        }
    }

    fn dummy_factory(_: &Config) -> anyhow::Result<Box<dyn crate::adapter::PlatformAdapter>> {
        anyhow::bail!("not implemented")
    }

    #[test]
    fn test_resolve_asset_strategy_default() {
        let result = resolve_asset_strategy_from_config(None, &TEST_CAP).expect("resolve");
        assert_eq!(result, AssetStrategy::Upload);
    }

    #[test]
    fn test_resolve_asset_strategy_valid() {
        let mut cfg = make_config();
        cfg.asset_strategy = Some("embed".into());
        let result = resolve_asset_strategy_from_config(Some(&cfg), &TEST_CAP).expect("resolve");
        assert_eq!(result, AssetStrategy::Embed);
    }

    #[test]
    fn test_resolve_asset_strategy_invalid_string() {
        let mut cfg = make_config();
        cfg.asset_strategy = Some("invalid".into());
        let err = resolve_asset_strategy_from_config(Some(&cfg), &TEST_CAP).expect_err("invalid");
        assert!(err.to_string().contains("Invalid asset strategy"));
    }

    #[test]
    fn test_resolve_asset_strategy_disabled_platform() {
        let mut cfg = make_config();
        cfg.enabled = false;
        cfg.asset_strategy = Some("embed".into());
        let result = resolve_asset_strategy_from_config(Some(&cfg), &TEST_CAP).expect("resolve");
        assert_eq!(result, AssetStrategy::Upload);
    }

    #[test]
    fn test_resolve_asset_strategy_unsupported() {
        let mut cfg = make_config();
        cfg.asset_strategy = Some("copy".into());
        let err =
            resolve_asset_strategy_from_config(Some(&cfg), &TEST_CAP).expect_err("unsupported");
        assert!(err.to_string().contains("does not support"));
    }

    #[test]
    fn test_resolve_math_rendering_default() {
        let result = resolve_math_rendering_from_config(None, &TEST_CAP).expect("resolve");
        assert_eq!(result, MathRendering::Svg);
    }

    #[test]
    fn test_resolve_math_rendering_valid() {
        let mut cfg = make_config();
        cfg.math_rendering = Some("png".into());
        let result = resolve_math_rendering_from_config(Some(&cfg), &TEST_CAP).expect("resolve");
        assert_eq!(result, MathRendering::Png);
    }

    #[test]
    fn test_resolve_math_rendering_invalid() {
        let mut cfg = make_config();
        cfg.math_rendering = Some("invalid".into());
        let err = resolve_math_rendering_from_config(Some(&cfg), &TEST_CAP).expect_err("invalid");
        assert!(err.to_string().contains("Invalid math_rendering"));
    }

    #[test]
    fn test_resolve_math_rendering_unsupported() {
        let mut cfg = make_config();
        cfg.math_rendering = Some("latex".into());
        let err =
            resolve_math_rendering_from_config(Some(&cfg), &TEST_CAP).expect_err("unsupported");
        assert!(err.to_string().contains("does not support math_rendering"));
    }

    #[test]
    fn test_resolve_math_delimiters_default() {
        let result = resolve_math_delimiters_from_config(None, &TEST_CAP).expect("resolve");
        assert_eq!(result, MathDelimiters::Dollar);
    }

    #[test]
    fn test_resolve_math_delimiters_valid() {
        let mut cfg = make_config();
        cfg.math_delimiters = Some("brackets".into());
        let result = resolve_math_delimiters_from_config(Some(&cfg), &TEST_CAP).expect("resolve");
        assert_eq!(result, MathDelimiters::Brackets);
    }

    #[test]
    fn test_resolve_math_delimiters_invalid() {
        let mut cfg = make_config();
        cfg.math_delimiters = Some("invalid".into());
        let err = resolve_math_delimiters_from_config(Some(&cfg), &TEST_CAP).expect_err("invalid");
        assert!(err.to_string().contains("Invalid math_delimiters"));
    }

    #[test]
    fn test_default_render_config_for() {
        let config = default_render_config_for(AssetStrategy::Upload, &TEST_CAP);
        assert!(config.image_as_marker);
        assert_eq!(config.math_rendering, MathRendering::Svg);

        let config = default_render_config_for(AssetStrategy::Embed, &TEST_CAP);
        assert!(!config.image_as_marker);

        let config = default_render_config_for(AssetStrategy::External, &TEST_CAP);
        assert!(config.image_as_marker);
    }

    #[test]
    fn test_register_adapter_helper() {
        let mut registrar = AdapterRegistrar::new();
        register_adapter(&mut registrar, &TEST_CAP, dummy_factory).expect("register");
        assert!(registrar.capabilities().contains_key("test"));
    }

    #[test]
    fn test_prepare_deferred_assets_empty() {
        let document = Document {
            blocks: Vec::new(),
            footnotes: BTreeMap::new(),
            assets: BTreeMap::new(),
            meta: DocMeta::default(),
        };
        let path = std::path::PathBuf::from("/content");

        // Embed strategy - no deferred assets
        let deferred = prepare_deferred_assets(AssetStrategy::Embed, &document, &path);
        assert!(deferred.pending.assets.is_empty());
        assert!(!deferred.needs_materialize());
    }

    #[test]
    fn test_prepare_deferred_assets_external() {
        let document = Document {
            blocks: Vec::new(),
            footnotes: BTreeMap::new(),
            assets: BTreeMap::new(),
            meta: DocMeta::default(),
        };
        let path = std::path::PathBuf::from("/content");

        // External strategy with empty document assets
        let deferred = prepare_deferred_assets(AssetStrategy::External, &document, &path);
        assert!(deferred.pending.assets.is_empty());
        assert_eq!(deferred.strategy, AssetStrategy::External);
        // Empty pending assets means no materialization needed
        assert!(!deferred.needs_materialize());
    }
}
