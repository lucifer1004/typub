//! Build script: generates `builtin_profiles.rs` from `profiles.toml`.
//!
//! All string fields are baked into the binary as `&'static str` — zero runtime allocation.

// Build scripts are expected to panic on failure — allow expect/unwrap.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde::Deserialize;
use std::fmt::Write;
use typub_core::{AssetStrategy, MathRendering};

#[derive(Deserialize)]
struct Profiles {
    profile: Vec<Profile>,
}

#[derive(Deserialize)]
struct Profile {
    id: String,
    name: String,
    short_code: String,
    editor_url: String,
    format: String,
    serialize_rules: Option<Vec<String>>,
    default_theme: Option<String>,
    #[serde(default)]
    default_asset_strategy: Option<String>,
    #[serde(default)]
    math_delimiters: Option<String>,
    #[serde(default)]
    math_rendering: Option<MathRendering>,
    /// Whether to use inline HTML for sized images in Markdown output.
    #[serde(default)]
    use_inline_html_for_sized_images: bool,
    /// Markdown post-processing rules.
    #[serde(default)]
    markdown_processing_rules: Option<Vec<String>>,
    /// Whether lists should be tight (no blank lines between items).
    #[serde(default = "default_true")]
    tight_lists: bool,
}

fn default_true() -> bool {
    true
}

fn main() {
    generate_builtin_profiles();
}

/// Generate `builtin_profiles.rs` from `profiles.toml`.
fn generate_builtin_profiles() {
    println!("cargo::rerun-if-changed=profiles.toml");

    let toml_str = std::fs::read_to_string("profiles.toml").expect("failed to read profiles.toml");
    let profiles: Profiles = toml::from_str(&toml_str).expect("failed to parse profiles.toml");

    let mut out = String::new();

    writeln!(
        out,
        "/// Auto-generated from `profiles.toml` — do not edit."
    )
    .unwrap();
    // Types are already imported in lib.rs, only need enumset! macro
    writeln!(out, "use enumset::enum_set;").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "pub static BUILTIN_PROFILES: &[BuiltinProfile] = &[").unwrap();

    for p in &profiles.profile {
        let format = match p.format.as_str() {
            "markdown" | "md" => "CopyFormat::Markdown",
            _ => "CopyFormat::StyledHtml",
        };
        // Generate serialize_rules using enum_set! macro for const compatibility
        let serialize_rules = match &p.serialize_rules {
            Some(rules) if !rules.is_empty() => {
                let rule_exprs: Vec<&str> = rules
                    .iter()
                    .filter_map(|r| match r.as_str() {
                        "li_span_wrap" => Some("SerializeRule::LiSpanWrap"),
                        "blockquote_for_admonition" => {
                            Some("SerializeRule::BlockquoteForAdmonition")
                        }
                        "sibling_nested_lists" => Some("SerializeRule::SiblingNestedLists"),
                        "definition_list_to_paragraph" => {
                            Some("SerializeRule::DefinitionListToParagraph")
                        }
                        _ => None, // Unknown rules are ignored
                    })
                    .collect();
                if rule_exprs.is_empty() {
                    "enum_set!()".to_string()
                } else {
                    format!("enum_set!({})", rule_exprs.join(" | "))
                }
            }
            _ => "enum_set!()".to_string(),
        };
        let default_theme = match &p.default_theme {
            Some(theme) => format!("Some(\"{}\")", theme),
            None => "None".to_string(),
        };
        // Generate asset_strategies slice: default first, then all others
        // Per [[WI-2026-02-18-002]].
        let default_asset = match &p.default_asset_strategy {
            Some(s) => AssetStrategy::parse(s)
                .unwrap_or_else(|| panic!("Invalid asset_strategy '{}' in profile '{}'", s, p.id)),
            None => AssetStrategy::Embed, // Default fallback
        };
        let asset_strategies = generate_asset_strategies_slice(default_asset);

        // Generate math_delimiters slice: default first, then all others
        // Per [[WI-2026-02-18-002]].
        let default_delimiter = match &p.math_delimiters {
            Some(s) => match s.as_str() {
                "dollar" => typub_core::MathDelimiters::Dollar,
                "brackets" => typub_core::MathDelimiters::Brackets,
                "brackets_inline_dollar_block" => {
                    typub_core::MathDelimiters::BracketsInlineDollarBlock
                }
                _ => panic!("Invalid math_delimiters '{}' in profile '{}'", s, p.id),
            },
            None => typub_core::MathDelimiters::Dollar, // Default fallback
        };
        let math_delimiters = generate_math_delimiters_slice(default_delimiter);

        // Generate math_renderings slice: default first, then all others
        // Per [[WI-2026-02-18-002]].
        let default_rendering = match &p.math_rendering {
            Some(mr) => *mr,
            None => {
                // Default: Markdown uses Latex, HTML uses Svg
                match p.format.as_str() {
                    "markdown" | "md" => typub_core::MathRendering::Latex,
                    _ => typub_core::MathRendering::Svg,
                }
            }
        };
        let math_renderings = generate_math_renderings_slice(default_rendering);

        // Generate markdown_processing_rules using enum_set! macro
        let markdown_processing_rules = match &p.markdown_processing_rules {
            Some(rules) if !rules.is_empty() => {
                let rule_exprs: Vec<&str> = rules
                    .iter()
                    .filter_map(|r| match r.as_str() {
                        "noop" => Some("typub_adapters_core::MarkdownProcessingRule::Noop"),
                        _ => None, // Unknown rules are ignored
                    })
                    .collect();
                if rule_exprs.is_empty() {
                    "enum_set!()".to_string()
                } else {
                    format!("enum_set!({})", rule_exprs.join(" | "))
                }
            }
            _ => "enum_set!()".to_string(),
        };
        writeln!(
            out,
            "    BuiltinProfile {{ id: \"{}\", name: \"{}\", short_code: \"{}\", editor_url: \"{}\", format: {}, serialize_rules: {}, default_theme: {}, asset_strategies: {}, math_delimiters: {}, math_renderings: {}, use_inline_html_for_sized_images: {}, markdown_processing_rules: {}, tight_lists: {} }},",
            p.id, p.name, p.short_code, p.editor_url, format, serialize_rules, default_theme, asset_strategies, math_delimiters, math_renderings, p.use_inline_html_for_sized_images, markdown_processing_rules, p.tight_lists
        )
        .unwrap();
    }

    writeln!(out, "];").unwrap();

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    std::fs::write(format!("{out_dir}/builtin_profiles.rs"), out)
        .expect("failed to write builtin_profiles.rs");
}

/// Generate a static slice expression for asset strategies.
/// Default first, then all alternatives.
/// For copypaste profiles, only Embed and External are relevant.
fn generate_asset_strategies_slice(default: AssetStrategy) -> String {
    match default {
        AssetStrategy::Embed => "&[AssetStrategy::Embed, AssetStrategy::External]".to_string(),
        AssetStrategy::External => "&[AssetStrategy::External, AssetStrategy::Embed]".to_string(),
        // Copy and Upload are not used in copypaste profiles, fallback to Embed first
        AssetStrategy::Copy | AssetStrategy::Upload => {
            "&[AssetStrategy::Embed, AssetStrategy::External]".to_string()
        }
    }
}

/// Generate a static slice expression for math delimiters.
/// Default first, then all alternatives.
fn generate_math_delimiters_slice(default: typub_core::MathDelimiters) -> String {
    match default {
        typub_core::MathDelimiters::Dollar => "&[MathDelimiters::Dollar, MathDelimiters::Brackets, MathDelimiters::BracketsInlineDollarBlock]".to_string(),
        typub_core::MathDelimiters::Brackets => "&[MathDelimiters::Brackets, MathDelimiters::Dollar, MathDelimiters::BracketsInlineDollarBlock]".to_string(),
        typub_core::MathDelimiters::BracketsInlineDollarBlock => "&[MathDelimiters::BracketsInlineDollarBlock, MathDelimiters::Dollar, MathDelimiters::Brackets]".to_string(),
    }
}

/// Generate a static slice expression for math renderings.
/// Default first, then all alternatives.
fn generate_math_renderings_slice(default: typub_core::MathRendering) -> String {
    match default {
        typub_core::MathRendering::Svg => {
            "&[MathRendering::Svg, MathRendering::Png, MathRendering::Latex]".to_string()
        }
        typub_core::MathRendering::Png => {
            "&[MathRendering::Png, MathRendering::Svg, MathRendering::Latex]".to_string()
        }
        typub_core::MathRendering::Latex => {
            "&[MathRendering::Latex, MathRendering::Svg, MathRendering::Png]".to_string()
        }
    }
}
