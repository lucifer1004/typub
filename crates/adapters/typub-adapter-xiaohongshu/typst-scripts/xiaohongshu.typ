// ============================================================================
// Xiaohongshu (小红书) Theme for Typst
// ============================================================================
// A mobile-first slide theme optimized for Xiaohongshu's 3:5 aspect ratio.
// Features: custom heading decorations, task list styling, code highlighting,
// and table styling with the signature "XiaoHongShu Red" accent color.
// ============================================================================

#import "@preview/codly:1.3.0": *
#import "@preview/codly-languages:0.1.10": *

// ============================================================================
// Font Configuration
// ============================================================================

/// Sans-serif font fallback chain for Chinese text
#let sans-fonts = (
  "PingFang SC",
  "Noto Sans CJK SC",
  "Noto Sans SC",
  "Source Han Sans SC",
  "Heiti SC",
  "SimHei",
)

/// Serif font fallback chain for body text (preferred for readability)
#let serif-fonts = (
  "Noto Serif CJK SC",
  "Noto Serif SC",
  "Source Han Serif SC",
  "SimSun",
)

/// Monospace font fallback chain for code
#let code-fonts = (
  "Maple Mono",
  "Fira Code",
  "Cascadia Code",
  "JetBrains Mono",
  "Monaco",
  "Consolas",
)

// ============================================================================
// Color Palette
// ============================================================================

/// Theme colors matching Xiaohongshu's visual identity
#let colors = (
  bg: rgb("#fffef5"), // Background: warm white (slight yellow tint)
  text: rgb("#333333"), // Primary text: dark gray
  highlight: rgb("#fff6c6"), // Highlight: light yellow for emphasis
  accent: rgb("#ff6b6b"), // Accent: Xiaohongshu signature red
  accent-light: rgb("#ff6b6b").lighten(30%), // Lighten accent color
  footer-bg: rgb("#000000").transparentize(60%), // Footer: semi-transparent black
  white: rgb("#ffffff"), // Pure white for contrast
  code-bg: rgb("#2d2d2d"), // Code block background: dark gray
  code-text: rgb("#f8f8f2"), // Code text: light gray for dark theme
)

// ============================================================================
// Page Layout Constants
// ============================================================================

/// Page dimensions (Xiaohongshu uses 3:5 portrait ratio)
#let page-size = (
  width: 1080pt,
  height: 1800pt,
)

/// Spacing values for consistent layout
#let spacing = (
  margin-x: 80pt, // Horizontal margins
  margin-y: 120pt, // Vertical margins
  title-gap: 40pt, // Gap after title
  para-gap: 20pt, // Gap between paragraphs
  h1-gap: 40pt, // Space above h1
  h2-gap: 30pt, // Space above h2
  h3-gap: 20pt, // Space above h3
  h4-gap: 16pt, // Space above h4
  h3-after: 16pt, // Space below h3
  h4-after: 12pt, // Space below h4
  decor-gap: 16pt, // Gap between decoration bar and text
  decor-gap-sm: 12pt, // Smaller decoration gap
  cover-title-gap: 16pt, // Cover: gap between title elements
  cover-subtitle-gap: 24pt, // Cover: gap after subtitle
)

/// Font sizes scaled for mobile slide format
#let font-sizes = (
  body: 42pt, // Body text
  title: 60pt, // Main title (h1)
  footer: 30pt, // Footer text
  h2: 48pt, // Section headers
  h3: 42pt, // Subsection headers (same as body)
  h4: 38pt, // Smaller headers
  code: 32pt, // Code blocks
  cover-title: 72pt, // Cover: main title
  cover-subtitle: 36pt, // Cover: subtitle
  cover-author: 32pt, // Cover: author name
)

/// Decoration element dimensions
#let decorations = (
  // H1 decorations
  h1-bar-width: 12pt, // Left accent bar width
  h1-bar-height: 1.2em, // Left accent bar height
  h1-bar-radius: 4pt, // Left accent bar corner radius
  h1-underline-offset: 20pt, // Underline position below text
  h1-underline-stroke: 3pt, // Underline thickness
  // H2 decorations
  h2-bar-width: 6pt, // Left accent bar width
  h2-bar-height: 1.1em, // Left accent bar height
  h2-bar-radius: 3pt, // Left accent bar corner radius
  // H3 decorations
  h3-radius: 8pt, // Background box corner radius
  h3-inset-x: 16pt, // Horizontal padding
  h3-inset-y: 8pt, // Vertical padding
  // Inline code
  code-inline-radius: 4pt, // Corner radius
  code-inline-inset-x: 8pt, // Horizontal padding
  code-inline-inset-y: 4pt, // Vertical padding
  code-inline-outset-y: 4pt, // Vertical margin
  // Code blocks
  code-block-radius: 12pt, // Corner radius
  code-block-inset: 24pt, // Inner padding
  // Cover elements
  cover-image-radius: 24pt, // Cover image corner radius
  cover-author-radius: 50pt, // Author badge corner radius (pill shape)
  cover-author-inset-x: 24pt, // Author badge horizontal padding
  cover-author-inset-y: 12pt, // Author badge vertical padding
  // List/Task list markers
  list-marker-width: 1em, // Width reserved for marker in reconstructed list
  list-marker-offset: -1.5em, // Horizontal offset for positioned marker
  list-body-indent: 0.5em, // Indent between marker and body text
  list-checkbox-size: 30pt, // Checkbox width and height
)

// ============================================================================
// Utility Functions
// ============================================================================

/// Extract prefix text from a content node for pattern matching
///
/// Traverses the content tree to extract leading text characters.
/// Used for detecting task list markers ([ ], [x]) at the start of list items.
///
/// # Arguments
/// - `node`: The content node to extract text from
/// - `depth`: Recursion depth (internal use)
///
/// # Returns
/// - String containing the first ~5 characters of text content
#let get-prefix(node, depth: 0, threshold: 5) = {
  if type(node) == str { return node }
  if type(node) != content { return "" }

  if node.has("text") { return node.text }
  if repr(node.func()) == "space" { return " " }

  let res = ""
  if node.has("children") {
    for c in node.children {
      res += get-prefix(c, depth: depth + 1, threshold: threshold)
      if res.len() > threshold { break }
    }
  } else if node.has("child") {
    res += get-prefix(node.child, depth: depth + 1, threshold: threshold)
  } else if node.has("body") {
    res += get-prefix(node.body, depth: depth + 1, threshold: threshold)
  }
  return res
}

// ============================================================================
// Heading Style
// ============================================================================

/// Apply decorative heading styles
///
/// Creates visually distinct heading levels with accent bars, underlines,
/// and highlight backgrounds matching the Xiaohongshu aesthetic.
///
/// # Heading Levels:
/// - H1: Large + left accent bar + underline
/// - H2: Medium + left accent bar + highlight background
/// - H3: Small + rounded highlight background box
/// - H4: Smallest + semibold text
#let heading-style(
  body,
  accent-color: colors.accent,
) = {
  // H1: Large title with accent bar and underline
  show heading.where(level: 1): it => {
    v(spacing.h1-gap)
    box[
      #box(
        baseline: 0.3em,
        width: decorations.h1-bar-width,
        height: decorations.h1-bar-height,
        fill: accent-color,
        radius: decorations.h1-bar-radius,
      )
      #h(spacing.decor-gap)
      #underline(
        text(
          size: font-sizes.title,
          weight: "black",
          fill: colors.text,
          it.body,
        ),
        offset: decorations.h1-underline-offset,
        stroke: decorations.h1-underline-stroke + accent-color,
      )
    ]
  }

  // H2: Medium title with accent bar and highlight
  show heading.where(level: 2): it => {
    v(spacing.h2-gap)
    box[
      #box(
        baseline: 0.2em,
        width: decorations.h2-bar-width,
        height: decorations.h2-bar-height,
        fill: accent-color.lighten(20%),
        radius: decorations.h2-bar-radius,
      )
      #h(spacing.decor-gap-sm)
      #highlight(
        fill: accent-color.lighten(40%).opacify(-50%),
        text(
          size: font-sizes.h2,
          weight: "bold",
          fill: colors.text,
          it.body,
        ),
      )
    ]
  }

  // H3: Small title with rounded highlight box
  show heading.where(level: 3): it => {
    v(spacing.h3-gap)
    box(
      fill: colors.highlight,
      inset: (x: decorations.h3-inset-x, y: decorations.h3-inset-y),
      radius: decorations.h3-radius,
      text(size: font-sizes.h3, weight: "bold", fill: colors.text, it.body),
    )
    v(spacing.h3-after)
  }

  // H4: Smallest title, semibold
  show heading.where(level: 4): it => {
    v(spacing.h4-gap)
    text(size: font-sizes.h4, weight: "semibold", fill: colors.text.lighten(20%), it.body)
    v(spacing.h4-after)
  }

  show heading: it => {
    it
    parbreak()
  }

  body
}

// ============================================================================
// Code Style
// ============================================================================

/// Apply code styling for inline code and code blocks
///
/// Inline code gets a light highlight background.
/// Code blocks use a dark theme with rounded corners.
#let raw-style(body, accent-color: colors.accent) = {
  // Use monospace fonts for code
  show raw: set text(font: code-fonts)
  show: codly-init.with()
  codly(
    number-format: none,
    languages: codly-languages,
    zebra-fill: accent-color.lighten(90%),
  )

  // Inline code: highlighted background with rounded corners
  show raw.where(block: false): box.with(
    fill: colors.highlight,
    inset: (x: decorations.code-inline-inset-x, y: decorations.code-inline-inset-y),
    outset: (y: decorations.code-inline-outset-y),
    radius: decorations.code-inline-radius,
  )

  body
}

// ============================================================================
// Table Style
// ============================================================================

/// Apply table styling with accent header and alternating row colors
///
/// - Header row: accent color background with white bold text
/// - Data rows: alternating between lighter accent shades
/// - No visible borders for a clean, modern look
#let table-style(body, accent-color: colors.accent) = {
  set table(
    stroke: none,
    inset: 18pt,
    column-gutter: 2pt,
    fill: (x, y) => if y == 0 {
      accent-color
    } else if calc.rem(y, 2) == 0 {
      accent-color.lighten(90%)
    } else {
      accent-color.lighten(70%)
    },
  )
  show table.cell.where(y: 0): set text(fill: white, weight: "bold")
  show table.cell.where(y: 0): set align(center)

  body
}

// ============================================================================
// List & Enum Style
// ============================================================================

/// Checked checkbox: filled box with white checkmark
#let checked-box(accent-color: colors.accent) = box(
  width: decorations.list-checkbox-size,
  height: decorations.list-checkbox-size,
  fill: accent-color.lighten(30%),
  radius: 3pt,
  inset: (x: 4pt, y: 2pt),
  align(center + horizon, text(fill: white, weight: "bold")[✓]),
)

/// Unchecked checkbox: outlined empty box
#let unchecked-box(accent-color: colors.accent) = box(
  width: decorations.list-checkbox-size,
  height: decorations.list-checkbox-size,
  stroke: 2pt + accent-color.lighten(30%),
  radius: 3pt,
  inset: (x: 4pt, y: 2pt),
  text()[ ],
)

/// Generate a right-pointing arrow with configurable dimensions
///
/// Creates a deterministic arrow shape that renders consistently across platforms.
///
/// # Arguments
/// - `fill`: Fill color for the arrow
/// - `length`: Total length of the arrow (default: 0.7em)
/// - `shaft-thickness`: Vertical thickness of the shaft (default: 0.1em)
/// - `head-length`: Length of the arrow head (default: 0.3em)
/// - `head-height`: Height of the arrow head (default: 0.4em)
///
/// # Returns
/// - A polygon representing the arrow
#let right-arrow(
  fill: black,
  length: 0.7em,
  shaft-thickness: 0.1em,
  head-length: 0.3em,
  head-height: 0.4em,
) = {
  let shaft-start = 0em
  let shaft-end = length - head-length
  let center = 0.5em - shaft-thickness / 2
  let shaft-half = shaft-thickness / 2

  polygon(
    fill: fill,
    stroke: none,
    (shaft-start, center - shaft-half),
    (shaft-end, center - shaft-half),
    (shaft-end, center - head-height / 2),
    (length, center),
    (shaft-end, center + head-height / 2),
    (shaft-end, center + shaft-half),
    (shaft-start, center + shaft-half),
  )
}

/// Default list marker: arrow for non-task items
#let xhs-marker(accent-color: colors.accent, ..nums) = {
  box(
    width: decorations.list-checkbox-size,
    height: decorations.list-checkbox-size,
    stroke: 0pt,
    right-arrow(
      fill: accent-color.lighten(30%),
      shaft-thickness: 0.3em,
      head-length: 0.3em,
      head-height: 0.6em,
    ),
  )
}

/// Generate enumerated list marker with shape rotation
///
/// Creates markers that cycle through square → circle → triangle shapes,
/// with red background and white number. Matches Xiaohongshu's visual style.
///
/// # Arguments
/// - `index`: 1-based index for the marker number
/// - `size`: Width and height of the marker (default: decorations.list-checkbox-size)
///
/// # Returns
/// - A box containing the shaped marker
#let xhs-enum-marker-fn(..nums, accent-color: colors.accent) = {
  let size = decorations.list-checkbox-size
  let num = text(fill: white, weight: "bold", size: size, str(nums.at(-1)))
  let n = calc.rem(nums.pos().len() - 1, 3)
  if n == 0 {
    square(
      fill: accent-color.lighten(30%),
      size: size,
      align(center + horizon, num),
    )
  } else if n == 1 {
    circle(
      fill: accent-color.lighten(30%),
      radius: size * 0.55,
      align(center + horizon, num),
    )
  } else {
    box(
      width: size,
      height: size,
      stack(
        dir: ltr,
        polygon.regular(
          fill: accent-color.lighten(30%),
          size: size * 1.2,
          vertices: 6,
        ),
        place(dx: -0.85 * size, dy: 0.15 * size, num),
      ),
    )
  }
}

/// Apply list styling with special handling for task lists
///
/// Task lists (containing [ ] or [x] markers) are transformed:
/// - [ ] becomes an unchecked checkbox
/// - [x] or [X] becomes a checked checkbox
/// - Regular items get an arrow marker
///
/// Technical note: We reconstruct the list with custom markers because
/// Typst's `set list(marker: none)` only affects children, not the list
/// element itself. By using a hidden marker and positioning actual markers
/// via `move()`, we achieve the desired visual effect while maintaining
/// proper list structure and spacing.
#let list-style(body, accent-color: colors.accent) = {
  set list(marker: xhs-marker.with(accent-color: accent-color))
  show list: it => {
    // Step 1: Detect if this list contains any task items
    let has-task = false
    for item in it.children {
      let pt = get-prefix(item.body).trim()
      if pt.starts-with("[ ]") or pt.starts-with("[x]") or pt.starts-with("[X]") {
        has-task = true
        break
      }
    }

    // Not a task list, return unchanged
    if not has-task { return it }

    // Step 2: Reconstruct list with custom markers
    let new-children = ()
    for item in it.children {
      let pt = get-prefix(item.body).trim()
      let is-task = false
      let actual-marker = none

      // Determine marker type based on prefix
      if pt.starts-with("[ ]") {
        actual-marker = unchecked-box(accent-color: accent-color)
        is-task = true
      } else if pt.starts-with("[x]") or pt.starts-with("[X]") {
        actual-marker = checked-box(accent-color: accent-color)
        is-task = true
      } else {
        actual-marker = xhs-marker(accent-color: accent-color)
      }

      // Create a hidden marker + positioned actual marker
      let fake-marker = box(width: 0pt, move(dx: decorations.list-marker-offset, actual-marker))

      // For task items, strip the [x]/[ ] prefix from the body
      let clean-body = item.body
      if is-task {
        let seq-len = item.body.children.len()
        clean-body = item.body.children.slice(calc.min(seq-len, 4)).join()
      }

      new-children.push([#fake-marker#clean-body])
    }

    // Return new list with invisible markers (actual markers positioned via move)
    return list(
      marker: box(width: decorations.list-marker-width, []),
      body-indent: decorations.list-body-indent,
      ..new-children,
    )
  }

  body
}

// ============================================================================
// Callout style
// ============================================================================

#let callout-types = (
  ("[!NOTE]", "Note", "ℹ️", rgb("#2780e3")),
  ("[!WARNING]", "Warning", "⚠️", rgb("#ff7518")),
  ("[!TIP]", "Tip", "💡", rgb("#3fb618")),
  ("[!IMPORTANT]", "Important", "❗️", rgb("#ff0039")),
  ("[!CAUTION]", "Caution", "🚧", rgb("#f0ad4e")),
)

#let quarto-callout = (content, title: none, icon: [], paint: rgb("#343a40")) => align(
  center,
  block(
    stroke: (left: paint + 3pt, rest: paint + 0.5pt),
    radius: 3pt,
    clip: true,
    inset: (left: 1.5pt, right: 0.25pt),
    grid(
      columns: (1.5em, 1fr),
      rows: (1.5em, auto),
      fill: (x, y) => if y == 0 { paint.lighten(85%) } else { none },
      align: (center + horizon, left + horizon),
      icon,
      grid.cell(text(strong(title), rgb("#343a40"))),
      grid.cell(colspan: 2, align: left + top, block(width: 100%, content, inset: 0.5em)),
    ),
  ),
)

#let quote-style(body, accent-color: colors.accent) = {
  show quote: it => {
    let pt = get-prefix(it.body, threshold: 20)
    for (prefix, type, icon, paint) in callout-types {
      if pt.starts-with(prefix) {
        let title-pos = it.body.children.position(x => x == parbreak())
        let title = none
        if title-pos != none {
          title = it.body.children.slice(3, title-pos).join()
        }

        return quarto-callout(
          title: title,
          icon: icon,
          paint: paint,
        )[
          #let start-pos = if title-pos != none { title-pos } else { 3 }
          #it.body.children.slice(start-pos).join()
        ]
      }
    }

    block(
      width: 100%,
      fill: accent-color.lighten(90%),
      radius: 12pt,
      inset: (x: 20pt, y: 20pt),
    )[#it.body]
  }

  body
}

// ============================================================================
// Cover Component
// ============================================================================

/// Create a cover slide for Xiaohongshu posts
///
/// This is the first slide in a Xiaohongshu post. Features a centered
/// layout with optional cover image, title, subtitle, and author badge.
///
/// # Arguments
/// - `image-content`: Optional cover image content (default: none)
///   - Can be an `image()` call or any content
///   - Displayed at top with 40% height and rounded corners
/// - `title`: The main title content (default: none)
/// - `subtitle`: Optional subtitle content (default: none)
/// - `author`: Author name for badge (default: none)
///   - Displayed as a pill-shaped badge with accent color background
/// - `accent-color`: Accent color for author badge (default: colors.accent)
///
/// # Example
/// ```typst
/// #cover(
///   image-content: image("cover.jpg"),
///   title: [My Article Title],
///   subtitle: [A brief description],
///   author: "author_name",
/// )
/// ```
#let cover(
  image-content: none,
  title: none,
  subtitle: none,
  author: none,
  accent-color: colors.accent,
) = context {
  set align(center + horizon)
  // Cover image
  if image-content != none {
    block(
      width: 100%,
      height: 40%,
      block(
        clip: true,
        radius: decorations.cover-image-radius,
        image-content,
      ),
    )
  }

  // Title area
  block(
    width: 100%,
  )[
    #if title != none {
      text(
        size: font-sizes.cover-title,
        weight: "black",
        fill: colors.text,
        title,
      )
      v(spacing.cover-title-gap)
    }

    #if subtitle != none {
      text(
        size: font-sizes.cover-subtitle,
        weight: "medium",
        fill: colors.text.lighten(30%),
        subtitle,
      )
      v(spacing.cover-subtitle-gap)
    }

    #if author != none {
      box(
        fill: accent-color,
        inset: (x: decorations.cover-author-inset-x, y: decorations.cover-author-inset-y),
        radius: decorations.cover-author-radius,
        text(fill: colors.white, size: font-sizes.cover-author, weight: "bold", author),
      )
    }
  ]

  pagebreak()
}

// ============================================================================
// Main Theme Function
// ============================================================================

/// Apply the complete Xiaohongshu theme to the document
///
/// This is the main entry point for the theme. Apply it with:
/// ```typst
/// #import "xiaohongshu.typ": rewind-theme
/// #show: rewind-theme
/// ```
///
/// # Configuration Options
/// All parameters have sensible defaults matching Xiaohongshu's style:
/// - `font-family`: Default is serif fonts for better Chinese readability
/// - `bg-color`: Warm white background
/// - `text-color`: Dark gray for body text
/// - `highlight-color`: Light yellow for emphasis
/// - `accent-color`: Xiaohongshu signature red
/// - `page-width/height`: 3:5 aspect ratio (1080×1800pt)
/// - `margin-x/y`: Comfortable margins for mobile reading
/// - `body-size`: Large text for slide format
#let rewind-theme(
  font-family: serif-fonts,
  bg-color: colors.bg,
  text-color: colors.text,
  highlight-color: colors.highlight,
  accent-color: colors.accent,
  page-width: page-size.width,
  page-height: page-size.height,
  margin-x: spacing.margin-x,
  margin-y: spacing.margin-y,
  body-size: font-sizes.body,
  body,
) = {
  // Page configuration
  set page(
    width: page-width,
    height: page-height,
    margin: (x: margin-x, y: margin-y),
    fill: bg-color,
  )

  // Typography defaults
  set text(
    font: font-family,
    size: body-size,
    fill: text-color,
    weight: "regular",
    lang: "zh",
  )

  // Paragraph spacing
  set par(
    leading: 0.8em,
    first-line-indent: 0em,
  )

  // Highlight style (used by heading h2)
  show highlight: set highlight(fill: highlight-color)

  // Apply component styles
  show: raw-style.with(accent-color: accent-color)
  show: heading-style.with(accent-color: accent-color)

  // Link styling
  show link: underline
  show link: set text(fill: accent-color)

  // Table styling
  show: table-style.with(accent-color: accent-color)

  // List and task list styling
  show: list-style.with(accent-color: accent-color)
  show enum: set enum(full: true, numbering: xhs-enum-marker-fn.with(accent-color: accent-color), indent: 0em)

  // Footnote styling
  set footnote(numbering: (..nums) => {
    box[
      #circle(fill: accent-color.lighten(30%), radius: 16pt, align(center + top, text(
        fill: white,
        weight: "bold",
      )[
        #nums.at(-1)
      ]))
    ]
  })

  // Callout styling
  show: quote-style.with(accent-color: accent-color)

  body
}
