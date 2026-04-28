# PR 03 — Fully automate per-repo index publishing from GitHub Actions

## Position in sequence

Implement after PR 02 has branch-aware repo metadata and catalog selection helpers.

Current local reality before this PR:

- `install-github-workflow` renders a workflow for `main, master`, scheduled runs, `package-index --tag latest`, and `publish-index --tag latest`.
- `package-index` accepts a single `--tag`.
- `publish-index` accepts a single `--tag`.
- `analyze` has no `--repo`, `--branch`, or `--commit` flags.
- `publish-index` has no `--repo` or `--branch` flags.
- The CLI package is currently `publish = false`, so generated workflows cannot assume `cargo binstall greentic-coding-agent` until release packaging is solved.

## Goal

Make generated GitHub workflows publish branch-specific and SHA-specific indexes automatically for pushes to `main` and `develop`.

## Commands to update

- `install-github-workflow`
- `package-index`
- `publish-index`

Add a convenience command if useful:

```bash
greentic-coding-agent ci publish-index
```

## Required CLI/API updates

Choose one implementation path and update the plan/tests accordingly:

Implemented path: repeated `--tag` support for `package-index` and `publish-index`, plus GitHub Actions environment detection for repo/branch/commit metadata where local git data is detached or incomplete.

1. Add repeated tag support:

```bash
greentic-coding-agent package-index --tag develop --tag sha-<commit>
greentic-coding-agent publish-index --tag develop --tag sha-<commit>
```

2. Or add a dedicated CI command that owns analyze, package and publish for all tags:

```bash
greentic-coding-agent ci publish-index --branch develop --commit <sha>
```

If keeping separate commands, also add one of:

- `analyze --repo --branch --commit`
- environment-variable detection in CI (`GITHUB_REPOSITORY`, `GITHUB_REF_NAME`, `GITHUB_SHA`)

Do not put unsupported flags in generated workflow output.

## Required generated workflow

This target workflow is correct after the CLI/API updates above exist. If release packaging is not solved, replace `cargo binstall` with the current build-from-source path.

```yaml
name: Greentic Coding Agent Index

on:
  push:
    branches:
      - main
      - develop
  workflow_dispatch:

permissions:
  contents: read
  packages: write

jobs:
  publish-index:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Install greentic-coding-agent
        run: |
          cargo binstall greentic-coding-agent --no-confirm

      - name: Analyze repository
        run: |
          greentic-coding-agent analyze \
            --repo "${{ github.repository }}" \
            --branch "${{ github.ref_name }}" \
            --commit "${{ github.sha }}" \
            --format json

      - name: Package index
        run: |
          greentic-coding-agent package-index \
            --tag "${{ github.ref_name }}" \
            --tag "sha-${{ github.sha }}" \
            --format json

      - name: Publish index
        env:
          GHCR_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          greentic-coding-agent publish-index \
            --backend ghcr \
            --token-env GHCR_TOKEN \
            --repo "${{ github.repository }}" \
            --branch "${{ github.ref_name }}" \
            --tag "${{ github.ref_name }}" \
            --tag "sha-${{ github.sha }}" \
            --format json
```

Current fallback install step until the crate is publishable:

```yaml
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Build greentic-coding-agent
        run: cargo build --release --package greentic-coding-agent
```

## Publishing rules

Publish all of these where possible:

- `:<branch>`
- `:sha-<commit>`
- optional `:latest-<branch>`

Do not rely on plain `:latest` as the canonical branch index.

## Acceptance criteria

- Generated workflow triggers on `main` and `develop`.
- Workflow embeds branch and commit metadata.
- Publish supports multiple tags in one run.
- Docs explain GHCR permissions and token requirements.
- Existing `latest` workflow tests are updated or retained as legacy compatibility tests.
