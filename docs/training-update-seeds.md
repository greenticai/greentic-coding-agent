# Training and Update Seeds

This repository carries starter training-course and knowledge-update files that authoritative Greentic repos can copy into their own indexes.

Copy courses into:

```text
.greentic/training/*.course.v1.json
```

Copy updates into:

```text
.greentic/updates/*.update.v1.json
```

Recommended seed mapping:

| Owning repo | Files |
|---|---|
| `greenticai/greentic-component` | `examples/training/greentic-component/create-component.course.v1.json`, `examples/updates/greentic-component/component-creation-uses-wizard-answers.update.v1.json` |
| `greenticai/greentic-pack` | `examples/training/greentic-pack/create-extension-pack.course.v1.json`, `examples/training/greentic-pack/create-application-pack.course.v1.json`, `examples/updates/greentic-pack/extension-pack-control-hooks-available.update.v1.json` |
| `greenticai/greentic-bundle` | `examples/training/greentic-bundle/assemble-bundle.course.v1.json`, `examples/training/greentic-bundle/validate-bundle.course.v1.json` |
| `greenticai/greentic-flow` | `examples/training/greentic-flow/create-flow.course.v1.json`, `examples/training/greentic-flow/step-mapping.course.v1.json` |
| `greenticai/greentic-dev` | `examples/training/greentic-dev/wizard-launcher.course.v1.json`, `examples/training/greentic-dev/gtc-dev-coding-agent-launcher.course.v1.json` |
| `greenticai/greentic-types` | `examples/training/greentic-types/shared-contract-change.course.v1.json` |

Before copying, adjust repo-local command names, source paths, and validations to match the owning repo. Keep these fields populated because agents use them for recommendation and validation:

- `owner_repo`
- `teaches_concepts`
- `canonical_commands`
- `deprecated_commands`
- `required_validations`
- `examples`
- `source_paths`

After copying, run:

```bash
greentic-coding-agent analyze --print --format json
greentic-coding-agent course recommend --task "create component" --format json
greentic-coding-agent updates --task "create component" --format json
```

The examples are validated by `cargo test -p gca-core --test examples`.
