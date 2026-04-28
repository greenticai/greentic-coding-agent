# PR 01 — Reframe product around organisation-wide knowledge fabric

## Position in sequence

This is a documentation PR that should land after PR 02, PR 03 and PR 04, or be written with explicit "coming next" wording.

Do not document commands as the default working flow until the implementation exists:

- `greentic-coding-agent sync --channel <channel>`
- `greentic-coding-agent status --channel <channel>`
- `cargo binstall greentic-coding-agent`

Current local reality before this series:

- The CLI crate is `publish = false`, so `cargo binstall greentic-coding-agent` is not a valid default install path yet.
- `sync` exists, but it is tag/catalog based and has no `--channel`.
- There is no top-level `status` command, only HTTP `/status` in server mode.
- Repo-local `analyze` is currently the safest documented first command.

## Goal

Change the primary user story from “run inside each repo” to “install once, sync all Greentic knowledge, serve it to coding agents”.

## Files to update

- `README.md`
- `ADMIN.md`
- `docs/catalogs.md`
- `docs/server.md`
- new `docs/producer-vs-consumer.md`
- new `docs/local-cache-layout.md`
- new `docs/agent-global-usage.md`

## Required README structure

1. What Greentic Coding Agent is
2. Current stable workflow
3. Future default developer workflow after PR 02-04
4. Default Codex/Claude workflow
5. How indexes are produced in CI
6. Branch/channel model: `main`, `develop`, SHA tags
7. Repo-local analysis as advanced/producer mode
8. Troubleshooting

## Replace current opening with once PR 02-04 are implemented

```md
# Greentic Coding Agent

Greentic Coding Agent gives developers, Codex, Claude Code and other coding agents an always-current local knowledge base for the Greentic engineering ecosystem.

Each Greentic repository publishes branch-specific indexes from GitHub Actions. Developers install one binary, sync the Greentic catalog, and agents can search across all repos, tutorials, courses, ownership rules, validation rules, workflows and recent updates.
```

Before PR 02-04 are implemented, keep the opening closer to:

```md
Greentic Coding Agent indexes Greentic repositories and serves that knowledge to developers, Codex, Claude Code and other coding agents.

Today it supports repo-local analysis, generated agent files, published indexes, catalogs, merged search, and MCP/HTTP serving. The next series makes the branch-aware global sync workflow the default daily flow.
```

## Future default workflow to document after implementation

```bash
cargo binstall greentic-coding-agent
greentic-coding-agent sync --channel develop
greentic-coding-agent status --channel develop
greentic-coding-agent serve --stdio
```

## Current compatible workflow to keep documented

```bash
greentic-coding-agent analyze --print --format json
greentic-coding-agent sync --format json
greentic-coding-agent search --mode instruction --scope merged "component manifest" --format json
greentic-coding-agent serve --stdio
```

## Repo-local mode wording after PR 02-04

Repo-local `analyze` remains supported, but describe it as:

- CI producer command
- local debugging command
- new repo bootstrap command
- working-tree overlay support

## Acceptance criteria

- New readers understand that the tool is installed once by developers.
- README does not present repo-local `analyze` as the future primary daily workflow after PR 02-04.
- Until PR 02-04 land, README clearly marks channel/global behavior as planned.
- Docs clearly explain producer vs consumer responsibilities.
- Existing commands remain documented for compatibility.

## Implementation notes

- Reframed the README opening around the organization-wide knowledge fabric and install-once/sync-once developer workflow.
- Added a current stable workflow using built binaries or GitHub release binaries, `init`, `sync --channel`, `status --channel`, merged `search`, and `serve --stdio`.
- Kept repo-local `analyze` documented as producer/debug/bootstrap/working-tree overlay mode and preserved existing compatibility command examples.
- Documented the branch/channel model for `main`, `develop`, and `sha-<commit>` tags.
- Added `docs/producer-vs-consumer.md` for CI producer and developer/agent consumer responsibilities.
- Added `docs/local-cache-layout.md` for user-local cache, repo-local producer output, notifications, and channel selection.
- Added `docs/agent-global-usage.md` for direct agent commands, MCP tool names, Codex bootstrap, Claude Code bootstrap, and watch/daemon usage.
- Linked the new docs from `README.md`, `ADMIN.md`, `docs/catalogs.md`, and `docs/server.md`.
- Kept `cargo install` / `cargo binstall` out of the default install flow because the workspace still uses unpublished internal path crates.

## Verification

- Documentation-only change; no code tests run.
- Checked docs for `cargo binstall` references to ensure they are warnings/prerequisite notes rather than default install instructions.
