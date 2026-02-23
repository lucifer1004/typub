# Asset Handling

typub provides flexible strategies for handling images and other assets in your content.

## Audience

- This page is platform-agnostic and user-facing.
- For provider-level details and examples, see [External Storage](./storage/README.md).

## Basic Usage

### Choose an asset strategy

| Strategy   | How it works                 | Typical usage                                    |
| ---------- | ---------------------------- | ------------------------------------------------ |
| `embed`    | Base64 encode inline         | Small images, no upload dependency               |
| `upload`   | Upload to platform storage   | Platforms with native media APIs                 |
| `copy`     | Copy to local output         | Local/static outputs                             |
| `external` | Upload to S3-compatible host | CDN, large assets, or platforms rejecting base64 |

## Configuration

### Per-platform Strategy

```toml
[platforms.devto]
enabled = true
asset_strategy = "embed"

[platforms.notion]
enabled = true
asset_strategy = "upload"

[platforms.astro]
enabled = true
asset_strategy = "copy"
```

### Basic image reference

```typst
#image("./images/diagram.png", width: 80%)
```

typub resolves and rewrites image references based on the configured strategy.

## Advanced Usage

### External storage

Configure `[storage]` when using `external`:

```toml
[storage]
endpoint = "https://s3.amazonaws.com"
bucket = "my-content-bucket"
region = "us-east-1"
url_prefix = "https://cdn.example.com"
```

Credentials should come from environment variables.

### Strategy selection guidance

- Prefer `upload` when the platform supports native media upload.
- Use `external` for large assets or CDN portability.
- Use `embed` for simplicity when content size remains acceptable.
- Use `copy` for local/static outputs.

### Tracking

typub tracks uploaded assets to avoid duplicate uploads:

```bash
# See asset status
typub status --assets posts/my-post
```

Asset mappings are stored in `.typub/status.db`.

## Related Docs

- External storage details: [External Storage](./storage/README.md)
- Adapter overview: [Adapters](./adapters.md)
- Spec: [RFC-0004](../rfc/RFC-0004.md)
