// math-to-string-ir.typ
//
// A precedence-aware math-to-string pipeline:
//   Typst math content -> Math IR -> pretty string
//
// Public API:
// - `math-to-ir(content)`
// - `ir-to-string(ir, mode: "minimal")`
// - `math-to-string-ir(content, mode: "minimal")`

#let _accents = (
  "\u{302}": "hat",
  "\u{303}": "tilde",
  "\u{304}": "macron",
  "\u{301}": "acute",
  "\u{300}": "grave",
  "\u{306}": "breve",
  "\u{307}": "dot",
  "\u{308}": "ddot",
  "\u{20db}": "dddot",
  "\u{20dc}": "ddddot",
  "\u{20d7}": "arrow",
  "\u{20d6}": "arrow.l",
)

#let _body_only = (
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

#let _sizes = (
  display: (math.display, false),
  inline: (math.inline, false),
  script: (math.script, true),
  sscript: (math.sscript, true),
)

#let _clean_repr(value) = {
  let r = repr(value)
  if r.starts-with("[") and r.ends-with("]") {
    return r.slice(1, -1)
  }
  r
}

#let _ir_atom(text) = (k: "atom", text: text)

#let _ir_seq(items) = {
  if items.len() == 0 {
    return _ir_atom("")
  }
  if items.len() == 1 {
    return items.at(0)
  }
  (k: "seq", items: items)
}

#let _ir_group(body, explicit: true) = (k: "group", body: body, explicit: explicit)

#let _ir_frac(num, den) = (k: "frac", num: num, den: den)

#let _ir_attach(base, bl: none, br: none, tl: none, tr: none) = (
  k: "attach",
  base: base,
  bl: bl,
  br: br,
  tl: tl,
  tr: tr,
)

#let _ir_call(name, args: (), kwargs: (:)) = (k: "call", name: name, args: args, kwargs: kwargs)

#let _ir_mat(rows) = (k: "mat", rows: rows)

#let _is_ir(node) = type(node) == dictionary and node.at("k", default: none) != none

#let _to_ir(content) = {
  if content == math.dif {
    return _ir_atom("dif")
  }

  if type(content) == array {
    return _ir_seq(content.map(_to_ir))
  }
  if type(content) == str {
    return _ir_atom(content)
  }
  if content == none {
    return _ir_atom("none")
  }

  let func = content.func()

  if func == text {
    return if content.has("text") { _ir_atom(content.text) } else { _ir_atom(_clean_repr(content)) }
  }

  if func == [].func() {
    let children = content.at("children", default: ())
    return _ir_seq(children.map(_to_ir))
  }

  if func == math.frac {
    return _ir_frac(_to_ir(content.num), _to_ir(content.denom))
  }

  if func == math.attach {
    let bl = if content.has("bl") { _to_ir(content.bl) } else { none }
    let br = if content.has("br") {
      _to_ir(content.br)
    } else if content.has("b") {
      _to_ir(content.b)
    } else {
      none
    }
    let tl = if content.has("tl") { _to_ir(content.tl) } else { none }
    let tr = if content.has("tr") {
      _to_ir(content.tr)
    } else if content.has("t") {
      _to_ir(content.t)
    } else {
      none
    }

    return _ir_attach(
      _to_ir(content.base),
      bl: bl,
      br: br,
      tl: tl,
      tr: tr,
    )
  }

  if func == math.root {
    let radicand = _to_ir(content.radicand)
    if content.has("index") and content.index != none {
      return _ir_call("root", args: (_to_ir(content.index), radicand))
    }
    return _ir_call("sqrt", args: (radicand,))
  }

  // Preserve group semantics from lr(...), printer decides how to render it.
  if func == math.lr {
    return _ir_group(_to_ir(content.body), explicit: true)
  }

  if func == math.accent {
    let base = _to_ir(content.base)
    if content.has("accent") and _accents.keys().contains(content.accent) {
      return _ir_call(_accents.at(content.accent), args: (base,))
    }
    return _ir_call("accent", args: (base, _ir_atom("...")))
  }

  if func == math.op {
    return if content.has("text") { _ir_atom(content.text) } else { _ir_atom(_clean_repr(content)) }
  }

  if func == math.primes {
    return _ir_atom("'" * content.at("count", default: 1))
  }

  if func == math.vec {
    return if content.has("children") {
      _ir_call("vec", args: content.children.map(_to_ir))
    } else {
      _ir_call("vec", args: (_ir_atom("..."),))
    }
  }

  if func == math.mat {
    return if content.has("rows") {
      _ir_mat(content.rows.map(row => row.map(_to_ir)))
    } else {
      _ir_mat(((_ir_atom("..."),),))
    }
  }

  if func == math.cases {
    return if content.has("children") {
      _ir_call("cases", args: content.children.map(_to_ir))
    } else {
      _ir_call("cases", args: (_ir_atom("..."),))
    }
  }

  if func == math.binom {
    return _ir_call("binom", args: (_to_ir(content.upper), _to_ir(content.lower)))
  }

  for (name, f) in _body_only {
    if func == f {
      return _ir_call(name, args: (_to_ir(content.body),))
    }
  }

  for (name, f) in _underover {
    if func == f {
      let body = _to_ir(content.body)
      if content.has("annotation") and content.annotation != none {
        return _ir_call(name, args: (body, _to_ir(content.annotation)))
      }
      return _ir_call(name, args: (body,))
    }
  }

  for (name, (f, cramped-default)) in _sizes {
    if func == f {
      let body = _to_ir(content.body)
      let cramped = content.at("cramped", default: cramped-default)
      if cramped != cramped-default {
        return _ir_call(name, args: (body,), kwargs: (cramped: repr(cramped)))
      }
      return _ir_call(name, args: (body,))
    }
  }

  if func == math.stretch {
    let body = _to_ir(content.body)
    return if content.has("size") {
      _ir_call("stretch", args: (body,), kwargs: (size: repr(content.size)))
    } else {
      _ir_call("stretch", args: (body,))
    }
  }

  if func == math.class {
    return _to_ir(content.body)
  }

  if func == math.equation {
    return _to_ir(content.body)
  }

  if content.has("child") and content.has("styles") {
    return _to_ir(content.child)
  }

  let r = repr(content)
  if r == "align-point()" {
    return _ir_atom(" & ")
  }
  if r == "linebreak()" {
    return _ir_atom(" \\\\ ")
  }

  _ir_atom(_clean_repr(content))
}

#let _ir_kind(node) = if _is_ir(node) { node.k } else { "atom" }

#let _is_atomic_ir(node) = {
  let kind = _ir_kind(node)
  kind == "atom" or kind == "call" or kind == "group"
}

#let _is_parenthesized_text(text) = {
  text.len() >= 2 and text.starts-with("(") and text.ends-with(")")
}

#let _script_atom_needs_wrap(node) = {
  if _ir_kind(node) != "atom" {
    return false
  }
  node.at("text", default: "").len() > 1
}

#let _need_wrap(node, role, mode) = {
  let kind = _ir_kind(node)

  if kind == "group" {
    return false
  }

  if mode == "strict" {
    if role == "base" {
      return not _is_atomic_ir(node)
    }
    if role == "script" {
      return _script_atom_needs_wrap(node) or not (kind == "atom" or kind == "group")
    }
    if role == "frac-num" or role == "frac-den" {
      return not _is_atomic_ir(node)
    }
    return false
  }

  if mode == "readable" {
    if role == "base" {
      return kind == "seq" or kind == "frac"
    }
    if role == "script" {
      return _script_atom_needs_wrap(node) or kind == "seq" or kind == "frac" or kind == "attach"
    }
    if role == "frac-num" or role == "frac-den" {
      return kind == "seq" or kind == "frac"
    }
    return false
  }

  // default: minimal
  if role == "base" {
    return kind == "seq" or kind == "frac"
  }
  if role == "script" {
    return _script_atom_needs_wrap(node) or kind == "seq" or kind == "frac" or kind == "attach"
  }
  if role == "frac-num" or role == "frac-den" {
    return kind == "seq" or kind == "frac"
  }
  false
}

#let _emit_ir(node, mode) = {
  let _emit_in_role = (child, role) => {
    if _ir_kind(child) == "group" {
      let body_raw = _emit_ir(child.body, mode)
      if _is_parenthesized_text(body_raw) {
        return body_raw
      }
      return "(" + body_raw + ")"
    }
    let raw = _emit_ir(child, mode)
    if _need_wrap(child, role, mode) {
      return "(" + raw + ")"
    }
    raw
  }

  let kind = _ir_kind(node)

  if kind == "atom" {
    return node.at("text", default: "")
  }

  if kind == "seq" {
    return node.at("items", default: ()).map(item => _emit_ir(item, mode)).join("")
  }

  if kind == "group" {
    return _emit_ir(node.body, mode)
  }

  if kind == "frac" {
    let num = _emit_in_role(node.num, "frac-num")
    let den = _emit_in_role(node.den, "frac-den")
    return num + "/" + den
  }

  if kind == "attach" {
    let out = _emit_in_role(node.base, "base")

    let bl = node.at("bl", default: none)
    let br = node.at("br", default: none)
    let tl = node.at("tl", default: none)
    let tr = node.at("tr", default: none)

    if bl != none {
      out += "_" + _emit_in_role(bl, "script")
    }
    if br != none {
      out += "_" + _emit_in_role(br, "script")
    }
    if tl != none {
      out += "^" + _emit_in_role(tl, "script")
    }
    if tr != none {
      out += "^" + _emit_in_role(tr, "script")
    }

    return out
  }

  if kind == "call" {
    let args = node.at("args", default: ())
    let kwargs = node.at("kwargs", default: (:))

    let out = ""
    let first = true

    for arg in args {
      if not first {
        out += ", "
      }
      out += _emit_ir(arg, mode)
      first = false
    }

    for (key, value) in kwargs {
      if not first {
        out += ", "
      }
      out += key + ": " + value
      first = false
    }

    return node.name + "(" + out + ")"
  }

  if kind == "mat" {
    let rows = node.at("rows", default: ())
    let body = rows.map(row => row.map(item => _emit_ir(item, mode)).join(", ")).join("; ")
    return "mat(" + body + ")"
  }

  _clean_repr(node)
}

/// Convert math content to intermediate representation.
#let math-to-ir(content) = _to_ir(content)

/// Pretty print IR to Typst math source.
///
/// Modes:
/// - `minimal`: minimal parentheses while preserving binding.
/// - `readable`: slightly more conservative in script/base contexts.
/// - `strict`: preserve more grouping for maximum clarity.
#let ir-to-string(ir, mode: "minimal") = _emit_ir(ir, mode)

/// Convert Typst math content to source string through IR.
#let math-to-string(content, mode: "minimal") = ir-to-string(math-to-ir(content), mode: mode)
