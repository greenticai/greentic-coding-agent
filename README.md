# greentic-coding-agent

`greentic-coding-agent` is the dedicated Greentic repository for coding-agent tooling. Its long-term direction is a Rust CLI and MCP-backed knowledge layer that helps coding agents orient in Greentic repos quickly, follow reuse rules, and generate consistent agent-facing guidance.

The architecture and planned feature sequence are documented in [docs/architecture.md](docs/architecture.md). This repository is intentionally separate from `greentic-dev`: `greentic-dev` is expected to stay a launcher, while `greentic-coding-agent` owns the actual product logic, data model, indexing, and agent-facing workflows.

## Current Status

- Cargo workspace with implemented crates for CLI, core types, indexing, query, Greentic enrichment, agent-file generation, OCI-style local packaging/sync, and MCP-style helper responses.
- Working CLI commands for `describe --here`, `concepts`, `workflows`, `search`, `locate-owner`, `required-validations`, `generate-agent-files`, `package-index`, `publish-index`, `show-catalog`, `check-refresh`, `install-github-workflow`, `impact`, `detect-changes`, `validate-plan`, and `serve`.
- Seeded adapter knowledge for `greentic-types`, `greentic-pack`, `greentic-bundle`, `greentic-dev`, `greentic-x`, and `greentic-sorla`.
- CI, release, package validation, perf smoke checks, and nightly coverage hooks.

## Workspace Layout

```text
crates/
  gca-cli/
  gca-core/
  gca-index/
  gca-query/
  gca-greentic/
  gca-agent-files/
  gca-oci/
  gca-mcp/
schemas/
templates/
docs/
.codex/
```

## Local vs Remote Model

- Local-first: the CLI is intended to analyze the current checkout and write repo-local outputs.
- Remote-aware: later PRs add OCI/GHCR publication and discovery so repo intelligence can be shared across Greentic repos.

## Seeded Adapters

The repo now includes curated seeded adapters for a first set of high-value Greentic repositories:

- `greentic-types`
- `greentic-pack`
- `greentic-bundle`
- `greentic-dev`
- `greentic-x`
- `greentic-sorla`

These adapters live in `crates/gca-greentic/src/lib.rs` and currently enrich:

- repo role
- concept ownership hints
- repo-specific workflows
- docs of interest for those repos

To add another adapter, follow the existing `SEEDED_ADAPTERS` pattern:

1. Add a new `SeededAdapter` entry with a repo name, role, docs, concepts, and workflows.
2. Add or reuse seeded concept ownership in `seeded_concept` / `concept_owners`.
3. If the adapter implies stable cross-repo ownership, add matching reuse policy entries in `crates/gca-query/src/lib.rs`.
4. Add a focused test in `crates/gca-greentic` and, if needed, a policy test in `crates/gca-query`.

## Local Development

Run the standard local validation wrapper from the repository root:

```bash
bash ci/local_check.sh
```

Try the CLI:

```bash
cargo run -p greentic-coding-agent -- --help
cargo run -p greentic-coding-agent -- describe --here --format markdown
cargo run -p greentic-coding-agent -- concepts --format json
cargo run -p greentic-coding-agent -- workflows --format markdown
cargo run -p greentic-coding-agent -- search --mode instruction wizard --format json
cargo run -p greentic-coding-agent -- check-refresh --format json
cargo run -p greentic-coding-agent -- serve --format json
cargo run -p greentic-coding-agent -- serve --request-file examples/mcp-request.describe-repo.json --format json
cargo run -p greentic-coding-agent -- validate-plan examples/plan.v1.json --format markdown
```

To run only packaging and publish dry-run validation:

```bash
bash ci/local_check.sh --mode package
```

## Examples

Committed example artifacts live under [`examples/`](examples/README.md).

- `examples/greentic-agent-index.workflow.yml`
  Example output from `install-github-workflow`.
- `examples/mcp-request.describe-repo.json`
  Example MCP-style request payload for `serve --request-file`.
- `examples/plan.v1.json`
  Example input for `validate-plan`.
- `examples/repo-manifest.v1.json`, `examples/repo-index.v1.json`, `examples/catalog.v1.json`
  Example machine-readable outputs for the current schema layer.

## CI and Releases

The repository uses a small set of consistent automation entrypoints:

- `ci/local_check.sh`
  Runs formatting, clippy, tests, build, docs, package content checks, `cargo package`, and `cargo publish --dry-run` for every publishable crate in the workspace.
- `.github/workflows/ci.yml`
  Runs lint, tests, and package dry-run checks on pull requests and pushes to `master` / `main`.
- `.github/workflows/publish.yml`
  Verifies the repository, confirms the tag matches the CLI crate version from `crates/gca-cli/Cargo.toml`, publishes to crates.io, builds release archives for six runner targets, uploads release assets, and publishes an OCI artifact bundle to GHCR.
- `.github/workflows/perf.yml`
  Runs lightweight concurrency guards and a Criterion smoke benchmark for `gca-core`.

### Relationship to `greentic-dev`

The intended invocation is:

```bash
gtc dev coding-agent ...
```

That launcher integration is planned externally in `greentic-dev`. This repository already provides the binary and CLI contract that integration will delegate to.

### How to cut a release

1. Bump the version in the workspace and `crates/gca-cli`.
2. Run `bash ci/local_check.sh`.
3. Commit the release changes.
4. Create and push a matching tag:

```bash
git tag vX.Y.Z
git push origin vX.Y.Z
```

Pushing the tag triggers `.github/workflows/publish.yml`. A manual `workflow_dispatch` run is also supported; it still derives the release tag from the CLI crate version and enforces `v<version>`.

### Required GitHub secrets

- `CARGO_REGISTRY_TOKEN` for crates.io publication.
- `GHCR_TOKEN` is optional when the default `GITHUB_TOKEN` has package write access; otherwise provide it for GHCR pushes.
