# PR-03: Local registry + `analyze` bootstrap

## Title

feat(coding-agent): implement local analyze pipeline, repo-local storage, and user-global registry

## Objective

Implement the single bootstrap command:
```bash
gtc dev coding-agent analyze
```
so it can:
- detect repo root
- gather basic git metadata
- create `.greentic-agent/`
- write a minimal manifest/index
- update `~/.greentic-agent/registry.json`

## Why

This is the first core productivity feature and mirrors the best lesson from GitNexus: one command should get the repo into an agent-usable state.

## Scope

### In `gca-index`
Implement:
- repo root detection
- git metadata capture
- file fingerprinting
- repo-local output writing

### In `gca-core`
Add registry model and safe read/write helpers.

### In `gca-cli`
Implement:
```bash
gtc dev coding-agent analyze
gtc dev coding-agent describe --here
```

## Local storage layout

```text
.greentic-agent/
  manifest.json
  repo-index.json
  fingerprints.json
```

## Registry layout

```json
{
  "version": "v1",
  "repos": [
    {
      "repo_name": "...",
      "repo_path": "...",
      "repo_role": "...",
      "last_analyzed_commit": "...",
      "manifest_path": "...",
      "repo_index_path": "...",
      "updated_at": "..."
    }
  ]
}
```

## Minimum analyze output

Even before rich extraction, `analyze` must capture:
- repo name
- repo root
- git HEAD sha
- default branch if available
- generated_at
- basic repo role heuristic
- list of candidate docs
- list of Cargo manifests discovered

## CLI behavior

### `analyze`
- default: write outputs
- `--print` may print summary
- `--format json|markdown`

### `describe --here`
Return minimal summary of current repo using local index if present.

## Acceptance criteria

- repo-local index files created
- registry is updated idempotently
- repeated analyze updates same repo entry rather than duplicating
- if no git repo is found, user gets a clean error

## Test plan

- temp git repo integration tests
- registry update idempotency tests
- analyze output snapshot tests

## Out of scope

- symbol-level Rust parsing
- generated agent files
- remote sync
