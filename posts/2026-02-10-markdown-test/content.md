# Markdown Support Test

This post is written in **Markdown** and rendered via [cmarker](https://typst.app/universe/package/cmarker/).

## Features

Here's what works:

- **Bold text** and _italic text_
- ~~Strikethrough text~~
- `inline code` blocks
- [Links](https://example.com)
- Lists (like this one)

### New Inline Formatting

- <u>Underlined text</u> (HTML)
- <mark>Highlighted text</mark> (HTML)
- Text with <sup>superscript</sup> and <sub>subscript</sub>
- Keyboard shortcut: <kbd>Ctrl</kbd> + <kbd>C</kbd>

### Markdown Platform Support

Markdown content can be published to all 33 supported platforms:

| Category          | Platforms                                              |
| ----------------- | ------------------------------------------------------ |
| API Adapters      | Ghost, WordPress, Dev.to, Notion, Confluence, Hashnode |
| Static Adapters   | Xiaohongshu, Astro (Markdown), Static (HTML)           |
| Copy-Paste (HTML) | WeChat, Zhihu, Bilibili, Toutiao, 51CTO...             |
| Copy-Paste (MD)   | Medium, CSDN, Juejin, SegmentFault, CNBlogs...         |

## Nested Lists

- Item 1
  - Nested item 1.1
  - Nested item 1.2
- Item 2
  - Nested item 2.1
    - Deep nested item 2.1.1
    - Deep nested item 2.1.2

Numbered list with nesting:

1. First
   1. First sub-item
   2. Second sub-item
      1. Sub-sub item
2. Second
3. Third

## Task Lists

- [x] Implement strikethrough support
- [x] Add nested list support
- [x] Create task list parsing
- [x] Unified styled text (TextStyle enum)
- [ ] More inline fragments
- [ ] Comprehensive test suite
  - [x] Full platform coverage (33 platforms)

## Code Blocks

```rust
fn main() {
    println!("Hello from Markdown!");
}
```

## Tables

| Feature                      | Status |
| ---------------------------- | ------ |
| **Bold**                     | ✓      |
| _Italic_                     | ✓      |
| [Links](https://example.com) | ✓      |
| Tables                       | ✓      |
| Equations $\sqrt{x}$         | ✓      |

## Definition Lists

<dl>
<dt>Markdown</dt>
<dd>A lightweight markup language for creating formatted text.</dd>
<dt>Typst</dt>
<dd>A modern typesetting system with powerful features.</dd>
<dt>typub</dt>
<dd>A publishing tool that bridges Markdown/Typst and various platforms.</dd>
</dl>

## Images

Both markdown-native image and HTML img are supported.

<img src="assets/test-image.jpg" width="50%" align="center" title="Image title"/>

![Test Image](assets/test-image.jpg)

<img src="assets/test-image.jpg" width="50%" align="right" title="Image title"/>

## Math Support

Inline math: $E = mc^2$[^fn1]

Display math:

$$
\int_0^\infty e^{-x^2} dx = \frac{\sqrt{\pi}}{2}
$$

### Math Rendering Options

Math equations are rendered via Typst and output as SVG or PNG depending on platform:

| Platform    | Math Rendering |
| ----------- | -------------- |
| Ghost       | SVG inline     |
| Notion      | Equation block |
| Confluence  | SVG attachment |
| WeChat      | PNG image      |
| Xiaohongshu | PNG slide      |

## Quotes

> This is a quote.
>
> - Nested list within a quote.

## Admonition Blocks

> [!NOTE]
>
> This is a note admonition block for important information.

> [!WARNING] Custom Warning
>
> - Be careful when **editing** configuration files!
> - Note that formats can be embedded in admonition blocks!

## Conclusion

Markdown support makes typub more accessible to users familiar with this format. Combined with the 33 supported platforms, typub provides a unified publishing workflow for content creators.

Related reading: [Hello World: Getting Started](../2026-02-09-hello-world/)

[^fn1]: This is a footnote.
