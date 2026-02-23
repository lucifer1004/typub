// math-to-string.typ
// Recursively converts Typst math content back to Typst source string.
// This enables embedding the source in HTML for later conversion to LaTeX.

// Accent: Unicode combining character -> name
// Note: math.accent stores the accent as a Unicode combining character.
// We use reverse lookup: character -> name for O(1) access.
#let _accents = (
  "\u{302}": "hat",         // Combining circumflex accent
  "\u{303}": "tilde",       // Combining tilde
  "\u{304}": "macron",      // Combining macron
  "\u{301}": "acute",       // Combining acute accent
  "\u{300}": "grave",       // Combining grave accent
  "\u{306}": "breve",       // Combining breve
  "\u{307}": "dot",         // Combining dot above
  "\u{308}": "ddot",        // Combining diaeresis (two dots)
  "\u{20db}": "dddot",      // Combining three dots above
  "\u{20dc}": "ddddot",     // Combining four dots above
  "\u{20d7}": "arrow",      // Combining right arrow above
  "\u{20d6}": "arrow.l",    // Combining left arrow above
)

// Simple body-only handlers: name -> func
#let _body-only = (
  upright: math.upright,
  italic: math.italic,
  bold: math.bold,
  serif: math.serif,
  sans: math.sans,
  frak: math.frak,
  mono: math.mono,
  bb: math.bb,
  cal: math.cal,
  scr: math.scr,
  cancel: math.cancel,
  underline: math.underline,
  overline: math.overline,
)

// Under/over with annotation: name -> func
#let _underover = (
  underbrace: math.underbrace,
  overbrace: math.overbrace,
  underbracket: math.underbracket,
  overbracket: math.overbracket,
  underparen: math.underparen,
  overparen: math.overparen,
  undershell: math.undershell,
  overshell: math.overshell,
)

// Size handlers: name -> (func, cramped-default)
#let _sizes = (
  display: (math.display, false),
  inline: (math.inline, false),
  script: (math.script, true),
  sscript: (math.sscript, true),
)

/// Convert math content to Typst source string.
#let math-to-string(content) = {
  // Special handler for dif operator
  if content.has("children") and content.children.len() == 2 and repr(content.children.at(0)) == "h(amount: 0.17em, weak: true)" and repr(content.children.at(1)) == "styled(child: [d], ..)" {
    return "dif"
  }

  if type(content) == array {
    return content.map(math-to-string).join("")
  }
  if type(content) == str {
    return content
  }
  if content == none {
    return "none"
  }

  let func = content.func()

  // Text
  if func == text {
    return if content.has("text") { content.text } else {
      let r = repr(content)
      if r.starts-with("[") and r.ends-with("]") { r.slice(1, -1) } else { r }
    }
  }

  // Sequence
  if func == [].func() {
    return content.at("children", default: ()).map(math-to-string).join("")
  }

  // Fraction
  if func == math.frac {
    return "(" + math-to-string(content.num) + ")/(" + math-to-string(content.denom) + ")"
  }

  // Attach (subscript/superscript)
  if func == math.attach {
    let base = math-to-string(content.base)
    let result = base

    // Bottom-left subscript
    if content.has("bl") {
      let sub = math-to-string(content.bl)
      result += if sub.len() > 1 { "_(" + sub + ")" } else { "_" + sub }
    }
    // Bottom-right subscript
    if content.has("b") or content.has("br") {
      let sub = math-to-string(content.at("br", default: content.at("b", default: none)))
      result += if sub.len() > 1 { "_(" + sub + ")" } else { "_" + sub }
    }
    // Top-left superscript
    if content.has("tl") {
      let sup = math-to-string(content.tl)
      result += if sup.len() > 1 { "^(" + sup + ")" } else { "^" + sup }
    }
    // Top-right superscript (includes primes)
    if content.has("t") or content.has("tr") {
      let sup = math-to-string(content.at("tr", default: content.at("t", default: none)))
      result += if sup.len() > 1 { "^(" + sup + ")" } else { "^" + sup }
    }
    return result
  }

  // Root
  if func == math.root {
    let radicand = math-to-string(content.radicand)
    if content.has("index") and content.index != none {
      return "root(" + math-to-string(content.index) + ", " + radicand + ")"
    }
    return "sqrt(" + radicand + ")"
  }

  // LR delimiters
  if func == math.lr {
    return math-to-string(content.body)
  }

  // Accent - direct lookup by character
  if func == math.accent {
    let base = math-to-string(content.base)
    if content.has("accent") and _accents.keys().contains(content.accent) {
      return _accents.at(content.accent) + "(" + base + ")"
    }
    return "accent(" + base + ", ...)"
  }

  // Op
  if func == math.op {
    return if content.has("text") { content.text } else { repr(content) }
  }

  // Primes
  if func == math.primes {
    return "'" * content.at("count", default: 1)
  }

  // Vector/Matrix/Cases/Binom - structures with children
  if func == math.vec {
    return if content.has("children") {
      "vec(" + content.children.map(math-to-string).join(", ") + ")"
    } else { "vec(...)" }
  }
  if func == math.mat {
    return if content.has("rows") {
      "mat(" + content.rows.map(row => row.map(math-to-string).join(", ")).join("; ") + ")"
    } else { "mat(...)" }
  }
  if func == math.cases {
    return if content.has("children") {
      "cases(" + content.children.map(math-to-string).join(", ") + ")"
    } else { "cases(...)" }
  }
  if func == math.binom {
    return "binom(" + math-to-string(content.upper) + ", " + math-to-string(content.lower) + ")"
  }

  // Simple body-only handlers
  for (name, f) in _body-only {
    if func == f {
      return name + "(" + math-to-string(content.body) + ")"
    }
  }

  // Under/over with optional annotation
  for (name, f) in _underover {
    if func == f {
      let body = math-to-string(content.body)
      if content.has("annotation") and content.annotation != none {
        return name + "(" + body + ", " + math-to-string(content.annotation) + ")"
      }
      return name + "(" + body + ")"
    }
  }

  // Sizes
  for (name, (f, cramped-default)) in _sizes {
    if func == f {
      let body = math-to-string(content.body)
      let cramped = content.at("cramped", default: cramped-default)
      if cramped != cramped-default {
        return name + "(" + body + ", cramped: " + repr(cramped) + ")"
      }
      return name + "(" + body + ")"
    }
  }

  // Stretch
  if func == math.stretch {
    let body = math-to-string(content.body)
    return if content.has("size") {
      "stretch(" + body + ", size: " + repr(content.size) + ")"
    } else { "stretch(" + body + ")" }
  }

  // Class - just body
  if func == math.class {
    return math-to-string(content.body)
  }

  // Equation
  if func == math.equation {
    return math-to-string(content.body)
  }

  // Styled element (from math.bold, math.italic, etc.)
  // Note: We cannot detect the specific style at runtime since styles is opaque.
  // We just pass through the child content.
  // This means bold(x) -> x (style info lost but content preserved)
  if content.has("child") and content.has("styles") {
    return math-to-string(content.child)
  }

  // Fallback - check for special internal representations
  let r = repr(content)

  // Detect alignment point (&) - internally represented as align-point()
  if r == "align-point()" {
    return " & "
  }

  // Detect line break (\\) - internally represented as linebreak()
  if r == "linebreak()" {
    return " \\ "
  }

  // Default fallback
  return if r.starts-with("[") and r.ends-with("]") { r.slice(1, -1) } else { r }
}
