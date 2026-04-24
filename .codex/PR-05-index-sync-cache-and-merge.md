# PR-05 — Add local cache, downloaded indexes, and merged index rebuild

## Goal

Create a proper local multi-repo cache and merge downloaded indexes into one fast query surface.

## Depends on

- PR-01 repo identity migration.
- PR-02 Tantivy local search.
- PR-03 remote backend/config abstraction.
- PR-04 catalog `enabled` and membership semantics.

## Local cache layout

```text
~/.greentic-agent/
  catalogs/
    public/catalog.json
    tenants/<tenant>/catalog.json

  indexes/
    public/<org>/<repo>/
      repo-index.json
      repo-index.cbor
      manifest.json
      tantivy/

    tenants/<tenant>/<org>/<repo>/
      repo-index.json
      repo-index.cbor
      manifest.json
      tantivy/

  tantivy/
    merged/
      meta.json
      ...

  registry.json
  sync-state.json
```

## Sync state

Add a durable cache manifest:

```rust
pub struct SyncState {
    pub version: String,
    pub updated_at: String,
    pub repos: Vec<SyncedRepoState>,
}

pub struct SyncedRepoState {
    pub repo_id: String,
    pub tenant: Option<String>,
    pub visibility: IndexVisibility,
    pub package_ref: String,
    pub digest: Option<String>,
    pub source_commit: Option<String>,
    pub downloaded_at: String,
    pub local_index_path: PathBuf,
    pub local_tantivy_path: Option<PathBuf>,
}
```

The watcher in PR-07 should read this state instead of rediscovering local cache contents heuristically.

## Commands

```bash
greentic-coding-agent sync
greentic-coding-agent sync --tenant meeza --token $TOKEN
greentic-coding-agent sync --repo greenticai/greentic-types
greentic-coding-agent sync --prune-disabled
greentic-coding-agent rebuild-merged-index
```

## Sync algorithm

1. Pull public catalog.
2. If `--tenant` is provided, pull tenant catalog.
3. Merge catalogs by `repo_id`.
4. For each enabled repo:
   - skip if local digest/source commit matches
   - otherwise pull index package
5. Rebuild merged Tantivy index.
6. Write sync report.
7. Write `sync-state.json`.

## Sync report

```rust
pub struct SyncReport {
    pub public_catalog: Option<String>,
    pub tenant_catalog: Option<String>,
    pub downloaded: Vec<String>,
    pub skipped: Vec<String>,
    pub failed: Vec<SyncFailure>,
    pub merged_index_path: PathBuf,
}
```

## Merged index

Create:

```text
~/.greentic-agent/tantivy/merged/
```

All Tantivy documents must include:

```text
repo_id
tenant
visibility
source_package_ref
```

Merged index rebuild should read cached `RepoIndex` files as canonical truth. Downloaded per-repo Tantivy directories may be reused as an optimization, but the merged output must be reproducible from cached repo indexes.

## Query behaviour

For `search`:

- Current repo local index has highest priority.
- Merged global index is searched for cross-repo results.
- Results include repo ID.

Add flags:

```bash
--scope local
--scope merged
--scope all
--repo greenticai/greentic-types
--tenant meeza
```

Default: `all`.

## Tests

- Sync skips unchanged digest.
- Sync downloads changed digest.
- Disabled repo skipped.
- `--prune-disabled` removes local disabled repo cache.
- Merged Tantivy contains docs from two repos with same short name but different org.
- `sync-state.json` records repo id, digest, package ref, source commit, tenant, visibility, and local paths.
- Merged rebuild is reproducible from cached repo indexes.

## Acceptance criteria

- Local coding agent can download public and tenant indexes.
- Merged search works across orgs and tenants.
- Sync is idempotent.
- Watcher can determine changes from catalog metadata plus `sync-state.json`.
