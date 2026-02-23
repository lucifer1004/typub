//! Notion adapter capability spec.
//!
//! Defines how semantic IR constructs map to Notion's block/rich_text model and
//! the expected fidelity level for each mapping.

use std::collections::HashMap;
use typub_ir::Inline;

/// Notion-supported code block languages.
/// Source: https://developers.notion.com/reference/block#code
pub const NOTION_SUPPORTED_LANGUAGES: &[&str] = &[
    "abap",
    "abc",
    "agda",
    "arduino",
    "ascii art",
    "assembly",
    "bash",
    "basic",
    "bnf",
    "c",
    "c#",
    "c++",
    "clojure",
    "coffeescript",
    "coq",
    "css",
    "dart",
    "dhall",
    "diff",
    "docker",
    "ebnf",
    "elixir",
    "elm",
    "erlang",
    "f#",
    "flow",
    "fortran",
    "gherkin",
    "glsl",
    "go",
    "graphql",
    "groovy",
    "haskell",
    "hcl",
    "html",
    "idris",
    "java",
    "javascript",
    "json",
    "julia",
    "kotlin",
    "latex",
    "less",
    "lisp",
    "livescript",
    "llvm ir",
    "lua",
    "makefile",
    "markdown",
    "markup",
    "matlab",
    "mathematica",
    "mermaid",
    "nix",
    "notion formula",
    "objective-c",
    "ocaml",
    "pascal",
    "perl",
    "php",
    "plain text",
    "powershell",
    "prolog",
    "protobuf",
    "purescript",
    "python",
    "r",
    "racket",
    "reason",
    "ruby",
    "rust",
    "sass",
    "scala",
    "scheme",
    "scss",
    "shell",
    "smalltalk",
    "solidity",
    "sql",
    "swift",
    "toml",
    "typescript",
    "vb.net",
    "verilog",
    "vhdl",
    "visual basic",
    "webassembly",
    "xml",
    "yaml",
    "java/c/c++/c#",
];

/// Language aliases mapping common language names to Notion's expected names.
/// Keys are lowercase for case-insensitive matching.
const LANGUAGE_ALIASES: &[(&str, &str)] = &[
    ("js", "javascript"),
    ("ts", "typescript"),
    ("py", "python"),
    ("rb", "ruby"),
    ("sh", "shell"),
    ("zsh", "shell"),
    ("csharp", "c#"),
    ("cs", "c#"),
    ("cpp", "c++"),
    ("objective-c", "objective-c"),
    ("objc", "objective-c"),
    ("fsharp", "f#"),
    ("fs", "f#"),
    ("hs", "haskell"),
    ("golang", "go"),
    ("rs", "rust"),
    ("kt", "kotlin"),
    ("scala", "scala"),
    ("clj", "clojure"),
    ("cljs", "clojure"),
    ("ex", "elixir"),
    ("exs", "elixir"),
    ("erl", "erlang"),
    ("ml", "ocaml"),
    ("mli", "ocaml"),
    ("pl", "perl"),
    ("ps1", "powershell"),
    ("psm1", "powershell"),
    ("dockerfile", "docker"),
    ("make", "makefile"),
    ("mk", "makefile"),
    ("tex", "latex"),
    ("typst", "plain text"),
    ("txt", "plain text"),
    ("text", "plain text"),
    ("", "plain text"),
];

/// Normalize a language name to a Notion-supported language.
/// Returns "plain text" for unsupported languages.
pub fn normalize_language(lang: &str) -> &'static str {
    // Handle empty/None case
    let lang_lower = lang.to_lowercase();
    if lang_lower.is_empty() {
        return "plain text";
    }

    // Build alias map (could be cached, but this is fast enough for typical usage)
    let mut alias_map: HashMap<&str, &str> = HashMap::new();
    for (alias, target) in LANGUAGE_ALIASES {
        alias_map.insert(alias, target);
    }

    // Check if it's a direct match
    for &supported in NOTION_SUPPORTED_LANGUAGES {
        if supported == lang_lower {
            return supported;
        }
    }

    // Check aliases
    if let Some(&canonical) = alias_map.get(lang_lower.as_str()) {
        // Verify the canonical name is supported
        for &supported in NOTION_SUPPORTED_LANGUAGES {
            if supported == canonical {
                return canonical;
            }
        }
    }

    // Fallback to plain text
    "plain text"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FidelityGrade {
    /// Semantics are preserved natively.
    Native,
    /// Semantics are preserved with approximation.
    Approximate,
    /// Semantics are preserved only via textual fallback.
    Fallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlinePlacement {
    /// Can stay inside Notion rich_text.
    RichText,
    /// Must be emitted as standalone block(s).
    BlockOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InlineCapability {
    pub placement: InlinePlacement,
    pub fidelity: FidelityGrade,
}

pub fn inline_capability(inline: &Inline) -> InlineCapability {
    match inline {
        Inline::Image { .. } => InlineCapability {
            placement: InlinePlacement::BlockOnly,
            fidelity: FidelityGrade::Approximate,
        },
        Inline::MathInline { .. } | Inline::SvgInline { .. } => InlineCapability {
            placement: InlinePlacement::RichText,
            fidelity: FidelityGrade::Approximate,
        },
        Inline::UnknownInline { .. } | Inline::RawInline { .. } => InlineCapability {
            placement: InlinePlacement::RichText,
            fidelity: FidelityGrade::Fallback,
        },
        Inline::Text(_)
        | Inline::Code(_)
        | Inline::SoftBreak
        | Inline::HardBreak
        | Inline::Styled { .. }
        | Inline::Link { .. }
        | Inline::FootnoteRef(_) => InlineCapability {
            placement: InlinePlacement::RichText,
            fidelity: FidelityGrade::Native,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use typub_ir::{AssetId, AssetRef, ImageAttrs, InlineAttrs, MathSource, RenderPayload};

    #[test]
    fn test_normalize_language_supported() {
        assert_eq!(normalize_language("rust"), "rust");
        assert_eq!(normalize_language("python"), "python");
        assert_eq!(normalize_language("javascript"), "javascript");
        assert_eq!(normalize_language("plain text"), "plain text");
    }

    #[test]
    fn test_normalize_language_case_insensitive() {
        assert_eq!(normalize_language("Rust"), "rust");
        assert_eq!(normalize_language("PYTHON"), "python");
        assert_eq!(normalize_language("JavaScript"), "javascript");
    }

    #[test]
    fn test_normalize_language_aliases() {
        assert_eq!(normalize_language("js"), "javascript");
        assert_eq!(normalize_language("ts"), "typescript");
        assert_eq!(normalize_language("py"), "python");
        assert_eq!(normalize_language("rb"), "ruby");
        assert_eq!(normalize_language("sh"), "shell");
        assert_eq!(normalize_language("csharp"), "c#");
        assert_eq!(normalize_language("cpp"), "c++");
        assert_eq!(normalize_language("golang"), "go");
        assert_eq!(normalize_language("rs"), "rust");
        assert_eq!(normalize_language("kt"), "kotlin");
        assert_eq!(normalize_language("dockerfile"), "docker");
        assert_eq!(normalize_language("tex"), "latex");
    }

    #[test]
    fn test_normalize_language_unsupported() {
        assert_eq!(normalize_language("typst"), "plain text");
        assert_eq!(normalize_language("unknown-lang"), "plain text");
        assert_eq!(normalize_language("random"), "plain text");
    }

    #[test]
    fn test_normalize_language_empty() {
        assert_eq!(normalize_language(""), "plain text");
    }

    #[test]
    fn image_is_block_only() {
        let inline = Inline::Image {
            asset: AssetRef(AssetId("asset-1".to_string())),
            alt: String::new(),
            title: None,
            attrs: ImageAttrs::default(),
        };
        let cap = inline_capability(&inline);
        assert_eq!(cap.placement, InlinePlacement::BlockOnly);
        assert_eq!(cap.fidelity, FidelityGrade::Approximate);
    }

    #[test]
    fn inline_equation_stays_rich_text() {
        let inline = Inline::MathInline {
            math: RenderPayload {
                src: Some(MathSource::Latex("x".to_string())),
                rendered: None,
                id: None,
            },
            attrs: InlineAttrs::default(),
        };
        let cap = inline_capability(&inline);
        assert_eq!(cap.placement, InlinePlacement::RichText);
    }
}
