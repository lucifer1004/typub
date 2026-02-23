# Static

The `static` adapter generates standalone HTML files that can be directly deployed to static hosting platforms like GitHub Pages, Netlify, and Vercel.

## Platform Features

- Generates complete `index.html` files with embedded styles
- Supports multiple themes
- Directly deployable to static hosting platforms
- Code syntax highlighting support

## Capabilities

| Feature        | Support                               |
| -------------- | ------------------------------------- |
| Tags           | No (static HTML has no metadata)      |
| Categories     | No                                    |
| Internal Links | Yes                                   |
| Draft Support  | None (local output, no draft concept) |
| Math Rendering | SVG / PNG                             |
| Local Output   | Yes                                   |

### Asset Strategies

| Strategy   | Supported | Default | Notes                     |
| ---------- | --------- | ------- | ------------------------- |
| `copy`     | Yes       | \*      | Copy images to assets dir |
| `embed`    | Yes       |         | Base64 inline             |
| `external` | Yes       |         | Use external CDN URLs     |
| `upload`   | No        |         | Not applicable            |

### Math Rendering

| Strategy | Support | Default | Notes                      |
| -------- | ------- | ------- | -------------------------- |
| `svg`    | Yes     | \*      | Vector graphics, scalable  |
| `png`    | Yes     |         | Bitmap, good compatibility |
| `latex`  | No      |         | Requires MathJax runtime   |

## Prerequisites

- [Typst](https://typst.app/) installed (for rendering)

## Configuration

```toml
[platforms.static]
output_dir = "output/static"  # HTML output directory
theme = "minimal"             # Theme name
asset_strategy = "copy"       # Default, copy images locally
```

### Available Themes

| Theme     | Description                                     |
| --------- | ----------------------------------------------- |
| `minimal` | Clean white background, good for technical docs |
| `elegant` | Elegant typography, good for blog posts         |
| `dark`    | Dark theme                                      |

For creating your own theme IDs or overriding built-ins, see [Theme Customization](../../theme-customization.md).

## Content Format

### Output Structure

```
output/static/
└── your-post-slug/
    ├── index.html      # Complete HTML file
    └── assets/         # Image resources (if using copy strategy)
        └── image1.png
```

### HTML Structure

The generated `index.html` includes:

- Complete `<html>`, `<head>`, `<body>` structure
- Inline CSS styles
- Code highlighting (using highlight.js)
- Math formula rendering

## Usage

### Preview

```bash
typub dev posts/my-post -p static
```

Opens a preview page in your browser showing the generated HTML.

### Publish

```bash
typub publish posts/my-post -p static
```

Outputs HTML file to `output/static/{slug}/index.html`.

### Deploy to GitHub Pages

```bash
# 1. Publish content
typub publish posts/ -p static

# 2. Copy output to gh-pages branch
cp -r output/static/* .

# 3. Push to GitHub
git add .
git commit -m "Publish static pages"
git push origin gh-pages
```

### Deploy to Netlify

1. Set `output/static` as your publish directory
2. Or use Netlify CLI:

```bash
netlify deploy --dir=output/static --prod
```

## Example Output

```html
<!DOCTYPE html>
<html>
  <head>
    <meta charset="utf-8" />
    <title>Your Post Title</title>
    <style>
      /* Inline styles */
      body {
        font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto;
        max-width: 800px;
        margin: 0 auto;
        padding: 20px;
      }
      /* ... more styles ... */
    </style>
    <link
      rel="stylesheet"
      href="https://cdn.jsdelivr.net/gh/highlightjs/cdn-release@11.9.0/build/styles/github.min.css"
    />
  </head>
  <body>
    <h1>Your Post Title</h1>
    <p>Content here...</p>
  </body>
</html>
```

## Advanced Configuration

### Custom Theme

```toml
[platforms.static]
theme = "elegant"
```

### Use External Image Hosting

```toml
[platforms.static]
asset_strategy = "external"
```

Requires external storage (S3/R2) configuration. See [External Storage Configuration](../../storage/).

### Output to Project Root

```toml
[platforms.static]
output_dir = "."  # Output directly to current directory
```

## Comparison with Astro Adapter

| Feature       | Static Adapter      | Astro Adapter               |
| ------------- | ------------------- | --------------------------- |
| Output Format | Complete HTML       | Markdown + frontmatter      |
| Deployment    | Direct hosting      | Requires Astro project      |
| Theme Support | Built-in themes     | Controlled by Astro project |
| Content Mgmt  | None                | None                        |
| Use Case      | Simple static sites | Astro Content Collections   |

## Troubleshooting

### Styles not applied

Ensure CSS links in the generated HTML are accessible. If using CDN resources (like highlight.js), you need network connectivity.

### Math formulas appear blank

Check Typst installation and version:

```bash
typst --version
```

### Images not displaying

1. When using `copy` strategy, ensure image paths are correct
2. When using `embed` strategy, check Base64 encoding
3. When using `external` strategy, ensure CDN URLs are accessible

## Related

- [Astro Adapter](../astro/) — Output Markdown for Astro projects
- [External Storage Configuration](../../storage/) — Configure S3/R2 image hosting
