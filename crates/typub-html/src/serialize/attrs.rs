use super::*;

pub(super) fn ordered_marker_value(marker: OrderedListMarker) -> &'static str {
    match marker {
        OrderedListMarker::Decimal => "1",
        OrderedListMarker::LowerAlpha => "a",
        OrderedListMarker::UpperAlpha => "A",
        OrderedListMarker::LowerRoman => "i",
        OrderedListMarker::UpperRoman => "I",
    }
}

pub(super) fn admonition_kind_class(kind: &AdmonitionKind) -> String {
    match kind {
        AdmonitionKind::Note => "note".to_string(),
        AdmonitionKind::Tip => "tip".to_string(),
        AdmonitionKind::Warning => "warning".to_string(),
        AdmonitionKind::Danger => "danger".to_string(),
        AdmonitionKind::Info => "info".to_string(),
        AdmonitionKind::Custom(kind) => kind.as_str().replace([':', '/'], "-"),
    }
}

pub(super) fn text_align_css_value(align: TextAlign) -> &'static str {
    match align {
        TextAlign::Left => "left",
        TextAlign::Center => "center",
        TextAlign::Right => "right",
    }
}

pub(super) fn block_attrs_to_html(
    attrs: &BlockAttrs,
    extra: &[(&str, String)],
    skip_passthrough: &[&str],
) -> String {
    attrs_to_html(
        &attrs.classes,
        attrs.style.as_deref(),
        &attrs.passthrough,
        extra,
        skip_passthrough,
    )
}

pub(super) fn inline_attrs_to_html(
    attrs: &InlineAttrs,
    extra: &[(&str, String)],
    skip_passthrough: &[&str],
) -> String {
    attrs_to_html(
        &attrs.classes,
        attrs.style.as_deref(),
        &attrs.passthrough,
        extra,
        skip_passthrough,
    )
}

pub(super) fn inline_attrs_with_base_class(attrs: &InlineAttrs, base_class: &str) -> String {
    let mut classes = vec![base_class.to_string()];
    for cls in &attrs.classes {
        if cls != base_class && !classes.iter().any(|c| c == cls) {
            classes.push(cls.clone());
        }
    }
    attrs_to_html(
        &classes,
        attrs.style.as_deref(),
        &attrs.passthrough,
        &[],
        &["class"],
    )
}

pub(super) fn block_attrs_with_base_class(attrs: &BlockAttrs, base_class: &str) -> String {
    let mut classes = vec![base_class.to_string()];
    for cls in &attrs.classes {
        if cls != base_class && !classes.iter().any(|c| c == cls) {
            classes.push(cls.clone());
        }
    }
    attrs_to_html(
        &classes,
        attrs.style.as_deref(),
        &attrs.passthrough,
        &[],
        &["class"],
    )
}

pub(super) fn inline_asset_attrs_with_base_class(
    attrs: &InlineAttrs,
    base_class: &str,
    default_style: &str,
) -> String {
    let mut classes = vec![base_class.to_string()];
    for cls in &attrs.classes {
        if cls != base_class && !classes.iter().any(|c| c == cls) {
            classes.push(cls.clone());
        }
    }
    let mut passthrough = attrs.passthrough.clone();
    passthrough
        .entry("data-css-inline".to_string())
        .or_insert_with(|| "ignore".to_string());
    let style = merge_style(Some(default_style), attrs.style.as_deref());
    attrs_to_html(
        &classes,
        Some(style.as_str()),
        &passthrough,
        &[],
        &["class", "style"],
    )
}

pub(super) fn block_asset_attrs_with_base_class(
    attrs: &BlockAttrs,
    base_class: &str,
    default_style: &str,
) -> String {
    let mut classes = vec![base_class.to_string()];
    for cls in &attrs.classes {
        if cls != base_class && !classes.iter().any(|c| c == cls) {
            classes.push(cls.clone());
        }
    }
    let mut passthrough = attrs.passthrough.clone();
    passthrough
        .entry("data-css-inline".to_string())
        .or_insert_with(|| "ignore".to_string());
    let style = merge_style(Some(default_style), attrs.style.as_deref());
    attrs_to_html(
        &classes,
        Some(style.as_str()),
        &passthrough,
        &[],
        &["class", "style"],
    )
}

pub(super) fn extra_attrs_to_html(extra: &[(&str, String)]) -> String {
    let mut map = BTreeMap::new();
    for (k, v) in extra {
        map.insert((*k).to_string(), v.clone());
    }

    let mut out = String::new();
    for (k, v) in map {
        out.push_str(&format!(r#" {}="{}""#, k, escape_html_attr(&v)));
    }
    out
}

pub(super) fn attrs_to_html(
    classes: &[String],
    style: Option<&str>,
    passthrough: &BTreeMap<String, String>,
    extra: &[(&str, String)],
    skip_passthrough: &[&str],
) -> String {
    let mut map = BTreeMap::new();

    for (k, v) in passthrough {
        if !skip_passthrough.contains(&k.as_str()) {
            map.insert(k.clone(), v.clone());
        }
    }

    if !classes.is_empty() {
        map.insert("class".to_string(), classes.join(" "));
    }
    if let Some(style_value) = style {
        map.insert("style".to_string(), style_value.to_string());
    }
    for (k, v) in extra {
        map.insert((*k).to_string(), v.clone());
    }

    let mut out = String::new();
    for (k, v) in map {
        out.push_str(&format!(r#" {}="{}""#, k, escape_html_attr(&v)));
    }
    out
}

pub(super) fn merge_style(base: Option<&str>, extra: Option<&str>) -> String {
    let mut pieces = Vec::new();
    if let Some(s) = base
        && !s.trim().is_empty()
    {
        pieces.push(s.trim_end_matches(';').trim().to_string());
    }
    if let Some(s) = extra
        && !s.trim().is_empty()
    {
        pieces.push(s.trim_end_matches(';').trim().to_string());
    }
    pieces.join(";")
}

pub(super) fn inject_svg_style(svg_content: &str, injected_style: &str) -> String {
    let fragment = Html::parse_fragment(svg_content);
    let selector = match Selector::parse("svg") {
        Ok(s) => s,
        Err(_) => return svg_content.to_string(),
    };

    if let Some(svg_element) = fragment.select(&selector).next() {
        let mut attrs = Vec::new();
        let mut has_style = false;

        for (name, value) in svg_element.value().attrs() {
            if name == "style" {
                let merged = merge_style(Some(value), Some(injected_style));
                attrs.push((name.to_string(), merged));
                has_style = true;
            } else {
                attrs.push((name.to_string(), value.to_string()));
            }
        }

        if !has_style {
            attrs.push(("style".to_string(), injected_style.to_string()));
        }

        let mut attr_str = String::new();
        for (name, value) in attrs {
            attr_str.push_str(&format!(r#" {}="{}""#, name, escape_html_attr(&value)));
        }

        return format!("<svg{}>{}</svg>", attr_str, svg_element.inner_html());
    }

    svg_content.to_string()
}
