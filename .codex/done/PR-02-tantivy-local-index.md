# PR-02 — Add Tantivy local search index

## Goal

Add Tantivy as the fast local search layer while keeping `RepoIndex` as the canonical truth.

Current behaviour scans vectors in memory. That is fine for small repos, but not for 50+ repos and agent servers.

## Depends on

- PR-01 repo identity migration.

Do not add Tantivy documents or result shapes that only identify repos by short `repo_name`.

## Files to modify

- root `Cargo.toml`
- `crates/gca-index/Cargo.toml`
- `crates/gca-query/Cargo.toml`
- `crates/gca-index/src/lib.rs`
- new `crates/gca-index/src/tantivy_index.rs`
- new `crates/gca-query/src/tantivy_search.rs`
- `crates/gca-cli/src/main.rs`

## Dependencies

Add at workspace level:

```toml
[workspace.dependencies]
tantivy = "0.22"
```

Then in crates:

```toml
tantivy.workspace = true
```

## Local directory layout

```text
.greentic-agent/
  manifest.json
  repo-index.json
  fingerprints.json
  tantivy/
    local/
      meta.json
      ...
```

## Tantivy schema

Index these fields:

```rust
repo_id: STRING | STORED
path: STRING | STORED
kind: STRING | STORED
title: TEXT | STORED
body: TEXT | STORED
concept_ids: TEXT | STORED
phase: STRING | STORED
provenance: STRING | STORED
```

Kinds:

- `concept`
- `workflow`
- `instruction`
- `reuse`
- `validation`
- `code_symbol`
- `module`
- `dependency`
- `summary`

## Index build function

Add:

```rust
pub fn build_local_tantivy_index(
    repo_index: &RepoIndex,
    index_dir: &Path,
) -> Result<TantivyBuildReport, TantivyIndexError>
```

Report:

```rust
pub struct TantivyBuildReport {
    pub index_path: PathBuf,
    pub documents_indexed: usize,
}
```

## What to index immediately

From existing `RepoIndex`:

- `concept_graph`
- `workflow_graph`
- `instruction_graph`
- `validations`
- `reuse`
- `source_stats.modules`
- `source_stats.public_items`
- `source_stats.dependencies`
- `source_stats.markdown_docs`
- `source_stats.workflow_files`
- `source_stats.example_paths`

## CLI behaviour

After `analyze`, build the local Tantivy index automatically.

Output in markdown:

```text
- Tantivy documents indexed: 123
- Tantivy index: .greentic-agent/tantivy/local
```

## Query behaviour

Add a search path:

1. If Tantivy local index exists, use Tantivy.
2. Otherwise fallback to existing in-memory search.

Add CLI flag:

```bash
greentic-coding-agent search --mode instruction wizard --engine tantivy
greentic-coding-agent search --mode instruction wizard --engine fallback
greentic-coding-agent search --mode instruction wizard --engine auto
```

Default: `auto`.

## Query engine abstraction

Keep the current in-memory implementation as a first-class fallback instead of treating it as legacy code.

Add a small trait in `gca-query`:

```rust
pub trait SearchEngine {
    fn search(&self, request: SearchRequest) -> Result<SearchResponse, SearchError>;
}
```

Implement:

- `FallbackSearchEngine`, backed by the existing `RepoIndex` vector scan.
- `TantivySearchEngine`, backed by the local Tantivy index.
- `AutoSearchEngine`, which selects Tantivy when available and falls back otherwise.

`SearchResult` should include:

```rust
pub struct SearchResult {
    pub repo_id: String,
    pub tenant: Option<String>,
    pub visibility: Option<IndexVisibility>,
    ...
}
```

For local-only indexes, `tenant` and `visibility` may be `None`, but `repo_id` must be present.

## Tests

- Build index from synthetic `RepoIndex`.
- Query concept by title.
- Query workflow by command.
- Query instruction by path.
- Fallback works when no Tantivy index exists.
- Tantivy and fallback return compatible JSON response shapes.
- Results include `repo_id` even before merged/global search exists.

## Acceptance criteria

- `analyze` creates `.greentic-agent/tantivy/local`.
- `search --engine auto` uses Tantivy when present.
- Existing JSON output shape is preserved.
- Results include `repo_id` and provenance.
