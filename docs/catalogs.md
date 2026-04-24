# Catalogs

Catalogs are the discovery layer for Greentic coding-agent indexes. A catalog is a `v1` JSON document with a sorted `repos` array. Each repo entry is keyed by canonical `repo_id` in `org/repo` form and points at an index package, usually `ghcr.io/greenticai/indexes/<org>/<repo>:latest`.

Public catalogs live at:

```text
ghcr.io/greenticai/indexes/catalog:latest
```

Tenant catalogs live at:

```text
ghcr.io/greenticai/indexes/tenants/<tenant>/catalog:latest
```

Tenant entries override public entries with the same `repo_id` during sync. Use `disable-repo` when a repo should stop syncing but audit history should remain; use `remove-repo` only when the entry should disappear.

Common commands:

```bash
greentic-coding-agent catalog add-repo --repo greenticai/greentic-types --index-uri ghcr.io/greenticai/indexes/greenticai/greentic-types:latest
greentic-coding-agent catalog disable-repo --repo greenticai/greentic-types
greentic-coding-agent catalog validate --format json
greentic-coding-agent catalog publish --backend ghcr --token-env GHCR_TOKEN
greentic-coding-agent sync --tenant meeza --token-env TENANT_GHCR_TOKEN
```

Migration warning text for legacy catalog inputs:

```text
legacy repo_name-only input: repo_id missing; using inferred repo_id unknown/<repo_name> for this version
```

New outputs always include `repo_id`. `repo_name` remains as display metadata for one compatibility version.
