# PR 02 — Add branch-aware catalog and repo-index metadata

## Position in sequence

Implement this before PR 01, PR 03, PR 04, PR 05, PR 06 and PR 07 rely on channel-aware behavior.

Current local reality before this PR:

- `Catalog` is v1-style with `version`, `generated_at`, `repos`, and `change_log`.
- `CatalogRepo` uses `latest_tag`, `package_ref`, `digest`, and `source_commit`; it has no `branches` map.
- `PackageMetadata` stores one `tag`, not branch/channel/SHA metadata.
- `SyncState` has no channel or branch field.
- `RepoAgentManifest` and `RepoIndex` include generated/index data but do not carry the full metadata block shown below.

## Goal

Introduce catalog v2 and branch-aware repo index metadata while preserving v1 catalog compatibility.

## Implementation scope

Update model crates, schema files and tests.

Likely areas:

- `crates/gca-core`
- `crates/gca-oci`
- `crates/gca-query`
- `schemas/`
- `examples/catalogs/` or equivalent

## Repo index metadata

Add a backward-compatible metadata object rather than reshaping existing top-level `RepoIndex` fields destructively.

Recommended model:

```rust
pub struct RepoIndexMetadata {
    pub repo_id: String,
    pub branch: Option<String>,
    pub commit_sha: Option<String>,
    pub commit_time: Option<String>,
    pub indexed_at: String,
    pub index_schema_version: String,
    pub tool_version: String,
    pub source_tree_hash: Option<String>,
}
```

Then add `#[serde(default)] pub metadata: Option<RepoIndexMetadata>` to `RepoIndex`, or another defaulted field that keeps old JSON readable.

Every newly generated index should include equivalent metadata:

```json
{
  "repo_id": "greenticai/greentic-pack",
  "branch": "develop",
  "commit_sha": "...",
  "commit_time": "...",
  "indexed_at": "...",
  "index_schema_version": "gca.repo_index.v1",
  "tool_version": "...",
  "source_tree_hash": "..."
}
```

Branch and commit should come from git by default, with CLI/CI override flags added in later PRs if needed.

## Catalog v2 shape

```json
{
  "schema_version": "gca.catalog.v2",
  "catalog_id": "greenticai/public",
  "default_channel": "develop",
  "repos": [
    {
      "repo_id": "greenticai/greentic-pack",
      "role": "pack_authoring",
      "default_branch": "main",
      "preferred_branch": "develop",
      "branches": {
        "main": {
          "index_uri": "ghcr.io/greenticai/indexes/greenticai/greentic-pack:main",
          "commit_sha": "...",
          "updated_at": "..."
        },
        "develop": {
          "index_uri": "ghcr.io/greenticai/indexes/greenticai/greentic-pack:develop",
          "commit_sha": "...",
          "updated_at": "..."
        }
      }
    }
  ]
}
```

## Compatibility

- v1 flat catalog entries must still parse.
- v1 entries should be normalised internally into v2-like branch entries.
- Existing `latest` behavior should continue but be marked legacy.
- Keep current v1 `Catalog`/`CatalogRepo` deserialization green.
- Do not remove `latest_tag` or `package_ref` until all package/sync code has migrated.
- Add helper APIs for selecting a branch/channel from either v1 or v2 catalog data.
- Decide whether the serialized discriminator is `version` or `schema_version`; if adding `schema_version`, support existing `version: "v1"` inputs.

## CLI/API changes enabled by this PR

Add the model and selection layer needed for, but do not fully require, these later commands:

```bash
greentic-coding-agent sync --channel develop
greentic-coding-agent status --channel develop
```

## Tests

Add tests for:

- v2 parsing
- v1 parsing compatibility
- branch selection
- missing preferred branch fallback to default branch
- invalid repo_id/branch rejection
- metadata round-trip in generated index
- v1 fixture compatibility for existing `examples/catalog*.json`
- v1 package/sync behavior still using `latest`

## Acceptance criteria

- `catalog validate` accepts v1 and v2.
- Catalog branch-selection helpers can choose `branches.develop` and `branches.main`.
- If this PR adds CLI flags, `sync --channel develop` and `sync --channel main` work; otherwise PR 04 owns the CLI wiring.
- Index package metadata identifies repo, branch and commit.
