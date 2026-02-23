//! HTML content-model helpers used by parser dispatch.
//!
//! These rules are intentionally centralized here so list/container parsing
//! does not maintain divergent tag classifications.

/// Returns true if `tag` is treated as phrasing content for inline parsing.
///
/// In `li` parsing, phrasing elements are accumulated into inline runs, while
/// non-phrasing elements are parsed through block dispatch.
pub(crate) fn is_phrasing_content_tag(tag: &str) -> bool {
    matches!(
        tag,
        "a" | "abbr"
            | "audio"
            | "b"
            | "bdi"
            | "bdo"
            | "br"
            | "button"
            | "canvas"
            | "cite"
            | "code"
            | "data"
            | "del"
            | "dfn"
            | "em"
            | "embed"
            | "i"
            | "iframe"
            | "img"
            | "input"
            | "ins"
            | "kbd"
            | "label"
            | "map"
            | "mark"
            | "math"
            | "meter"
            | "noscript"
            | "object"
            | "output"
            | "picture"
            | "progress"
            | "q"
            | "ruby"
            | "s"
            | "samp"
            | "script"
            | "select"
            | "slot"
            | "small"
            | "span"
            | "strong"
            | "sub"
            | "sup"
            | "svg"
            | "template"
            | "textarea"
            | "time"
            | "u"
            | "var"
            | "video"
            | "wbr"
    )
}

#[cfg(test)]
mod tests {
    use super::is_phrasing_content_tag;

    #[test]
    fn phrasing_examples() {
        assert!(is_phrasing_content_tag("span"));
        assert!(is_phrasing_content_tag("a"));
        assert!(is_phrasing_content_tag("img"));
        assert!(!is_phrasing_content_tag("p"));
        assert!(!is_phrasing_content_tag("div"));
        assert!(!is_phrasing_content_tag("ul"));
    }
}
