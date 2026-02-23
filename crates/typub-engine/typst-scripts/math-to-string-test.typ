// math-to-string-test.typ
// Test suite for math-to-string.typ
// Validates that math content is converted to correct Typst source strings
// which can then be converted to LaTeX via typax t2l
//
// Run: typst compile math-to-string-test.typ math-to-string-test.pdf

#import "math-to-string.typ": math-to-string

// ============================================================================
// 1. Primitives
// ============================================================================

#assert(math-to-string("hello") == "hello", message: "string passthrough")
#assert(math-to-string(none) == "none", message: "none value")
#assert(math-to-string("") == "", message: "empty string")

// ============================================================================
// 2. Arrays and Sequences
// ============================================================================

#assert(math-to-string(("a", "b", "c")) == "abc", message: "array of strings")
#assert(math-to-string($a b c$) == "a b c", message: "sequence from math")

// ============================================================================
// 3. Fractions (math.frac)
// ============================================================================

#assert(math-to-string($a/b$) == "(a)/(b)", message: "frac: simple")
#assert(math-to-string($1/2$) == "(1)/(2)", message: "frac: numbers")
#assert(math-to-string($(a+b)/(c-d)$) == "(a+b)/(c−d)", message: "frac: expressions")
#assert(math-to-string($(a/b)/c$) == "((a)/(b))/(c)", message: "frac: nested left")
#assert(math-to-string($a/(b/c)$) == "(a)/((b)/(c))", message: "frac: nested right")

// ============================================================================
// 4. Subscript and Superscript (math.attach)
// ============================================================================

#assert(math-to-string($x_1$) == "x_1", message: "attach: subscript single")
#assert(math-to-string($x_("ij")$) == "x_(ij)", message: "attach: subscript multi")
#assert(math-to-string($x^2$) == "x^2", message: "attach: superscript single")
#assert(math-to-string($x^("ab")$) == "x^(ab)", message: "attach: superscript multi")
#assert(math-to-string($x_1^2$) == "x_1^2", message: "attach: both")
#assert(math-to-string($x_("ij")^("kl")$) == "x_(ij)^(kl)", message: "attach: both multi")

// ============================================================================
// 5. Roots (math.root)
// ============================================================================

#assert(math-to-string($sqrt(x)$) == "sqrt(x)", message: "root: sqrt")
#assert(math-to-string($sqrt(a+b)$) == "sqrt(a+b)", message: "root: sqrt expression")
#assert(math-to-string($root(3, x)$) == "root(3, x)", message: "root: nth")
#assert(math-to-string($root(n, a+b)$) == "root(n, a+b)", message: "root: nth expression")

// ============================================================================
// 6. LR Delimiters (math.lr)
// ============================================================================

#assert(math-to-string($lr( a + b )$) == "a + b", message: "lr: parens")

// ============================================================================
// 7. Vector/Matrix (math.vec, math.mat)
// ============================================================================

#assert(math-to-string($vec(a, b, c)$) == "vec(a, b, c)", message: "vec: simple")
#assert(math-to-string($vec(x)$) == "vec(x)", message: "vec: single")
#assert(math-to-string($vec(a+b, c-d)$) == "vec(a+b, c−d)", message: "vec: expressions")
#assert(math-to-string($mat(a, b; c, d)$) == "mat(a, b; c, d)", message: "mat: 2x2")
#assert(math-to-string($mat(a, b, c)$) == "mat(a, b, c)", message: "mat: 1x3")
#assert(math-to-string($mat(a; b; c)$) == "mat(a; b; c)", message: "mat: 3x1")

// ============================================================================
// 8. Cases (math.cases)
// ============================================================================

#assert(math-to-string($cases(a, b, c)$) == "cases(a, b, c)", message: "cases: simple")
#assert(math-to-string($cases(x)$) == "cases(x)", message: "cases: single")

// ============================================================================
// 9. Binomial (math.binom)
// ============================================================================

#assert(math-to-string($binom(n, k)$) == "binom(n, k)", message: "binom: simple")
#assert(math-to-string($binom(n+1, k-1)$) == "binom(n+1, k−1)", message: "binom: expressions")

// ============================================================================
// 10. Style Functions
// Note: Styles are NOT preserved at runtime because styled.styles is opaque.
// The child content is returned without style information.
// ============================================================================

#assert(math-to-string($bold(x)$) == "x", message: "style: bold (content only)")
#assert(math-to-string($italic(x)$) == "x", message: "style: italic (content only)")
#assert(math-to-string($upright(A)$) == "A", message: "style: upright (content only)")
#assert(math-to-string($sans(x)$) == "x", message: "style: sans (content only)")
#assert(math-to-string($bb(N)$) == "N", message: "style: bb (content only)")
#assert(math-to-string($cal(L)$) == "L", message: "style: cal (content only)")
#assert(math-to-string($frak(x)$) == "x", message: "style: frak (content only)")

// ============================================================================
// 11. Cancel (math.cancel)
// ============================================================================

#assert(math-to-string($cancel(x)$) == "cancel(x)", message: "cancel")

// ============================================================================
// 12. Under/Over Braces (_underover)
// ============================================================================

#assert(math-to-string($underbrace(x)$) == "underbrace(x)", message: "underbrace: simple")
#assert(math-to-string($overbrace(x)$) == "overbrace(x)", message: "overbrace: simple")
#assert(math-to-string($underbrace(x, "note")$) == "underbrace(x, note)", message: "underbrace: annotation")
#assert(math-to-string($overbrace(x, "label")$) == "overbrace(x, label)", message: "overbrace: annotation")
#assert(math-to-string($underbracket(x)$) == "underbracket(x)", message: "underbracket")
#assert(math-to-string($overbracket(x)$) == "overbracket(x)", message: "overbracket")
#assert(math-to-string($underparen(x)$) == "underparen(x)", message: "underparen")
#assert(math-to-string($overparen(x)$) == "overparen(x)", message: "overparen")
#assert(math-to-string($undershell(x)$) == "undershell(x)", message: "undershell")
#assert(math-to-string($overshell(x)$) == "overshell(x)", message: "overshell")

// ============================================================================
// 13. Underline/Overline
// ============================================================================

#assert(math-to-string($underline(x)$) == "underline(x)", message: "underline")
#assert(math-to-string($overline(x)$) == "overline(x)", message: "overline")

// ============================================================================
// 14. Size Functions (_sizes)
// Note: Size functions also create styled elements, so size info is lost.
// ============================================================================

#assert(math-to-string($display(x)$) == "x", message: "size: display (content only)")
#assert(math-to-string($inline(x)$) == "x", message: "size: inline (content only)")
#assert(math-to-string($script(x)$) == "x", message: "size: script (content only)")
#assert(math-to-string($sscript(x)$) == "x", message: "size: sscript (content only)")

// ============================================================================
// 15. Stretch (math.stretch)
// ============================================================================

#assert(math-to-string($stretch(x)$) == "stretch(x)", message: "stretch: simple")

// ============================================================================
// 16. Class (math.class)
// ============================================================================

#assert(math-to-string($class("normal", x)$) == "x", message: "class: returns body")

// ============================================================================
// 17. Equation (math.equation)
// ============================================================================

#assert(math-to-string($x = 1$) == "x = 1", message: "equation: inline")

// ============================================================================
// 18. Accents (math.accent)
// ============================================================================

#assert(math-to-string($hat(x)$) == "hat(x)", message: "accent: hat")

// ============================================================================
// 19. Primes (math.primes)
// ============================================================================

#assert(math-to-string($f'$) == "f^'", message: "primes: single")
#assert(math-to-string($f''$) == "f^('')", message: "primes: double")

// ============================================================================
// 20. Complex Expressions
// ============================================================================

// Quadratic formula (note: minus is Unicode U+2212)
#assert(math-to-string($x = (-b + sqrt(b^2))/(2a)$) == "x = (−b + sqrt(b^2))/(2a)", message: "complex: quadratic")

// ============================================================================
// Summary
// ============================================================================

= Test Summary

All test cases passed! The Typst string output is ready for t2l conversion.

Note: Style functions (bold, italic, etc.) lose style information at runtime
because Typst's styled.styles is opaque. Only the content is preserved.
