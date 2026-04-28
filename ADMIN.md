# Admin Setup For Greentic Repositories

This guide explains how to enable Greentic Coding Agent across Greentic repositories so indexes are generated, published, synced, and usable by developers and coding agents.

For the split between CI producer work and developer/agent consumer work, see [docs/producer-vs-consumer.md](docs/producer-vs-consumer.md). For local cache paths used by `sync`, `status`, `watch`, and org-level updates, see [docs/local-cache-layout.md](docs/local-cache-layout.md).

## Goal

For each Greentic repo, the setup should:

1. Run `greentic-coding-agent analyze`.
2. Generate a repo index under `.greentic-agent/`.
3. Optionally generate root agent files: `AGENTS.md`, `CODEX.md`, `CLAUDE.md`, `llms.txt`.
4. Package the index.
5. Publish the package to GHCR.
6. Add the repo package to a public or tenant catalog.
7. Let developers and agents run `sync` and search across all published indexes.

## Prerequisites

On developer machines or CI runners:

- Rust toolchain
- Git
- Access to the target Greentic repositories
- `greentic-coding-agent` binary
- `oras` CLI when publishing to GHCR
- GitHub token or `GITHUB_TOKEN` with package write access for GHCR publishing

For CI/GitHub Actions:

- `contents: read`
- `packages: write`
- `GHCR_TOKEN` if the default `GITHUB_TOKEN` is not enough
- `TENANT_GHCR_TOKEN` for tenant/private catalog publishing when needed

## Install Or Build The Tool

During active development, build from this repository:

```bash
cargo build --release -p greentic-coding-agent
```

The binary is then:

```text
target/release/greentic-coding-agent
```

When release binaries are available, install the release binary instead and put it on `PATH`.

Verify:

```bash
greentic-coding-agent --help
```

## Per-Repo Setup

Run these commands from the root of the target Greentic repo.

### 1. Analyze The Repo

```bash
greentic-coding-agent analyze --print --format json
```

This writes local index data under:

```text
.greentic-agent/
```

### 2. Generate Agent Files

```bash
greentic-coding-agent generate-agent-files --write-root
```

Review generated root files before committing them:

```text
AGENTS.md
CODEX.md
CLAUDE.md
llms.txt
```

If a repo should not keep these files at root, omit `--write-root`; generated copies still live under `.greentic-agent/generated/`.

### 3. Check Freshness

```bash
greentic-coding-agent check-refresh --format markdown
```

This reports whether the repo has changed since the last local index.

### 4. Package The Index

```bash
greentic-coding-agent package-index \
  --tag main \
  --tag sha-<commit> \
  --format json
```

This creates an OCI-style package layout for the repo index.

### 5. Publish The Index To GHCR

```bash
greentic-coding-agent publish-index \
  --tag main \
  --tag sha-<commit> \
  --backend ghcr \
  --token-env GHCR_TOKEN \
  --format json
```

The expected package reference follows this shape:

```text
ghcr.io/greenticai/indexes/<org>/<repo>:main
ghcr.io/greenticai/indexes/<org>/<repo>:sha-<commit>
```

## Install The GitHub Workflow

The preferred repo automation is the generated GitHub workflow.

Install a standard index publishing workflow:

```bash
greentic-coding-agent install-github-workflow --publish-ghcr
```

Install a tenant index workflow:

```bash
greentic-coding-agent install-github-workflow --publish-ghcr --tenant <tenant>
```

This writes:

```text
.github/workflows/greentic-agent-index.yml
```

Commit the workflow and open a normal PR in the target repo.

The generated workflow analyzes the repo, builds/checks the local index, packages it for the current branch and commit SHA, and publishes both tags to GHCR. It uses `GITHUB_TOKEN` through `GHCR_TOKEN`, so the workflow needs `packages: write`.

## Catalog Setup

Published indexes become useful across repos when they are listed in a catalog.

Public catalog reference:

```text
ghcr.io/greenticai/indexes/catalog:latest
```

Tenant catalog reference:

```text
ghcr.io/greenticai/indexes/tenants/<tenant>/catalog:latest
```

Add or update a repo in the editable local catalog:

```bash
greentic-coding-agent catalog add-repo \
  --repo greenticai/greentic-pack \
  --index-uri ghcr.io/greenticai/indexes/greenticai/greentic-pack:latest
```

Validate:

```bash
greentic-coding-agent catalog validate --format json
```

Publish:

```bash
greentic-coding-agent catalog publish \
  --channel develop \
  --backend ghcr \
  --token-env GHCR_TOKEN \
  --format json
```

For tenant catalogs, pass the tenant options used by your environment.

Rebuild the central branch-aware catalog from published repo indexes:

```bash
greentic-coding-agent catalog rebuild-from-ghcr \
  --org greenticai \
  --channel develop \
  --format json
greentic-coding-agent catalog validate --format json
greentic-coding-agent catalog publish --channel develop --backend ghcr --token-env GHCR_TOKEN --format json
```

## Organization-Wide Rollout

To plan installing the index workflow across many repos:

```bash
greentic-coding-agent org plan-index-rollout \
  --org greenticai \
  --repo-list-file repos.json \
  --format json > rollout-plan.json
```

`repos.json` can be:

```json
{
  "repos": [
    {
      "repo_id": "greenticai/greentic-pack",
      "default_branch": "main",
      "archived": false
    }
  ]
}
```

Dry run:

```bash
greentic-coding-agent org apply-index-rollout \
  --plan rollout-plan.json \
  --dry-run \
  --format json
```

Create branches and PRs:

```bash
greentic-coding-agent org apply-index-rollout \
  --plan rollout-plan.json \
  --open-prs \
  --format json
```

See [docs/org-index-rollout.md](docs/org-index-rollout.md).

## Developer And Agent Consumption

After indexes are published and cataloged, developers and agents can sync them locally:

```bash
greentic-coding-agent init --channel main --format json
greentic-coding-agent sync --format json
```

To use a branch-specific channel, sync and inspect that channel explicitly:

```bash
greentic-coding-agent sync --channel develop --format json
greentic-coding-agent status --channel develop --format json
```

For tenant/private indexes:

```bash
greentic-coding-agent sync \
  --backend ghcr \
  --channel main \
  --tenant <tenant> \
  --token-env TENANT_GHCR_TOKEN \
  --format json
```

For public GHCR indexes:

```bash
greentic-coding-agent sync \
  --backend ghcr \
  --channel main \
  --token-env GHCR_TOKEN \
  --format json
```

Search merged cross-repo knowledge:

```bash
greentic-coding-agent search \
  --mode instruction \
  --scope merged \
  "component wizard" \
  --format json
```

Run server mode for agent hosts:

```bash
greentic-coding-agent serve --stdio
greentic-coding-agent serve --http --host 127.0.0.1 --port 7757 --watch
```

## Training Courses And Knowledge Updates

Repos can include authored agent guidance:

```text
.greentic/training/*.course.v1.json
.greentic/updates/*.update.v1.json
```

These are indexed automatically by `analyze`.

Seed examples live under:

```text
examples/training/
examples/updates/
```

Validate examples through the normal local check:

```bash
bash ci/local_check.sh
```

## Recommended Rollout Order

1. Enable index workflows in core repos first, such as shared types, pack, bundle, flow, dev, and X/Sorla repos.
2. Publish their indexes to GHCR.
3. Add them to the public catalog.
4. Ask developers and coding agents to run `greentic-coding-agent sync`.
5. Enable tenant catalogs for customer/private repos.
6. Add generated root agent files where they are useful for common agent tools.

## Verification Checklist

For each repo:

- `greentic-coding-agent analyze --print --format json` succeeds.
- `.greentic-agent/repo-index.json` is created.
- `greentic-coding-agent search --mode instruction <keyword>` returns useful results.
- `greentic-coding-agent generate-agent-files --write-root` produces sensible root files, if enabled.
- `.github/workflows/greentic-agent-index.yml` exists, if CI publishing is enabled.
- The workflow can publish to GHCR.
- The repo appears in the public or tenant catalog.
- `greentic-coding-agent sync` downloads the index.
- `greentic-coding-agent search --scope merged ...` can find it.

## Troubleshooting

### `publish-index` cannot push to GHCR

Check:

- `oras` is installed.
- `GHCR_TOKEN` is available to the process.
- The token has package write permission.
- The repo owner/package namespace is correct.

### `sync` cannot see a repo

Check:

- The repo index package was published.
- The catalog contains the correct `repo_id`.
- The catalog entry points at the correct `index_uri`.
- Tenant sync uses the correct tenant and token.

### Generated files look stale

Run:

```bash
greentic-coding-agent analyze --print --format json
greentic-coding-agent generate-agent-files --write-root
```

Then review the generated files and commit the intended changes.

### A coding agent ignores new guidance

Ask it to run:

```bash
greentic-coding-agent updates --new --format json
greentic-coding-agent train --task "<task>" --audience coding_agent --format markdown
greentic-coding-agent validate-plan <plan.json> --format json
```
