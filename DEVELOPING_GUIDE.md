# Developing Guide for typub

A practical guide for day-to-day development, debugging, and governance in the typub repository.

Audience: typub contributors and maintainers.

For user publishing workflows, read `docs/guide/getting-started.md` first.

## Read This First

- Core agent/development guardrails: `AGENTS.md` (symlink to `CLAUDE.md`)
- Local-only machine notes: `LOCAL_DEVELOPMENT.md`
- Project governance artifacts: `gov/`

## Level 1: Basic Contributor Loop

```bash
# Build and test
cargo build
cargo test --workspace

# Format and lint
cargo fmt
cargo clippy --all-targets

# Or via just
just test
just fmt
just lint
```

## Level 1: CLI Smoke Workflows

Use these commands to validate behavior during development:

```bash
# Initialize current directory as a typub project
typub init

# Create a post
typub new "My Post"

# List posts (alias: ls)
typub list
typub ls -u                # only posts with pending platforms
typub ls -p ghost -u       # pending on ghost

# Development mode (live preview server for one platform)
typub dev posts/my-post -p ghost

# Publish
typub publish posts/my-post
typub publish posts/my-post -p ghost

# Dry-run publish (recommended when debugging)
typub publish posts/my-post -d -v

# Inspect publish status
typub status posts/my-post
typub status posts/my-post --assets

# Interactive dashboard
typub tui
```

If you run through Cargo during development:

```bash
cargo run -- <subcommand>
# Example:
cargo run -- publish posts/my-post -d -v
```

## Level 2: Advanced Debugging

### 1. Start with dry-run

Use dry-run first when diagnosing conversion/publish issues:

```bash
typub publish posts/my-post -d -v
```

### 2. Dump intermediate pipeline stage

Use `-D/--debug-stage` to inspect output after a specific stage.

```bash
# By stage number
typub publish posts/my-post -d -D 4

# By stage name
typub publish posts/my-post -d -D parse
typub publish posts/my-post -d -D transform
typub publish posts/my-post -d -D serialize
```

Valid stage names: `resolve`, `render`, `parse`, `transform`, `specialize`, `provision`, `materialize`, `serialize`, `publish`, `persist`.

### 3. Narrow scope to one platform

```bash
typub publish posts/my-post -p notion -d -v
```

This removes cross-platform noise and speeds up root-cause analysis.

### 4. Use targeted logging

```bash
RUST_LOG=debug typub publish posts/my-post -d
RUST_LOG=typub_engine=trace typub publish posts/my-post -d
```

In Rust code, prefer `tracing` over ad-hoc prints:

```rust
use tracing::{debug, info, warn, error};

debug!("processing element: {:?}", element);
info!("publish completed");
```

## Level 2: Advanced Testing

```bash
# Full workspace tests
cargo test --workspace

# Single package
cargo test -p typub-adapter-confluence

# Single test with output
cargo test test_name -- --nocapture

# Snapshot updates
just update-snapshots
```

## Repository Structure

```text
typub/
├── src/                       # CLI entry and command wiring
├── crates/
│   ├── typub-engine/          # Build/publish pipeline
│   ├── typub-core/            # Shared domain models
│   ├── typub-html/            # HTML/IR types
│   ├── typub-markdown/        # Markdown conversion/parsing
│   ├── typub-config/          # Config loading and validation
│   ├── typub-storage/         # Persistent state/status tracking
│   ├── typub-tui/             # TUI app
│   └── adapters/              # Platform adapters (ghost/notion/etc.)
├── tests/                     # Integration tests
├── gov/                       # RFCs, ADRs, work items
└── scripts/                   # Developer tooling scripts
```

## Level 3: Governance and Spec Work

Use `govctl` as the interface for governed artifacts.

### Read before editing governance docs

```bash
govctl status
govctl work show <WI-ID>
govctl rfc show <RFC-ID>
govctl clause show <RFC-ID>:<CLAUSE-ID>
govctl adr show <ADR-ID>
```

### Typical governance loop

```bash
# Create and activate work item
govctl work new --active "Task description"

# Update acceptance criteria
govctl work add WI-ID acceptance_criteria "add: Feature implemented"
govctl work tick WI-ID acceptance_criteria "Feature implemented" -s done

# Complete item
govctl work move WI-ID done

# Render and validate
govctl render
govctl check
```

### AST refactor governance constraints

- Keep migration strategy decisions in ADRs, not RFC clauses.
- For AST v2 migration, policy belongs in ADR-0013 and semantic constraints in RFC-0009 clauses.
- When retiring an RFC line, run:

```bash
govctl rfc deprecate <RFC-ID>
```

Then update `C-SUMMARY` to point to the replacement RFC.

## Version Control

Project workflow uses Jujutsu (`jj`) primarily.

```bash
jj status
jj diff
jj log
jj new
jj describe -m "message"
```

## Non-Negotiable Error Handling Rule

Production Rust code must not use `unwrap()` or `expect()`.

- Production (`src/**/*.rs`, excluding test modules): use `?`, `.ok_or_else(...)`, `.with_context(...)`, `anyhow::bail!(...)`.
- Tests: `unwrap()` still forbidden; `expect("descriptive message")` allowed.

Clippy enforces this in workspace lints.
