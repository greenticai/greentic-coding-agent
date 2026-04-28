# PR 06 — Add watch/daemon mode and notification feed

## Position in sequence

Implement after PR 04/05 so notifications can be keyed by channel and global agent context can surface them.

Current local reality before this PR:

- There is `watch-indexes` and `serve --watch`; there is no top-level `watch` or `daemon`.
- User-local update seen state exists at `~/.greentic-agent/agent-knowledge-state.json`.
- `updates --new` works for repo knowledge updates, but has no `--scope org`.
- There is no notification feed schema or feed path.

## Goal

Keep local indexes current and notify coding agents when Greentic knowledge changes.

## Commands

```bash
greentic-coding-agent watch --channel develop --poll 10m
greentic-coding-agent daemon --channel develop --poll 10m
greentic-coding-agent updates --new --scope org --format json
greentic-coding-agent updates mark-seen --all
```

If full daemonization is too much for this PR, implement foreground `watch` first and leave service install docs for later.

Command compatibility choices:

- Either add `watch` as a friendlier alias around existing `watch-indexes`, or rename only with a compatibility alias.
- `daemon` should be foreground-safe first; true OS service installation can be a later PR.
- Extend `updates` with `--scope repo|org` only after the notification feed is implemented.

## Notification feed

Write:

```text
~/.greentic/coding-agent/notifications/feed.json
~/.greentic/coding-agent/notifications/seen.json
```

If PR 04 keeps the existing `~/.greentic-agent` path, use that path instead. Do not split notification state into a second home root without migration.

Schema:

```json
{
  "schema_version": "gca.notifications.v1",
  "items": [
    {
      "id": "greenticai/greentic-pack/develop/<sha>",
      "repo_id": "greenticai/greentic-pack",
      "branch": "develop",
      "type": "index_updated",
      "title": "greentic-pack develop index updated",
      "old_commit": "...",
      "new_commit": "...",
      "created_at": "...",
      "agent_impact": "Review updated validation guidance before editing packs."
    }
  ]
}
```

## Behavior

- Poll catalog.
- Detect repo branch commit changes.
- Download changed indexes.
- Rebuild merged index.
- Append notification items.
- Avoid duplicate notifications.

## Acceptance criteria

- `watch` detects catalog changes.
- `updates --new --scope org` reads notification feed.
- Mark-seen works across org-level update items.
- Existing repo-level `updates --new` behavior remains intact.

## Implementation notes

- Added top-level `watch` and foreground-safe `daemon` commands as friendly aliases around the existing watch loop.
- Added `--channel` to `watch-indexes`, `watch`, and `daemon`; added `--poll` parsing for `s`, `m`, and `h` intervals on the new commands.
- Watch ticks now compare sync state before and after sync and append deduplicated org notification items to `~/.greentic-agent/notifications/feed.json`.
- Added `updates --scope org --new` and `updates mark-seen --scope org --all` using `~/.greentic-agent/notifications/seen.json`.
- Kept repo-level `updates --new` behavior as the default scope.
