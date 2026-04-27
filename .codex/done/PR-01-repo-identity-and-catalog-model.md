# PR-01 — Use fully qualified repo identity and extend catalog model

## Goal

Make repo identity unambiguous across multiple GitHub orgs such as:

- `greenticai/greentic-types`
- `greenticai/greentic-pack`
- `greentic-biz/greentic-demo`
- `greentic-biz/meeza-store`

The current model uses `repo_name` in several places. That is not enough because repos can have the same short name in different orgs.

## Dependency role

This PR is the hard dependency for PR-02 through PR-08.

Do this identity migration before Tantivy, GHCR sync, catalog membership, merged cache, server, watcher, or workflow work. Those features must not introduce new short-name-only paths or keys while this migration is still incomplete.

Every persisted surface that currently uses `repo_name` as an identifier must either:

- move to `repo_id`
- retain `repo_name` only as display/backwards-compatible metadata
- document why short-name-only identity is still safe

## User-facing behaviour

All catalog and sync commands must accept repo IDs in this form:

```bash
greentic-coding-agent catalog add-repo --repo greenticai/greentic-types
greentic-coding-agent catalog remove-repo --repo greentic-biz/greentic-demo
greentic-coding-agent sync --repo greenticai/greentic-types
```

Short repo names may be accepted only as a convenience when the current checkout can determine the org/repo from Git remote.

## Files to modify

- `crates/gca-core/src/model.rs`
- `crates/gca-core/src/lib.rs`
- `crates/gca-index/src/lib.rs`
- `crates/gca-query/src/lib.rs`
- `crates/gca-cli/src/main.rs`
- `examples/repo-manifest.v1.json`
- `examples/repo-index.v1.json`
- `examples/catalog.v1.json`
- `schemas/*catalog*`
- `docs/architecture.md`

## New and changed types

Add:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoId {
    pub org: String,
    pub name: String,
}
```

Also add helper methods:

```rust
impl RepoId {
    pub fn parse(value: &str) -> Result<Self, String>;
    pub fn as_str(&self) -> String; // "org/repo"
    pub fn ghcr_path(&self) -> String; // "org/repo"
}
```

Keep internal storage simple by storing a canonical string `repo_id: String` in serialized types.

Update `RepoAgentManifest`:

```rust
pub struct RepoAgentManifest {
    pub version: String,
    pub repo_id: String,
    pub repo_name: String,
    pub org: Option<String>,
    pub repo_root: String,
    pub repo_role: RepoRole,
    pub primary_language: String,
    pub generated_at: String,
    pub candidate_docs: Vec<String>,
    pub cargo_manifests: Vec<String>,
}
```

Update `RepoIndex`:

```rust
pub struct RepoIndex {
    pub version: String,
    pub repo_id: String,
    pub repo_name: String,
    pub repo_role: RepoRole,
    ...
}
```

Update `CatalogRepo`:

```rust
pub struct CatalogRepo {
    pub repo_id: String,
    pub repo_role: RepoRole,
    pub latest_tag: String,
    pub package_ref: String,
    pub updated_at: String,
    pub visibility: IndexVisibility,
    pub tenant: Option<String>,
    pub required_auth: Option<AuthKind>,
    pub digest: Option<String>,
    pub source_commit: Option<String>,
    pub enabled: bool,
}
```

Update `RegistryEntry` and any local/global cache metadata:

```rust
pub struct RegistryEntry {
    pub repo_id: String,
    pub repo_name: String,
    pub org: Option<String>,
    ...
}
```

Add `repo_id` to search/MCP result surfaces before cross-repo search exists, so consumers do not need a breaking response change later.

Add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexVisibility {
    Public,
    Tenant,
    Private,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthKind {
    GhcrToken,
    BearerToken,
}
```

## Repo ID detection

Implement detection in `gca-index`:

1. Try `git config --get remote.origin.url`.
2. Parse SSH form: `git@github.com:greenticai/greentic-coding-agent.git`.
3. Parse HTTPS form: `https://github.com/greenticai/greentic-coding-agent.git`.
4. Fallback to `unknown/<repo_name>` only if no remote exists.

Add tests for all formats.

## Migration rules

Existing JSON with only `repo_name` remains readable for one compatibility version.

Rules:

- New outputs always include `repo_id`.
- Old inputs without `repo_id` are accepted only when unambiguous.
- If a Git remote is available, infer `repo_id` from the remote and keep the old `repo_name`.
- If no remote is available, use `unknown/<repo_name>` and emit a validation warning.
- Catalog validation warns when an entry has `repo_name` without `repo_id`.
- Registry and cache migration must preserve existing local paths where possible, but new paths should be keyed by `org/repo`.

Migration tests must cover:

- old manifest JSON without `repo_id`
- old repo index JSON without `repo_id`
- old catalog JSON with `repo_name`
- old registry entry with `repo_name`
- new `repo_id` paths that avoid collisions for `greenticai/foo` and `greentic-biz/foo`

## GHCR package reference convention

Public:

```text
ghcr.io/greenticai/indexes/<org>/<repo>:latest
```

Tenant:

```text
ghcr.io/greenticai/indexes/tenants/<tenant>/<org>/<repo>:latest
```

Examples:

```text
ghcr.io/greenticai/indexes/greenticai/greentic-types:latest
ghcr.io/greenticai/indexes/greentic-biz/greentic-demo:latest
ghcr.io/greenticai/indexes/tenants/meeza/greentic-biz/meeza-store:latest
```

## Tests

Add tests for:

- `RepoId::parse("greenticai/greentic-types")`.
- Invalid `RepoId::parse("greentic-types")`.
- Git remote parsing for SSH and HTTPS.
- Catalog JSON round-trip with public and tenant entries.
- `greenticai/foo` and `greentic-biz/foo` do not collide.

## Acceptance criteria

- Serialized manifest contains both `repo_id` and short `repo_name`.
- Catalog entries are keyed by `repo_id`.
- Registry entries are keyed by `repo_id`.
- Package/cache paths use `repo_id` or its `org/repo` components, not short `repo_name`.
- Search and MCP results include `repo_id` where provenance points at a repo.
- Existing commands still work for the current repo.
- Catalog output no longer relies on short repo name for uniqueness.
