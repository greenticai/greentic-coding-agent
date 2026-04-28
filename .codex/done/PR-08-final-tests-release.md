# PR 08 — Final compatibility, tests, release checklist

## Position in sequence

This is the final hardening PR after PR 02-07. Do not start release work until the branch/channel model, producer workflow, consumer sync/status, agent context, notifications, and catalog automation are implemented.

Current local reality before this series:

- The CLI crate is `publish = false`.
- The workspace contains internal path-only crates, so crates.io packaging strategy must be resolved before release/binstall docs are true.
- Existing tests heavily cover v1/latest behavior and should be preserved as compatibility tests.

## Goal

Harden the migration, preserve compatibility, and prepare release.

## Test matrix

Add or verify tests for:

- catalog v1 parse
- catalog v2 parse
- branch fallback behavior
- repo index metadata generation
- generated workflow content
- multiple publish tags
- sync without current repo
- status without current repo
- merged search branch selection
- MCP/agent context output stability
- updates notification seen/new behavior

## Compatibility checks

Existing commands must still work:

```bash
greentic-coding-agent analyze --print --format json
greentic-coding-agent describe --here --format json
greentic-coding-agent search --mode instruction wizard --format json
greentic-coding-agent catalog validate --format json
```

## Release notes

Create release notes explaining:

- new global default workflow
- branch-aware indexes
- develop/main channels
- local cache
- agent/MCP global usage
- daemon/watch preview if applicable
- v1 catalog compatibility

## Release layout prerequisite

Before documenting `cargo binstall greentic-coding-agent` as the install path, choose and implement one:

- publish/version internal crates in dependency order and re-enable the CLI package for crates.io; or
- create a separate publishable wrapper that does not depend on unpublished path crates; or
- document GitHub release binary installation instead of `cargo binstall`.

## Acceptance criteria

- `bash ci/local_check.sh` passes.
- Docs and CLI help agree.
- Migration guide exists.
- Existing users have a clear compatibility path.
- The README, ADMIN guide, generated workflow docs, and `--help` output do not advertise unavailable commands or flags.

## Implementation notes

- Added `docs/release-notes-0.1.2.md` covering the global workflow default, branch-aware indexes, main/develop channel publishing, local cache/status, agent/MCP tools, watch/daemon preview, catalog automation, and catalog v1 compatibility.
- Added `docs/migration-0.1.2.md` with upgrade steps, compatibility behavior, command checks, and rollback notes.
- Linked the release notes and migration guide from `README.md`.
- Clarified in `docs/architecture.md` that installation should use built binaries or GitHub release assets until the crates.io package layout is resolved; `cargo install` / `cargo binstall` are intentionally not documented as the current install path.
- Added `compatibility_commands_remain_available` in `crates/gca-cli/tests/cli.rs` for:
  - `analyze --print --format json`
  - `describe --here --format json`
  - `search --mode instruction wizard --format json`
  - `catalog validate --format json`

## Verification

- `cargo test -p greentic-coding-agent --test cli compatibility_commands_remain_available`
- `bash ci/local_check.sh`
