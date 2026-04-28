# PR 05 — Make agent/MCP server global by default

## Position in sequence

Implement after PR 04 provides channel-aware global cache/status. PR 05 should consume that channel/status model rather than inventing a parallel cache.

Current local reality before this PR:

- `serve --stdio` and `serve --http` exist.
- There is no `agent` command group.
- Existing MCP-style tools are un-namespaced command names, not `gca.*`.
- `serve` can use cached/merged data in some paths, but repo-local behavior and transitional CLI paths still exist.

## Goal

Codex, Claude Code and MCP-style hosts should use the merged global index by default.

## Commands

Add or complete:

```bash
greentic-coding-agent serve --stdio
greentic-coding-agent serve --http --host 127.0.0.1 --port 7757
greentic-coding-agent agent context --task "add static route support" --format json
greentic-coding-agent agent preflight --task "..." --repo greenticai/greentic-pack --format json
greentic-coding-agent agent owner --concept greentic.static-routes.v1 --format json
```

This PR adds the `agent` command group. Do not describe these commands as already existing.

## Default context priority

When running inside a repo:

1. merged global channel index
2. current repo published branch index
3. local working-tree overlay
4. root agent bootstrap files, if present

When running outside a repo:

1. merged global channel index
2. tutorials/courses/update indexes

## Required agent context response

```json
{
  "task": "add static route support",
  "channel": "develop",
  "relevant_repos": [],
  "owner_candidates": [],
  "required_validations": [],
  "recent_updates": [],
  "tutorials": [],
  "warnings": []
}
```

## MCP tools

Expose stable tools. Decide whether these are additions alongside the existing tool names or a breaking rename:

- `gca.search`
- `gca.agent_context`
- `gca.find_owner`
- `gca.required_validations`
- `gca.recent_updates`
- `gca.branch_status`

## Acceptance criteria

- `serve --stdio` no longer assumes repo-local-only context.
- JSON outputs are deterministic and testable.
- Docs include Codex and Claude Code bootstrap examples.
- Existing MCP tests continue to pass, or migration tests cover old and new tool names.

## Implementation notes

- Added the `agent` command group with `context`, `preflight`, and `owner` subcommands.
- Agent context defaults to the configured channel, falling back to `develop`, and combines channel-matched synced indexes with the current repo as a local overlay.
- Added stable MCP aliases: `gca.search`, `gca.agent_context`, `gca.find_owner`, `gca.required_validations`, `gca.recent_updates`, and `gca.branch_status`.
- Kept the existing MCP tool names for compatibility.
- Added CLI integration coverage for the new `agent` commands and stdio dispatch of `gca.agent_context`.
- Added `gca-mcp` unit coverage for the stable `gca.*` aliases.
- Added `examples/mcp-request.gca-agent-context.json` as a concrete stable-tool request payload.
- Documented stdio/HTTP serving, agent helper commands, stable MCP names, and Codex/Claude Code bootstrap snippets in `README.md`, `docs/server.md`, and `docs/architecture.md`.
