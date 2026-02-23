= Typst Feature Test

#set heading(numbering: (..nums) => {
  numbering("1.", ..nums.pos().slice(1))
})
#set math.equation(numbering: "(1)")

This post demonstrates various Typst features supported by typub for publishing to different platforms.

== Text Formatting

Typst supports rich text formatting:

- *Bold text* and _italic text_
- `inline code` and ```raw text```
- #strike[Strikethrough text]
- #underline[Underlined text]
- #highlight[Highlighted text]
- Super#super[script] and Sub#sub[script]

=== Nested Formatting

- #strong[Bold #emph[Italic]]

== Links and References

Links to external sites: https://typst.app

Links with custom text: #link("https://typst.app")[Typst Homepage]

Internal references: See @sec-conclusion for conclusion.

== Lists

=== Unordered Lists

- Item 1
- Item 2
  - Nested item 2.1
  - Nested item 2.2
    - Deep nested item 2.2.1
    - Deep nested item 2.2.2
- Item 3

=== Ordered Lists

+ First item
+ Second item
  + Nested item 2.1
  + Nested item 2.2
+ Third item

=== Definition Lists

/ Term 1: Definition of term 1
/ Term 2: Definition of term 2
  with multiple lines
/ Typst: A modern typesetting system
    designed for the future

== Code Blocks

```rust
fn main() {
    println!("Hello from Typst!");

    let numbers: Vec<i32> = (1..=10)
        .filter(|x| x % 2 == 0)
        .collect();

    println!("Even numbers: {:?}", numbers);
}
```

```python
def fibonacci(n):
    """Calculate the nth Fibonacci number."""
    if n <= 1:
        return n
    return fibonacci(n - 1) + fibonacci(n - 2)

# Print first 10 Fibonacci numbers
for i in range(10):
    print(f"F({i}) = {fibonacci(i)}")
```

== Tables

Basic table:

#table(
  columns: 3,
  [Feature], [Status], [Notes],
  [Bold], [✓], [Supported],
  [Italic], [✓], [Supported],
  [Links], [✓], [Supported],
  [Tables], [✓], [Supported],
  [Math], [✓], [SVG/LaTeX/PNG]
)

Formatted table with alignments:

#table(
  columns: (auto, 1fr, auto),
  align: (center, left, center),
  [*Platform*], [*Math Rendering*], [*Notes*],
  [Ghost], [SVG inline], [Full support],
  [Notion], [Equation block], [Native support],
  [Confluence], [SVG attachment], [Plugin fallback],
  [WeChat], [PNG image], [Rasterized],
  [Xiaohongshu], [PNG slide], [Optimized],
)

== Math Equations

Inline math: $E = m c^2$ is Einstein's famous equation.

Block math with numbering:

$ integral_0^∞ e^(-x^2) dif x = sqrt(pi)/2 $ <eq-gaussian>

Complex formulas:

$ sum_(n=1)^∞ 1/n^2 = pi^2/6 $

Matrix example:

$ A = mat(
  1, 2, 3;
  4, 5, 6;
  7, 8, 9
) $

Aligned equations:

$ (a + b)^2 &= a^2 + 2a b + b^2 \
           &= a^2 + 2a b + b^2 $

== Images

Basic image:

#image("assets/test-image.jpg", width: 50%)

Image with caption:

#figure(
  image("assets/test-image.jpg", width: 60%),
  caption: [A test image with caption],
) <fig-example>

Image alignment test:

// NOTE: Typst HTML export ignores all `align` elements, so this only works
// on Xiaohongshu platform (PNG export).

#align(right)[
  #image("assets/test-image.jpg", width: 40%)
]

== Figures

Figures can contain various content:

#figure(
  ```typst
  let x = 10
  let y = 20
  #calc(x + y)
  ```,
  kind: raw,
  caption: [Source code example],
) <fig-code>

== Quotes

Basic quote:

#quote[This is a blockquote in Typst. It can span multiple lines and contain other content.]

Nested content in quotes:

#quote[
  Here is a quote with:
  - List item 1
  - List item 2

  And some _formatted text_.
]

== Raw Blocks

Raw text without processing:

```
This is raw text
*Not bold* _Not italic_
No interpretation of Typst markup
```

Raw with language:

```typst
= My Document

This is Typst source code shown as-is.

#set text(font: "Libertinus Serif")
#show par: set block(spacing: 0.65em)

Hello, world!
```

== Conditional Content

Typst allows conditional content:

#if true [
  This content appears when the condition is true.
]

#for i in range(3) [
  - Item number #i
]

== Variables and Functions

#let greeting(name) = {
  [Hello, ] + name + [!]
}

#greeting[Typst User]

#let name = "typub"
The project name is #name.

== Metadata and Labels

This section demonstrates labels and references.

- Reference to @fig-example shows figure numbering
- Reference to @eq-gaussian shows equation reference
- Reference to @sec-conclusion shows section labels

== Platform Support

Typst content can be published to all supported platforms:

#table(
  columns: 2,
  [Category], [Platforms],
  [API Adapters], [Ghost, WordPress, Dev.to, Notion, Confluence, Hashnode],
  [Static Adapters], [Xiaohongshu, Astro (Markdown), Static (HTML)],
  [Copy-Paste (HTML)], [WeChat, Zhihu, Bilibili, Toutiao, 51CTO...],
  [Copy-Paste (MD)], [Medium, CSDN, Juejin, SegmentFault, CNBlogs...]
)

== Conclusion <sec-conclusion>

Typst provides a powerful and modern typesetting system with clean syntax. Combined with typub, you can publish your Typst documents to over 30 platforms with consistent formatting and layout.

Key advantages of Typst:
- Clean and intuitive syntax
- Powerful scripting capabilities
- Excellent math support
- Modern layout engine
- Fast compilation

For more examples, see @fig-example and the equations in @eq-gaussian.
