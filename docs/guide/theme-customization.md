# Theme Customization

This guide explains how to customize typub themes at the project level.

## What You Can Customize

- Add new theme IDs (for example `my-brand`)
- Override built-in themes (for example replace `elegant`)
- Override base CSS shared by all themes
- Customize preview-page CSS for specific platforms

## Theme Loading Model

typub loads themes in this order:

1. Built-in themes embedded in the binary
2. Project-local themes from `templates/themes/`

User themes override built-ins when IDs are the same.

## Theme Files Directory

Create this directory in your project root:

```bash
mkdir -p templates/themes
```

Supported files:

- `<theme-id>.css`: a normal theme file (loaded as a theme)
- `_base.css`: optional base override applied before all themes
- `_preview-<platform>.css`: optional preview page style override

Files with names starting with `_` are not treated as themes, except `_base.css` and `_preview-*.css`.

## Quick Start: Create a New Theme

Create `templates/themes/my-brand.css`:

```css
h1 {
  color: #0f4c81;
  border-bottom: 2px solid #0f4c81;
  padding-bottom: 0.25em;
}

h2 {
  color: #17324d;
}

a {
  color: #0f4c81;
}

blockquote {
  border-left: 4px solid #0f4c81;
  background: #f3f8fd;
}

p code,
li code {
  background: #ecf4fc;
  color: #17324d;
}
```

Enable it in `typub.toml`:

```toml
[platforms.wechat]
theme = "my-brand"
```

Preview:

```bash
typub dev posts/my-post -p wechat
```

## Override a Built-in Theme

To override a built-in theme, create a file with the same ID:

- `templates/themes/elegant.css`
- `templates/themes/github.css`
- `templates/themes/notion.css`
- etc.

typub will use your file instead of the embedded one.

## Configure Theme by Scope

### Global default (`typub.toml`)

```toml
theme = "minimal"
```

### Per-platform (`typub.toml`)

```toml
[platforms.wechat]
theme = "wechat-green"
```

### Per-post default (`meta.toml`)

```toml
theme = "elegant"
```

### Per-post per-platform (`meta.toml`)

```toml
[platforms.wechat]
theme = "my-brand"
```

## Theme Resolution Order

Highest priority wins:

1. `meta.toml[platforms.<id>].theme`
2. `meta.toml.theme`
3. `typub.toml[platforms.<id>].theme`
4. `typub.toml.theme`
5. Profile default theme

## Advanced: Override Shared Base CSS

If you provide `templates/themes/_base.css`, typub uses it as the base layer for all themes.

Use this when you want consistent typography/spacing across all theme IDs.

## Advanced: Customize Preview Page Styles

Preview CSS is independent from article theme CSS.

Examples:

- `templates/themes/_preview-copypaste.css`
- `templates/themes/_preview-confluence.css`

Use this for toolbar/layout styling in preview pages.

## Typst Custom Header (Current Model)

typub now supports user-defined Typst preamble via the `preamble` config key.
This key is resolved through the standard configuration chain and then merged
with adapter defaults.

Current hook points in `RenderConfig`:

- `imports`
- `preamble`
- `template_before`
- `template_after`
- `content_transform` (for include/render behavior)

At render time, typub generates a wrapper Typst file in this order:

1. `imports`
2. HTML math rule (when output format is HTML/fragment)
3. `preamble`
4. `template_before`
5. content include/render
6. `template_after`

### What this means for users

- Theme CSS files (`templates/themes/*.css`) customize output style, not Typst
  wrapper header logic.
- You can set `preamble` in config files:
  - `typub.toml[platforms.<id>].preamble`
  - `typub.toml.preamble`
  - `meta.toml[platforms.<id>].preamble`
  - `meta.toml.preamble`
- Resolution order follows RFC-0005 (layer 1 to 4), then adapter default.
- Merge behavior is append-only for compatibility:
  `final_preamble = adapter_preamble + "\\n\\n" + user_preamble` when user
  preamble exists.

Example (`typub.toml`):

```toml
preamble = """
#set text(lang: "zh")
"""

[platforms.wechat]
preamble = """
#set text(size: 11pt)
"""
```

### Adapter examples in this repository

- `confluence` sets `preamble` to disable raw-theme wrapping:
  `#set raw(theme: none)`.
- `xiaohongshu` injects a full preamble (embedded Typst template + `#show` +
  `#cover`) and customizes `content_transform`.

### Contributor entry points

- `crates/typub-adapters-core/src/types.rs` (`RenderConfig`)
- `crates/typub-engine/src/renderer.rs` (`generate_wrapper` injection order)
- each adapter's `render_config()` implementation

## Authoring Tips

- Use direct element selectors (`h1`, `p`, `blockquote`) instead of wrapper-based selectors.
- Keep selectors simple for better inline-style compatibility on copy-paste platforms.
- Test with `typub dev` on your target platform, not only in static HTML output.

## Troubleshooting

### Theme not applied

- Verify `theme = "<id>"` matches filename `<id>.css`
- Verify `templates/themes/` is under the project root where you run `typub`
- Restart `typub dev` after changing theme files

### Built-in style still visible

- Check for syntax errors in your custom CSS file
- Confirm file name exactly matches built-in ID when overriding

## Related Docs

- [Getting Started](./getting-started.md)
- [Adapters](./adapters.md)
- [Advanced Customization](./advanced-customization.md)
- [Platforms Overview](./platforms/README.md)
