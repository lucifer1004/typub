// ============================================================================
// cmarker Configuration Module
// ============================================================================
// Provides configurable cmarker.render() wrapper for Markdown content.
// This module handles:
// - Image path resolution (scope.image)
// - Math rendering (scope.math)
// - HTML <img> element handling (html.img)
// ============================================================================
#import "@preview/mitex:0.2.6": mitex, mi
#import "@preview/cmarker:0.1.8"

/// Parse a CSS-style length value to Typst length.
/// Supports: "50%", "100px", "2em", "50pt"
#let parse-length(value) = {
  if value == none { return none }
  let v-str = str(value).trim()
  if v-str == "" { return none }

  // Percentage: "50%" → 50%
  if v-str.ends-with("%") {
    let num = float(v-str.trim("%", at: end))
    return num * 1%
  }
  // Pixels: "100px" → 75pt (96px = 72pt)
  if v-str.ends-with("px") {
    let num = float(v-str.trim("px", at: end))
    return num * 0.75pt
  }
  // Points: "50pt" → 50pt
  if v-str.ends-with("pt") {
    return float(v-str.trim("pt", at: end)) * 1pt
  }
  // Em: "2em" → relative (approximate as 12pt * value)
  if v-str.ends-with("em") {
    let num = float(v-str.trim("em", at: end))
    return num * 1em
  }
  // Plain number: treat as pixels
  let num = float(v-str)
  return num * 0.75pt
}

/// Parse alignment attribute to Typst alignment.
/// Supports: "left", "right", "center", "middle"
#let parse-align(align-value) = {
  if align-value == none { return center }
  let a = lower(str(align-value).trim())
  if a == "left" { return left }
  if a == "right" { return right }
  center
}

/// Parse inline style attribute for additional image properties.
/// Returns dict with: width, height
#let parse-style(style) = {
  if style == none { return (:) }
  let result = (:)
  let styles = str(style).split(";")
  for s in styles {
    let parts = s.split(":")
    if parts.len() == 2 {
      let key = lower(parts.at(0).trim())
      let val = parts.at(1).trim()
      if key == "width" {
        result.insert("width", parse-length(val))
      } else if key == "height" {
        result.insert("height", parse-length(val))
      }
    }
  }
  result
}

/// Create image scope function for Markdown images: ![](path)
///
/// # Arguments
/// - `content-dir`: Base directory for relative image paths
/// - `as-marker`: If true, emit raw <img> placeholders for deferred asset pipeline
///
/// # Returns
/// A function suitable for cmarker's scope.image parameter
#let make-image-scope(content-dir, image-as-marker: false) = {
  if image-as-marker {
    // Emit raw HTML img placeholder (no local file loading in Typst render stage).
    (source, alt: none, format: auto) => {
      let attrs = ("src": source)
      if alt != none and str(alt) != "" {
        attrs.insert("alt", str(alt))
      }
      html.elem("img", attrs: attrs)
    }
  } else {
    // Direct image loading with path resolution
    (source, alt: none, format: auto) => image("/" + content-dir + "/" + source, alt: alt, format: format)
  }
}

/// Create HTML img handler for raw HTML <img> tags
///
/// Priority:
/// 1. If image-as-marker=true: keep raw <img> placeholders for asset pipeline
/// 2. Otherwise (PNG/PDF output): convert to Typst image() function
///
/// # Arguments
/// - `content-dir`: Base directory for relative image paths
/// - `as-marker`: If true, keep raw <img> placeholders for asset pipeline
///
/// # Returns
/// A cmarker html.img configuration tuple
#let make-img-handler(content-dir, image-as-marker: false) = {
  // Priority 1: Marker output takes precedence (for asset pipeline)
  if image-as-marker {
    // Keep raw HTML img node so downstream parser can register asset source from src.
    ("void", attrs => {
      let image-title = attrs.at("title", default: none)
      let img = html.elem("img", attrs: attrs)
      if image-title != none {
        figure(
          img,
          kind: image,
          caption: image-title,
        )
      } else {
        img
      }
    })
  } else {
    // Convert to Typst image()
    ("void", attrs => {
      let src = attrs.at("src", default: "")
      let alt-text = attrs.at("alt", default: "")

      // Parse width: from attribute or style
      let w = parse-length(attrs.at("width", default: none))
      if w == none {
        let style-props = parse-style(attrs.at("style", default: none))
        w = style-props.at("width", default: none)
      }
      if w == none {
        w = auto
      }

      // Parse height: from attribute or style
      let h = parse-length(attrs.at("height", default: none))
      if h == none {
        let style-props = parse-style(attrs.at("style", default: none))
        h = style-props.at("height", default: none)
      }
      if h == none {
        h = auto
      }

      // Parse title:
      let image-title = attrs.at("title", default: none)

      // Parse alignment
      let align-val = parse-align(attrs.at("align", default: none))

      // Build image with resolved parameters
      let img = image(
        "/" + content-dir + "/" + src,
        width: w,
        height: h,
        alt: if alt-text != "" { alt-text } else { none },
      )
      if image-title != none {
        img = figure(img, caption: image-title, kind: image)
      }

      img
    })
  }
}

/// Create math callback for equation rendering
///
/// # Arguments
/// - `mode`: "latex" for LaTeX data attributes, "svg" for direct SVG
///
/// # Returns
/// A function suitable for cmarker's math parameter
#let make-math-callback(mode: "svg") = {
  if mode == "latex" {
    (it, block: false) => {
      let rendered = if block { mitex(it) } else { mi(it) }
      if block {
        html.elem("div", attrs: (class: "typst-svg-block", "data-latex-src": it), html.frame(rendered))
      } else {
        html.elem("span", attrs: (class: "typst-svg-inline", "data-latex-src": it), html.frame(rendered))
      }
    }
  } else {
    (it, block: false) => if block { mitex(it) } else { mi(it) }
  }
}

/// Render Markdown content with cmarker
///
/// # Arguments
/// - `markdown-path`: Path to the Markdown file
/// - `content-dir`: Base directory for relative paths
/// - `math-mode`: "latex" or "svg"
/// - `image-as-marker`: If true, keep raw `<img>` placeholders for deferred asset pipeline
///
/// # Returns
/// Rendered content
#let render-md(
  markdown-path,
  content-dir: "",
  math-mode: "svg",
  image-as-marker: false,
) = {
  let image-scope = make-image-scope(content-dir, image-as-marker: image-as-marker)
  let math-callback = make-math-callback(mode: math-mode)
  let img-handler = make-img-handler(
    content-dir,
    image-as-marker: image-as-marker
  )

  cmarker.render(
    read(markdown-path),
    math: math-callback,
    scope: (image: image-scope),
    html: (img: img-handler,),
  )
}
