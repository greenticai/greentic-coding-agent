# PR-08 — Generate nightly GitHub Actions for GHCR publishing

## Goal

Generate workflows that keep repo indexes and catalogs current in GHCR.

## Depends on

- PR-01 repo identity and package reference conventions.
- PR-03 GHCR/ORAS backend.
- PR-04 catalog publish semantics.
- PR-05 local/merged index cache shape.

## Commands

```bash
greentic-coding-agent install-github-workflow --publish-ghcr
greentic-coding-agent install-github-workflow --publish-ghcr --tenant meeza
greentic-coding-agent install-github-workflow --catalog public
greentic-coding-agent install-github-workflow --catalog tenant --tenant meeza
```

## Generated workflow: repo index

Path:

```text
.github/workflows/greentic-agent-index.yml
```

Behaviour:

- runs on push to default branch
- runs nightly
- runs manually
- installs Rust
- installs/calls greentic-coding-agent
- runs analyze
- builds Tantivy index
- packages index
- pushes to GHCR with `oras`

The generated workflow should not rely on the local fixture backend. It must pass the production GHCR backend/config explicitly when publishing.

## Workflow template

```yaml
name: Greentic Agent Index

on:
  push:
    branches: [main, master]
  schedule:
    - cron: "17 2 * * *"
  workflow_dispatch:

permissions:
  contents: read
  packages: write

jobs:
  index:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Install ORAS
        uses: oras-project/setup-oras@v1

      - name: Build greentic-coding-agent
        run: cargo build --release -p greentic-coding-agent

      - name: Analyze repo
        run: ./target/release/greentic-coding-agent analyze --print --format json

      - name: Build local Tantivy index
        run: ./target/release/greentic-coding-agent search --engine auto --mode concept greentic --format json

      - name: Package index
        run: ./target/release/greentic-coding-agent package-index --tag latest

      - name: Publish index to GHCR
        env:
          GHCR_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          ./target/release/greentic-coding-agent publish-index             --tag latest             --ghcr             --token "$GHCR_TOKEN"
```

## Tenant workflow

For tenant/private repos, generated workflow should use:

```text
GREENTIC_AGENT_TENANT
GHCR_TOKEN or TENANT_GHCR_TOKEN
```

Do not echo token values. Workflow examples must use environment variables and placeholders only.

If tenant publishing uses a separate secret, default to:

```text
TENANT_GHCR_TOKEN
```

and document fallback to `GITHUB_TOKEN` only when package permissions allow it.

## Catalog publishing workflow

Path:

```text
.github/workflows/greentic-agent-catalog.yml
```

Behaviour:

- reads catalog source file
- validates catalog
- pushes catalog artifact to GHCR
- supports public and tenant catalogs

## Tests

- Generated workflow contains `packages: write`.
- Generated workflow installs ORAS.
- Tenant workflow uses tenant catalog path.
- Public workflow uses org/repo package ref.
- Workflow passes production GHCR backend/config, not local fixture defaults.
- Workflow does not include literal token values.
- Workflow validates as YAML.

## Acceptance criteria

- Every repo can install a standard nightly indexing workflow.
- Greentic-coding-agent can install a catalog publishing workflow.
- Workflows support public and tenant/private indexes.
