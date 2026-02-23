# Astro

The `astro` adapter outputs Markdown files with YAML frontmatter, designed for [Astro](https://astro.build/) Content Collections.

## Platform Features

- Outputs Markdown files consumable by Astro projects
- Supports YAML frontmatter (title, date, tags, categories)
- Preserves LaTeX math formulas with `$...$` syntax
- Images copied to local directory with relative paths

## Capabilities

| Feature        | Support                                   |
| -------------- | ----------------------------------------- |
| Tags           | Yes (output to frontmatter)               |
| Categories     | Yes (output to frontmatter)               |
| Internal Links | Yes                                       |
| Draft Support  | None (local output, no draft concept)     |
| Math Rendering | LaTeX only (must preserve formula source) |
| Local Output   | Yes                                       |

### Asset Strategies

| Strategy   | Supported | Default | Notes                     |
| ---------- | --------- | ------- | ------------------------- |
| `copy`     | Yes       | \*      | Copy images to assets dir |
| `embed`    | Yes       |         | Base64 inline             |
| `external` | Yes       |         | Use external CDN URLs     |
| `upload`   | No        |         | Not applicable            |

### Math Rendering

| Strategy | Support | Default | Notes                                    |
| -------- | ------- | ------- | ---------------------------------------- |
| `latex`  | Yes     | \*      | Preserves `$...$` syntax for MathJax     |
| `svg`    | No      |         | SVG cannot reconstruct Markdown formulas |
| `png`    | No      |         | PNG cannot reconstruct Markdown formulas |

> **Note**: The Astro adapter only supports `latex` math rendering mode because Markdown output requires preserving formula source code for proper display.

## Prerequisites

- An [Astro](https://astro.build/) project
- [Typst](https://typst.app/) installed (for rendering)

## Configuration

```toml
[platforms.astro]
output_dir = "output/astro"  # Markdown output directory
asset_strategy = "copy"      # Default, copy images locally
```

## Content Format

### Output Structure

```
output/astro/
└── your-post-slug/
    ├── index.md      # Markdown + YAML frontmatter
    └── assets/       # Image resources (if using copy strategy)
        └── image1.png
```

### Frontmatter Format

The generated `index.md` includes YAML frontmatter:

```yaml
---
title: Your Post Title
date: 2026-02-17
tags:
  - rust
  - typst
categories:
  - programming
---
# Your Post Title

Content here...
```

### Math Formulas

Inline formula: `$E = mc^2$`

Block formula:

```
$$
\int_0^\infty e^{-x^2} dx = \frac{\sqrt{\pi}}{2}
$$
```

## Usage

### Preview

```bash
typub dev posts/my-post -p astro
```

### Publish

```bash
typub publish posts/my-post -p astro
```

Outputs Markdown file to `output/astro/{slug}/index.md`.

### Integrate with Astro Project

Configure the output directory in Astro's Content Collections:

```typescript
// src/content/config.ts
import { defineCollection, z } from "astro:content";
import { glob } from "astro/loaders";

const blog = defineCollection({
  loader: glob({ pattern: "**/index.md", base: "./output/astro" }),
  schema: z.object({
    title: z.string(),
    date: z.date(),
    tags: z.array(z.string()).optional(),
    categories: z.array(z.string()).optional(),
  }),
});

export const collections = { blog };
```

## Using with astro-typst

Besides using typub to generate Markdown, you can also use [astro-typst](https://github.com/OverflowCat/astro-typst) to render Typst content directly in Astro.

### astro-typst Approach

```typescript
// astro-typst renders .typ files directly
import TypstDocument from 'astro-typst';

<TypstDocument src="./content.typ" />
```

### Comparison

| Approach    | Pros                                  | Cons                        |
| ----------- | ------------------------------------- | --------------------------- |
| typub → MD  | Standard Markdown, good compatibility | Extra build step required   |
| astro-typst | Direct rendering, live preview        | Requires Astro plugin setup |

### Recommended Combination

1. **Use typub for content management**: Write in Typst in `posts/` directory
2. **astro-typst for live preview**: Render `.typ` files directly during development
3. **typub for multi-platform publishing**: Also supports Xiaohongshu, WeChat, etc.

## Advanced Configuration

### Custom Slug

Specify in your article's `meta.toml`:

```toml
[platforms.astro]
slug = "custom-url-slug"
```

### Output Directory

```toml
[platforms.astro]
output_dir = "content/blog"  # Output directly to Astro content directory
asset_strategy = "copy"
```

## Troubleshooting

### Math formulas not displaying

Ensure your Astro project has MathJax or KaTeX configured:

```html
<!-- Add to Astro layout -->
<script src="https://polyfill.io/v3/polyfill.min.js?features=es6"></script>
<script
  id="MathJax-script"
  async
  src="https://cdn.jsdelivr.net/npm/mathjax@3/es5/tex-mml-chtml.js"
></script>
```

### Image path issues

When using `copy` strategy, images use relative paths `./assets/image.png`. Ensure your Markdown renderer supports relative paths.

### Tags/Categories not showing

Check your `meta.toml` configuration:

```toml
tags = ["tag1", "tag2"]
categories = ["category1"]
```

## Related

- [Static Adapter](../static/) — Generate standalone HTML files
- [Hashnode Adapter](../hashnode/) — Publish to Hashnode blogging platform
