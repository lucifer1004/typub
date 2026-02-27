//! Tests for Markdown rendering (v2 Document IR).

#![allow(clippy::expect_used)]

use super::*;
use std::collections::BTreeMap;
use typub_html::{
    asset_variant, bullet_list_text, code_block, divider, document, document_with_assets,
    heading_text, image, math_block_latex, numbered_list_text, paragraph_text, parse_html_document,
    quote_text, table_text, task_item, task_list,
};
use typub_ir::{
    AdmonitionKind, Asset, AssetId, AssetRef, AssetSource, Block, BlockAttrs, Document,
    FlowListItem, FlowListItemMarker, FootnoteDef, FootnoteId, HeadingLevel, ImageAsset,
    ImageAttrs, Inline, InlineAttrs, List, ListKind, MathSource, OrderedListMarker, RawOrigin,
    RawTrust, RelativePath, RenderPayload, RenderedArtifact, StyleSet, TaskListItem, TextStyle,
    Url,
};

fn render_blocks(blocks: Vec<Block>) -> String {
    document_to_markdown(&document(blocks)).expect("render markdown")
}

fn render_doc(doc: &Document) -> String {
    document_to_markdown(doc).expect("render markdown")
}

fn render_doc_with_options(doc: &Document, options: &MarkdownRenderOptions<'_>) -> String {
    document_to_markdown_with_options(doc, options).expect("render markdown with options")
}

fn inline_image_doc(src: &str, alt: &str, attrs: ImageAttrs) -> Document {
    let asset_id = AssetId("img-1".to_string());
    let block = Block::Paragraph {
        content: vec![Inline::Image {
            asset: AssetRef(asset_id.clone()),
            alt: alt.to_string(),
            title: None,
            attrs,
        }],
        attrs: BlockAttrs::default(),
    };
    let asset = Asset::Image(ImageAsset {
        source: if src.starts_with("data:") {
            AssetSource::DataUri {
                uri: src.to_string(),
            }
        } else if src.contains("://") || src.starts_with("//") {
            AssetSource::RemoteUrl {
                url: Url(src.to_string()),
            }
        } else {
            let path = RelativePath::new(src.to_string()).expect("valid relative path");
            AssetSource::LocalPath { path }
        },
        meta: None,
        variants: Vec::new(),
    });
    document_with_assets(vec![block], [(asset_id, asset)])
}

#[test]
fn test_heading_levels() {
    for level in 1..=6u8 {
        let md = render_blocks(vec![heading_text(level, "Title")]);
        let prefix = "#".repeat(level as usize);
        assert_eq!(md, format!("{prefix} Title"));
    }
}

#[test]
fn test_heading_builder_clamped_to_valid_range() {
    let md = render_blocks(vec![heading_text(10, "Deep")]);
    assert_eq!(md, "###### Deep");
}

#[test]
fn test_heading_level_type_rejects_invalid_input() {
    assert!(HeadingLevel::new(0).is_err());
    assert!(HeadingLevel::new(7).is_err());
}

#[test]
fn test_paragraph() {
    let md = render_blocks(vec![paragraph_text("Hello world")]);
    assert_eq!(md, "Hello world");
}

#[test]
fn test_paragraph_rich_with_link() {
    let block = Block::Paragraph {
        content: vec![
            Inline::Text("See ".to_string()),
            Inline::Link {
                content: vec![Inline::Text("here".to_string())],
                href: Url("https://example.com".to_string()),
                title: None,
                attrs: InlineAttrs::default(),
            },
            Inline::Text(" for details.".to_string()),
        ],
        attrs: BlockAttrs::default(),
    };
    let md = render_blocks(vec![block]);
    assert_eq!(md, "See [here](https://example.com) for details.");
}

#[test]
fn test_strikethrough() {
    let block = Block::Paragraph {
        content: vec![
            Inline::Text("This is ".to_string()),
            Inline::Styled {
                styles: StyleSet::single(TextStyle::Strikethrough),
                content: vec![Inline::Text("deleted".to_string())],
                attrs: InlineAttrs::default(),
            },
            Inline::Text(" text.".to_string()),
        ],
        attrs: BlockAttrs::default(),
    };
    let md = render_blocks(vec![block]);
    assert_eq!(md, "This is ~~deleted~~ text.");
}

#[test]
fn test_nested_strikethrough_bold() {
    let block = Block::Paragraph {
        content: vec![Inline::Styled {
            styles: StyleSet::single(TextStyle::Strikethrough),
            content: vec![
                Inline::Text("deleted ".to_string()),
                Inline::Styled {
                    styles: StyleSet::single(TextStyle::Bold),
                    content: vec![Inline::Text("and bold".to_string())],
                    attrs: InlineAttrs::default(),
                },
            ],
            attrs: InlineAttrs::default(),
        }],
        attrs: BlockAttrs::default(),
    };
    let md = render_blocks(vec![block]);
    assert_eq!(md, "~~deleted **and bold**~~");
}

#[test]
fn test_code_block_with_language() {
    let md = render_blocks(vec![code_block("fn main() {}", "rust")]);
    assert_eq!(md, "```rust\nfn main() {}\n```");
}

#[test]
fn test_code_block_plain_text_info() {
    let md = render_blocks(vec![code_block("hello", "plain text")]);
    assert_eq!(md, "```plain text\nhello\n```");
}

#[test]
fn test_bullet_list() {
    let md = render_blocks(vec![bullet_list_text(&["Alpha", "Beta"])]);
    assert_eq!(md, "- Alpha\n- Beta");
}

#[test]
fn test_bullet_list_rich() {
    let block = Block::List {
        list: List {
            kind: ListKind::Bullet {
                items: vec![FlowListItem {
                    marker: Some(FlowListItemMarker::Bullet),
                    blocks: vec![Block::Paragraph {
                        content: vec![
                            Inline::Text("Visit ".to_string()),
                            Inline::Link {
                                content: vec![Inline::Text("site".to_string())],
                                href: Url("https://example.com".to_string()),
                                title: None,
                                attrs: InlineAttrs::default(),
                            },
                        ],
                        attrs: BlockAttrs::default(),
                    }],
                }],
            },
        },
        attrs: BlockAttrs::default(),
    };
    let md = render_blocks(vec![block]);
    assert_eq!(md, "- Visit [site](https://example.com)");
}

#[test]
fn test_numbered_list() {
    let md = render_blocks(vec![numbered_list_text(&["First", "Second"])]);
    assert_eq!(md, "1. First\n2. Second");
}

#[test]
fn test_numbered_list_with_start() {
    let block = Block::List {
        list: List {
            kind: ListKind::Numbered {
                start: 3,
                reversed: false,
                marker: None,
                items: vec![
                    FlowListItem {
                        marker: Some(FlowListItemMarker::Number(3)),
                        blocks: vec![paragraph_text("Step one")],
                    },
                    FlowListItem {
                        marker: Some(FlowListItemMarker::Number(4)),
                        blocks: vec![paragraph_text("Step two")],
                    },
                ],
            },
        },
        attrs: BlockAttrs::default(),
    };
    let md = render_blocks(vec![block]);
    assert_eq!(md, "3. Step one\n4. Step two");
}

#[test]
fn test_table() {
    let md = render_blocks(vec![table_text(&["Name", "Value"], &[vec!["foo", "42"]])]);
    assert_eq!(md, "| Name | Value |\n| --- | --- |\n| foo | 42 |");
}

#[test]
fn test_image() {
    let (block, asset) = image("img-1", "https://example.com/img.png", "My image");
    let md = render_doc(&document_with_assets(vec![block], [asset]));
    assert_eq!(md, "![My image](https://example.com/img.png)");
}

#[test]
fn test_inline_image_in_paragraph() {
    let asset_id = AssetId("img-inline".to_string());
    let block = Block::Paragraph {
        content: vec![
            Inline::Text("Inline math: ".to_string()),
            Inline::Image {
                asset: AssetRef(asset_id.clone()),
                alt: "x^2".to_string(),
                title: None,
                attrs: ImageAttrs {
                    width: Some(57),
                    height: Some(10),
                    align: None,
                    passthrough: BTreeMap::new(),
                },
            },
            Inline::Text(" is simple.".to_string()),
        ],
        attrs: BlockAttrs::default(),
    };
    let doc = document_with_assets(
        vec![block],
        [(
            asset_id,
            Asset::Image(ImageAsset {
                source: AssetSource::DataUri {
                    uri: "data:image/png;base64,abc".to_string(),
                },
                meta: None,
                variants: Vec::new(),
            }),
        )],
    );
    let md = render_doc(&doc);
    assert_eq!(
        md,
        "Inline math: ![x^2](data:image/png;base64,abc) is simple."
    );
}

#[test]
fn test_inline_image_with_html_dims() {
    let mut passthrough = BTreeMap::new();
    passthrough.insert(
        "style".to_string(),
        "display:inline;vertical-align:middle".to_string(),
    );
    let doc = inline_image_doc(
        "https://cdn.example.com/math-0.png",
        "x^2",
        ImageAttrs {
            width: Some(57),
            height: Some(10),
            align: None,
            passthrough,
        },
    );
    let options = MarkdownRenderOptions {
        use_inline_html_for_sized_images: true,
        ..Default::default()
    };
    let md = render_doc_with_options(&doc, &options);
    assert_eq!(
        md,
        r#"<img src="https://cdn.example.com/math-0.png" alt="x^2" width="57" height="10" style="display:inline;vertical-align:middle" />"#
    );
}

#[test]
fn test_block_image_with_html_dims() {
    let asset_id = AssetId("img-block".to_string());
    let mut passthrough = BTreeMap::new();
    passthrough.insert(
        "style".to_string(),
        "display:block;margin:0 auto".to_string(),
    );
    let doc = document_with_assets(
        vec![
            paragraph_text("Before block image."),
            Block::Paragraph {
                content: vec![Inline::Image {
                    asset: AssetRef(asset_id.clone()),
                    alt: "integral".to_string(),
                    title: None,
                    attrs: ImageAttrs {
                        width: Some(200),
                        height: Some(50),
                        align: None,
                        passthrough,
                    },
                }],
                attrs: BlockAttrs::default(),
            },
            paragraph_text("After block image."),
        ],
        [(
            asset_id,
            Asset::Image(ImageAsset {
                source: AssetSource::RemoteUrl {
                    url: Url("https://cdn.example.com/block-math.png".to_string()),
                },
                meta: None,
                variants: Vec::new(),
            }),
        )],
    );

    let options = MarkdownRenderOptions {
        use_inline_html_for_sized_images: true,
        ..Default::default()
    };
    let md = render_doc_with_options(&doc, &options);
    assert_eq!(
        md,
        "Before block image.\n\n<img src=\"https://cdn.example.com/block-math.png\" alt=\"integral\" width=\"200\" height=\"50\" style=\"display:block;margin:0 auto\" />\n\nAfter block image."
    );
}

#[test]
fn test_unresolved_asset_uses_fallback_marker() {
    let block = Block::Paragraph {
        content: vec![Inline::Image {
            asset: AssetRef(AssetId("missing".to_string())),
            alt: "missing".to_string(),
            title: None,
            attrs: ImageAttrs::default(),
        }],
        attrs: BlockAttrs::default(),
    };
    let md = render_blocks(vec![block]);
    assert_eq!(md, "<code>[[ASSET:missing]]</code>");
}

#[test]
fn test_asset_url_override_option() {
    let (block, asset) = image("photo-1", "assets/photo.png", "photo");
    let doc = document_with_assets(vec![block], [asset]);
    let mut asset_urls = BTreeMap::new();
    asset_urls.insert(
        AssetId("photo-1".to_string()),
        Url("https://cdn.example.com/photo.png".to_string()),
    );
    let options = MarkdownRenderOptions {
        asset_urls: Some(&asset_urls),
        ..Default::default()
    };
    let md = render_doc_with_options(&doc, &options);
    assert_eq!(md, "![photo](https://cdn.example.com/photo.png)");
}

#[test]
fn test_local_path_asset_without_override() {
    let doc = inline_image_doc("assets/photo.png", "photo", ImageAttrs::default());
    let md = render_doc(&doc);
    assert_eq!(md, "![photo](assets/photo.png)");
}

#[test]
fn test_divider() {
    let md = render_blocks(vec![divider()]);
    assert_eq!(md, "-----");
}

#[test]
fn test_quote() {
    let md = render_blocks(vec![quote_text("To be or not to be")]);
    assert_eq!(md, "> To be or not to be");
}

#[test]
fn test_raw_html_passthrough() {
    let block = Block::RawBlock {
        html: "<details>Custom</details>".to_string(),
        origin: RawOrigin::Markdown,
        trust: RawTrust::Trusted,
        attrs: BlockAttrs::default(),
    };
    let md = render_blocks(vec![block]);
    assert_eq!(md, "<details>Custom</details>");
}

#[test]
fn test_empty_document() {
    let md = render_doc(&document(Vec::new()));
    assert!(md.is_empty());
}

#[test]
fn test_mixed_elements() {
    let md = render_blocks(vec![
        heading_text(1, "Title"),
        paragraph_text("Intro paragraph."),
        divider(),
        bullet_list_text(&["Item A", "Item B"]),
    ]);
    assert_eq!(
        md,
        "# Title\n\nIntro paragraph.\n\n-----\n\n- Item A\n- Item B"
    );
}

#[test]
fn test_inline_math_latex_not_escaped() {
    let block = Block::Paragraph {
        content: vec![
            Inline::Text("The integral ".to_string()),
            Inline::MathInline {
                math: RenderPayload {
                    src: Some(MathSource::Latex(r"\int_0^\infty e^{-x^2} dx".to_string())),
                    rendered: None,
                    id: None,
                },
                attrs: InlineAttrs::default(),
            },
            Inline::Text(" is famous.".to_string()),
        ],
        attrs: BlockAttrs::default(),
    };
    let md = render_blocks(vec![block]);
    assert_eq!(md, r"The integral $\int_0^\infty e^{-x^2} dx$ is famous.");
}

#[test]
fn test_block_math_latex_not_escaped() {
    let md = render_blocks(vec![math_block_latex(r"\frac{\sqrt{\pi}}{2}")]);
    assert_eq!(md, r"$$\frac{\sqrt{\pi}}{2}$$");
}

#[test]
fn test_inline_math_typst_converted_to_latex() {
    let block = Block::Paragraph {
        content: vec![Inline::MathInline {
            math: RenderPayload {
                src: Some(MathSource::Typst("alpha + beta".to_string())),
                rendered: None,
                id: None,
            },
            attrs: InlineAttrs::default(),
        }],
        attrs: BlockAttrs::default(),
    };
    let md = render_blocks(vec![block]);
    let latex = crate::latex::typst_math_to_latex("alpha + beta");
    assert_eq!(md, format!("${latex}$"));
}

#[test]
fn test_block_math_typst_converted_to_latex() {
    let block = Block::MathBlock {
        math: RenderPayload {
            src: Some(MathSource::Typst("sum_(i=1)^n i".to_string())),
            rendered: None,
            id: None,
        },
        attrs: BlockAttrs::default(),
    };
    let md = render_blocks(vec![block]);
    let latex = crate::latex::typst_math_to_latex("sum_(i=1)^n i");
    assert_eq!(md, format!("$${latex}$$"));
}

#[test]
fn test_inline_svg_without_math_source_preserved() {
    let block = Block::Paragraph {
        content: vec![
            Inline::Text("diagram: ".to_string()),
            Inline::SvgInline {
                svg: RenderPayload {
                    src: None,
                    rendered: Some(RenderedArtifact::Svg("<svg>diagram</svg>".to_string())),
                    id: None,
                },
                attrs: InlineAttrs::default(),
            },
        ],
        attrs: BlockAttrs::default(),
    };
    let md = render_blocks(vec![block]);
    assert_eq!(md, "diagram: <svg>diagram</svg>");
}

#[test]
fn test_block_svg_without_math_source_preserved() {
    let block = Block::SvgBlock {
        svg: RenderPayload {
            src: None,
            rendered: Some(RenderedArtifact::Svg("<svg>block</svg>".to_string())),
            id: None,
        },
        attrs: BlockAttrs::default(),
    };
    let md = render_blocks(vec![block]);
    assert_eq!(md, "<svg>block</svg>");
}

#[test]
fn test_paragraph_with_mixed_text_and_math() {
    let block = Block::Paragraph {
        content: vec![
            Inline::Text("Given ".to_string()),
            Inline::MathInline {
                math: RenderPayload {
                    src: Some(MathSource::Latex(r"x = \frac{a}{b}".to_string())),
                    rendered: None,
                    id: None,
                },
                attrs: InlineAttrs::default(),
            },
            Inline::Text(", we have ".to_string()),
            Inline::MathInline {
                math: RenderPayload {
                    src: Some(MathSource::Latex(r"y = \sqrt{x}".to_string())),
                    rendered: None,
                    id: None,
                },
                attrs: InlineAttrs::default(),
            },
            Inline::Text(".".to_string()),
        ],
        attrs: BlockAttrs::default(),
    };
    let md = render_blocks(vec![block]);
    assert_eq!(md, r"Given $x = \frac{a}{b}$, we have $y = \sqrt{x}$.");
}

#[test]
fn test_html_to_markdown_math_e2e() {
    let html = r#"<html><body>
            <p>Inline math: <span class="typst-svg-inline" data-latex-src="E = mc^2"><svg>...</svg></span></p>
            <div class="typst-svg-block" data-latex-src="\int_0^\infty e^{-x^2} dx = \frac{\sqrt{\pi}}{2}"><svg>...</svg></div>
        </body></html>"#;

    let doc = parse_html_document(html).expect("parse html document");
    let md = render_doc(&doc);
    assert_eq!(
        md,
        "Inline math: $E = mc^2$\n\n$$\\int_0^\\infty e^{-x^2} dx = \\frac{\\sqrt{\\pi}}{2}$$"
    );
}

#[test]
fn test_task_list_checked_not_escaped() {
    let md = render_blocks(vec![task_list(vec![
        task_item("Done", true),
        task_item("Todo", false),
    ])]);
    assert_eq!(md, "- [x] Done\n- [ ] Todo");
}

#[test]
fn test_nested_task_list_not_escaped() {
    let nested = Block::List {
        list: List {
            kind: ListKind::Task {
                items: vec![TaskListItem {
                    checked: false,
                    blocks: vec![paragraph_text("Child task")],
                }],
            },
        },
        attrs: BlockAttrs::default(),
    };
    let parent = Block::List {
        list: List {
            kind: ListKind::Task {
                items: vec![TaskListItem {
                    checked: true,
                    blocks: vec![paragraph_text("Parent task"), nested],
                }],
            },
        },
        attrs: BlockAttrs::default(),
    };
    let md = render_blocks(vec![parent]);
    assert_eq!(md, "- [x] Parent task\n  - [ ] Child task");
}

#[test]
fn test_style_set_canonicalization_in_rendering() {
    let styles = StyleSet::new(vec![TextStyle::Italic, TextStyle::Bold, TextStyle::Italic])
        .expect("valid styleset");
    let block = Block::Paragraph {
        content: vec![Inline::Styled {
            styles,
            content: vec![Inline::Text("text".to_string())],
            attrs: InlineAttrs::default(),
        }],
        attrs: BlockAttrs::default(),
    };
    let md = render_blocks(vec![block]);
    assert_eq!(md, "***text***");
}

#[test]
fn test_admonition_renders_comrak_alert() {
    let block = typub_html::admonition_text(AdmonitionKind::Warning, "Careful");
    let md = render_blocks(vec![block]);
    assert_eq!(md, "> [!WARNING]\n> Careful");
}

#[test]
fn test_rendered_asset_math_block_uses_asset_url() {
    let asset_id = AssetId("math-png".to_string());
    let block = Block::MathBlock {
        math: RenderPayload {
            src: None,
            rendered: Some(RenderedArtifact::Asset {
                asset: AssetRef(asset_id.clone()),
                mime: Some("image/png".to_string()),
                width: Some(120),
                height: Some(40),
            }),
            id: None,
        },
        attrs: BlockAttrs::default(),
    };
    let doc = document_with_assets(
        vec![block],
        [(
            asset_id,
            Asset::Image(ImageAsset {
                source: AssetSource::LocalPath {
                    path: RelativePath::new("assets/math.png".to_string()).expect("valid relative"),
                },
                meta: None,
                variants: vec![asset_variant(
                    "original",
                    "https://cdn.example.com/math.png",
                    Some(120),
                    Some(40),
                )],
            }),
        )],
    );
    let md = render_doc(&doc);
    assert_eq!(md, r#"<img src="https://cdn.example.com/math.png" />"#);
}

#[test]
fn test_footnote_reference_matches_definition_name() {
    let mut doc = document(vec![Block::Paragraph {
        content: vec![
            Inline::Text("note".to_string()),
            Inline::FootnoteRef(FootnoteId(1)),
        ],
        attrs: BlockAttrs::default(),
    }]);
    doc.footnotes.insert(
        FootnoteId(1),
        FootnoteDef {
            blocks: vec![paragraph_text("footnote body")],
        },
    );
    let md = render_doc(&doc);
    assert_eq!(md, "note[^fn:1]\n\n[^fn:1]:\n    footnote body");
}

#[test]
fn test_underline_preserves_nested_inline_structure() {
    let block = Block::Paragraph {
        content: vec![Inline::Styled {
            styles: StyleSet::single(TextStyle::Underline),
            content: vec![Inline::Link {
                content: vec![Inline::Code("x".to_string())],
                href: Url("https://example.com".to_string()),
                title: None,
                attrs: InlineAttrs::default(),
            }],
            attrs: InlineAttrs::default(),
        }],
        attrs: BlockAttrs::default(),
    };
    let md = render_blocks(vec![block]);
    assert_eq!(md, "<u>[`x`](https://example.com)</u>");
}

#[test]
fn test_numbered_list_reversed_falls_back_to_html() {
    let block = Block::List {
        list: List {
            kind: ListKind::Numbered {
                start: 3,
                reversed: true,
                marker: None,
                items: vec![
                    FlowListItem {
                        marker: None,
                        blocks: vec![paragraph_text("Three")],
                    },
                    FlowListItem {
                        marker: None,
                        blocks: vec![paragraph_text("Two")],
                    },
                ],
            },
        },
        attrs: BlockAttrs::default(),
    };
    let md = render_blocks(vec![block]);
    assert_eq!(
        md,
        "<ol start=\"3\" reversed>\n<li>Three</li>\n<li>Two</li>\n</ol>"
    );
}

#[test]
fn test_numbered_list_marker_type_falls_back_to_html() {
    let block = Block::List {
        list: List {
            kind: ListKind::Numbered {
                start: 1,
                reversed: false,
                marker: Some(OrderedListMarker::UpperRoman),
                items: vec![
                    FlowListItem {
                        marker: None,
                        blocks: vec![paragraph_text("One")],
                    },
                    FlowListItem {
                        marker: None,
                        blocks: vec![paragraph_text("Two")],
                    },
                ],
            },
        },
        attrs: BlockAttrs::default(),
    };
    let md = render_blocks(vec![block]);
    assert_eq!(md, "<ol type=\"I\">\n<li>One</li>\n<li>Two</li>\n</ol>");
}

#[test]
fn test_numbered_list_item_value_override_falls_back_to_html() {
    let block = Block::List {
        list: List {
            kind: ListKind::Numbered {
                start: 1,
                reversed: false,
                marker: None,
                items: vec![
                    FlowListItem {
                        marker: Some(FlowListItemMarker::Number(10)),
                        blocks: vec![paragraph_text("Ten")],
                    },
                    FlowListItem {
                        marker: Some(FlowListItemMarker::Number(20)),
                        blocks: vec![paragraph_text("Twenty")],
                    },
                ],
            },
        },
        attrs: BlockAttrs::default(),
    };
    let md = render_blocks(vec![block]);
    assert_eq!(
        md,
        "<ol>\n<li value=\"10\">Ten</li>\n<li value=\"20\">Twenty</li>\n</ol>"
    );
}
