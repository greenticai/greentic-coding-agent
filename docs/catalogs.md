# Catalogs

Catalogs are the discovery layer for Greentic coding-agent indexes. Current catalogs use the `gca.catalog.v2` JSON shape with a sorted `repos` array and branch-aware entries. Each repo entry is keyed by canonical `repo_id` in `org/repo` form and points at branch/channel package entries such as `ghcr.io/greenticai/indexes/<org>/<repo>:main` or `ghcr.io/greenticai/indexes/<org>/<repo>:develop`.

For the producer and consumer responsibilities around catalogs, see [producer-vs-consumer.md](producer-vs-consumer.md).

Public catalogs live at:

```text
ghcr.io/greenticai/indexes/catalog:latest
ghcr.io/greenticai/indexes/catalog:main
ghcr.io/greenticai/indexes/catalog:develop
```

Tenant catalogs live at:

```text
ghcr.io/greenticai/indexes/tenants/<tenant>/catalog:latest
ghcr.io/greenticai/indexes/tenants/<tenant>/catalog:main
```

Tenant entries override public entries with the same `repo_id` during sync. Use `disable-repo` when a repo should stop syncing but audit history should remain; use `remove-repo` only when the entry should disappear.

Common commands:

```bash
greentic-coding-agent catalog add-repo --repo greenticai/greentic-types --index-uri ghcr.io/greenticai/indexes/greenticai/greentic-types:latest
greentic-coding-agent catalog rebuild-from-ghcr --org greenticai --channel develop --format json
greentic-coding-agent catalog disable-repo --repo greenticai/greentic-types
greentic-coding-agent catalog validate --format json
greentic-coding-agent catalog publish --channel develop --backend ghcr --token-env GHCR_TOKEN
greentic-coding-agent sync --channel main --tenant meeza --token-env TENANT_GHCR_TOKEN
greentic-coding-agent status --channel main --format json
```

Legacy `v1` catalog inputs remain accepted for compatibility. Migration warning text for legacy catalog inputs:

```text
legacy repo_name-only input: repo_id missing; using inferred repo_id unknown/<repo_name> for this version
```

New outputs always include `repo_id`. `repo_name` remains as display metadata for one compatibility version.

Rebuild from published repo indexes:

```bash
greentic-coding-agent catalog rebuild-from-ghcr \
  --org greenticai \
  --channel develop \
  --format json
```

This writes a deterministic branch-aware catalog from published repo packages under the selected organization. The rebuilt catalog includes branch entries for available tags such as `main` and `develop`, sets `default_channel`, and can then be validated and published.
