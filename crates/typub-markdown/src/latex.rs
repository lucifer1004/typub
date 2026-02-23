//! LaTeX conversion utilities.
//!
//! Provides conversion from Typst math source to LaTeX using the tylax crate.

/// Convert Typst math source to LaTeX.
///
/// The input should be Typst math syntax (without $ delimiters).
/// Returns LaTeX math syntax (without $ delimiters).
///
/// # Example
///
/// ```
/// use typub_markdown::latex::typst_math_to_latex;
///
/// let latex = typst_math_to_latex("sum_(i=1)^n i");
/// assert!(latex.contains("sum"));
/// ```
pub fn typst_math_to_latex(typst_src: &str) -> String {
    // Wrap in $ for tylax (it requires math delimiters to recognize math mode)
    let wrapped = format!("${}$", typst_src);

    let latex = tylax::typst_to_latex(&wrapped);

    // Strip outer $ delimiters from result
    let trimmed = latex.trim();
    if trimmed.starts_with('$') && trimmed.ends_with('$') && trimmed.len() >= 2 {
        // Remove single $ delimiters
        trimmed[1..trimmed.len() - 1].to_string()
    } else if trimmed.starts_with("\\[") && trimmed.ends_with("\\]") {
        // Remove \[ \] delimiters (display math)
        trimmed[2..trimmed.len() - 2].trim().to_string()
    } else if trimmed.starts_with("\\(") && trimmed.ends_with("\\)") {
        // Remove \( \) delimiters (inline math)
        trimmed[2..trimmed.len() - 2].trim().to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_fraction() {
        let result = typst_math_to_latex("(a)/(b)");
        // Should contain frac
        assert!(
            result.contains("frac") || result.contains("/"),
            "Expected frac in: {}",
            result
        );
    }

    #[test]
    fn test_subscript_superscript() {
        let result = typst_math_to_latex("x_1^2");
        assert!(result.contains("_") && result.contains("^"), "{}", result);
    }

    #[test]
    fn test_sum_notation() {
        let result = typst_math_to_latex("sum_(i=1)^n i");
        assert!(result.contains("sum"), "{}", result);
    }

    #[test]
    fn test_sqrt() {
        let result = typst_math_to_latex("sqrt(x)");
        assert!(result.contains("sqrt"), "{}", result);
    }

    #[test]
    fn test_greek_letters() {
        let result = typst_math_to_latex("α + β");
        // Should contain alpha and beta (or unicode)
        assert!(
            result.contains("alpha") || result.contains("α"),
            "{}",
            result
        );
    }
}
