# PR-04 — Add/remove/enable/disable repos in the overall catalog

## Goal

Allow operators to control which repo indexes are included in the public or tenant catalog.

## Depends on

- PR-01 repo identity migration.
- PR-03 remote backend/config abstraction.

## Commands

Add command group:

```bash
greentic-coding-agent catalog show
greentic-coding-agent catalog add-repo
greentic-coding-agent catalog remove-repo
greentic-coding-agent catalog enable-repo
greentic-coding-agent catalog disable-repo
greentic-coding-agent catalog validate
greentic-coding-agent catalog publish
```

## Add repo

Public:

```bash
greentic-coding-agent catalog add-repo   --repo greenticai/greentic-types   --index-uri ghcr.io/greenticai/indexes/greenticai/greentic-types:latest
```

Tenant:

```bash
greentic-coding-agent catalog add-repo   --tenant meeza   --repo greentic-biz/meeza-store   --index-uri ghcr.io/greenticai/indexes/tenants/meeza/greentic-biz/meeza-store:latest   --token $TOKEN
```

## Remove vs disable

Implement both:

- `remove-repo`: physically removes entry.
- `disable-repo`: keeps entry but sets `enabled=false`.

Prefer `disable-repo` in docs because it preserves audit history.

## Catalog change log

Extend `Catalog`:

```rust
pub struct Catalog {
    pub version: String,
    pub generated_at: String,
    pub repos: Vec<CatalogRepo>,
    pub change_log: Vec<CatalogChange>,
}
```

Add:

```rust
pub struct CatalogChange {
    pub action: CatalogAction,
    pub repo_id: String,
    pub tenant: Option<String>,
    pub at: String,
    pub by: Option<String>,
    pub reason: Option<String>,
}
```

Actions:

```rust
pub enum CatalogAction {
    AddRepo,
    RemoveRepo,
    EnableRepo,
    DisableRepo,
    Publish,
}
```

## Publish behaviour

Commands may update local catalog only, or publish immediately:

```bash
greentic-coding-agent catalog add-repo ... --publish
```

`catalog publish` pushes:

```text
ghcr.io/greenticai/indexes/catalog:latest
```

or tenant:

```text
ghcr.io/greenticai/indexes/tenants/<tenant>/catalog:latest
```

## Concurrency and audit rules

Catalog publish must avoid silent overwrites when two operators or CI jobs update the same catalog.

Add one of:

- `--expected-digest <digest>` / `if_match` style publish checks.
- backend-specific optimistic locking when available.
- a clear conflict error when the remote catalog changed since it was read.

Before writing or publishing a catalog:

- sort repos deterministically by `repo_id`
- dedupe by `repo_id`
- reject duplicate entries with conflicting tenant/visibility/package data
- append a `CatalogChange`
- preserve disabled entries unless `remove-repo` was explicitly used

Prefer `disable-repo` in docs and examples because it keeps audit history intact.

## Tests

- Add repo to empty catalog.
- Add repo updates existing entry, does not duplicate.
- Add repo rejects conflicting duplicate `repo_id` data.
- Disable repo sets `enabled=false`.
- Remove repo deletes entry.
- Change log records action.
- Tenant catalog path is correct.
- Publish with stale `expected_digest` reports a conflict.
- Published catalog is sorted deterministically.

## Acceptance criteria

- Operators can add/remove repo indexes without editing JSON manually.
- `sync` respects `enabled=false`.
- Catalog mutation supports public and tenant catalogs.
- Concurrent catalog updates fail clearly instead of overwriting silently.
