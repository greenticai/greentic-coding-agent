# PR-11: Cross-repo Greentic adapters and seeded knowledge packs

## Title

feat(coding-agent): add seeded adapters/knowledge for core Greentic repos and concept families

## Objective

Seed the system with curated high-value knowledge for the first target repos so results are strong immediately.

## Why

Pure extraction is useful, but curated high-confidence seeded knowledge will make the system much more effective for real Greentic work.

## Scope

Add seeded adapters or descriptor packs for:
- `greentic-types`
- `greentic-pack`
- `greentic-bundle`
- `greentic-dev`
- `greentic-x`
- `greentic-sorla`

Each adapter should enrich:
- repo role
- likely workflows
- concept ownership
- validation hints
- known docs of interest

## Deliverables

- adapter registry
- repo-specific enrichers
- fixtures showing expected enriched output

## Acceptance criteria

- `describe --here` in seeded repos is significantly better than generic mode
- owner/reuse answers for seeded concepts are stable
- docs explain how to add new adapters

## Test plan

- per-repo fixture snapshots
- adapter registration tests
