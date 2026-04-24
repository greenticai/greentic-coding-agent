# PR-09: Central catalog discovery and generated nightly GitHub workflow

## Title

feat(coding-agent): add GHCR catalog format, auto-discovery, refresh checks, and workflow installer

## Objective

Implement the remote discovery model and the per-repo CI automation that:
- checks whether refresh is needed
- publishes updated indexes
- enables Greentic-coding-agent to auto-discover new or updated repos

## Why

This is the key distributed design requirement: indexes should self-refresh and self-publish in a common format so the tool can discover them automatically.

## Scope

### Catalog
Implement `greentic.agent.catalog.v1` support in `gca-ghcr-catalog`.

Add CLI:
```bash
gtc dev coding-agent show-catalog
gtc dev coding-agent sync
```

### Refresh check
Implement:
```bash
gtc dev coding-agent check-refresh
```

Refresh if:
- source commit changed
- indexed file fingerprint changed
- generator version changed
- schema version changed

### GitHub workflow installer
Add:
```bash
gtc dev coding-agent install-github-workflow
```

This should generate/update:
```text
.github/workflows/greentic-agent-index.yml
```

## Workflow requirements

### Triggers
- push to main
- nightly schedule
- workflow_dispatch

### Steps
1. checkout
2. install Rust/toolchain and/or greentic-coding-agent binary
3. analyze
4. check-refresh
5. package-index
6. publish-index when needed
7. upload summary artifact/log

### Permissions
Document required GitHub permissions for GHCR publish.

## Acceptance criteria

- workflow file can be generated idempotently
- catalog format loads cleanly
- sync can read catalog and discover multiple repos
- check-refresh reports explicit reasons

## Test plan

- workflow template rendering tests
- refresh-decision tests
- catalog parsing fixtures

## Out of scope

- impact analysis
- detect changes
