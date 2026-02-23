# Medium

Medium is a popular online publishing platform with a focus on long-form content and thought leadership.

## Capabilities

| Feature        | Support           |
| -------------- | ----------------- |
| Output Format  | HTML (rich text)  |
| Default Theme  | notion            |
| Tables         | **No**            |
| Images         | **No** (manual)   |
| Math Formulas  | **No**            |
| Code Highlight | Partial (no lang) |

### Asset Strategies

| Strategy   | Supported   | Notes                                      |
| ---------- | ----------- | ------------------------------------------ |
| `embed`    | Problematic | Medium rejects base64 images               |
| `external` | Problematic | Medium strips external image URLs on paste |

> ⚠️ Neither `embed` nor `external` strategy works for images on Medium due to platform restrictions.

## Usage

Medium uses the copy-paste workflow:

```bash
# Preview content
typub dev posts/my-post -p medium
```

1. Browser opens with the rendered preview
2. Click **Copy Content** button
3. Open [Medium Editor](https://medium.com/new-story)
4. Paste the content

## Platform Limitations

> ⚠️ **Important**: Medium has significant platform limitations that are **outside typub's control**. Consider using a different platform if your content heavily relies on images, tables, or formulas.

### Images Not Supported

**Both `embed` and `external` strategies fail on Medium:**

| Strategy                | Result                                                     |
| ----------------------- | ---------------------------------------------------------- |
| `embed` (Base64)        | Medium rejects base64 data URIs entirely                   |
| `external` (S3/R2 URLs) | Medium strips `<img>` tags with external URLs during paste |

**Workaround**: After pasting text content, manually upload images through Medium's editor using the image upload button.

### Math Formulas Not Supported

Math rendering does not work on Medium:

- **PNG images**: Stripped along with other images
- **SVG**: Not supported by Medium
- **LaTeX**: Medium has no LaTeX support

**Workaround**:

- Convert formulas to images and manually upload them
- Use Unicode symbols for simple equations (e.g., `×`, `÷`, `²`, `π`)
- Write formulas in plain text notation

### Tables Not Supported

**Medium does not support HTML tables.** Tables will display as raw HTML code or error messages.

**Workaround**: Convert tables to:

- Bullet/numbered lists
- Text descriptions
- Screenshots of rendered tables (then manually upload)

### Code Blocks Lose Language Labels

Syntax highlighting is preserved (inline `<span>` styles), but Medium does not recognize language attributes. Code blocks display with colors but:

- No language indicator
- No syntax-aware editing

**Workaround**: Manually add a caption above code blocks to indicate the language.

## What Works Well

- Headings (H1, H2, H3)
- Paragraphs and text formatting (bold, italic, links)
- Bullet and numbered lists
- Blockquotes
- Horizontal rules
- Inline code (backticks)
- Colored code blocks (no language label)

## Tips

- Keep titles concise—Medium has character limits
- Use headers (H2, H3) for clear structure
- Avoid tables; use lists or text descriptions instead
- Plan to manually upload all images after pasting
- Convert math formulas to images or Unicode symbols
- Add language hints above code blocks for context

## When to Choose Another Platform

Consider using an alternative platform if your content:

- Contains many images
- Requires mathematical formulas
- Uses data tables
- Needs preserved image/asset URLs

Alternatives with better support: Dev.to, Ghost, Hashnode, WordPress
