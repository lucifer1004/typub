//! Typub IR v2 (semantic document IR).
//!
//! This is a semantic-first IR surface aligned with RFC-0009.

use relative_path::RelativePathBuf;
use serde::de::{self, Deserializer};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::path::Path;

pub type AttrMap = BTreeMap<String, String>;
pub type ExtensionMap = BTreeMap<String, JsonValue>;

pub fn empty_map() -> AttrMap {
    AttrMap::new()
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AnchorId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FootnoteId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AssetId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Url(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeadingLevel(u8);

impl HeadingLevel {
    pub fn new(level: u8) -> Result<Self, String> {
        if (1..=6).contains(&level) {
            Ok(Self(level))
        } else {
            Err("heading level must be in 1..=6".to_string())
        }
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

impl Serialize for HeadingLevel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(self.0)
    }
}

impl<'de> Deserialize<'de> for HeadingLevel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        HeadingLevel::new(value).map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExtensionKind(String);

impl ExtensionKind {
    pub fn new(kind: String) -> Result<Self, String> {
        let k = kind.trim();
        if k.is_empty() {
            return Err("extension kind must not be empty".to_string());
        }
        if !(k.contains(':') || k.contains('/')) {
            return Err("extension kind must be namespaced (use ':' or '/')".to_string());
        }
        Ok(Self(k.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for ExtensionKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ExtensionKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        ExtensionKind::new(value).map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AssetRef(pub AssetId);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub blocks: Vec<Block>,
    pub footnotes: BTreeMap<FootnoteId, FootnoteDef>,
    pub assets: BTreeMap<AssetId, Asset>,
    pub meta: DocMeta,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct DocMeta {
    pub title: Option<String>,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub toc: Option<Vec<TocEntry>>,
    pub extensions: ExtensionMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TocEntry {
    pub level: HeadingLevel,
    pub id: AnchorId,
    pub title: InlineSeq,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct BlockAttrs {
    pub classes: Vec<String>,
    pub style: Option<String>,
    pub passthrough: AttrMap,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct InlineAttrs {
    pub classes: Vec<String>,
    pub style: Option<String>,
    pub passthrough: AttrMap,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ImageAttrs {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub align: Option<TextAlign>,
    pub passthrough: AttrMap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

pub type InlineSeq = Vec<Inline>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Block {
    Heading {
        level: HeadingLevel,
        id: Option<AnchorId>,
        content: InlineSeq,
        attrs: BlockAttrs,
    },
    Paragraph {
        content: InlineSeq,
        attrs: BlockAttrs,
    },
    Quote {
        blocks: Vec<Block>,
        cite: Option<Url>,
        attrs: BlockAttrs,
    },
    CodeBlock {
        code: String,
        language: Option<String>,
        filename: Option<String>,
        highlight_lines: Vec<u32>,
        highlighted_html: Option<String>,
        attrs: BlockAttrs,
    },
    Divider {
        attrs: BlockAttrs,
    },
    List {
        list: List,
        attrs: BlockAttrs,
    },
    DefinitionList {
        items: Vec<DefinitionItem>,
        attrs: BlockAttrs,
    },
    Table {
        caption: Option<Vec<Block>>,
        sections: Vec<TableSection>,
        attrs: BlockAttrs,
    },
    Figure {
        content: Vec<Block>,
        caption: Option<Vec<Block>>,
        attrs: BlockAttrs,
    },
    Admonition {
        kind: AdmonitionKind,
        title: Option<InlineSeq>,
        blocks: Vec<Block>,
        attrs: BlockAttrs,
    },
    Details {
        summary: Option<InlineSeq>,
        blocks: Vec<Block>,
        open: bool,
        attrs: BlockAttrs,
    },
    MathBlock {
        math: RenderPayload,
        attrs: BlockAttrs,
    },
    SvgBlock {
        svg: RenderPayload,
        attrs: BlockAttrs,
    },
    UnknownBlock {
        tag: String,
        attrs: BlockAttrs,
        children: Vec<UnknownChild>,
        data: ExtensionMap,
        note: Option<String>,
        source: Option<String>,
    },
    RawBlock {
        html: String,
        origin: RawOrigin,
        trust: RawTrust,
        attrs: BlockAttrs,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Inline {
    Text(String),
    Code(String),
    SoftBreak,
    HardBreak,
    Styled {
        styles: StyleSet,
        content: InlineSeq,
        attrs: InlineAttrs,
    },
    Link {
        content: InlineSeq,
        href: Url,
        title: Option<String>,
        attrs: InlineAttrs,
    },
    Image {
        asset: AssetRef,
        alt: String,
        title: Option<String>,
        attrs: ImageAttrs,
    },
    FootnoteRef(FootnoteId),
    MathInline {
        math: RenderPayload,
        attrs: InlineAttrs,
    },
    SvgInline {
        svg: RenderPayload,
        attrs: InlineAttrs,
    },
    UnknownInline {
        tag: String,
        attrs: InlineAttrs,
        content: InlineSeq,
        data: ExtensionMap,
        note: Option<String>,
        source: Option<String>,
    },
    RawInline {
        html: String,
        origin: RawOrigin,
        trust: RawTrust,
        attrs: InlineAttrs,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StyleSet {
    styles: Vec<TextStyle>,
}

impl StyleSet {
    pub fn new(styles: Vec<TextStyle>) -> Result<Self, String> {
        let mut set = Self { styles };
        set.canonicalize();
        if set.styles.is_empty() {
            return Err("StyleSet must contain at least one style".to_string());
        }
        Ok(set)
    }

    pub fn single(style: TextStyle) -> Self {
        Self {
            styles: vec![style],
        }
    }

    pub fn styles(&self) -> &[TextStyle] {
        &self.styles
    }

    pub fn canonicalize(&mut self) {
        self.styles.sort_by_key(|s| s.canonical_rank());
        self.styles.dedup();
    }
}

impl<'de> Deserialize<'de> for StyleSet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawStyleSet {
            styles: Vec<TextStyle>,
        }

        let raw = RawStyleSet::deserialize(deserializer)?;
        StyleSet::new(raw.styles).map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TextStyle {
    Bold,
    Italic,
    Strikethrough,
    Underline,
    Mark,
    Superscript,
    Subscript,
    Kbd,
}

impl TextStyle {
    fn canonical_rank(&self) -> u8 {
        match self {
            Self::Bold => 0,
            Self::Italic => 1,
            Self::Strikethrough => 2,
            Self::Underline => 3,
            Self::Mark => 4,
            Self::Superscript => 5,
            Self::Subscript => 6,
            Self::Kbd => 7,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct List {
    pub kind: ListKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ListKind {
    Bullet {
        items: Vec<FlowListItem>,
    },
    Numbered {
        start: u32,
        reversed: bool,
        marker: Option<OrderedListMarker>,
        items: Vec<FlowListItem>,
    },
    Task {
        items: Vec<TaskListItem>,
    },
    Custom {
        kind: ExtensionKind,
        items: Vec<CustomListItem>,
        data: ExtensionMap,
    },
}

impl ListKind {
    pub fn item_blocks_mut(&mut self) -> impl Iterator<Item = &mut Vec<Block>> {
        enum ItemBlocksMut<'a> {
            Flow(std::slice::IterMut<'a, FlowListItem>),
            Task(std::slice::IterMut<'a, TaskListItem>),
            Custom(std::slice::IterMut<'a, CustomListItem>),
        }

        impl<'a> Iterator for ItemBlocksMut<'a> {
            type Item = &'a mut Vec<Block>;

            fn next(&mut self) -> Option<Self::Item> {
                match self {
                    Self::Flow(iter) => iter.next().map(|item| &mut item.blocks),
                    Self::Task(iter) => iter.next().map(|item| &mut item.blocks),
                    Self::Custom(iter) => iter.next().map(|item| &mut item.blocks),
                }
            }
        }

        match self {
            Self::Bullet { items } | Self::Numbered { items, .. } => {
                ItemBlocksMut::Flow(items.iter_mut())
            }
            Self::Task { items } => ItemBlocksMut::Task(items.iter_mut()),
            Self::Custom { items, .. } => ItemBlocksMut::Custom(items.iter_mut()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowListItem {
    pub marker: Option<FlowListItemMarker>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskListItem {
    pub checked: bool,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomListItem {
    pub blocks: Vec<Block>,
    pub data: ExtensionMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FlowListItemMarker {
    Bullet,
    Number(u32),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UnknownChild {
    Block(Block),
    Inline(Inline),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DefinitionItem {
    pub terms: Vec<Vec<Block>>,
    pub definitions: Vec<Vec<Block>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableCell {
    pub kind: TableCellKind,
    pub blocks: Vec<Block>,
    pub colspan: u32,
    pub rowspan: u32,
    pub scope: Option<TableHeaderScope>,
    pub align: Option<TextAlign>,
    pub attrs: BlockAttrs,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableRow {
    pub cells: Vec<TableCell>,
    pub attrs: BlockAttrs,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableSection {
    pub kind: TableSectionKind,
    pub rows: Vec<TableRow>,
    pub attrs: BlockAttrs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TableSectionKind {
    Head,
    Body,
    Foot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TableCellKind {
    Header,
    Data,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TableHeaderScope {
    Row,
    Col,
    RowGroup,
    ColGroup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderedListMarker {
    Decimal,
    LowerAlpha,
    UpperAlpha,
    LowerRoman,
    UpperRoman,
}

impl TableCell {
    pub fn simple(blocks: Vec<Block>) -> Self {
        Self {
            kind: TableCellKind::Data,
            blocks,
            colspan: 1,
            rowspan: 1,
            scope: None,
            align: None,
            attrs: BlockAttrs::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FootnoteDef {
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AdmonitionKind {
    Note,
    Tip,
    Warning,
    Danger,
    Info,
    Custom(ExtensionKind),
}

impl AdmonitionKind {
    pub fn default_title(&self) -> &str {
        match self {
            Self::Note => "Note",
            Self::Tip => "Tip",
            Self::Warning => "Warning",
            Self::Danger => "Caution",
            Self::Info => "Important",
            Self::Custom(s) => s.as_str(),
        }
    }
}

pub type MathPayload = RenderPayload;
pub type SvgPayload = RenderPayload;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MathSource {
    Typst(String),
    Latex(String),
    Custom { kind: ExtensionKind, src: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderPayload {
    pub src: Option<MathSource>,
    pub rendered: Option<RenderedArtifact>,
    pub id: Option<String>,
}

impl RenderPayload {
    pub fn has_source_or_rendered(&self) -> bool {
        self.src.is_some() || self.rendered.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RenderedArtifact {
    Svg(String),
    MathMl(String),
    Asset {
        asset: AssetRef,
        mime: Option<String>,
        width: Option<u32>,
        height: Option<u32>,
    },
    Custom {
        kind: ExtensionKind,
        data: ExtensionMap,
    },
}

pub type RenderedMath = RenderedArtifact;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Asset {
    Image(ImageAsset),
    Video(MediaAsset),
    Audio(MediaAsset),
    File(FileAsset),
    Custom(CustomAsset),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageAsset {
    pub source: AssetSource,
    pub meta: Option<ImageMeta>,
    pub variants: Vec<AssetVariant>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaAsset {
    pub source: AssetSource,
    pub mime: Option<String>,
    pub duration_ms: Option<u64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub sha256: Option<String>,
    pub variants: Vec<AssetVariant>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileAsset {
    pub source: AssetSource,
    pub mime: Option<String>,
    pub size_bytes: Option<u64>,
    pub sha256: Option<String>,
    pub variants: Vec<AssetVariant>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomAsset {
    pub kind: ExtensionKind,
    pub source: AssetSource,
    pub meta: ExtensionMap,
    pub variants: Vec<AssetVariant>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AssetSource {
    LocalPath { path: RelativePath },
    RemoteUrl { url: Url },
    DataUri { uri: String },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RelativePath(RelativePathBuf);

impl RelativePath {
    pub fn new(path: String) -> Result<Self, String> {
        if path.trim().is_empty() {
            return Err("path must not be empty".to_string());
        }

        let p = Path::new(&path);
        if p.is_absolute() {
            return Err("absolute path is not allowed".to_string());
        }

        // Guard against absolute-like Windows forms on non-Windows hosts.
        // Examples: C:\foo, C:/foo, \\server\share\foo
        let bytes = path.as_bytes();
        if bytes.len() >= 3
            && bytes[1] == b':'
            && (bytes[2] == b'\\' || bytes[2] == b'/')
            && bytes[0].is_ascii_alphabetic()
        {
            return Err("windows drive absolute path is not allowed".to_string());
        }
        if path.starts_with("\\\\") {
            return Err("windows UNC absolute path is not allowed".to_string());
        }
        if path.starts_with('\\') {
            return Err("windows rooted path is not allowed".to_string());
        }

        // Canonicalize lexically into a unique project-relative form:
        // - normalize separators to '/'
        // - remove empty and '.' segments
        // - resolve '..' and reject root escape
        let mut parts: Vec<&str> = Vec::new();
        for raw in path.split(['/', '\\']) {
            if raw.is_empty() || raw == "." {
                continue;
            }
            if raw == ".." {
                if parts.pop().is_none() {
                    return Err("path escapes root".to_string());
                }
                continue;
            }
            parts.push(raw);
        }

        if parts.is_empty() {
            return Err("path must resolve to a non-empty relative path".to_string());
        }

        let canonical = parts.join("/");
        Ok(Self(RelativePathBuf::from(canonical)))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Serialize for RelativePath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RelativePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        RelativePath::new(value).map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageMeta {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub format: Option<String>,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetVariant {
    pub name: String,
    /// Publish-conformance URL only.
    /// Preview-only resolution data MUST remain outside conformance IR.
    pub publish_url: Url,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RawOrigin {
    Typst,
    Markdown,
    User,
    Extension(ExtensionKind),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RawTrust {
    Trusted,
    Untrusted,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn heading_level_accepts_valid_range() {
        for n in 1..=6 {
            let lvl = HeadingLevel::new(n).expect("valid heading level");
            assert_eq!(lvl.get(), n);
        }
    }

    #[test]
    fn heading_level_rejects_out_of_range() {
        assert!(HeadingLevel::new(0).is_err());
        assert!(HeadingLevel::new(7).is_err());
        assert!(HeadingLevel::new(255).is_err());
    }

    #[test]
    fn heading_level_deserialize_enforces_range() {
        let ok: HeadingLevel = serde_json::from_str("3").expect("deserialize valid level");
        assert_eq!(ok.get(), 3);

        let bad = serde_json::from_str::<HeadingLevel>("0");
        assert!(bad.is_err());
    }

    #[test]
    fn extension_kind_requires_namespace() {
        assert!(ExtensionKind::new("".to_string()).is_err());
        assert!(ExtensionKind::new("custom".to_string()).is_err());
        assert!(ExtensionKind::new("vendor:feature".to_string()).is_ok());
        assert!(ExtensionKind::new("vendor/feature".to_string()).is_ok());
    }

    #[test]
    fn extension_kind_deserialize_enforces_namespace() {
        let ok: ExtensionKind =
            serde_json::from_str("\"vendor:feature\"").expect("deserialize namespaced kind");
        assert_eq!(ok.as_str(), "vendor:feature");

        let bad = serde_json::from_str::<ExtensionKind>("\"feature\"");
        assert!(bad.is_err());
    }

    #[test]
    fn style_set_canonicalizes_and_deduplicates() {
        let set = StyleSet::new(vec![
            TextStyle::Italic,
            TextStyle::Bold,
            TextStyle::Italic,
            TextStyle::Underline,
        ])
        .expect("non-empty style set");
        assert_eq!(
            set.styles(),
            &[TextStyle::Bold, TextStyle::Italic, TextStyle::Underline]
        );
    }

    #[test]
    fn style_set_rejects_empty() {
        assert!(StyleSet::new(vec![]).is_err());
    }

    #[test]
    fn style_set_deserialize_rejects_empty() {
        let bad = serde_json::from_str::<StyleSet>(r#"{"styles":[]}"#);
        assert!(bad.is_err());
    }

    #[test]
    fn style_set_deserialize_canonicalizes() {
        let set: StyleSet =
            serde_json::from_str(r#"{"styles":["Underline","Italic","Bold","Italic"]}"#)
                .expect("deserialize style set");
        assert_eq!(
            set.styles(),
            &[TextStyle::Bold, TextStyle::Italic, TextStyle::Underline]
        );
    }

    #[test]
    fn relative_path_canonicalizes_input() {
        let p = RelativePath::new("./assets\\images/../hero.png".to_string())
            .expect("canonicalizable relative path");
        assert_eq!(p.as_str(), "assets/hero.png");
    }

    #[test]
    fn relative_path_rejects_escape_or_absolute_forms() {
        assert!(RelativePath::new("../outside.png".to_string()).is_err());
        assert!(RelativePath::new("/abs/path.png".to_string()).is_err());
        assert!(RelativePath::new("C:\\\\abs\\\\path.png".to_string()).is_err());
        assert!(RelativePath::new("\\\\server\\\\share\\\\x.png".to_string()).is_err());
        assert!(RelativePath::new("\\rooted\\\\x.png".to_string()).is_err());
    }

    #[test]
    fn relative_path_deserialize_applies_canonicalization() {
        let p: RelativePath =
            serde_json::from_str(r#""a/b/../c.png""#).expect("deserialize and canonicalize");
        assert_eq!(p.as_str(), "a/c.png");

        let bad = serde_json::from_str::<RelativePath>(r#""../../escape.png""#);
        assert!(bad.is_err());
    }

    #[test]
    fn relative_path_rejects_root_escape_after_normalization() {
        assert!(RelativePath::new("assets/../../escape.png".to_string()).is_err());
    }

    #[test]
    fn list_kind_custom_deserialize_requires_namespaced_kind() {
        let bad = serde_json::from_str::<ListKind>(
            r#"{"Custom":{"kind":"custom","items":[],"data":{}}}"#,
        );
        assert!(bad.is_err());

        let ok = serde_json::from_str::<ListKind>(
            r#"{"Custom":{"kind":"vendor:custom","items":[],"data":{}}}"#,
        );
        assert!(ok.is_ok());
    }

    #[test]
    fn render_payload_accepts_source_only() {
        let payload = RenderPayload {
            src: Some(MathSource::Latex("x+y".to_string())),
            rendered: None,
            id: None,
        };
        assert!(payload.has_source_or_rendered());
    }

    #[test]
    fn render_payload_accepts_rendered_only() {
        let payload = RenderPayload {
            src: None,
            rendered: Some(RenderedArtifact::Asset {
                asset: AssetRef(AssetId("asset-1".to_string())),
                mime: Some("image/png".to_string()),
                width: Some(10),
                height: Some(20),
            }),
            id: None,
        };
        assert!(payload.has_source_or_rendered());
    }

    #[test]
    fn render_payload_detects_empty_payload() {
        let payload = RenderPayload {
            src: None,
            rendered: None,
            id: None,
        };
        assert!(!payload.has_source_or_rendered());
    }
}
