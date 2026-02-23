# AGENTS.md - Agent Guide for typub

This guide defines the default working rules for agents in this repository.

## Scope

Audience: contributors and coding agents working on typub internals.

This file is not a user guide for publishing content. User-facing documentation lives under `docs/guide/`.

## Priority References

- For full development workflow and architecture context, read `DEVELOPING_GUIDE.md`.
- For machine-specific or operator-specific notes, use `LOCAL_DEVELOPMENT.md`.

## Working Levels

### Baseline (always required)

- Follow repository guardrails in this file.
- Keep changes minimal, testable, and consistent with existing architecture.
- Use `govctl` for governed artifacts.

### Advanced (only when task requires it)

- RFC/ADR clause updates and version bumps.
- Cross-crate refactors and semantic IR evolution.
- Adapter capability model changes and migration work.

## Core Coding Rule: No `unwrap()` or `expect()` in Production

This is the strictest rule in the codebase.

### Production code (`src/**/*.rs`, excluding `#[cfg(test)]` modules)

- `unwrap()` is forbidden.
- `expect()` is forbidden.
- Use `?`, `.ok_or_else(...)`, `.with_context(...)`, or `anyhow::bail!(...)`.
- Treat even seemingly infallible operations (for example constant regexes/selectors) as fallible and propagate errors.

### Test code (`#[cfg(test)]` modules, `tests/`)

- `unwrap()` is forbidden.
- `expect("descriptive message")` is allowed and preferred for clearer failure messages.

### Enforcement

Clippy lints in `Cargo.toml` enforce this policy:

```toml
[lints.clippy]
unwrap_used = "deny"
expect_used = "deny"
```

Test modules may use `#[allow(clippy::expect_used)]` when needed.

### Quick Reference

| Context    | `unwrap()` | `expect()` | `?` operator |
| ---------- | ---------- | ---------- | ------------ |
| Production | DENIED     | DENIED     | Required     |
| Tests      | DENIED     | Allowed    | Allowed      |

## Governance Workflow Rules

Use `govctl` as the only interface for governed artifacts.

### Read Before Changing Governance Artifacts

Run these commands before edits:

```bash
govctl status
govctl work show <WI-ID>
govctl rfc show <RFC-ID>
govctl clause show <RFC-ID>:<CLAUSE-ID>
govctl adr show <ADR-ID>
```

### Governance Modeling Rules for AST Refactor

- Migration strategy decisions belong in ADRs, not RFC clauses.
- For AST v2 migration, keep policy in ADR-0013 and enforce semantics in RFC-0009 clauses.
- When retiring an RFC line, run `govctl rfc deprecate <RFC-ID>` and update `C-SUMMARY` to point to the replacement RFC.

## Local Notes Boundary

`LOCAL_DEVELOPMENT.md` should contain only local/machine-specific notes, for example:

- Local environment variables
- Local tool versions
- Machine-specific configuration
- IDE/editor preferences

Do not move machine-specific notes into this shared guide.
