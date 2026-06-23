# Contributing to lanner

Thanks for your interest in improving lanner.

## Getting started

1. Install the build and runtime dependencies (see the README).
2. Fork and clone the repository.
3. Build with `cargo build` and run the tests with `cargo test`.

## Before you open a pull request

Run these locally and make sure they pass:

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

This project forbids `unsafe`, denies `unwrap`, `expect`, and `panic`, and keeps
the code formatted with `rustfmt`. CI runs the same checks.

## Style

- British spelling in identifiers, comments, and documentation.
- No em-dashes or en-dashes anywhere. Use hyphens, commas, or parentheses.
- Keep functions small and focused. Prefer the standard library and existing
  dependencies over adding new ones.

## Commit messages

Use Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `chore:`).
Write in plain, human prose. Do not add tool or assistant attribution.

## Scope

See the roadmap in the README for planned milestones. If you want to take on
something large, open an issue first so we can agree on the approach.
