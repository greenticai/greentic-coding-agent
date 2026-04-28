# Workflow Installation

Install a repo index workflow:

```bash
greentic-coding-agent install-github-workflow --publish-ghcr
```

Install a tenant index workflow:

```bash
greentic-coding-agent install-github-workflow --publish-ghcr --tenant meeza
```

Install catalog publishing workflows:

```bash
greentic-coding-agent install-github-workflow --catalog public
greentic-coding-agent install-github-workflow --catalog tenant --tenant meeza
```

The generated repo workflow runs on pushes to `main` and `develop`, builds the CLI from source, analyzes the repo, builds/checks the local Tantivy index, packages both `:<branch>` and `:sha-<commit>` tags, and publishes them with `--backend ghcr --token-env GHCR_TOKEN`.

The generated catalog workflow also runs on `main` and `develop` and publishes the catalog with `--channel "${{ github.ref_name }}"`, so branch-specific catalog tags such as `catalog:main` and `catalog:develop` stay current.

The workflow needs `contents: read` and `packages: write`. Tenant workflows use `TENANT_GHCR_TOKEN` when configured.
