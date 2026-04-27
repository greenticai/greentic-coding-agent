# PR-01 — Introduce `gca-engine` and Refactor CLI into a Thin Adapter

## Objective

Create a shared service layer so CLI, MCP, HTTP/server mode, and future `gtc dev coding-agent` integration all call the same operations.

## Why

`crates/gca-cli/src/main.rs` currently contains too much product logic and duplicates constants/models that belong in library crates. This PR prevents drift and makes future concepts reusable.

## Scope

### Add crate

Create:

```text
crates/gca-engine/
  Cargo.toml
  src/lib.rs
  src/service.rs
  src/options.rs
  src/responses.rs
```

### Workspace update

Add `crates/gca-engine` to root `Cargo.toml` members.

### Dependencies

`gca-engine` should depend on:

```toml
gca-core = { path = "../gca-core" }
gca-index = { path = "../gca-index" }
gca-query = { path = "../gca-query" }
gca-greentic = { path = "../gca-greentic" }
gca-agent-files = { path = "../gca-agent-files" }
gca-oci = { path = "../gca-oci" }
gca-mcp = { path = "../gca-mcp" }
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
```

`gca-cli` should depend on `gca-engine` and stop duplicating core/index/query behaviour.

### Packaging constraint

The original `greentic-coding-agent` package was crates.io dry-run verified by `ci/local_check.sh`.
Before making `gca-cli` depend on `gca-engine` or other workspace crates, make the package strategy explicit:

- either make every path dependency publishable with matching `version.workspace = true`, license, description, README/include metadata, and registry-safe dependency declarations;
- or preserve a documented package layout that keeps `cargo package` and `cargo publish --dry-run` green.

The current implementation chooses the second path by making `gca-cli` an internal workspace binary with `publish = false` while it depends on unpublished internal crates.
A future release-layout PR should either publish/version the shared crates in dependency order or add a separate publishable wrapper crate.

## Service API

Add:

```rust
pub struct CodingAgentService {
    pub cwd: PathBuf,
    pub home_dir: PathBuf,
}
```

Methods:

```rust
impl CodingAgentService {
    pub fn new(cwd: PathBuf, home_dir: PathBuf) -> Self;
    pub fn analyze(&self, options: AnalyzeOptions) -> Result<AnalyzeResponse, EngineError>;
    pub fn describe_here(&self, options: DescribeOptions) -> Result<DescribeResponse, EngineError>;
    pub fn concepts(&self, options: ConceptsOptions) -> Result<ConceptsResponse, EngineError>;
    pub fn workflows(&self, options: WorkflowsOptions) -> Result<WorkflowsResponse, EngineError>;
    pub fn search(&self, options: SearchOptions) -> Result<SearchResponse, EngineError>;
    pub fn locate_owner(&self, options: LocateOwnerOptions) -> Result<OwnerLookup, EngineError>;
    pub fn required_validations(&self, options: RequiredValidationsOptions) -> Result<RequiredValidationsResponse, EngineError>;
    pub fn generate_agent_files(&self, options: GenerateAgentFilesOptions) -> Result<GenerateAgentFilesResponse, EngineError>;
    pub fn package_index(&self, options: PackageIndexOptions) -> Result<PackageIndexResponse, EngineError>;
    pub fn publish_index(&self, options: PublishIndexOptions) -> Result<PublishIndexResponse, EngineError>;
    pub fn sync(&self, options: SyncOptions) -> Result<SyncResponse, EngineError>;
    pub fn rebuild_merged_index(&self, options: RebuildMergedIndexOptions) -> Result<RebuildMergedIndexResponse, EngineError>;
    pub fn check_refresh(&self, options: CheckRefreshOptions) -> Result<CheckRefreshResponse, EngineError>;
    pub fn impact(&self, options: ImpactOptions) -> Result<ImpactResponse, EngineError>;
    pub fn detect_changes(&self, options: DetectChangesOptions) -> Result<DetectChangesResponse, EngineError>;
    pub fn validate_plan(&self, options: ValidatePlanOptions) -> Result<ValidatePlanResponse, EngineError>;
}
```

Use smaller return types if current crates already expose suitable structures. Avoid inventing parallel models when existing types are good.

## CLI refactor

`gca-cli` should only do:

1. Parse args with Clap.
2. Convert args into engine option structs.
3. Call `CodingAgentService`.
4. Render JSON or Markdown.
5. Exit with correct code.

Remove duplicated constants such as:

```rust
LOCAL_INDEX_DIR
SCHEMA_VERSION_V1
BUILTIN_CONCEPT_IDS
KNOWN_COMMANDS
```

Use canonical constants/types from library crates.

## Documentation

Update `docs/architecture.md`:

- Make current implemented crate layout canonical.
- Add `gca-engine` as the shared service layer.
- Move old proposed crates into a “future optional extraction” section or remove them.
- Update `.codex/repo_overview.md` after the refactor so it no longer reports CLI/domain duplication as current state if that duplication is removed.

## Tests

Add engine-level tests:

```text
crates/gca-engine/tests/engine_analyze.rs
crates/gca-engine/tests/engine_query.rs
crates/gca-engine/tests/engine_package_sync.rs
```

Keep CLI tests but reduce future logic assertions to smoke tests.

## Acceptance criteria

- All existing CLI commands still work.
- No user-facing command names or flags are removed.
- `gca-cli` depends on internal crates and routes primary command behavior through shared engine/OCI APIs.
- `ci/local_check.sh` still passes after the dependency-layout change.
- `cargo test --workspace --all-features` passes.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes.

## Current implementation note

An internal `gca-engine` crate has been added with typed service methods and engine-level tests.
The engine now covers analyze, describe, concept/workflow listing, command catalog listing, local search, owner/validation lookup, generated agent-file writing, default GitHub workflow installation, package/local publish/sync helpers, remote repo/catalog reads, merged-index rebuild, refresh checks, MCP snapshot/dispatch, impact, change detection, and structured plan validation with task summary, owner hints, validation hints, freshness warning, and issues.
The local sync-state and normalized cache/index model has been extracted into `gca-oci`, including `SyncState`, `SyncedRepoState`, `SyncReport`, `MergedIndexReport`, CLI-compatible default paths, catalog/repo sync with digest-based skip detection, normalized indexes under `~/.greentic-agent/indexes`, per-repo Tantivy cache creation, sync-state persistence/recovery, cached-index loading, and shared atomic merged-index rebuilds with metadata.
Local publish/sync defaults now match the existing CLI storage layout: `~/.greentic-agent/remote-oci`, `~/.greentic-agent/cache-oci`, `~/.greentic-agent/indexes`, and `~/.greentic-agent/sync-state.json`.
`gca-cli` now depends on `gca-engine` and `gca-oci` and routes concepts, workflows, command catalog, owner lookup, required validations, generated agent files, package/publish for the local backend, local sync/cache/merged-index rebuild, remote repo/catalog display, check-refresh, validate-plan, and describe through shared APIs.
The CLI remains transitional for commands whose current behavior is broader than the engine surface or transport-specific: full search scope handling, catalog mutation, watcher orchestration, serve transports, GHCR publish/pull boundary behavior, analyze/bootstrap output shaping, impact, and detect-changes.
The package strategy chosen in this PR is to mark `greentic-coding-agent` as `publish = false` while internal path dependencies are used.
The remaining follow-up is a release-layout PR: either publish/version the shared crates in dependency order and re-enable CLI publication, or add a publishable wrapper that does not rely on unpublished path dependencies.

## Codex prompt

```text
Refactor greenticai/greentic-coding-agent by adding a shared `gca-engine` crate and making `gca-cli` a thin Clap adapter.

Preserve all existing command names and flags. Move product logic, indexing, search, policy, package/sync, catalog, and serve request behaviour out of `crates/gca-cli/src/main.rs` and into reusable engine/library functions. Use existing crates where possible rather than creating duplicate models.

Update docs/architecture.md to make the current crate layout canonical and show `gca-engine` as the shared service layer used by CLI, MCP, HTTP/server mode, and future `gtc dev coding-agent` integration.

Add engine-level tests and keep existing CLI tests passing.

Run cargo fmt, clippy, and tests. Complete as much as possible without repeatedly asking permission. Only stop for destructive changes, credentials, or irreversible operations.
```
