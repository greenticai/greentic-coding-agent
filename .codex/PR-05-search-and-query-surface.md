# PR-05: Search and query surface

## Title

feat(coding-agent): add structured search and query commands for code, instruction, and concept discovery

## Objective

Expose a useful first query layer for coding agents.

## Why

Indexing without query commands does not help Codex or Claude Code become productive.

## Scope

Implement in `gca-query` and `gca-cli`:
- `describe --here`
- `commands`
- `concepts`
- `workflows`
- `search --mode code`
- `search --mode instruction`
- `search --mode concept`

## Search design

### Modes
- `code`
- `instruction`
- `concept`

Later PRs will add `reuse`.

### Results
Every result should include:
- stable ID
- title / label
- result type
- path or concept ID
- explanation snippet
- provenance
- freshness marker

### Output
Support:
- JSON
- markdown

## Commands catalog
Add a static/generated command catalog that explains:
- command
- purpose
- phase
- inputs
- outputs
- when to use it

This is the beginning of the workflow intelligence layer.

## Acceptance criteria

- searches return deterministic, structured results
- empty results are explicit, not ambiguous
- markdown output is readable by humans
- JSON output is easy for coding agents to consume

## Test plan

- search fixture snapshots
- ranking sanity tests
- command catalog output tests

## Out of scope

- plan generation
- ownership/reuse policy
- impact analysis
