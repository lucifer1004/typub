//! Generic copy-paste adapter for multi-platform publishing.
//!
//! Generalizes the WeChat "preview + copy button" pattern into a data-driven
//! adapter parameterized by platform profiles.  Each platform is a
//! `CopyPasteProfile` entry — one Rust type, many platform instances.
//!
//! Per [[RFC-0002:C-PIPELINE-STAGES]], platform-specific output variations are
//! controlled via `SerializeRules` at serialization time.

mod adapter;
mod config;
mod model;

pub use adapter::CopyPasteAdapter;
pub use config::{create_for_profile, create_manual, register};
pub use model::{BuiltinProfile, CopyFormat, all_profiles, find_profile, known_profile_ids};

#[cfg(test)]
mod tests;
