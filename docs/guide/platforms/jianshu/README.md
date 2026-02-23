# 简书

简书是一个简洁的写作和阅读平台，支持Markdown格式。

## 平台能力

| 特性     | 支持情况         |
| -------- | ---------------- |
| 输出格式 | Markdown         |
| 资源策略 | external（外链） |
| 数学渲染 | LaTeX            |
| 代码高亮 | 平台内置         |

### 资源策略

| 策略       | 支持 | 默认 | 说明             |
| ---------- | ---- | ---- | ---------------- |
| `embed`    | 否   |      | 不支持Base64图片 |
| `external` | 是   | \*   | 使用S3/R2外链    |

## 使用方法

```bash
# 预览内容
typub dev posts/my-post -p jianshu
```

1. 浏览器打开预览页面
2. 点击 **复制内容** 按钮
3. 打开 [简书写作](https://www.jianshu.com/writer)
4. 粘贴内容

## 平台注意事项

- **图片要求**：简书不支持Base64内嵌图片，必须使用外部链接
- **格式兼容**：简书对Markdown的支持较为标准
- **数学公式**：支持LaTeX格式
- **字数统计**：简书显示实时字数统计

## 配置示例

```toml
[storage]
type = "s3"
endpoint = "https://your-r2-endpoint.r2.cloudflarestorage.com"
bucket = "your-bucket"
region = "auto"
url_prefix = "https://cdn.your-domain.com"

[platforms.jianshu]
asset_strategy = "external"
```

## 提示

- 简书适合长文写作，排版简洁
- 可设置文章为仅自己可见（私密）
- 支持文集功能整理系列文章
- 发布后可在我的文章中管理
