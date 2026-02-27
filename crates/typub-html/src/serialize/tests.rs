#![allow(clippy::expect_used)]

use super::*;
use typub_ir::{
    AssetId, AssetRef, AssetSource, Block, BlockAttrs, Document, FlowListItem, FlowListItemMarker,
    FootnoteDef, HeadingLevel, ImageAsset, ImageAttrs, Inline, List, ListKind, MathPayload,
    MathSource, RenderedMath, StyleSet, TextStyle, Url,
};

fn empty_doc(blocks: Vec<Block>) -> Document {
    Document {
        blocks,
        footnotes: BTreeMap::new(),
        assets: BTreeMap::new(),
        meta: Default::default(),
    }
}

#[test]
fn serialize_basic_heading_and_paragraph() {
    let level = HeadingLevel::new(2).expect("valid level");
    let doc = empty_doc(vec![
        Block::Heading {
            level,
            id: None,
            content: vec![Inline::Text("Title".to_string())],
            attrs: BlockAttrs::default(),
        },
        Block::Paragraph {
            content: vec![
                Inline::Text("Hello ".to_string()),
                Inline::Code("world".to_string()),
            ],
            attrs: BlockAttrs::default(),
        },
    ]);

    let html = document_to_html(&doc);
    assert!(html.contains("<h2>Title</h2>"));
    assert!(html.contains("<p>Hello <code>world</code></p>"));
}

#[test]
fn serialize_code_block_highlight_option() {
    let doc = empty_doc(vec![Block::CodeBlock {
        code: "fn main() {}".to_string(),
        language: Some("rust".to_string()),
        filename: None,
        highlight_lines: vec![],
        highlighted_html: Some("<span>hl</span>".to_string()),
        attrs: BlockAttrs::default(),
    }]);

    let plain = document_to_html(&doc);
    assert!(plain.contains("fn main() {}"));

    let highlighted = document_to_html_with_options(
        &doc,
        &SerializeOptions {
            use_code_highlight: true,
            ..Default::default()
        },
    );
    assert!(highlighted.contains("<span>hl</span>"));
}

#[test]
fn serialize_math_inline_and_block_svg() {
    let doc = empty_doc(vec![
        Block::Paragraph {
            content: vec![Inline::MathInline {
                math: MathPayload {
                    src: Some(MathSource::Latex("x+y".to_string())),
                    rendered: Some(RenderedMath::Svg(
                        "<svg viewBox=\"0 0 10 10\"><path/></svg>".to_string(),
                    )),
                    id: None,
                },
                attrs: Default::default(),
            }],
            attrs: BlockAttrs::default(),
        },
        Block::MathBlock {
            math: MathPayload {
                src: Some(MathSource::Latex("x^2".to_string())),
                rendered: Some(RenderedMath::Svg(
                    "<svg viewBox=\"0 0 10 10\"><path/></svg>".to_string(),
                )),
                id: None,
            },
            attrs: BlockAttrs::default(),
        },
    ]);

    let html = document_to_html(&doc);
    assert!(html.contains("typst-svg-inline"));
    assert!(html.contains("typst-svg-block"));
    assert!(html.contains("data-latex-src=\"x+y\""));
    assert!(html.contains("data-latex-src=\"x^2\""));
}

#[test]
fn serialize_math_does_not_duplicate_class_attribute() {
    let doc = empty_doc(vec![
        Block::Paragraph {
            content: vec![Inline::MathInline {
                math: MathPayload {
                    src: Some(MathSource::Latex("x".to_string())),
                    rendered: Some(RenderedMath::Svg("<svg>...</svg>".to_string())),
                    id: None,
                },
                attrs: InlineAttrs {
                    classes: vec!["typst-svg-inline".to_string(), "extra".to_string()],
                    style: None,
                    passthrough: BTreeMap::new(),
                },
            }],
            attrs: BlockAttrs::default(),
        },
        Block::MathBlock {
            math: MathPayload {
                src: Some(MathSource::Latex("y".to_string())),
                rendered: Some(RenderedMath::Svg("<svg>...</svg>".to_string())),
                id: None,
            },
            attrs: BlockAttrs {
                classes: vec!["typst-svg-block".to_string(), "extra".to_string()],
                style: None,
                passthrough: BTreeMap::new(),
            },
        },
    ]);

    let html = document_to_html(&doc);
    assert!(!html.contains("class=\"typst-svg-inline\" class="));
    assert!(!html.contains("class=\"typst-svg-block\" class="));
    assert!(html.contains("class=\"typst-svg-inline extra\""));
    assert!(html.contains("class=\"typst-svg-block extra\""));
}

#[test]
fn serialize_math_png_does_not_duplicate_class_attribute() {
    let doc = empty_doc(vec![
        Block::Paragraph {
            content: vec![Inline::MathInline {
                math: MathPayload {
                    src: Some(MathSource::Latex("x".to_string())),
                    rendered: Some(RenderedMath::Asset {
                        asset: AssetRef(AssetId("png-inline".to_string())),
                        mime: Some("image/png".to_string()),
                        width: Some(10),
                        height: Some(8),
                    }),
                    id: None,
                },
                attrs: InlineAttrs {
                    classes: vec!["typst-math-asset-inline".to_string(), "extra".to_string()],
                    style: None,
                    passthrough: BTreeMap::new(),
                },
            }],
            attrs: BlockAttrs::default(),
        },
        Block::MathBlock {
            math: MathPayload {
                src: Some(MathSource::Latex("y".to_string())),
                rendered: Some(RenderedMath::Asset {
                    asset: AssetRef(AssetId("png-block".to_string())),
                    mime: Some("image/png".to_string()),
                    width: Some(12),
                    height: Some(9),
                }),
                id: None,
            },
            attrs: BlockAttrs {
                classes: vec!["typst-math-asset-block".to_string(), "extra".to_string()],
                style: None,
                passthrough: BTreeMap::new(),
            },
        },
    ]);

    let html = document_to_html(&doc);
    assert!(!html.contains("class=\"typst-math-asset-inline\" class="));
    assert!(!html.contains("class=\"typst-math-asset-block\" class="));
    assert!(html.contains("class=\"typst-math-asset-inline extra\""));
    assert!(html.contains("class=\"typst-math-asset-block extra\""));
}

#[test]
fn serialize_math_png_preserves_display_semantics() {
    let mut doc = empty_doc(vec![
        Block::Paragraph {
            content: vec![Inline::MathInline {
                math: MathPayload {
                    src: Some(MathSource::Latex("x".to_string())),
                    rendered: Some(RenderedMath::Asset {
                        asset: AssetRef(AssetId("png-inline".to_string())),
                        mime: Some("image/png".to_string()),
                        width: Some(10),
                        height: Some(8),
                    }),
                    id: None,
                },
                attrs: InlineAttrs::default(),
            }],
            attrs: BlockAttrs::default(),
        },
        Block::MathBlock {
            math: MathPayload {
                src: Some(MathSource::Latex("y".to_string())),
                rendered: Some(RenderedMath::Asset {
                    asset: AssetRef(AssetId("png-block".to_string())),
                    mime: Some("image/png".to_string()),
                    width: Some(12),
                    height: Some(9),
                }),
                id: None,
            },
            attrs: BlockAttrs::default(),
        },
    ]);
    doc.assets.insert(
        AssetId("png-inline".to_string()),
        Asset::Image(ImageAsset {
            source: AssetSource::DataUri {
                uri: "data:image/png;base64,AAA=".to_string(),
            },
            meta: None,
            variants: Vec::new(),
        }),
    );
    doc.assets.insert(
        AssetId("png-block".to_string()),
        Asset::Image(ImageAsset {
            source: AssetSource::DataUri {
                uri: "data:image/png;base64,BBB=".to_string(),
            },
            meta: None,
            variants: Vec::new(),
        }),
    );

    let html = document_to_html(&doc);
    assert!(html.contains("class=\"typst-math-asset-inline\""));
    assert!(html.contains("class=\"typst-math-asset-block\""));
    assert!(html.contains("data-css-inline=\"ignore\""));
    assert!(html.contains("display:inline;vertical-align:middle;overflow:visible"));
    assert!(html.contains("display:block;margin:0 auto"));
}

#[test]
fn serialize_svg_asset_uses_svg_classes_not_math_classes() {
    let mut doc = empty_doc(vec![
        Block::Paragraph {
            content: vec![Inline::SvgInline {
                svg: MathPayload {
                    src: None,
                    rendered: Some(RenderedMath::Asset {
                        asset: AssetRef(AssetId("svg-inline".to_string())),
                        mime: Some("image/png".to_string()),
                        width: Some(10),
                        height: Some(8),
                    }),
                    id: None,
                },
                attrs: InlineAttrs::default(),
            }],
            attrs: BlockAttrs::default(),
        },
        Block::SvgBlock {
            svg: MathPayload {
                src: None,
                rendered: Some(RenderedMath::Asset {
                    asset: AssetRef(AssetId("svg-block".to_string())),
                    mime: Some("image/png".to_string()),
                    width: Some(12),
                    height: Some(9),
                }),
                id: None,
            },
            attrs: BlockAttrs::default(),
        },
    ]);
    doc.assets.insert(
        AssetId("svg-inline".to_string()),
        Asset::Image(ImageAsset {
            source: AssetSource::DataUri {
                uri: "data:image/png;base64,AAA=".to_string(),
            },
            meta: None,
            variants: Vec::new(),
        }),
    );
    doc.assets.insert(
        AssetId("svg-block".to_string()),
        Asset::Image(ImageAsset {
            source: AssetSource::DataUri {
                uri: "data:image/png;base64,BBB=".to_string(),
            },
            meta: None,
            variants: Vec::new(),
        }),
    );

    let html = document_to_html(&doc);
    assert!(html.contains("class=\"typst-svg-inline\""));
    assert!(html.contains("class=\"typst-svg-block\""));
    assert!(!html.contains("typst-math-asset-inline"));
    assert!(!html.contains("typst-math-asset-block"));
    assert!(html.contains("display:inline;vertical-align:middle;overflow:visible"));
    assert!(html.contains("display:block;margin:0 auto"));
}

#[test]
fn serialize_attrs_passthrough_is_deterministic() {
    let mut block_passthrough = BTreeMap::new();
    block_passthrough.insert("z-k".to_string(), "1".to_string());
    block_passthrough.insert("a-k".to_string(), "2".to_string());

    let mut inline_passthrough = BTreeMap::new();
    inline_passthrough.insert("z-i".to_string(), "1".to_string());
    inline_passthrough.insert("a-i".to_string(), "2".to_string());

    let doc = empty_doc(vec![Block::Paragraph {
        content: vec![Inline::Link {
            content: vec![Inline::Text("x".to_string())],
            href: Url("https://example.com".to_string()),
            title: None,
            attrs: InlineAttrs {
                classes: Vec::new(),
                style: None,
                passthrough: inline_passthrough,
            },
        }],
        attrs: BlockAttrs {
            classes: Vec::new(),
            style: None,
            passthrough: block_passthrough,
        },
    }]);

    let html = document_to_html(&doc);
    let p_idx = html.find("<p ").expect("paragraph with attrs");
    let p_end = html[p_idx..].find('>').expect("paragraph close tag") + p_idx;
    let p_tag = &html[p_idx..=p_end];
    assert!(
        p_tag.find("a-k=\"2\"").expect("a-k in p") < p_tag.find("z-k=\"1\"").expect("z-k in p")
    );

    let a_idx = html.find("<a ").expect("anchor with attrs");
    let a_end = html[a_idx..].find('>').expect("anchor close tag") + a_idx;
    let a_tag = &html[a_idx..=a_end];
    assert!(
        a_tag.find("a-i=\"2\"").expect("a-i in a") < a_tag.find("z-i=\"1\"").expect("z-i in a")
    );
}

#[test]
fn serialize_figure_and_table_sections() {
    let doc = empty_doc(vec![
        Block::Figure {
            content: vec![Block::Paragraph {
                content: vec![Inline::Text("img".to_string())],
                attrs: BlockAttrs::default(),
            }],
            caption: Some(vec![Block::Paragraph {
                content: vec![Inline::Text("cap".to_string())],
                attrs: BlockAttrs::default(),
            }]),
            attrs: BlockAttrs::default(),
        },
        Block::Table {
            caption: None,
            sections: vec![],
            attrs: BlockAttrs::default(),
        },
    ]);

    let html = document_to_html(&doc);
    assert!(html.contains("<figure>"));
    assert!(html.contains("<figcaption>"));
    assert!(html.contains("<table>"));
}

#[test]
fn serialize_definition_list_paragraph_fallback() {
    let item = DefinitionItem {
        terms: vec![vec![Block::Paragraph {
            content: vec![Inline::Text("Term".to_string())],
            attrs: BlockAttrs::default(),
        }]],
        definitions: vec![vec![Block::Paragraph {
            content: vec![Inline::Text("Definition".to_string())],
            attrs: BlockAttrs::default(),
        }]],
    };

    let doc = empty_doc(vec![Block::DefinitionList {
        items: vec![item],
        attrs: BlockAttrs::default(),
    }]);

    let html = document_to_html_with_options(
        &doc,
        &SerializeOptions {
            definition_list_to_paragraph: true,
            ..Default::default()
        },
    );
    assert!(html.contains("<p><strong>Term</strong>: Definition</p>"));
}

#[test]
fn serialize_definition_list_paragraph_fallback_preserves_inline_markup() {
    let item = DefinitionItem {
        terms: vec![vec![Block::Paragraph {
            content: vec![Inline::Styled {
                styles: StyleSet::single(TextStyle::Italic),
                content: vec![Inline::Text("Term".to_string())],
                attrs: InlineAttrs::default(),
            }],
            attrs: BlockAttrs::default(),
        }]],
        definitions: vec![vec![Block::Paragraph {
            content: vec![
                Inline::Text("See ".to_string()),
                Inline::Link {
                    content: vec![Inline::Text("docs".to_string())],
                    href: Url("https://example.com".to_string()),
                    title: None,
                    attrs: InlineAttrs::default(),
                },
            ],
            attrs: BlockAttrs::default(),
        }]],
    };

    let doc = empty_doc(vec![Block::DefinitionList {
        items: vec![item],
        attrs: BlockAttrs::default(),
    }]);

    let html = document_to_html_with_options(
        &doc,
        &SerializeOptions {
            definition_list_to_paragraph: true,
            ..Default::default()
        },
    );
    assert!(html.contains(
        "<p><strong><em>Term</em></strong>: See <a href=\"https://example.com\">docs</a></p>"
    ));
}

#[test]
fn serialize_footnotes_and_refs() {
    let mut footnotes = BTreeMap::new();
    footnotes.insert(
        typub_ir::FootnoteId(1),
        FootnoteDef {
            blocks: vec![Block::Paragraph {
                content: vec![Inline::Text("note".to_string())],
                attrs: BlockAttrs::default(),
            }],
        },
    );
    let doc = Document {
        blocks: vec![Block::Paragraph {
            content: vec![Inline::FootnoteRef(typub_ir::FootnoteId(1))],
            attrs: BlockAttrs::default(),
        }],
        footnotes,
        assets: BTreeMap::new(),
        meta: Default::default(),
    };

    let html = document_to_html(&doc);
    assert!(html.contains("id=\"fnref-1\""));
    assert!(html.contains("id=\"fn-1\""));
    assert!(html.contains("class=\"footnotes\""));
    assert!(html.contains("<p>note<a href=\"#fnref-1\">↩</a></p>"));
}

#[test]
fn serialize_footnotes_does_not_duplicate_existing_backlink() {
    let mut footnotes = BTreeMap::new();
    footnotes.insert(
        typub_ir::FootnoteId(1),
        FootnoteDef {
            blocks: vec![Block::Paragraph {
                content: vec![
                    Inline::Text("note ".to_string()),
                    Inline::Link {
                        content: vec![Inline::Text("↩".to_string())],
                        href: Url("#fnref-1".to_string()),
                        title: None,
                        attrs: InlineAttrs::default(),
                    },
                ],
                attrs: BlockAttrs::default(),
            }],
        },
    );

    let doc = Document {
        blocks: Vec::new(),
        footnotes,
        assets: BTreeMap::new(),
        meta: Default::default(),
    };

    let html = document_to_html(&doc);
    assert_eq!(html.matches("href=\"#fnref-1\"").count(), 1);
}

#[test]
fn serialize_image_uses_asset_variant_then_source() {
    let mut assets = BTreeMap::new();
    assets.insert(
        AssetId("asset-1".to_string()),
        Asset::Image(ImageAsset {
            source: AssetSource::LocalPath {
                path: typub_ir::RelativePath::new("img/a.png".to_string())
                    .expect("valid relative path"),
            },
            meta: None,
            variants: vec![typub_ir::AssetVariant {
                name: "original".to_string(),
                publish_url: Url("https://cdn/a.png".to_string()),
                width: None,
                height: None,
            }],
        }),
    );

    let doc = Document {
        blocks: vec![Block::Paragraph {
            content: vec![Inline::Image {
                asset: AssetRef(AssetId("asset-1".to_string())),
                alt: "a".to_string(),
                title: None,
                attrs: ImageAttrs::default(),
            }],
            attrs: BlockAttrs::default(),
        }],
        footnotes: BTreeMap::new(),
        assets,
        meta: Default::default(),
    };

    let html = document_to_html(&doc);
    assert!(html.contains("src=\"https://cdn/a.png\""));
}

#[test]
fn serialize_multistyle_inline_is_nested_in_canonical_order() {
    let styles =
        StyleSet::new(vec![TextStyle::Italic, TextStyle::Bold]).expect("valid non-empty style set");
    let doc = empty_doc(vec![Block::Paragraph {
        content: vec![Inline::Styled {
            styles,
            content: vec![Inline::Text("x".to_string())],
            attrs: InlineAttrs::default(),
        }],
        attrs: BlockAttrs::default(),
    }]);

    let html = document_to_html(&doc);
    assert!(html.contains("<strong><em>x</em></strong>"));
}

#[test]
fn serialize_numbered_list_honors_item_value_override() {
    let doc = empty_doc(vec![Block::List {
        list: List {
            kind: ListKind::Numbered {
                start: 1,
                reversed: false,
                marker: None,
                items: vec![
                    FlowListItem {
                        marker: Some(FlowListItemMarker::Number(5)),
                        blocks: vec![Block::Paragraph {
                            content: vec![Inline::Text("five".to_string())],
                            attrs: BlockAttrs::default(),
                        }],
                    },
                    FlowListItem {
                        marker: None,
                        blocks: vec![Block::Paragraph {
                            content: vec![Inline::Text("next".to_string())],
                            attrs: BlockAttrs::default(),
                        }],
                    },
                ],
            },
        },
        attrs: BlockAttrs::default(),
    }]);

    let html = document_to_html(&doc);
    assert!(html.contains("<li value=\"5\">five</li>"));
}
