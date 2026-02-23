# 掘金

掘金是面向开发者的技术社区平台，支持Markdown格式文章发布。

## 平台能力

| 特性     | 支持情况         |
| -------- | ---------------- |
| 输出格式 | Markdown         |
| 默认主题 | github           |
| 资源策略 | external（外链） |
| 数学渲染 | LaTeX            |
| 代码高亮 | 平台内置         |

### 资源策略

| 策略       | 支持 | 默认 | 说明             |
| ---------- | ---- | ---- | ---------------- |
| `embed`    | 否   |      | 不支持Base64图片 |
| `external` | 是   | \*   | 使用S3/R2外链    |

## 使用方法

掘金使用复制粘贴工作流：

```bash
# 预览内容
typub dev posts/my-post -p juejin

# 发布到掘金
typub pub -p juejin posts/my-post
```

1. 内容自动复制到剪贴板
2. 浏览器自动打开 [掘金创作中心](https://juejin.cn/editor/drafts/new)
3. 粘贴内容

## 平台注意事项

- **图片来源**：掘金不支持Base64内嵌图片，必须使用外部链接
- **配置存储**：需要在 `profiles.toml` 中配置S3/R2存储
- **代码高亮**：掘金编辑器会自动处理代码块语法高亮
- **数学公式**：支持LaTeX格式，使用 `$...$` 和 `$$...$$` 分隔符

### 换行保留

掘金编辑器在粘贴时会去除"多余"的空白行。typub 已针对此问题进行了优化：

- 自动在段落、标题、列表等块级元素之间输出双倍换行
- 确保粘贴后格式正确保留

## 配置示例

```toml
[storage]
type = "s3"
endpoint = "https://your-r2-endpoint.r2.cloudflarestorage.com"
bucket = "your-bucket"
region = "auto"
url_prefix = "https://cdn.your-domain.com"

[platforms.juejin]
asset_strategy = "external"
```

## 提示

- 掘金对文章标题有字数限制，建议控制在30字以内
- 封面图需要单独在编辑器中设置
- 标签在掘金编辑器中选择，最多5个
- 文章发布后可在创作中心管理
