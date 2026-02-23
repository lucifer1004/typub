//! Helper constructors for typub HTML IR v2.
//!
//! These are intended for tests and small fixture construction.

use std::collections::BTreeMap;

use typub_ir::{
    AdmonitionKind, Asset, AssetId, AssetSource, AssetVariant, AttrMap, Block, BlockAttrs,
    Document, FlowListItem, FlowListItemMarker, FootnoteDef, HeadingLevel, ImageAsset, Inline,
    List, ListKind, MathSource, RelativePath, RenderPayload, RenderedArtifact, TableCell,
    TableCellKind, TableRow, TableSection, TableSectionKind, TaskListItem, Url,
};

/// Empty passthrough map helper.
pub fn empty_attrs() -> AttrMap {
    AttrMap::new()
}

/// Build a document with default metadata and no footnotes.
pub fn document(blocks: Vec<Block>) -> Document {
    Document {
        blocks,
        footnotes: BTreeMap::new(),
        assets: BTreeMap::new(),
        meta: Default::default(),
    }
}

/// Build a document with explicit assets.
pub fn document_with_assets(
    blocks: Vec<Block>,
    assets: impl IntoIterator<Item = (AssetId, Asset)>,
) -> Document {
    Document {
        blocks,
        footnotes: BTreeMap::new(),
        assets: assets.into_iter().collect(),
        meta: Default::default(),
    }
}

/// Build a document with explicit footnotes and assets.
pub fn document_full(
    blocks: Vec<Block>,
    footnotes: impl IntoIterator<Item = (typub_ir::FootnoteId, FootnoteDef)>,
    assets: impl IntoIterator<Item = (AssetId, Asset)>,
) -> Document {
    Document {
        blocks,
        footnotes: footnotes.into_iter().collect(),
        assets: assets.into_iter().collect(),
        meta: Default::default(),
    }
}

/// Create a paragraph block with plain text.
pub fn paragraph_text(text: &str) -> Block {
    Block::Paragraph {
        content: vec![Inline::Text(text.to_string())],
        attrs: BlockAttrs::default(),
    }
}

/// Create a paragraph block from inline content.
pub fn paragraph(content: Vec<Inline>) -> Block {
    Block::Paragraph {
        content,
        attrs: BlockAttrs::default(),
    }
}

/// Create a heading block from text.
///
/// Out-of-range levels are clamped to 1..=6 for convenience in tests.
pub fn heading_text(level: u8, text: &str) -> Block {
    let normalized = level.clamp(1, 6);
    let heading_level = match HeadingLevel::new(normalized) {
        Ok(v) => v,
        Err(_) => {
            // unreachable due clamp, keep graceful fallback
            return paragraph_text(text);
        }
    };

    Block::Heading {
        level: heading_level,
        id: None,
        content: vec![Inline::Text(text.to_string())],
        attrs: BlockAttrs::default(),
    }
}

/// Create a quote block with a single paragraph child.
pub fn quote_text(text: &str) -> Block {
    Block::Quote {
        blocks: vec![paragraph_text(text)],
        cite: None,
        attrs: BlockAttrs::default(),
    }
}

/// Create a code block with optional language.
pub fn code_block(code: &str, language: &str) -> Block {
    Block::CodeBlock {
        code: code.to_string(),
        language: if language.is_empty() {
            None
        } else {
            Some(language.to_string())
        },
        filename: None,
        highlight_lines: Vec::new(),
        highlighted_html: None,
        attrs: BlockAttrs::default(),
    }
}

/// Create a code block with pre-rendered highlighted HTML.
pub fn code_block_highlighted(code: &str, highlighted: &str, language: &str) -> Block {
    Block::CodeBlock {
        code: code.to_string(),
        language: if language.is_empty() {
            None
        } else {
            Some(language.to_string())
        },
        filename: None,
        highlight_lines: Vec::new(),
        highlighted_html: Some(highlighted.to_string()),
        attrs: BlockAttrs::default(),
    }
}

/// Build an image block and its backing remote image asset.
///
/// Returns `(block, (asset_id, asset))` so callers can place the asset into `Document.assets`.
pub fn image(asset_id: &str, src: &str, alt: &str) -> (Block, (AssetId, Asset)) {
    let id = AssetId(asset_id.to_string());
    let asset = Asset::Image(ImageAsset {
        source: AssetSource::RemoteUrl {
            url: Url(src.to_string()),
        },
        meta: None,
        variants: Vec::new(),
    });

    let block = Block::Paragraph {
        content: vec![Inline::Image {
            asset: typub_ir::AssetRef(id.clone()),
            alt: alt.to_string(),
            title: None,
            attrs: Default::default(),
        }],
        attrs: BlockAttrs::default(),
    };

    (block, (id, asset))
}

/// Build an image block + local-path image asset (legacy ImageMarker-equivalent fixture).
pub fn image_marker(
    asset_id: &str,
    path: &str,
    alt: &str,
) -> Result<(Block, (AssetId, Asset)), String> {
    let id = AssetId(asset_id.to_string());
    let rel = RelativePath::new(path.to_string())?;
    let asset = Asset::Image(ImageAsset {
        source: AssetSource::LocalPath { path: rel },
        meta: None,
        variants: Vec::new(),
    });

    let block = Block::Paragraph {
        content: vec![Inline::Image {
            asset: typub_ir::AssetRef(id.clone()),
            alt: alt.to_string(),
            title: None,
            attrs: Default::default(),
        }],
        attrs: BlockAttrs::default(),
    };

    Ok((block, (id, asset)))
}

/// Create a divider block.
pub fn divider() -> Block {
    Block::Divider {
        attrs: BlockAttrs::default(),
    }
}

/// Create a flow list item from inline content.
pub fn list_item(content: Vec<Inline>) -> FlowListItem {
    FlowListItem {
        marker: None,
        blocks: vec![paragraph(content)],
    }
}

/// Create a bullet list from plain text items.
pub fn bullet_list_text(items: &[&str]) -> Block {
    let items = items
        .iter()
        .map(|s| FlowListItem {
            marker: Some(FlowListItemMarker::Bullet),
            blocks: vec![paragraph_text(s)],
        })
        .collect();

    Block::List {
        list: List {
            kind: ListKind::Bullet { items },
        },
        attrs: BlockAttrs::default(),
    }
}

/// Create a numbered list from plain text items.
pub fn numbered_list_text(items: &[&str]) -> Block {
    let items = items
        .iter()
        .enumerate()
        .map(|(i, s)| FlowListItem {
            marker: Some(FlowListItemMarker::Number((i + 1) as u32)),
            blocks: vec![paragraph_text(s)],
        })
        .collect();

    Block::List {
        list: List {
            kind: ListKind::Numbered {
                start: 1,
                reversed: false,
                marker: None,
                items,
            },
        },
        attrs: BlockAttrs::default(),
    }
}

/// Create a task list item with paragraph content.
pub fn task_item(text: &str, checked: bool) -> TaskListItem {
    TaskListItem {
        checked,
        blocks: vec![paragraph_text(text)],
    }
}

/// Create a task list block.
pub fn task_list(items: Vec<TaskListItem>) -> Block {
    Block::List {
        list: List {
            kind: ListKind::Task { items },
        },
        attrs: BlockAttrs::default(),
    }
}

/// Create a simple table data cell from inline content.
pub fn table_cell(content: Vec<Inline>) -> TableCell {
    TableCell {
        kind: TableCellKind::Data,
        blocks: vec![paragraph(content)],
        colspan: 1,
        rowspan: 1,
        scope: None,
        align: None,
        attrs: BlockAttrs::default(),
    }
}

/// Create a simple table from text headers and rows.
pub fn table_text(headers: &[&str], rows: &[Vec<&str>]) -> Block {
    let head_rows = if headers.is_empty() {
        Vec::new()
    } else {
        vec![TableRow {
            cells: headers
                .iter()
                .map(|h| TableCell {
                    kind: TableCellKind::Header,
                    blocks: vec![paragraph_text(h)],
                    colspan: 1,
                    rowspan: 1,
                    scope: None,
                    align: None,
                    attrs: BlockAttrs::default(),
                })
                .collect(),
            attrs: BlockAttrs::default(),
        }]
    };

    let body_rows = rows
        .iter()
        .map(|r| TableRow {
            cells: r
                .iter()
                .map(|c| TableCell {
                    kind: TableCellKind::Data,
                    blocks: vec![paragraph_text(c)],
                    colspan: 1,
                    rowspan: 1,
                    scope: None,
                    align: None,
                    attrs: BlockAttrs::default(),
                })
                .collect(),
            attrs: BlockAttrs::default(),
        })
        .collect();

    let mut sections = Vec::new();
    if !head_rows.is_empty() {
        sections.push(TableSection {
            kind: TableSectionKind::Head,
            rows: head_rows,
            attrs: BlockAttrs::default(),
        });
    }
    sections.push(TableSection {
        kind: TableSectionKind::Body,
        rows: body_rows,
        attrs: BlockAttrs::default(),
    });

    Block::Table {
        caption: None,
        sections,
        attrs: BlockAttrs::default(),
    }
}

/// Create a basic admonition block with paragraph content.
pub fn admonition_text(kind: AdmonitionKind, text: &str) -> Block {
    Block::Admonition {
        kind,
        title: None,
        blocks: vec![paragraph_text(text)],
        attrs: BlockAttrs::default(),
    }
}

/// Create a math inline fragment with latex source.
pub fn math_inline_latex(src: &str) -> Inline {
    Inline::MathInline {
        math: RenderPayload {
            src: Some(MathSource::Latex(src.to_string())),
            rendered: None,
            id: None,
        },
        attrs: Default::default(),
    }
}

/// Create a math block with latex source.
pub fn math_block_latex(src: &str) -> Block {
    Block::MathBlock {
        math: RenderPayload {
            src: Some(MathSource::Latex(src.to_string())),
            rendered: None,
            id: None,
        },
        attrs: BlockAttrs::default(),
    }
}

/// Create an inline SVG payload.
pub fn svg_inline(svg: &str) -> Inline {
    Inline::SvgInline {
        svg: RenderPayload {
            src: None,
            rendered: Some(RenderedArtifact::Svg(svg.to_string())),
            id: None,
        },
        attrs: Default::default(),
    }
}

/// Create a block SVG payload.
pub fn svg_block(svg: &str) -> Block {
    Block::SvgBlock {
        svg: RenderPayload {
            src: None,
            rendered: Some(RenderedArtifact::Svg(svg.to_string())),
            id: None,
        },
        attrs: BlockAttrs::default(),
    }
}

/// Create an image asset variant helper.
pub fn asset_variant(
    name: &str,
    publish_url: &str,
    width: Option<u32>,
    height: Option<u32>,
) -> AssetVariant {
    AssetVariant {
        name: name.to_string(),
        publish_url: Url(publish_url.to_string()),
        width,
        height,
    }
}
