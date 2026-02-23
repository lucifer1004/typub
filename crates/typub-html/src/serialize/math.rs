use super::*;

pub(super) fn serialize_math_inline(
    ctx: &SerializeCtx<'_>,
    math: &MathPayload,
    attrs: &InlineAttrs,
) -> String {
    match &math.rendered {
        Some(RenderedArtifact::Svg(svg)) => {
            let data_attr = math_source_attrs_to_html(&math.src);
            let attr_str = inline_attrs_with_base_class(attrs, "typst-svg-inline");
            let svg_with_style = inject_svg_style(svg, "display:inline;overflow:visible");
            format!("<span{}{}>{}</span>", attr_str, data_attr, svg_with_style)
        }
        Some(RenderedArtifact::MathMl(mathml)) => {
            let data_attr = math_source_attrs_to_html(&math.src);
            let attr_str = inline_attrs_with_base_class(attrs, "typst-mathml-inline");
            format!("<span{}{}>{}</span>", attr_str, data_attr, mathml)
        }
        Some(RenderedArtifact::Asset {
            asset,
            width,
            height,
            ..
        }) => {
            let data_attr = math_source_attrs_to_html(&math.src);
            let attr_str = inline_asset_attrs_with_base_class(
                attrs,
                "typst-math-asset-inline",
                "display:inline;vertical-align:middle;overflow:visible",
            );
            let mut size_attr = String::new();
            if let Some(w) = width {
                size_attr.push_str(&format!(r#" width="{}""#, w));
            }
            if let Some(h) = height {
                size_attr.push_str(&format!(r#" height="{}""#, h));
            }
            let src = resolve_asset_src(asset, ctx.assets).unwrap_or_default();
            format!(
                "<img{} src=\"{}\" alt=\"{}\"{}{}>",
                attr_str,
                escape_html_attr(&src),
                escape_html_attr(&math_source_text(&math.src)),
                size_attr,
                data_attr
            )
        }
        Some(RenderedArtifact::Custom { kind, data }) => {
            let src_attr = math_source_attrs_to_html(&math.src);
            let attr_str = inline_attrs_with_base_class(attrs, "typst-math-custom-inline");
            let payload = match serde_json::to_string(data) {
                Ok(v) => v,
                Err(_) => "{}".to_string(),
            };
            format!(
                "<span data-rendered-kind=\"{}\"{}{}>{}</span>",
                escape_html_attr(kind.as_str()),
                attr_str,
                src_attr,
                escape_html_text(&payload)
            )
        }
        None => {
            let data_attr = math_source_attrs_to_html(&math.src);
            let attr_str = inline_attrs_with_base_class(attrs, "typst-math-inline");
            format!(
                "<span{}{}>{}</span>",
                attr_str,
                data_attr,
                escape_html_text(&math_source_text(&math.src))
            )
        }
    }
}

pub(super) fn serialize_math_block(
    ctx: &SerializeCtx<'_>,
    math: &MathPayload,
    attrs: &BlockAttrs,
) -> String {
    match &math.rendered {
        Some(RenderedArtifact::Svg(svg)) => {
            let data_attr = math_source_attrs_to_html(&math.src);
            let attr_str = block_attrs_with_base_class(attrs, "typst-svg-block");
            let svg_with_style = inject_svg_style(svg, "display:block;overflow:visible");
            format!("<div{}{}>{}</div>", attr_str, data_attr, svg_with_style)
        }
        Some(RenderedArtifact::MathMl(mathml)) => {
            let data_attr = math_source_attrs_to_html(&math.src);
            let attr_str = block_attrs_with_base_class(attrs, "typst-mathml-block");
            format!("<div{}{}>{}</div>", attr_str, data_attr, mathml)
        }
        Some(RenderedArtifact::Asset {
            asset,
            width,
            height,
            ..
        }) => {
            let data_attr = math_source_attrs_to_html(&math.src);
            let attr_str = block_asset_attrs_with_base_class(
                attrs,
                "typst-math-asset-block",
                "display:block;margin:0 auto",
            );
            let mut size_attr = String::new();
            if let Some(w) = width {
                size_attr.push_str(&format!(r#" width="{}""#, w));
            }
            if let Some(h) = height {
                size_attr.push_str(&format!(r#" height="{}""#, h));
            }
            let src = resolve_asset_src(asset, ctx.assets).unwrap_or_default();
            format!(
                "<img{} src=\"{}\" alt=\"{}\"{}{}>",
                attr_str,
                escape_html_attr(&src),
                escape_html_attr(&math_source_text(&math.src)),
                size_attr,
                data_attr
            )
        }
        Some(RenderedArtifact::Custom { kind, data }) => {
            let src_attr = math_source_attrs_to_html(&math.src);
            let attr_str = block_attrs_with_base_class(attrs, "typst-math-custom-block");
            let payload = match serde_json::to_string(data) {
                Ok(v) => v,
                Err(_) => "{}".to_string(),
            };
            format!(
                "<div data-rendered-kind=\"{}\"{}{}>{}</div>",
                escape_html_attr(kind.as_str()),
                attr_str,
                src_attr,
                escape_html_text(&payload)
            )
        }
        None => {
            let data_attr = math_source_attrs_to_html(&math.src);
            let attr_str = block_attrs_with_base_class(attrs, "typst-math-block");
            format!(
                "<div{}{}>{}</div>",
                attr_str,
                data_attr,
                escape_html_text(&math_source_text(&math.src))
            )
        }
    }
}

pub(super) fn serialize_svg_inline(
    ctx: &SerializeCtx<'_>,
    svg: &MathPayload,
    attrs: &InlineAttrs,
) -> String {
    match &svg.rendered {
        Some(RenderedArtifact::Svg(svg_xml)) => {
            let data_attr = math_source_attrs_to_html(&svg.src);
            let attr_str = inline_attrs_with_base_class(attrs, "typst-svg-inline");
            let svg_with_style = inject_svg_style(svg_xml, "display:inline;overflow:visible");
            format!("<span{}{}>{}</span>", attr_str, data_attr, svg_with_style)
        }
        Some(RenderedArtifact::MathMl(mathml)) => {
            let data_attr = math_source_attrs_to_html(&svg.src);
            let attr_str = inline_attrs_with_base_class(attrs, "typst-svg-inline");
            format!("<span{}{}>{}</span>", attr_str, data_attr, mathml)
        }
        Some(RenderedArtifact::Asset {
            asset,
            width,
            height,
            ..
        }) => {
            let data_attr = math_source_attrs_to_html(&svg.src);
            let attr_str = inline_asset_attrs_with_base_class(
                attrs,
                "typst-svg-inline",
                "display:inline;vertical-align:middle;overflow:visible",
            );
            let mut size_attr = String::new();
            if let Some(w) = width {
                size_attr.push_str(&format!(r#" width="{}""#, w));
            }
            if let Some(h) = height {
                size_attr.push_str(&format!(r#" height="{}""#, h));
            }
            let src = resolve_asset_src(asset, ctx.assets).unwrap_or_default();
            format!(
                "<img{} src=\"{}\" alt=\"{}\"{}{}>",
                attr_str,
                escape_html_attr(&src),
                escape_html_attr(&math_source_text(&svg.src)),
                size_attr,
                data_attr
            )
        }
        Some(RenderedArtifact::Custom { kind, data }) => {
            let src_attr = math_source_attrs_to_html(&svg.src);
            let attr_str = inline_attrs_with_base_class(attrs, "typst-svg-inline");
            let payload = match serde_json::to_string(data) {
                Ok(v) => v,
                Err(_) => "{}".to_string(),
            };
            format!(
                "<span data-rendered-kind=\"{}\"{}{}>{}</span>",
                escape_html_attr(kind.as_str()),
                attr_str,
                src_attr,
                escape_html_text(&payload)
            )
        }
        None => {
            let data_attr = math_source_attrs_to_html(&svg.src);
            let attr_str = inline_attrs_with_base_class(attrs, "typst-svg-inline");
            format!(
                "<span{}{}>{}</span>",
                attr_str,
                data_attr,
                escape_html_text(&math_source_text(&svg.src))
            )
        }
    }
}

pub(super) fn serialize_svg_block(
    ctx: &SerializeCtx<'_>,
    svg: &MathPayload,
    attrs: &BlockAttrs,
) -> String {
    match &svg.rendered {
        Some(RenderedArtifact::Svg(svg_xml)) => {
            let data_attr = math_source_attrs_to_html(&svg.src);
            let attr_str = block_attrs_with_base_class(attrs, "typst-svg-block");
            let svg_with_style = inject_svg_style(svg_xml, "display:block;overflow:visible");
            format!("<div{}{}>{}</div>", attr_str, data_attr, svg_with_style)
        }
        Some(RenderedArtifact::MathMl(mathml)) => {
            let data_attr = math_source_attrs_to_html(&svg.src);
            let attr_str = block_attrs_with_base_class(attrs, "typst-svg-block");
            format!("<div{}{}>{}</div>", attr_str, data_attr, mathml)
        }
        Some(RenderedArtifact::Asset {
            asset,
            width,
            height,
            ..
        }) => {
            let data_attr = math_source_attrs_to_html(&svg.src);
            let attr_str = block_asset_attrs_with_base_class(
                attrs,
                "typst-svg-block",
                "display:block;margin:0 auto",
            );
            let mut size_attr = String::new();
            if let Some(w) = width {
                size_attr.push_str(&format!(r#" width="{}""#, w));
            }
            if let Some(h) = height {
                size_attr.push_str(&format!(r#" height="{}""#, h));
            }
            let src = resolve_asset_src(asset, ctx.assets).unwrap_or_default();
            format!(
                "<img{} src=\"{}\" alt=\"{}\"{}{}>",
                attr_str,
                escape_html_attr(&src),
                escape_html_attr(&math_source_text(&svg.src)),
                size_attr,
                data_attr
            )
        }
        Some(RenderedArtifact::Custom { kind, data }) => {
            let src_attr = math_source_attrs_to_html(&svg.src);
            let attr_str = block_attrs_with_base_class(attrs, "typst-svg-block");
            let payload = match serde_json::to_string(data) {
                Ok(v) => v,
                Err(_) => "{}".to_string(),
            };
            format!(
                "<div data-rendered-kind=\"{}\"{}{}>{}</div>",
                escape_html_attr(kind.as_str()),
                attr_str,
                src_attr,
                escape_html_text(&payload)
            )
        }
        None => {
            let data_attr = math_source_attrs_to_html(&svg.src);
            let attr_str = block_attrs_with_base_class(attrs, "typst-svg-block");
            format!(
                "<div{}{}>{}</div>",
                attr_str,
                data_attr,
                escape_html_text(&math_source_text(&svg.src))
            )
        }
    }
}

pub(super) fn math_source_text(src: &Option<MathSource>) -> String {
    match src {
        Some(MathSource::Typst(s)) | Some(MathSource::Latex(s)) => s.clone(),
        Some(MathSource::Custom { src, .. }) => src.clone(),
        None => String::new(),
    }
}

pub(super) fn math_source_attrs_to_html(src: &Option<MathSource>) -> String {
    match src {
        Some(MathSource::Latex(s)) => format!(r#" data-latex-src="{}""#, escape_html_attr(s)),
        Some(MathSource::Typst(s)) => format!(r#" data-typst-src="{}""#, escape_html_attr(s)),
        Some(MathSource::Custom { kind, src }) => format!(
            r#" data-math-kind="{}" data-math-src="{}""#,
            escape_html_attr(kind.as_str()),
            escape_html_attr(src)
        ),
        None => String::new(),
    }
}
