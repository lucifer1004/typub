# Introduction

**typub** is a multi-platform content publishing pipeline for Typst documents. Write once in Typst, publish everywhere.

## Audience and Reading Paths

### User Path (publishing content)

- Start here: [Guide Overview](./guide/)
- [Getting Started](./guide/getting-started.md) for first publish
- [Adapters](./guide/adapters.md) for platform setup
- [Asset Handling](./guide/assets.md) for image strategy
- [Theme Customization](./guide/theme-customization.md) for custom CSS and overrides
- [Copy-paste Profiles](./guide/profiles.md) for manual publishing targets
- [Platforms Overview](./guide/platforms/) for per-platform instructions
- [Advanced Customization](./guide/advanced-customization.md) for layered config and advanced overrides

### Developer Path (contributing to typub)

- RFC specs: [RFC Index](./rfc/)
- ADR decisions: [ADR Index](./adr/)
- Governance history and execution trace: `docs/work/` entries

## At a Glance

```text
                      content.typ ┬ content.md
                                  │
                    ┌─────────────┴─────────────┐
                    │        typub render       │
                    │            ↓              │
                    │    Semantic Document IR   │
                    └─────────────┬─────────────┘
                                  │
           ┌────────┬────────┬────────┬────────┬────────┐
           ↓        ↓        ↓        ↓        ↓        ↓
        ┌──────┐┌──────┐┌──────┐┌──────┐┌──────┐┌──────┐
        │ Ghost││Dev.to││ Hash ││Notion││  WP  ││Conf. │
        └──────┘└──────┘└──────┘└──────┘└──────┘└──────┘
           ┌────────┬────────┬────────┐
           ↓        ↓        ↓        ↓
        ┌──────┐┌──────┐┌──────┐┌──────┐
        │ Astro││Static││  XHS ││20+ CP│
        └──────┘└──────┘└──────┘└──────┘
```

## Core Capabilities

| Feature            | Description                                            |
| ------------------ | ------------------------------------------------------ |
| **Typst-native**   | First-class support for Typst documents                |
| **Multi-platform** | Publish to 20+ platforms with one command              |
| **AST-centric**    | Unified internal representation for consistent output  |
| **Asset handling** | Automatic image embedding, upload, or external storage |
| **RFC-driven**     | Formal specifications ensure predictable behavior      |

## Supported Target Types

- API-based adapters (direct publish)
- Local-output adapters (generated local artifacts)
- Copy-paste profiles (manual publish via prepared content)

See [Adapters](./guide/adapters.md) for setup model and [Platforms](./guide/platforms/) for concrete per-platform instructions.

## Quick Start

```bash
# Install
cargo install --git https://github.com/lucifer1004/typub

# Initialize a content project
typub init

# Publish to Dev.to
typub publish path/to/post -p devto

# Preview for WeChat (copy-paste)
typub dev path/to/post -p wechat
```

## Pipeline (Conceptual)

typub processes content through a 10-stage pipeline:

1. **Resolve** — Resolve content input and metadata
2. **Render** — Render source content into HTML string
3. **Parse** — Parse HTML string into unified AST
4. **Transform** — Apply shared AST transformations
5. **Specialize** — Create platform-specific payload
6. **Provision** — Ensure remote resources exist
7. **Materialize** — Upload/resolve assets
8. **Serialize** — Convert to platform format
9. **Publish** — Send to platform API
10. **Persist** — Save publish status

Each adapter implements stages 5-9, inheriting common behavior from stages 1-4.
