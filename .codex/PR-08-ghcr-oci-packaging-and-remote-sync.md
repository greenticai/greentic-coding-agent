# PR-08: GHCR OCI packaging and remote sync

## Title

feat(coding-agent): package repo indexes as OCI artifacts and add remote sync/discovery client

## Objective

Support publication and retrieval of repo index artifacts via GHCR in a common format.

## Why

This turns Greentic-coding-agent from a local helper into a cross-repo intelligence fabric.

## Scope

### In `gca-oci`
Implement:
- package local repo outputs into OCI artifact layout
- push to registry
- pull from registry
- parse artifact contents

### In `gca-cli`
Add:
```bash
gtc dev coding-agent package-index
gtc dev coding-agent publish-index
gtc dev coding-agent sync
gtc dev coding-agent list-remote-repos
```

## Artifact layout

Package:
- `repo-manifest.json`
- `repo-index.json`
- `repo-index.cbor` (optional this PR; if postponed, document)
- `agents/*.md`

## Naming convention
Default target path:
```text
ghcr.io/greenticai/indexes/<repo-name>:latest
```
Also support explicit `--tag`.

## Sync behavior
- pull manifest-only metadata where possible
- record local cache metadata
- prepare for catalog integration

## Acceptance criteria

- local package command produces artifact-ready directory/tar representation
- publish command can push in CI-friendly mode
- sync can pull an explicitly named repo index
- provenance and compatibility metadata are preserved

## Test plan

- OCI layout snapshot tests
- mock registry integration tests if possible
- serialization roundtrip tests

## Out of scope

- central catalog
- workflow installation
