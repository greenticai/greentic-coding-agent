# PR-08 — Improve Repo Extraction with Cargo Metadata and Rust Symbols

## Objective

Make indexing more accurate by replacing brittle string parsing with structured extraction.

## Cargo metadata

Add module:

```text
crates/gca-index/src/extract/cargo_metadata.rs
```

Use `cargo metadata --format-version 1 --no-deps` initially.

Extract:

- workspace members
- package names
- dependencies
- dev-dependencies
- features
- targets
- crate root paths
- test targets

Keep graceful fallback if `cargo metadata` fails.

The current `SourceStats` already contains workspace members, crate names, modules, public items, test targets, feature names, dependencies, markdown docs, workflow files, and example paths from lighter-weight extraction. Treat `cargo metadata` as the authoritative replacement/supplement for the Cargo-related fields while preserving the serialized shape unless a new optional field is necessary.

## Rust symbols

Add module:

```text
crates/gca-index/src/extract/rust_symbols.rs
```

Use `syn` initially, not rust-analyzer.

Add `syn` as a workspace dependency with the features needed for parsing full Rust files. Keep it out of published crate dependencies unless the PR-01 package strategy has already made workspace library dependencies publish-safe.

Extract:

- public functions
- structs
- enums
- traits
- impl blocks where practical
- modules
- pub uses
- test functions

Add to `SourceStats` or add a new richer model if needed:

```rust
pub struct RustSymbolDescriptor {
    pub name: String,
    pub kind: RustSymbolKind,
    pub visibility: String,
    pub path: String,
    pub line: Option<u32>,
}
```

If adding this to `RepoIndex` or `SourceStats`, use `#[serde(default)]` for backward compatibility and update schemas/examples accordingly.

## Search

Improve `search --mode code` to use structured symbols.

## Acceptance criteria

- Cargo workspace extraction handles inherited workspace dependencies and multiple members.
- Rust symbol extraction handles normal public items and tests.
- Existing source stats remain backward compatible.
- Search code mode improves without breaking old output.

## Codex prompt

```text
Improve greentic-coding-agent repo extraction by using cargo metadata and structured Rust symbol parsing.

Replace or supplement manual Cargo.toml parsing with `cargo metadata --format-version 1 --no-deps`. Extract workspace members, packages, dependencies, features, targets, crate roots, and tests. Add graceful fallback if cargo metadata fails.

Add a `syn`-based Rust symbol extractor for public functions, structs, enums, traits, modules, pub uses, impls where practical, and tests. Improve code search to use structured symbols.

Preserve backward compatibility for existing SourceStats/RepoIndex JSON. Add tests for workspaces, inherited dependencies, multiple crates, public symbols, pub(crate), re-exports, and tests.

Run fmt, clippy, and tests.
```
