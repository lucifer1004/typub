# Release Guide for typub Multi-Crate Workspace

This guide explains how to publish the typub crates to crates.io using cargo-release.

## Overview

typub is a workspace containing **26 crates**:

- **1 main binary crate**: `typub` (CLI tool)
- **15 core library crates**: typub-core, typub-ir, typub-html, etc.
- **10 adapter crates**: typub-adapter-ghost, typub-adapter-devto, etc.

**Version Strategy**: "Minor sync, Patch independent"

Each crate has its own version number in `Cargo.toml`, but with a coordinated strategy:

- **Minor versions**: synced across all crates (e.g., all crates move from 0.1.x to 0.2.0 together)
- **Patch versions**: independent per crate (e.g., typub-ir can be 0.2.1 while typub-html stays at 0.2.0)

This allows:

1. **Coordinated feature releases** - All crates align on minor versions for major features
2. **Independent bugfix releases** - Only affected crates need patch releases
3. **Automatic dependency updates** - When a crate is released, dependents auto-update

```toml
# Example: crates/typub-ir/Cargo.toml
[package]
name = "typub-ir"
version = "0.2.1"  # Independent version

# Internal dependencies use workspace format
[dependencies]
typub-core = { workspace = true }  # No explicit version needed
```

## Prerequisites

1. **Install cargo-release**:

```bash
cargo install cargo-release
```

2. **Verify crates.io credentials**:

```bash
# Login to crates.io (if not already)
cargo login

# Verify token
cargo registry token crates-io
```

3. **Ensure clean git state**:

```bash
git status
# All changes should be committed
```

4. **Verify you're on main branch**:

```bash
git branch
# Should show * main
```

## Understanding the Configuration

The `release.toml` file implements the "Minor sync, Patch independent" strategy:

### Key Settings

```toml
# Allow different patch versions (0.1.0 vs 0.1.1)
shared-version = false

# Auto-update dependencies when a crate is released
dependent-version = "upgrade"

# Consolidate all version bumps into one commit
consolidate-commits = true

# Tag format per crate
tag-name = "{{crate_name}}-v{{version}}"
```

### Publishing Strategy

Two workflows depending on the change scope:

**Scenario 1: New features (Minor version)**

```bash
# Sync all crates to new minor version
cargo release minor --execute
```

**Scenario 2: Bug fixes (Patch version)**

```bash
# Release only the affected crate
cargo release patch -p typub-adapter-confluence --execute
```

### Publishing Order

cargo-release automatically determines publishing order based on dependencies:

1. **First tier** (no internal dependencies):
   - typub-core
   - typub-log
   - typub-config
   - typub-ir

2. **Second tier** (depend on first tier):
   - typub-html
   - typub-storage
   - typub-theme
   - typub-assets-ast
   - typub-markdown
   - typub-passes

3. **Third tier** (depend on second tier):
   - typub-adapters-core
   - typub-ui
   - typub-engine
   - typub-project
   - typub-tui

4. **Fourth tier** (adapters, depend on third tier):
   - All typub-adapter-\* crates

5. **Last** (depends on everything):
   - typub (main binary)

## Release Process

### Step 1: Determine Release Type

**Minor version bump** (sync all crates):

- Major new features
- Breaking changes that affect multiple crates
- Planned coordinated release

**Patch version bump** (single crate):

- Bug fixes in one crate
- Small improvements
- Documentation updates

```bash
# Check what changed since last release
git log --oneline --since="2 weeks ago" -- crates/typub-adapter-confluence/

# Option: Use dry-run to see what will happen
cargo release minor  # See minor sync effect
cargo release patch -p typub-adapter-confluence  # See patch effect
```

### Step 2: Verify Package Metadata

Ensure each crate has required metadata for crates.io:

```bash
# Check a specific crate
cargo metadata --format-version 1 | jq '.packages[] | select(.name == "typub-adapter-confluence")'
```

Each crate must have:

- `description`
- `license`
- `repository`
- `homepage` (optional but recommended)

### Step 3: Run Tests

```bash
# Run tests for the specific crate
cargo test -p typub-adapter-confluence

# Or run all tests if changes affect multiple crates
cargo test --workspace

# Verify docs build
cargo doc -p typub-adapter-confluence --no-deps
```

### Step 4: Dry Run

Always dry-run first to see what will happen:

```bash
# Scenario 1: Minor version sync (all crates)
cargo release minor

# Scenario 2: Patch for specific crate
cargo release patch -p typub-adapter-confluence

# Scenario 3: Patch for multiple crates
cargo release patch -p typub-ir -p typub-html
```

For version bumps:

- `patch`: Bug fixes (0.2.0 → 0.2.1) - independent per crate
- `minor`: New features (0.1.x → 0.2.0) - syncs all crates
- `major`: Breaking changes (0.x.x → 1.0.0) - syncs all crates

**Note**: By default, cargo-release runs in dry-run mode. You must add `--execute` to actually publish. When doing minor releases, all crates' patch versions reset to 0.

### Step 5: Review Changes

The dry run will show:

- Version bump for the specific crate
- Git commit to be created
- Git tag to be created (e.g., `typub-adapter-confluence-v0.1.1`)
- Package to be published to crates.io

### Step 6: Execute Release

If dry run looks correct, execute the release:

```bash
# Scenario 1: Minor version sync
cargo release minor --execute

# Scenario 2: Independent patch release
cargo release patch -p typub-adapter-confluence --execute
```

The process will:

1. Bump version(s) as appropriate
2. Update dependencies (if `dependent-version = "upgrade"`)
3. Create a consolidated git commit
4. Create git tags (e.g., `typub-adapter-confluence-v0.2.1`)
5. Publish crate(s) to crates.io in dependency order
6. Push to git remote

### Step 7: Verify Dependencies Updated

When using `dependent-version = "upgrade"`, cargo-release automatically updates dependent crates' `Cargo.toml` files. Verify:

```bash
# Check that dependencies were updated
git diff HEAD~1 -- '**/Cargo.toml'

# Verify the crate works with new dependencies
cargo test -p typub-html
```

### Step 8: Verify Publication

After release completes:

```bash
# Check published crate
cargo search typub-adapter-confluence

# Verify on crates.io
open https://crates.io/crates/typub-adapter-confluence
```

## Releasing Multiple Crates

### Patch Releases (Independent)

If multiple crates need patch releases, release them individually:

```bash
# Release each affected crate
cargo release patch -p typub-ir --execute
cargo release patch -p typub-html --execute
```

### Minor Releases (Sync All)

For coordinated feature releases, sync all crates:

```bash
# This will bump all crates to the next minor version
cargo release minor --execute

# Example result:
# typub-core: 0.1.5 → 0.2.0
# typub-ir: 0.1.3 → 0.2.0
# typub-html: 0.1.8 → 0.2.0
# All patch versions reset to 0
```

### Automating Multi-Crate Releases

For complex releases, create a script:

```bash
#!/bin/bash
# release-adapter.sh

CRATE=$1
VERSION=${2:-patch}

# Check dependencies are published
cargo fetch -p $CRATE || exit 1

# Dry run first
cargo release -p $CRATE $VERSION || exit 1

read -p "Proceed with release? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]
then
    cargo release -p $CRATE $VERSION --execute
fi
```

### Force Re-publish

If a publish failed and you need to retry (same version):

```bash
# Re-publish without version bump
cargo publish -p typub-adapter-confluence

# Or with cargo-release (will skip if already exists)
cargo release -p typub-adapter-confluence --execute 0.1.1
```

### Update Version Without Publishing

To bump version and create tag without publishing:

```bash
# Dry run to see what would happen
cargo release -p typub-adapter-confluence patch --no-publish

# Execute
cargo release -p typub-adapter-confluence patch --no-publish --execute
```

## Troubleshooting

### "Crate already exists"

The crate version was already published. Bump the version:

```bash
# Bump and try again
cargo release --workspace patch
```

### "Dependency not found"

A dependency hasn't been published yet. cargo-release handles this automatically, but if it fails:

```bash
# Publish dependencies first
cargo publish -p typub-ir
cargo publish -p typub-html
# Then publish the dependent crate
cargo publish -p typub-adapters-core
```

### "Not authorized"

Verify your crates.io token:

```bash
cargo login
```

### "Git working directory not clean"

Commit or stash your changes:

```bash
git status
git add -A
git commit -m "chore: prepare for release"
```

### "Publish failed but version was bumped"

Manually continue publishing:

```bash
# Check which crates need publishing
cargo publish --dry-run -p typub-ir

# Publish remaining crates
cargo publish -p typub-ir
cargo publish -p typub-html
# ... continue in dependency order
```

### "Dependency version mismatch"

If internal dependency versions are inconsistent:

```bash
# Check all Cargo.toml files
grep -r "version" crates/*/Cargo.toml

# All should use version.workspace = true
# If not, update them manually or run:
cargo release --workspace --dry-run patch
```

## Advanced Configuration

### Skip Publishing for Specific Crates

Add to the crate's `Cargo.toml`:

```toml
# In crates/typub-tui/Cargo.toml
[package.metadata.release]
# Don't publish this crate to crates.io
publish = false
```

### Custom Version for Specific Crate

Remove `version.workspace = true` from the crate's Cargo.toml, add `version = "x.y.z"` directly, and disable shared-version in the crate's metadata:

```toml
# In crates/typub-ir/Cargo.toml
[package]
# version.workspace = true  # Remove this line
version = "0.2.0"            # Add explicit version

[package.metadata.release]
shared-version = false       # Opt out of workspace shared version
```

Note: This is generally not recommended as it breaks version consistency across the workspace.

### Pre-release Versions

For alpha/beta releases:

```bash
# Create alpha release
cargo release --workspace 0.2.0-alpha.1

# Create beta release
cargo release --workspace 0.2.0-beta.1

# Create release candidate
cargo release --workspace 0.2.0-rc.1
```

### Custom Changelog

For changelog generation, consider using tools like `git-cliff` separately before releases. cargo-release's changelog features are limited and may require additional setup. For workspace projects, it's often easier to manage CHANGELOG.md manually or via CI scripts.

See the [cargo-release documentation](https://github.com/crate-ci/cargo-release/blob/master/docs/reference.md) for the latest changelog configuration options.

## Best Practices

1. **Follow the version strategy**:
   - Use `minor` for coordinated feature releases (all crates sync)
   - Use `patch -p <crate>` for independent bugfix releases

2. **Always dry-run first**:

```bash
cargo release minor  # See what minor sync will do
cargo release patch -p typub-ir  # See what patch will do
```

3. **Understand the "patch reset" rule**:
   - When you do a minor release, all patch versions reset to 0
   - typub-ir 0.1.5 + minor → 0.2.0 (not 0.2.5)

4. **Run tests before release**:

```bash
cargo test -p <crate-name>
# or
cargo test --workspace
```

5. **Keep CHANGELOG updated**: Document changes for each crate

6. **Use semantic versioning correctly**:
   - **Patch** (0.2.0 → 0.2.1): Bug fixes, independent per crate
   - **Minor** (0.1.x → 0.2.0): New features, syncs all crates
   - **Major** (0.x.x → 1.0.0): Breaking changes, syncs all crates

7. **Document breaking changes**: In CHANGELOG and commit messages

8. **Tags are per-crate**: cargo-release creates tags like `typub-ir-v0.2.1`

9. **Verify after publish**:

```bash
cargo search typub-adapter-confluence
```

10. **Let dependent-version do its job**: Don't manually update dependency versions in Cargo.toml

11. **Check dependencies updated correctly**:

```bash
git diff HEAD~1 -- '**/Cargo.toml'
```

12. **Coordinate major version bumps**: Breaking changes require communication with users

## Workflow Summary

### Patch Release (Single Crate)

```bash
# 1. Identify changed crate
git log --oneline -- crates/typub-adapter-confluence/

# 2. Run tests
cargo test -p typub-adapter-confluence

# 3. Dry run
cargo release patch -p typub-adapter-confluence

# 4. Review output, then execute
cargo release patch -p typub-adapter-confluence --execute

# 5. Verify
cargo search typub-adapter-confluence

# Tag is already created and pushed by cargo-release
```

### Minor Release (All Crates)

```bash
# 1. Run all tests
cargo test --workspace

# 2. Dry run
cargo release minor

# 3. Review: all crates will sync to next minor version
#    e.g., 0.1.x → 0.2.0, patches reset to 0

# 4. Execute
cargo release minor --execute

# 5. Verify all crates published
cargo search typub-ir
cargo search typub-html
cargo search typub
```

## Quick Reference

| Task                         | Command                                                      |
| ---------------------------- | ------------------------------------------------------------ |
| List all crates              | `cargo metadata --format-version 1 \| jq '.packages[].name'` |
| Check dependencies           | `cargo tree -i <crate-name>`                                 |
| Minor sync (dry-run)         | `cargo release minor`                                        |
| Minor sync (execute)         | `cargo release minor --execute`                              |
| Patch single crate (dry-run) | `cargo release patch -p <crate>`                             |
| Patch single crate (execute) | `cargo release patch -p <crate> --execute`                   |
| Search published             | `cargo search <crate-name>`                                  |
| View on crates.io            | `open https://crates.io/crates/<crate-name>`                 |

## Additional Resources

- [cargo-release documentation](https://github.com/crate-ci/cargo-release)
- [cargo-workspace documentation](https://doc.rust-lang.org/cargo/reference/workspaces.html)
- [crates.io publishing guide](https://doc.rust-lang.org/cargo/publishing.html)
- [Semantic Versioning](https://semver.org/)

## Need Help?

- Check the `release.toml` comments for configuration details
- Run `cargo release --help` for command options
- Open an issue on GitHub if you encounter problems
