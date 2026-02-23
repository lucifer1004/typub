# Advanced Customization

This page groups advanced, platform-agnostic customization options for typub users.

Use this after you have completed the basic flow in [Getting Started](./getting-started.md).

## 1. Configuration Resolution Layers

Many fields follow layered resolution. Highest priority wins:

1. `meta.toml` platform-specific (`meta.platforms.<id>.*`)
2. `meta.toml` post-level defaults (field-dependent)
3. `typub.toml` platform-specific (`platforms.<id>.*`)
4. `typub.toml` global defaults (field-dependent)
5. Adapter/default fallback

See [RFC-0005](../rfc/RFC-0005.md) for normative rules.

## 2. Asset Strategy and Storage

### Per-platform strategy

```toml
[platforms.devto]
asset_strategy = "external"
```

### External storage

```toml
[storage]
endpoint = "https://s3.amazonaws.com"
bucket = "my-bucket"
region = "us-east-1"
url_prefix = "https://cdn.example.com"
```

Use environment variables for secrets.

See [External Storage](./storage/README.md) and [RFC-0004](../rfc/RFC-0004.md).

## 3. Node Policy Override

You can override adapter default node policy per platform.

Supported actions:

- `pass`
- `sanitize`
- `drop`
- `error`

### In `typub.toml`

```toml
[platforms.wechat]
node_policy = { raw = "sanitize", unknown = "drop" }
```

### In `meta.toml` (higher priority)

```toml
[platforms.wechat]
node_policy = { raw = "error" }
```

Partial override is allowed. Unset fields fall back to lower layers and then adapter defaults.

## 4. Copy-paste Profile Extension (Contributor-Level)

If you need new built-in copy-paste profile behavior:

- Edit `crates/adapters/typub-adapter-copypaste/profiles.toml`
- Adjust compat behavior in `crates/adapters/typub-adapter-copypaste/src/adapter.rs` when needed

This is a repository customization workflow, not a runtime per-project override.

## 5. Advanced Debugging

Use dry-run and stage dump to inspect behavior:

```bash
typub publish posts/my-post -p wechat -d -v
typub publish posts/my-post -p wechat -d -D transform
```

## 6. Typst Preamble Override

typub supports user-defined Typst preamble via `preamble`, resolved with the
same layered model as other fields:

1. `meta.toml[platforms.<id>].preamble`
2. `meta.toml.preamble`
3. `typub.toml[platforms.<id>].preamble`
4. `typub.toml.preamble`
5. adapter default preamble

When a user preamble is resolved, typub appends it after adapter preamble to
preserve platform-specific defaults.

See [Theme Customization](./theme-customization.md#typst-custom-header-current-model).

## Related Docs

- [Adapters](./adapters.md)
- [Asset Handling](./assets.md)
- [Theme Customization](./theme-customization.md)
- [Copy-paste Profiles](./profiles.md)
- [RFC-0002](../rfc/RFC-0002.md)
- [RFC-0005](../rfc/RFC-0005.md)
- [RFC-0009](../rfc/RFC-0009.md)
