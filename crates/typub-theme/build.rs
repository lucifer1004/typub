//! Build script: generates `builtin_themes.rs` with embedded CSS from `templates/themes/`.
//!
//! This embeds all theme CSS files at compile time, allowing the binary to work
//! standalone without external files while still supporting user overrides.

// Build scripts are expected to panic on failure — allow expect/unwrap.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fmt::Write;
use std::path::Path;

fn main() {
    generate_builtin_themes();
}

/// Generate `builtin_themes.rs` with embedded CSS from `templates/themes/`.
///
/// Uses `concat!` with `env!("CARGO_MANIFEST_DIR")` to build paths at compile time,
/// avoiding hardcoded absolute paths in generated code that would leak local info.
fn generate_builtin_themes() {
    let themes_dir = Path::new("templates/themes");
    println!("cargo::rerun-if-changed=templates/themes");

    let mut out = String::new();

    writeln!(
        out,
        "// Auto-generated from `templates/themes/` — do not edit."
    )
    .unwrap();
    writeln!(
        out,
        "// Contains embedded CSS for themes and preview styling."
    )
    .unwrap();
    writeln!(out).unwrap();

    // Embed _base.css using concat! + env! for portable path resolution
    writeln!(out, "/// Base CSS applied to all themes.").unwrap();
    writeln!(
        out,
        r#"pub const BUILTIN_BASE_CSS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/themes/_base.css"));"#
    )
    .unwrap();
    writeln!(out).unwrap();

    // Collect theme files (non-underscore-prefixed CSS files)
    let mut themes: Vec<String> = Vec::new();
    let mut preview_css: Vec<String> = Vec::new();

    if themes_dir.exists() {
        for entry in std::fs::read_dir(themes_dir).expect("failed to read templates/themes") {
            let entry = entry.expect("failed to read dir entry");
            let path = entry.path();

            if path.extension().is_some_and(|e| e == "css") {
                let filename = path.file_stem().unwrap().to_string_lossy().to_string();

                if filename.starts_with("_preview-") {
                    // Preview CSS files
                    let platform = filename.strip_prefix("_preview-").unwrap().to_string();
                    preview_css.push(platform);
                } else if !filename.starts_with('_') {
                    // Theme CSS files (skip _base.css and other underscore files)
                    themes.push(filename);
                }
            }
        }
    }

    // Sort for deterministic output
    themes.sort();
    preview_css.sort();

    // Generate BUILTIN_THEMES array using concat! + env! for portable paths
    writeln!(out, "/// Built-in theme CSS files (id, css_content).").unwrap();
    writeln!(out, "pub static BUILTIN_THEMES: &[(&str, &str)] = &[").unwrap();
    for theme in &themes {
        writeln!(
            out,
            r#"    ("{}", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/themes/{}.css"))),"#,
            theme, theme
        )
        .unwrap();
    }
    writeln!(out, "];").unwrap();
    writeln!(out).unwrap();

    // Generate BUILTIN_PREVIEW_CSS array
    writeln!(
        out,
        "/// Built-in preview CSS files (platform_id, css_content)."
    )
    .unwrap();
    writeln!(out, "pub static BUILTIN_PREVIEW_CSS: &[(&str, &str)] = &[").unwrap();
    for platform in &preview_css {
        writeln!(
            out,
            r#"    ("{}", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/themes/_preview-{}.css"))),"#,
            platform, platform
        )
        .unwrap();
    }
    writeln!(out, "];").unwrap();

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    std::fs::write(format!("{out_dir}/builtin_themes.rs"), out)
        .expect("failed to write builtin_themes.rs");
}
