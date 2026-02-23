# Adapters

Adapters are platform-specific components that transform and publish your content to each target.

## Audience

- This page is a **platform-agnostic user guide**.
- For platform-specific values and screenshots, use [Platforms Overview](./platforms/).

## Basic Setup

### 1. Enable a platform

Use `typub.toml`:

```toml
[platforms.devto]
enabled = true
published = true

[platforms.ghost]
enabled = true
published = false
```

### 2. Add platform credentials via environment variables

```bash
export DEVTO_API_KEY="..."
export GHOST_ADMIN_API_KEY="id:secret"
```

### 3. Publish

```bash
typub publish posts/my-post -p devto
```

## Adapter Categories

- API-based adapters: direct publish through remote APIs
- Local-output adapters: generate local files/artifacts
- Copy-paste profiles: generate content for manual paste workflow

See [Platforms Overview](./platforms/) for concrete platform entries.

## Common Platform Fields

```toml
[platforms.<platform_id>]
enabled = true
published = true
asset_strategy = "embed"   # optional, platform-dependent
theme = "..."              # optional
internal_link_target = "..." # optional
```

## Basic vs Advanced

### Basic

- Enable platform
- Configure required credentials
- Publish with default behavior

### Advanced

- Override asset strategy
- Use external storage
- Override node policy at platform level
- Fine-tune per-platform extra fields

See [Advanced Customization](./advanced-customization.md).

## Environment Variable Substitution

String values in `typub.toml` support shell-style environment expansion:

```toml
[platforms.hashnode]
api_key = "$HASHNODE_API_KEY"
publication_id = "${HASHNODE_PUBLICATION_ID}"
```

If a variable is not found and no default is provided, the raw value is kept unchanged.

## Related Docs

- Platform details: [Platforms Overview](./platforms/)
- Theme customization: [Theme Customization](./theme-customization.md)
- Pipeline contract: [RFC-0002](../rfc/RFC-0002.md)
