# Release Notes 0.1.2

This release moves Greentic Coding Agent from repo-local indexing only toward a global, branch-aware knowledge fabric for coding agents.

## Highlights

- Global developer workflow: `init`, `sync --channel`, `status --channel`, and merged search work outside a repository.
- Branch-aware indexes: repo indexes now carry repo, branch, commit, tool, schema, and source-tree metadata.
- Producer workflow: generated GitHub Actions publish both branch tags such as `:main` or `:develop` and immutable `:sha-<commit>` tags.
- Catalogs: catalogs remain v1-compatible but can now be rebuilt as branch-aware `gca.catalog.v2` payloads with `main` and `develop` entries.
- Agent and MCP use: `serve --stdio` and `serve --http` prefer the merged global index and expose stable `gca.*` tool names.
- Watch preview: `watch` and foreground-safe `daemon` keep the global cache current and write org-level notification feed items.
- Compatibility: existing repo-local commands and v1/latest catalog behavior remain supported.

## Default Local Flow

```bash
greentic-coding-agent init --channel develop
greentic-coding-agent sync --channel develop --format json
greentic-coding-agent status --channel develop --format json
greentic-coding-agent search --mode instruction --scope merged "component manifest" --format json
greentic-coding-agent serve --stdio
```

## Producer Flow

```bash
greentic-coding-agent analyze --format json
greentic-coding-agent package-index --tag develop --tag sha-<commit> --format json
greentic-coding-agent publish-index --tag develop --tag sha-<commit> --backend ghcr --token-env GHCR_TOKEN --format json
```

## Catalog Automation

```bash
greentic-coding-agent catalog rebuild-from-ghcr --org greenticai --channel develop --format json
greentic-coding-agent catalog validate --format json
greentic-coding-agent catalog publish --channel develop --backend ghcr --token-env GHCR_TOKEN --format json
```

## Installation Status

The workspace still uses internal path crates marked `publish = false`. Do not document `cargo binstall greentic-coding-agent` as the default install path for this release.

Supported install paths for now:

```bash
cargo build --release -p greentic-coding-agent
target/release/greentic-coding-agent --help
```

GitHub release binaries are the intended public distribution path until the crates.io package layout is resolved.
