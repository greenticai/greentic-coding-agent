# Migration Guide 0.1.2

## What Changed

Greentic Coding Agent now supports a machine-level global cache in addition to repo-local analysis.

Existing repo-local workflows still work:

```bash
greentic-coding-agent analyze --print --format json
greentic-coding-agent describe --here --format json
greentic-coding-agent search --mode instruction wizard --format json
greentic-coding-agent catalog validate --format json
```

New global workflows use branch/channel-aware sync:

```bash
greentic-coding-agent init --channel develop
greentic-coding-agent sync --channel develop --format json
greentic-coding-agent status --channel develop --format json
```

## Cache And State

This release keeps the compatibility-period home under:

```text
~/.greentic-agent/
```

Important paths:

```text
~/.greentic-agent/config.toml
~/.greentic-agent/sync-state.json
~/.greentic-agent/indexes/
~/.greentic-agent/tantivy/merged/
~/.greentic-agent/notifications/feed.json
~/.greentic-agent/notifications/seen.json
```

Existing `~/.greentic-agent` data is not deleted automatically.

## Catalog Compatibility

Flat v1 catalogs remain readable. New branch-aware catalogs may use:

```json
{
  "schema_version": "gca.catalog.v2",
  "default_channel": "develop",
  "repos": []
}
```

Legacy `latest` tag behavior remains supported, but `main`, `develop`, and `sha-<commit>` tags are preferred for newly published indexes.

## Agent Hosts

MCP hosts should prefer the stable `gca.*` tool names:

```text
gca.search
gca.agent_context
gca.find_owner
gca.required_validations
gca.recent_updates
gca.branch_status
```

Existing older tool names such as `search_all`, `locate_owner`, and `required_validations` remain available for compatibility.

## Watch Notifications

Org-level update notifications are separate from repo-authored knowledge updates:

```bash
greentic-coding-agent watch --channel develop --poll 10m
greentic-coding-agent updates --new --scope org --format json
greentic-coding-agent updates mark-seen --scope org --all
```

Repo-level update behavior remains the default:

```bash
greentic-coding-agent updates --new --format json
greentic-coding-agent updates mark-seen --all
```

## Release Packaging

The CLI and internal crates are still `publish = false`, so crates.io and `cargo binstall` are not the default release path yet. Use a built binary or a GitHub release binary once available.
