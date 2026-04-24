# PR-10 — Tests, docs, examples, and migration

## Goal

Tie together the new catalog, Tantivy, GHCR, tenant, server, and watcher features with tests and docs.

## Depends on

- PR-01 through PR-09.

This PR is the final integration and documentation pass, not the first time examples or schema compatibility are tested. Each earlier PR that changes serialized output must update examples and schema/serde tests in that same PR.

## Implementation sequence

Recommended order:

1. PR-01: migrate identity to `repo_id`, including registry/cache/search surfaces.
2. PR-02: add `SearchEngine` abstraction and local Tantivy.
3. PR-03: add remote backend abstraction and GHCR/ORAS backend.
4. PR-04: add catalog mutation, audit log, and publish conflict checks.
5. PR-05: add global cache, `sync-state.json`, and merged Tantivy rebuild.
6. PR-06: add shared `QueryService`, MCP stdio, and optionally HTTP as a second phase.
7. PR-07: add watcher orchestration and atomic query-handle swap.
8. PR-08: generate production GHCR workflows.
9. PR-09: generate first-run bootstrap instructions.
10. PR-10: complete integration docs, migration docs, examples, and CI coverage.

## Docs to add/update

- `docs/architecture.md`
- `docs/catalogs.md`
- `docs/tenant-indexes.md`
- `docs/server.md`
- `docs/ghcr-format.md`
- `docs/workflow-installation.md`
- `README.md`

## Examples to add/update

```text
examples/catalog.public.v1.json
examples/catalog.tenant.meeza.v1.json
examples/repo-manifest.v1.json
examples/repo-index.v1.json
examples/mcp-request.search-all.json
examples/server-search-request.json
examples/greentic-agent-index.workflow.yml
examples/greentic-agent-catalog.workflow.yml
```

## Migration notes

Existing `repo_name` fields remain readable for one version.

Rules:

- New outputs must include `repo_id`.
- Old inputs without `repo_id` should be accepted only when unambiguous.
- Catalog validation should warn when `repo_name` exists without `repo_id`.
- Future version may remove short-name-only identity.

Add a compatibility matrix:

```text
Input type                 Old repo_name only   New repo_id
repo manifest              read with warning    canonical
repo index                 read with warning    canonical
catalog                    read with warning    canonical
registry                   migrate on write     canonical
package/cache path         read old path        write new org/repo path
search/MCP response        add repo_id          canonical
```

Document the exact warning text for ambiguous old inputs so tests can assert it.

## Integration tests

Add tests for:

- analyze creates repo index and Tantivy local index
- search uses local Tantivy
- public catalog sync
- tenant catalog merge
- add/remove/enable/disable repo
- merged Tantivy search across two orgs
- MCP tool surface
- HTTP server health/search
- watcher updates merged index
- generated GitHub workflow validates expected fields
- migration from old `repo_name`-only artifacts to new `repo_id` artifacts
- local fixture backend and GHCR backend are both covered at the abstraction boundary
- token redaction is covered across sync, serve, watcher, and workflow/bootstrap output

## CI updates

Ensure `ci/local_check.sh` runs:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --no-deps
```

If server tests bind ports, use random local ports.

If tests need ORAS/GHCR behavior, keep network-free tests against `LocalFixtureBackend` by default and gate real GHCR smoke tests behind an explicit ignored/env-gated test.

## Acceptance criteria

- Full workspace tests pass.
- Docs explain public and tenant/private index flows.
- Examples are valid JSON/YAML.
- Backwards compatibility for existing local indexes is handled gracefully.
- Earlier PR examples/schema changes are consolidated and verified here without introducing new format drift.
