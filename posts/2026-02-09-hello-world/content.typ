= Hello World

Welcome to *typub* — a multi-platform content publishing system built with Rust and Typst.

== What is typub?

typub allows you to write your content once in Typst and publish it to multiple platforms:

=== API Adapters (9 platforms)

- *Ghost*: Full API support with draft management
- *WordPress*: REST API with taxonomy support
- *Dev.to*: API publishing with canonical URLs
- *Notion*: Block-based publishing with math support
- *Confluence*: Enterprise wiki with SVG/image attachments
- *Hashnode*: GraphQL API with draft workflow
- *Astro*: Static Markdown with YAML frontmatter
- *Static*: Standalone HTML generation
- *Xiaohongshu*: Slide image generation

=== Copy-Paste Profiles (24 platforms)

Clipboard-based publishing for platforms without public APIs:

- *WeChat*, *Zhihu*, *CSDN*, *Juejin*, *SegmentFault*
- *Bilibili*, *Toutiao*, *51CTO*, *Aliyun*, *TencentCloud*
- *Medium*, *InfoQ*, *CNBlogs*, *Jianshu*
- And more...

== Getting Started

To create a new post:

```bash
typub new "My New Post"
```

To preview with live reload during development:

```bash
typub dev posts/my-new-post/ -p xiaohongshu
```

To publish to all configured platforms:

```bash
typub publish posts/my-new-post/
```

== Key Features

+ *Single Source of Truth*: Write once in Typst or Markdown, publish everywhere
+ *AST-Centric Pipeline*: 10-stage pipeline with HTML AST at its core
+ *Platform-Specific Optimization*: Each output is tailored for its platform
+ *Asset Management*: Flexible strategies (embed, upload, external storage)
+ *Math Rendering*: SVG or PNG with platform-appropriate handling
+ *Theme System*: 4 built-in themes with custom theme support
+ *Status Tracking*: Know what's published where with SQLite persistence
+ *Watch Mode*: Auto-rebuild on file changes with live preview
+ *Draft Workflow*: Reversible draft/publish state control
+ *i18n Support*: Chinese and English UI strings

== Project Status

typub is under active development with a modular architecture:

- *14 core crates* for shared functionality
- *10 adapter crates* for platform integrations
- *Typst packages* for math rendering
- Comprehensive test coverage with snapshot testing

== Conclusion

Start writing in Typst today and reach your audience everywhere!

Related reading: #link("../2026-02-10-markdown-test/")[Markdown Support Test]
