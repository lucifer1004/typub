//! Shared adapter types for typub.

pub mod adapter;
pub mod capability;
pub mod context;
pub mod helpers;
pub mod http_utils;
pub mod image_utils;
pub mod metadata;
pub mod payload;
pub mod preview;
pub mod registrar;
pub mod types;

#[cfg(test)]
mod tests;

// Re-export tracing macros for adapter logging per [[ADR-0004]]
pub use typub_log::{debug, error, info, trace, warn};

// Re-export assets-ast utilities
pub use typub_assets_ast::{
    build_pending_asset_list_from_document, ensure_no_unresolved_image_markers, resolve_asset_urls,
};

// Re-export markdown utilities from typub-markdown crate
pub use typub_markdown::{
    MarkdownProcessingRule, MarkdownProcessingRules, MarkdownRenderOptions, document_to_markdown,
    document_to_markdown_with_options, parse_markdown_processing_rules, typst_math_to_latex,
};

// Re-export key types at crate root for convenience
pub use adapter::{PlatformAdapter, write_preview_file};
pub use capability::{AdapterCapability, ImageStrategyPolicy, LifecycleAction, NodePolicy};
// Re-export LinkResolution and TaxonomyCapability from typub_core
pub use context::{AdapterContext, LinkResolver, StatusLookup, StatusTracker};
pub use helpers::{
    convert_png_math_for_strategy, convert_svg_to_png_inline_if_configured,
    convert_svg_to_png_markers_if_configured, default_render_config_for,
    materialize_and_resolve_urls, mock_materialize_and_resolve_urls, prepare_deferred_assets,
    register_adapter, render_config_for_png_math, resolve_asset_strategy_from_config,
    resolve_asset_strategy_with_policy, resolve_math_delimiters_from_config,
    resolve_math_rendering_from_config,
};
pub use metadata::{DefaultMetadataService, MetadataService};
pub use payload::{AdapterPayload, PayloadInner, downcast_payload};
pub use preview::{PlatformBranding, build_unified_preview};
pub use registrar::{AdapterFactory, AdapterRegistrar};
pub use types::{
    ContentInfo, ContentTransform, OutputFormat, RenderConfig, ResolvedConfigDefaults,
};
pub use typub_core::{LinkResolution, NodePolicyAction, TaxonomyCapability};
pub use typub_storage::{AssetAnalysis, AssetInfo};
