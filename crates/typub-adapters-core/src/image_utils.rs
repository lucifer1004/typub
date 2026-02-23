//! Shared image reference mapping utilities.
//!
//! Pure path/URL mapping utilities are re-exported from `typub-storage`.

// Re-export pure URL mapping utilities from typub-storage
pub use typub_storage::{
    build_image_marker_url_map, build_image_src_url_map, build_preview_file_url_map,
    key_candidates, resolve_image_reference_url,
};
