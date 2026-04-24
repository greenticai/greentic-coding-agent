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

The generated workflows build the release CLI, analyze the repo, build/check the local Tantivy index, package the index, and publish with `--backend ghcr --token-env GHCR_TOKEN`. Tenant workflows use `TENANT_GHCR_TOKEN` when configured.
