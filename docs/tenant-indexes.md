# Tenant Indexes

Tenant indexes use the same repo index, catalog, package, sync-state, and Tantivy formats as public indexes, with tenant visibility and token-based access added at the catalog and sync boundaries.

The expected package path is:

```text
ghcr.io/greenticai/indexes/tenants/<tenant>/<org>/<repo>:latest
ghcr.io/greenticai/indexes/tenants/<tenant>/<org>/<repo>:main
```

The tenant catalog path is:

```text
ghcr.io/greenticai/indexes/tenants/<tenant>/catalog:latest
ghcr.io/greenticai/indexes/tenants/<tenant>/catalog:main
```

Recommended local flow:

```bash
greentic-coding-agent sync --channel main --tenant meeza --token-env TENANT_GHCR_TOKEN
greentic-coding-agent status --channel main --format json
greentic-coding-agent search --scope merged --tenant meeza --mode instruction wizard
greentic-coding-agent serve --mcp --tenant meeza --token-env TENANT_GHCR_TOKEN --watch
```

The merged index includes public repos plus matching tenant repos. If a tenant catalog contains the same `repo_id` as the public catalog, the tenant entry wins.
