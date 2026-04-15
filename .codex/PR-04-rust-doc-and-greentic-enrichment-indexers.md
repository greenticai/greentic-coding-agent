# PR-04: Rust, docs, and Greentic enrichment indexers

## Title

feat(coding-agent): add Rust/doc extraction and Greentic-specific enrichment

## Objective

Expand `analyze` so it produces a useful `RepoIndex` with:
- Cargo/Rust symbol metadata
- docs/instruction metadata
- Greentic concept/workflow enrichment

## Why

Without this, the system is only a repo registry. It needs enough structure for agents to understand concepts, workflows, and likely touchpoints.

## Scope

### Rust extraction (`gca-rust`)
Extract at least:
- workspace members
- crate names
- modules via file paths
- public types/functions where feasible
- test targets
- features
- Cargo dependencies

Prefer a pragmatic first pass; do not block on a perfect full semantic parser.

### Docs extraction (`gca-docs`)
Index:
- README.md
- ARCHITECTURE.md
- RUNBOOK.md
- TESTING.md
- CONTRIBUTING.md
- `.codex/*.md`
- `docs/**/*.md`
- `examples/`
- `.github/workflows/*.yml`

### Greentic enrichment (`gca-greentic`)
Heuristically infer:
- repo role
- concept matches
- workflow matches
- Greentic-specific command references:
  - `gtc wizard --schema`
  - `gtc wizard --answers`
  - `gtc setup --schema`
  - `gtc setup <bundle> --answers`
  - `gtc start <bundle>`

Also tag known concepts:
- Greentic-X
- Greentic-sorla
- digital worker
- application pack
- extension pack

## Deliverables

1. `RepoIndex` now contains non-trivial:
   - concept_graph
   - workflow_graph
   - instruction_graph
   - source_stats
2. `describe --here` becomes materially useful.
3. `concepts` and `workflows` subcommands become available.

## Required CLI additions

```bash
gtc dev coding-agent concepts
gtc dev coding-agent workflows
```

## Acceptance criteria

- analyzing a Greentic repo yields at least one inferred repo role
- `.codex` docs appear in instruction graph
- known Greentic command strings are discoverable in workflow graph when present
- snapshot examples committed

## Test plan

- fixture repos for:
  - generic Rust repo
  - Greentic-style repo
- snapshot tests for role/concept/workflow inference

## Out of scope

- advanced search ranking
- reuse policy
- remote catalog
