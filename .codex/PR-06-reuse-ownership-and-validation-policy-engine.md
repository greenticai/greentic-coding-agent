# PR-06: Reuse, ownership, and validation policy engine

## Title

feat(coding-agent): implement reuse-first ownership rules and validation mapping

## Objective

Add the policy layer that answers:
- where should this change live?
- what repo owns this concept?
- what should not be duplicated here?
- what validations are required?

## Why

This directly addresses the main failure mode you described: coding agents over-implementing in the wrong place.

## Scope

### Policy inputs
Support:
- built-in policy tables
- repo-local policy descriptors checked into repo
- future extension hooks

### Implement
- `ReuseDescriptor`
- owner lookup
- consumer repo lookup
- forbidden duplication zones
- required follow-up validations

### CLI additions
```bash
gtc dev coding-agent locate-owner --concept <id>
gtc dev coding-agent required-validations --task "<task>"
gtc dev coding-agent search --mode reuse "<query>"
```

## Initial policy content

Add seed policy coverage for at least:
- extension pack schema
- application pack schema
- setup/runtime schema changes
- component QA schema changes
- bundle activation/start concerns
- Greentic-X catalog changes
- Greentic-sorla provider/schema changes

## Validation mapping examples
- shared schema changed => downstream fixture and consumer tests
- docs-only change => docs lint / link checks
- new workflow added => command catalog + agent file regeneration

## Acceptance criteria

- system can answer owner repo for at least seeded concepts
- reuse search returns structured rationale
- required-validations returns concrete commands/check groups when known

## Test plan

- seeded policy snapshots
- conflict resolution tests
- unknown concept behavior tests

## Out of scope

- remote catalog merge
- impact graph
