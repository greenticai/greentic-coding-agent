# PR-10: MCP server, impact analysis, and detect-changes

## Title

feat(coding-agent): expose MCP tools and add impact / detect-changes query capabilities

## Objective

Finish the first serious coding-agent experience by exposing the graph and policy through MCP and adding blast-radius / changed-area awareness.

## Why

This is where Greentic-coding-agent becomes a true daily driver for Codex, Claude Code, and similar tools.

## Scope

### MCP server (`gca-mcp`)
Add tools:
- `describe_repo`
- `list_workflows`
- `explain_concept`
- `search_code`
- `search_instructions`
- `search_reuse`
- `locate_owner`
- `plan_change`
- `required_validations`
- `impact_analysis`
- `detect_changes`
- `show_freshness`
- `list_remote_repos`

### Impact analysis
Initial implementation can be heuristic:
- Cargo dependency graph
- symbol/file references
- workflow/policy consumers
- downstream repo consumers from seeded policy

### Detect changes
Compare:
- working tree vs index
- staged/unstaged state if available
- changed files mapped to concepts/workflows/validations

### CLI additions
```bash
gtc dev coding-agent serve
gtc dev coding-agent impact --symbol <id>
gtc dev coding-agent detect-changes
gtc dev coding-agent validate-plan <plan.json>
```

## Required UX behavior

- freshness warning when local index is stale vs HEAD
- impact output includes confidence and provenance
- detect-changes output includes:
  - changed files
  - likely concepts affected
  - suggested validations

## Acceptance criteria

- MCP server starts and serves documented tools
- CLI impact/detect-changes produce useful structured output
- stale-index warning is visible through both CLI and MCP

## Test plan

- MCP smoke tests
- impact fixture tests
- detect-changes temp repo integration tests

## Out of scope

- perfect semantic impact analysis
- browser UI
