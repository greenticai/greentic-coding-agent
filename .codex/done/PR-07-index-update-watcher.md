# PR-07 — Add index update watcher with automatic download and merge

## Goal

The server must listen for index updates and automatically download and merge them.

This should work in two modes:

1. Polling mode: periodically checks GHCR catalog digests.
2. Local filesystem mode: watches cached catalog/index files for changes.

## Depends on

- PR-05 `sync-state.json`, merged index rebuild, and sync report.
- PR-06 `QueryService` / server runtime.

The watcher should orchestrate sync and query-handle swapping. It should not duplicate catalog merge, download, or changed-digest logic from PR-05.

## Commands

```bash
greentic-coding-agent serve --watch
greentic-coding-agent watch-indexes
greentic-coding-agent watch-indexes --tenant meeza --token $TOKEN
```

## Watch behaviour

When enabled:

1. Check public catalog every N seconds.
2. Check tenant catalog if configured.
3. Compare catalog digest / generated_at / repo entry digests.
4. Pull changed repo indexes.
5. Rebuild merged Tantivy index.
6. Hot-swap query handle atomically.
7. Emit server event/log.

Use `sync-state.json` as the local source of truth for what was last downloaded and merged.

## Config

Add flags:

```bash
--watch
--sync-interval 300
--strict-sync
--prune-disabled
```

Default interval:

```text
300 seconds
```

## Atomic merge

Build new merged index in temp dir:

```text
~/.greentic-agent/tantivy/merged.next/
```

Then rename:

```text
merged -> merged.previous
merged.next -> merged
```

The server should maintain an in-memory `ArcSwap` or equivalent pointer to the active query handle.

If avoiding new dependency, use:

```rust
Arc<RwLock<QueryState>>
```

Do not replace the active query handle until:

- the new merged index was built successfully
- the new index can be opened by the query engine
- `sync-state.json` was written or intentionally left unchanged after a failed sync

## Failure handling

- Failed public catalog pull: warn, keep existing index.
- Failed private repo pull: warn unless `--strict-sync`.
- Failed merge rebuild: keep previous merged index.
- Token expired: warn once per interval category, do not spam.
- Missing or corrupt `sync-state.json`: rebuild state from cached repo indexes when possible, otherwise keep serving the current handle.

## Server status

Expose:

```json
{
  "watch_enabled": true,
  "last_sync_at": "...",
  "last_sync_status": "ok",
  "indexed_repos": 52,
  "tenant": "meeza"
}
```

## Tests

- Watcher detects changed catalog fixture.
- Watcher skips unchanged catalog fixture.
- Failed pull keeps previous merged index.
- Atomic swap does not leave missing merged index.
- Status reports last sync result.
- Corrupt next index never replaces active query handle.
- Missing/corrupt sync state is handled gracefully.

## Acceptance criteria

- Long-running MCP/HTTP server updates itself without restart.
- Queries continue working during failed sync.
- Merged index is rebuilt only when something changed.
