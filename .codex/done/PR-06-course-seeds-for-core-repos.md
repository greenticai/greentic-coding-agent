# PR-06 — Seed Training Courses and Knowledge Updates for Core Greentic Repos

## Objective

Add initial course/update files that authoritative repos can copy into their own repositories.

This PR can either live in `greentic-coding-agent` as examples/templates, or be applied as generated PRs to the owning repos later.

## Add templates/examples

```text
examples/training/greentic-component/create-component.course.v1.json
examples/training/greentic-pack/create-extension-pack.course.v1.json
examples/training/greentic-bundle/assemble-bundle.course.v1.json
examples/training/greentic-flow/create-flow.course.v1.json
examples/training/greentic-dev/wizard-launcher.course.v1.json
examples/updates/greentic-component/component-creation-uses-wizard-answers.update.v1.json
examples/updates/greentic-pack/extension-pack-control-hooks-available.update.v1.json
```

## Courses to seed

| Repo | Course |
|---|---|
| `greenticai/greentic-component` | `create_component` |
| `greenticai/greentic-pack` | `create_extension_pack`, `create_application_pack` |
| `greenticai/greentic-bundle` | `assemble_bundle`, `validate_bundle` |
| `greenticai/greentic-flow` | `create_flow`, `step_mapping` |
| `greenticai/greentic-dev` | `wizard_launcher`, `gtc_dev_coding_agent_launcher` |
| `greenticai/greentic-types` | `shared_contract_change` |

## Template quality bar

Each course must include:

- concept ownership
- canonical commands
- deprecated commands or old patterns to avoid
- required validations
- examples/source paths
- agent-oriented steps

## Optional command

Add:

```bash
greentic-coding-agent course init-template --kind component --out .greentic/training/create-component.course.v1.json
```

Kinds:

```text
component
extension-pack
application-pack
bundle
flow
shared-contract
provider
```

## Acceptance criteria

- Example courses validate against the new model.
- Example updates validate against the new model.
- Documentation explains how owning repos should copy/adapt them.

## Codex prompt

```text
Seed training course and knowledge update examples for core Greentic repos.

Add high-quality example course files for greentic-component, greentic-pack, greentic-bundle, greentic-flow, greentic-dev, and greentic-types. Add example knowledge updates for component wizard/answers flow and extension-pack control hooks.

Ensure examples validate against the TrainingCourseDescriptor and KnowledgeUpdateDescriptor models. Add tests that parse all examples. Add docs explaining how authoritative repos should copy these into `.greentic/training/` and `.greentic/updates/`.

Optionally add `course init-template --kind ...` if it fits cleanly.

Run fmt, clippy, and tests.
```
