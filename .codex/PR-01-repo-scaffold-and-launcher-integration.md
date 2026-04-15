# PR-01: Repository scaffold + `gtc dev coding-agent` launcher integration

## Title

feat(coding-agent): scaffold greentic-coding-agent and wire `gtc dev coding-agent` launcher

## Objective

Create the new `greentic-coding-agent` repository skeleton and the minimal `greentic-dev` integration needed so `gtc dev coding-agent ...` delegates to the standalone binary.

## Why

Before implementing indexing, policies, or GHCR publication, the product needs:
- a dedicated repo
- a stable binary
- a predictable launcher story
- shared architecture and schema placeholders
- a basic smoke-test path

## Scope

### In `greentic-coding-agent`
Create:
- Cargo workspace
- initial crates:
  - `gca-cli`
  - `gca-core`
  - `gca-index`
  - `gca-query`
  - `gca-greentic`
  - `gca-agent-files`
  - `gca-oci`
  - `gca-mcp`
- `schemas/`
- `templates/`
- `docs/`
- `.codex/`
- root `README.md`
- root `architecture.md` link

### In `greentic-dev`
Add launcher integration so:
```bash
gtc dev coding-agent ...
```
delegates to `greentic-coding-agent` binary following the existing `greentic-*` execution pattern.

## Deliverables

1. New workspace compiles.
2. `greentic-coding-agent --help` works.
3. `gtc dev coding-agent --help` works.
4. Root README explains:
   - product purpose
   - local vs remote model
   - relationship to Greentic-dev
5. Add a tiny smoke test for launcher delegation.

## Detailed implementation notes

### CLI shape
Initial subcommands can be placeholders:
- `analyze`
- `describe`
- `search`
- `plan`
- `serve`
- `generate-agent-files`
- `install-github-workflow`
- `sync`

Each can return “not implemented” except `describe --here`, which should at least return:
- repo root if detected
- binary version
- minimal repo metadata

### Config
Introduce a config type in `gca-core`:
```rust
pub struct AgentConfig {
    pub format: OutputFormat,
    pub registry_path: PathBuf,
    pub local_index_dir_name: String,
}
```

### Output formats
Support:
- json
- markdown

CBOR can come later.

### `greentic-dev` integration
Follow existing command resolution rules for `greentic-*` binaries. Do not embed business logic in `greentic-dev`; it should remain a launcher.

## Acceptance criteria

- `cargo test --workspace` passes in `greentic-coding-agent`
- launcher test passes in `greentic-dev`
- README and architecture link exist
- binary naming is stable and documented

## Test plan

### Unit
- CLI parse tests
- config default tests

### Integration
- launcher invokes correct binary name
- fallback/error message when binary is absent is clear

## Out of scope

- real indexing
- schema extraction
- GHCR publishing
- MCP tools
