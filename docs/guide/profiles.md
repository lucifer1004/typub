# Copy-paste Profiles

For platforms without public APIs, typub generates formatted content that you copy and paste manually.

## Audience

- This page is user-facing and platform-agnostic.
- For per-platform operation steps, see [Platforms Overview](./platforms/).

## Basic Usage

```bash
typub dev posts/my-post -p wechat
```

Then:

1. Open the local preview URL.
2. Copy rendered content.
3. Paste into the target platform editor.

## Built-in Profiles

### HTML Platforms

| Platform    | Description             |
| ----------- | ----------------------- |
| `wechat`    | WeChat Official Account |
| `zhihu`     | Zhihu columns           |
| `toutiao`   | Toutiao/Jinri Toutiao   |
| `bilibili`  | Bilibili articles       |
| `weibo`     | Weibo articles          |
| `baijiahao` | Baidu Baijiahao         |
| `wangyihao` | NetEase Wangyihao       |
| `sohu`      | Sohu media              |
| `sspai`     | Sspai                   |
| `oschina`   | OSChina                 |

### Markdown Platforms

| Platform       | Description             |
| -------------- | ----------------------- |
| `csdn`         | CSDN                    |
| `juejin`       | Juejin                  |
| `segmentfault` | SegmentFault            |
| `cnblogs`      | Cnblogs                 |
| `medium`       | Medium                  |
| `jianshu`      | Jianshu                 |
| `infoq`        | InfoQ China             |
| `51cto`        | 51CTO                   |
| `tencentcloud` | Tencent Cloud Developer |
| `aliyun`       | Aliyun Developer        |
| `huaweicloud`  | Huawei Cloud            |
| `elecfans`     | Elecfans                |
| `modelscope`   | ModelScope              |
| `volcengine`   | Volcengine Developer    |

## Advanced Customization

### Add or adjust built-in profiles (repository contributors)

Profiles are defined in:

`crates/adapters/typub-adapter-copypaste/profiles.toml`

Example:

```toml
[[profile]]
id = "my-platform"
name = "My Platform"
editor_url = "https://my-platform.com/write"
format = "markdown"   # or "html"
# compat = "wechat"   # optional: use wechat-style HTML transforms
```

### Field reference

| Field        | Required | Description                                          |
| ------------ | -------- | ---------------------------------------------------- |
| `id`         | Yes      | Unique identifier (used in commands)                 |
| `name`       | Yes      | Human-readable display name                          |
| `editor_url` | Yes      | URL to the platform's editor                         |
| `format`     | Yes      | `html` or `markdown`                                 |
| `compat`     | No       | Name of a compatibility function for HTML transforms |

### Compatibility functions

For HTML platforms that need special handling:

| Compat   | Description                                 |
| -------- | ------------------------------------------- |
| `wechat` | WeChat-specific CSS inlining and formatting |

To add a new compat function, implement it in `crates/adapters/typub-adapter-copypaste/src/adapter.rs`.

## Related Docs

- Platform pages: [Platforms Overview](./platforms/)
- Advanced user customization: [Advanced Customization](./advanced-customization.md)
