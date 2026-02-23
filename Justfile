set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]
set shell := ["bash", "-cu"]
set dotenv-load

# =============================================================================
# Build & Development
# =============================================================================

[unix]
pre-commit:
    @if command -v prek > /dev/null 2>&1; then prek run --all-files; else pre-commit run --all-files; fi

[windows]
pre-commit:
    if (Get-Command prek -ErrorAction SilentlyContinue) { prek run --all-files } else { pre-commit run --all-files }

# Build release binary
build:
    cargo build --release

# Run tests
test:
    cargo test

# Test coverage (summary)
cov:
    cargo llvm-cov --workspace --all-features --summary-only

# Test coverage (full report)
cov-html:
    cargo llvm-cov --workspace --all-features --html
    @echo "Coverage report: target/llvm-cov/html/index.html"

# Test coverage (lcov format for CI)
cov-lcov:
    cargo llvm-cov --workspace --all-features --lcov --output-path lcov.info

# Update snapshots
update-snapshots:
    cargo insta test --workspace --all-features --accept

# Run clippy lints
lint:
    cargo clippy --all-targets

# Format code
fmt:
    cargo fmt

# =============================================================================
# Documentation
# =============================================================================

# Build mdbook documentation
book:
    ./scripts/build-book.sh

# Serve mdbook with live reload
book-serve:
    ./scripts/build-book.sh --serve

# Build book without re-rendering governance artifacts
book-quick:
    ./scripts/build-book.sh --skip-render

# =============================================================================
# Quick Shortcuts
# =============================================================================

# Show governance status
status:
    govctl status

# Validate all governed documents
check:
    govctl check

# Render RFCs to markdown
render:
    govctl render

# Local development
claude args="":
    claude {{args}}
