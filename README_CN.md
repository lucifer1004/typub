# typub

<p align="center">
  <a href="https://github.com/lucifer1004/typub/actions/workflows/ci.yml"><img src="https://github.com/lucifer1004/typub/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://codecov.io/gh/lucifer1004/typub"><img src="https://codecov.io/gh/lucifer1004/typub/graph/badge.svg" alt="codecov"></a>
  <a href="https://crates.io/crates/typub"><img src="https://img.shields.io/crates/v/typub.svg" alt="Crates.io"></a>
  <a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/badge/License-MIT-yellow.svg" alt="License: MIT"></a>
  <a href="https://github.com/govctl-org/govctl"><img src="https://img.shields.io/badge/governed%20by-govctl-6366F1" alt="governed by govctl"></a>
</p>

[English](./README.md) | **中文**

typub 是一个以 Typst 为优先、支持多平台的发布工具。

## 核心特性

### 🎯 以 Typst 为主，兼容 Markdown

使用 Typst（`content.typ`）或 Markdown（`content.md`）编写内容。Typst 是主要格式，具有强大的排版能力；Markdown 提供熟悉的替代方案。两种格式都可以发布到任何支持的平台。

### 🌐 多平台兼容

从单一内容源发布到多个平台：

- **API 发布**：Dev.to、Ghost、HashNode、Confluence、Notion、WordPress
- **复制粘贴（HTML）**：微信公众号、知乎、今日头条、哔哩哔哩、微博、百家号、网易号、搜狐、少数派、开源中国
- **复制粘贴（Markdown）**：CSDN、掘金、SegmentFault、博客园、Medium、简书、InfoQ、51CTO、腾讯云、阿里云、华为云、电子发烧友、ModelScope、火山引擎
- **本地输出**：Astro、静态文件、小红书

### 👀 开发预览

在发布前本地预览内容：

- **实时刷新**：内置开发服务器，保存时自动刷新（`typub dev`）
- **平台预览**：查看内容在各平台的实际渲染效果
- **主题支持**：从内置主题（github、notion、minimal、tech 等）中选择或创建自定义主题
- **数学公式渲染**：MathJax 驱动的 LaTeX 渲染，匹配目标平台行为

```bash
# 实时预览
typub dev posts/my-post -p ghost

# 预览不同平台
typub dev posts/my-post -p confluence
```

主题通过 `meta.toml` 或 `typub.toml` 配置，而非命令行参数。

### 📦 四种资源策略

灵活处理图片和其他资源：

- **embed**：Base64 内联编码 — 小图片，无需上传依赖
- **upload**：上传到平台存储 — 支持原生媒体 API 的平台
- **copy**：复制到本地输出 — 本地/静态输出
- **external**：上传到 S3 兼容存储 — CDN、大文件、跨平台 URL

### 📐 三种数学公式渲染策略

根据平台能力渲染数学公式：

- **SVG**：平台支持内联 SVG — 使用 Typst 原生 SVG 渲染（默认）
- **LaTeX**：平台需要 LaTeX 数学宏 — 保留原始 LaTeX 源码
- **PNG**：平台支持 base64 图片但不支持 SVG — 通过 resvg 栅格化为 PNG

### ⚙️ 分层配置系统

5 级配置优先级（从高到低）：

1. **文章-平台**：单篇文章的平台特定配置（`meta.toml` → `[platforms.<id>]`）
2. **文章**：单篇文章的默认配置（`meta.toml` → 顶层）
3. **全局-平台**：全局的平台特定配置（`typub.toml` → `[platforms.<id>]`）
4. **全局**：全局默认配置（`typub.toml` → 顶层）
5. **适配器默认值**：适配器默认配置（兜底）

---

## 文档地图

### 用户指南（发布内容）

- 入门路径：`docs/guide/getting-started.md`
- 平台设置：`docs/guide/adapters.md`
- 资源处理：`docs/guide/assets.md`
- 复制粘贴配置：`docs/guide/profiles.md`
- 高级自定义：`docs/guide/advanced-customization.md`

### 开发者指南（贡献代码）

- 开发流程：`DEVELOPING_GUIDE.md`
- 贡献者规范：`CLAUDE.md`
- 规范和架构：`docs/rfc/` 和 `docs/adr/`

## 用户基础

### 安装

```bash
cargo install typub
```

### 最小流程

```bash
typub init
typub new "我的文章"
typub dev posts/my-post -p ghost
typub publish posts/my-post -p ghost
```

## 高级功能

- 平台级资源策略（`embed` / `upload` / `copy` / `external`）
- 外部存储集成，支持跨平台资源 URL
- 复制粘贴配置选择和自定义
- 通过平台配置覆盖节点策略（`raw` / `unknown`）

详见 `docs/guide/advanced-customization.md`。

## 许可证

MIT
