# PR 04 — Make local sync/cache/status the default consumer workflow

## Position in sequence

Implement after PR 02 introduces branch-aware catalog/index metadata and after PR 03 can publish branch tags.

Current local reality before this PR:

- User-global state currently lives under `~/.greentic-agent/`.
- Sync cache layout currently uses `remote-oci`, `cache-oci`, `indexes`, `sync-state.json`, and `tantivy/merged`.
- `sync` works outside a repo, but it has no `--channel`.
- There is no top-level `init` command.
- There is no top-level `status` command, though HTTP server mode has `GET /status`.
- Merged search exists via `search --scope merged|all`.

## Goal

Developers should install once and maintain a machine-level merged Greentic knowledge cache.

## Local cache layout

Implement or document and enforce. This is a breaking path change from `~/.greentic-agent`; include migration or compatibility reads.

```text
~/.greentic/coding-agent/
  config.toml
  catalogs/
    greenticai-main.json
    greenticai-develop.json
  indexes/
    greenticai/
      greentic-pack/
        main/
        develop/
  merged/
    main/
    develop/
  notifications/
    seen.json
    feed.json
```

Migration requirements:

- Continue reading existing `~/.greentic-agent/sync-state.json` and index caches for one compatibility period.
- Either migrate to `~/.greentic/coding-agent/` on `init`/`sync`, or keep the existing path and document why this PR’s proposed path is deferred.
- Avoid deleting old cache data automatically.

## Commands

Add or complete:

```bash
greentic-coding-agent init
greentic-coding-agent sync --channel develop
greentic-coding-agent sync --channel main
greentic-coding-agent status --channel develop
greentic-coding-agent search --scope merged "component manifest"
```

The current search syntax requires `--mode`, so either keep that requirement:

```bash
greentic-coding-agent search --mode instruction --scope merged "component manifest"
```

or explicitly add a default mode for search.

## Status output

JSON status should include:

```json
{
  "channel": "develop",
  "catalog": "ghcr.io/greenticai/indexes/catalog:develop",
  "repos": [
    {
      "repo_id": "greenticai/greentic-pack",
      "branch": "develop",
      "commit_sha": "...",
      "indexed_at": "...",
      "fresh": true
    }
  ]
}
```

## Sync behavior

- Download only changed indexes.
- Verify metadata matches catalog.
- Rebuild merged search index after changes.
- Keep old index until new one is fully written.
- Support public and tenant catalogs.

## Acceptance criteria

- `sync --channel develop` works without being inside a repo.
- `status` works without being inside a repo.
- Merged search uses synced indexes.
- Existing repo-local search still works.
- Existing tag-based `sync --repo <repo> --tag latest` remains compatible.

## Implementation notes

- Implemented `init`, `status`, and `sync --channel <branch>` on the CLI.
- Kept `~/.greentic-agent` as the compatibility-period home instead of migrating to `~/.greentic/coding-agent`; `init` documents this in its JSON response and creates the required cache/config directories there.
- Stored normalized synced indexes under branch/channel-specific paths such as `~/.greentic-agent/indexes/public/<org>/<repo>/<branch>/repo-index.json`.
- Extended sync state with channel, branch, and indexed-at metadata so `status --channel <branch>` can report freshness outside a repository.
- Kept existing tag-based `sync --repo <repo> --tag <tag>` behavior compatible; tag-only repos are cached under the tag name.
- Added focused coverage for no-repo `init`/`status`, no-repo `sync --channel develop`, branch-specific cache paths, unchanged-skip behavior, tenant cache paths, and compatibility recovery.
