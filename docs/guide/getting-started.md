# Getting Started

This guide walks you through installing typub and publishing your first content.

## Requirements

- **Rust 1.85+** (edition 2024)
- **Typst** (for document compilation)

## Installation

```bash
# From crates.io (when published)
cargo install typub

# Or build from source
git clone https://github.com/lucifer1004/typub
cd typub
cargo build --release
# Binary at ./target/release/typub
```

## Initialize a Content Project

```bash
typub init
```

This creates the default configuration:

```
.
├── typub.toml           # typub configuration
├── .typub/               # Status tracking database
└── posts/                # Your content directory
```

## Create Your First Post

Create a Typst document:

```typst
// posts/hello-world/content.typ
= Hello World

This is my first post published with typub!
```

And the metadata file:

```toml
# posts/hello-world/meta.toml
title = "Hello World"
created = 2026-02-12
tags = ["tutorial", "typub"]
```

## Configure a Platform

Edit `typub.toml` to enable a platform:

```toml
[platforms.devto]
enabled = true
# API key from environment: DEVTO_API_KEY
```

Set your API key:

```bash
export DEVTO_API_KEY="your-api-key-here"
```

## Publish

```bash
# Publish to Dev.to
typub publish posts/hello-world -p devto

# Or publish to all enabled platforms
typub publish posts/hello-world
```

## Development Mode

For local development with live reload:

```bash
# Start dev server with live reload
typub dev posts/hello-world -p xiaohongshu

# Or specify a custom port
typub dev posts/hello-world -p xiaohongshu --port 3000
```

## Check Status

```bash
# See what's been published where
typub status posts/hello-world
```

## Next Steps

### Basic path

- [Adapters Guide](./adapters.md) — platform setup model
- [Assets Guide](./assets.md) — image strategy basics
- [Theme Customization](./theme-customization.md) — custom CSS and theme overrides
- [Profiles Guide](./profiles.md) — copy-paste profile basics

### Advanced path

- [Advanced Customization](./advanced-customization.md) — layered overrides and advanced config
- [External Storage](./storage/README.md) — S3-compatible setup and operations
