# PR-07: Generated agent files

## Title

feat(coding-agent): generate AGENTS.md, CLAUDE.md, CODEX.md, and llms.txt from indexed knowledge

## Objective

Produce the agent-facing text artifacts automatically from the canonical graph.

## Why

These files are immediately useful to coding agents, but they must stay consistent with indexed facts and policy.

## Scope

Implement in `gca-agent-files`:
- template loading
- markdown rendering
- file writing into `.greentic-agent/generated/`
- optional `--write-root` mode to write to repo root when requested

### CLI
```bash
gtc dev coding-agent generate-agent-files
```

## Required content

### AGENTS.md
- repo summary
- top workflows
- reuse/ownership warnings
- mandatory validations
- command cheat sheet

### CLAUDE.md
- first recommended calls
- stale-index warning
- impact-before-edit policy placeholder
- validation reminders

### CODEX.md
- how to orient in the repo quickly
- expectations for complete-but-safe execution
- required checks before finishing
- reuse-first guidance

### llms.txt
- compact pointers to the most useful docs and commands

## Rendering requirements

- deterministic ordering
- no duplicated sections
- provenance note that files are generated
- generator version stamped in comment/footer

## Acceptance criteria

- generated files appear after analyze+generate-agent-files
- templates can be customized later without changing domain model
- output quality is high enough to use immediately

## Test plan

- golden file tests for generated markdown
- missing-data fallback tests

## Out of scope

- root git modifications by default
- agent-specific hooks
