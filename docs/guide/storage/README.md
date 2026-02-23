# External Storage Configuration

typub supports S3-compatible external storage for assets when publishing to platforms that use the `External` asset strategy. This enables automatic asset upload to cloud storage with deduplication and caching.

## Overview

When publishing to platforms like Dev.to, Hashnode, Medium, or Ghost, images and other assets need to be hosted externally. typub handles this automatically by:

1. Computing content hashes for all assets
2. Uploading to S3-compatible storage (deduplicated by hash)
3. Replacing local asset references with public URLs
4. Caching upload records to avoid re-uploading

## Configuration

### Basic Configuration

Add a `[storage]` section to your `typub.toml`:

```toml
[storage]
type = "s3"
endpoint = "https://your-s3-endpoint.com"
bucket = "your-bucket-name"
region = "us-east-1"
url_prefix = "https://cdn.yourdomain.com"
```

### Environment Variables

Credentials should be provided via environment variables for security:

```bash
# S3 credentials
export S3_ACCESS_KEY_ID="your-access-key"
export S3_SECRET_ACCESS_KEY="your-secret-key"
```

### Platform-Specific Configuration

You can override storage settings per platform:

```toml
[storage]
endpoint = "https://s3.amazonaws.com"
bucket = "default-bucket"
url_prefix = "https://cdn.example.com"

[platforms.devto.storage]
bucket = "devto-assets"
url_prefix = "https://devto-cdn.example.com"

[platforms.medium.storage]
bucket = "medium-assets"
url_prefix = "https://medium-cdn.example.com"
```

## Configuration Reference

| Field               | Description                                    | Required    | Environment Variable                              |
| ------------------- | ---------------------------------------------- | ----------- | ------------------------------------------------- |
| `type`              | Storage type (currently only `"s3"` supported) | No          | `S3_TYPE`                                         |
| `endpoint`          | S3-compatible endpoint URL                     | For non-AWS | `S3_ENDPOINT`                                     |
| `bucket`            | Bucket name                                    | Yes         | `S3_BUCKET`                                       |
| `region`            | AWS region or `"auto"` for R2                  | For AWS     | `S3_REGION`                                       |
| `url_prefix`        | Public URL prefix for assets                   | Yes         | `S3_URL_PREFIX`                                   |
| `access_key_id`     | S3 access key                                  | Yes         | `S3_ACCESS_KEY_ID` or `AWS_ACCESS_KEY_ID`         |
| `secret_access_key` | S3 secret key                                  | Yes         | `S3_SECRET_ACCESS_KEY` or `AWS_SECRET_ACCESS_KEY` |

## Provider Examples

### AWS S3

```toml
[storage]
bucket = "my-blog-assets"
region = "us-east-1"
url_prefix = "https://my-blog-assets.s3.amazonaws.com"
```

### Cloudflare R2

```toml
[storage]
endpoint = "https://<account-id>.r2.cloudflarestorage.com"
bucket = "my-bucket"
region = "auto"
url_prefix = "https://assets.myblog.com"  # Custom domain via R2 public access
```

### MinIO

```toml
[storage]
endpoint = "https://minio.example.com"
bucket = "blog-assets"
region = "us-east-1"
url_prefix = "https://minio.example.com/blog-assets"
```

### DigitalOcean Spaces

```toml
[storage]
endpoint = "https://nyc3.digitaloceanspaces.com"
bucket = "my-space"
region = "nyc3"
url_prefix = "https://my-space.nyc3.cdn.digitaloceanspaces.com"
```

## Content-Addressable Storage

Assets are stored using content-addressable keys for automatic deduplication:

**Key Format:** `{sha256-hash}.{extension}`

Example:

- Original: `images/screenshot.png`
- Object key: `a1b2c3d4e5f6...png`

This means:

- Identical files are uploaded only once
- Changing an image creates a new object (no cache invalidation needed)
- Old objects can be safely deleted if unreferenced

## Upload Caching

typub maintains a SQLite database (`.typub/status.db` in your project root) that tracks:

- Content hash of each uploaded asset
- Remote URL after upload
- Storage configuration used

When publishing the same content again:

1. Asset hash is computed
2. Database is checked for existing upload
3. If found with matching storage config: **skip upload**, return cached URL
4. If not found: upload to storage, record in database

This makes re-publishing near-instant when assets haven't changed.

## Asset Strategy Precedence

For platforms using `External` asset strategy (e.g., Dev.to, Hashnode, Medium):

```
Platform default → Platform config → Global config → Error
```

If no storage is configured and a platform requires external assets, publishing will fail with a clear error message.

## Security Best Practices

1. **Never commit credentials** to version control
2. Use **environment variables** for secrets
3. Use **dedicated IAM users** with minimal permissions
4. Consider **presigned URLs** for private assets
5. Enable **bucket versioning** for backup

### IAM Policy Example (AWS)

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": ["s3:PutObject", "s3:GetObject", "s3:HeadObject"],
      "Resource": "arn:aws:s3:::my-bucket/*"
    }
  ]
}
```

## Troubleshooting

### Upload Fails with "Access Denied"

1. Verify credentials: `S3_ACCESS_KEY_ID` and `S3_SECRET_ACCESS_KEY`
2. Check bucket permissions
3. Ensure endpoint URL is correct

### Assets Not Appearing

1. Check `url_prefix` is correct and publicly accessible
2. Verify bucket allows public reads (or has proper policy)
3. Check network connectivity to storage endpoint

### Duplicate Uploads

1. Check SQLite database is not corrupted: `.typub/status.db`
2. Verify storage config ID matches across runs
3. Use `typub status --list` to see recorded assets

### Environment Variables Not Recognized

Platform-specific variables take precedence. Use uppercase platform ID:

```bash
# Global credentials
export S3_ACCESS_KEY_ID="global-key"

# Platform-specific override for Dev.to
export DEVTO_S3_ACCESS_KEY_ID="devto-key"
```

## Platform-Specific Environment Variables

Each platform can have dedicated storage credentials:

| Platform | Access Key Variable         | Secret Key Variable             |
| -------- | --------------------------- | ------------------------------- |
| Dev.to   | `DEVTO_S3_ACCESS_KEY_ID`    | `DEVTO_S3_SECRET_ACCESS_KEY`    |
| Hashnode | `HASHNODE_S3_ACCESS_KEY_ID` | `HASHNODE_S3_SECRET_ACCESS_KEY` |
| Medium   | `MEDIUM_S3_ACCESS_KEY_ID`   | `MEDIUM_S3_SECRET_ACCESS_KEY`   |
| Ghost    | `GHOST_S3_ACCESS_KEY_ID`    | `GHOST_S3_SECRET_ACCESS_KEY`    |

## Related Documentation

- [Platform Guides](../platforms/README.md) — See which platforms require external storage
- [Asset Handling](../assets.md) — Understanding how different platforms handle images
