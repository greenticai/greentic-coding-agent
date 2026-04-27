# PR-04 — Integrate Training Courses and Knowledge Updates into Planning/Validation

## Objective

Make training and update intelligence visible at the moments where coding agents make decisions.

## Scope

Update these flows:

```bash
greentic-coding-agent train --task "..."
greentic-coding-agent course recommend --task "..."
greentic-coding-agent validate-plan plan.json
greentic-coding-agent required-validations --task "..."
greentic-coding-agent search --mode instruction|concept|reuse "..."
```

## Behaviour

When a task matches important/breaking/critical updates, output them before course steps.

Example:

```markdown
# Knowledge updates affecting this task

- `component_creation_uses_wizard_answers`
  - Severity: breaking
  - Agent instruction: Use the current wizard schema and answers.json flow.
  - Deprecated: `greentic component create`
  - Replacement: `gtc wizard component --schema && gtc wizard component --answers answers.json`
```

Then show recommended course.

## Plan validation

If a plan includes deprecated commands or replaced guidance, `validate-plan` should warn or fail depending on severity.

Rules:

| Severity | Plan behaviour |
|---|---|
| info | note |
| recommended | warning |
| important | warning |
| breaking | fail unless explicitly acknowledged |
| critical | fail |

Add optional plan acknowledgement field:

```json
{
  "acknowledged_updates": [
    "greenticai/greentic-component::component_creation_uses_wizard_answers"
  ]
}
```

## Search integration

Search results should include update hits when relevant. If changing `SearchMode` is too disruptive, add `SearchMode::Update` or include update hits in instruction mode.

Preferred:

```rust
pub enum SearchMode {
    Code,
    Instruction,
    Concept,
    Reuse,
    Update,
    Course,
}
```

The current `gca-query::SearchMode` and `SearchResultType` only contain `Code`, `Instruction`, `Concept`, and `Reuse`; update both enums, their parsers, CLI flag validation, Tantivy indexing/search mapping, and JSON/Markdown renderers together if adding `Update` and `Course`.

## MCP tools

Add or extend:

```text
recommend_training_course
recommend_updates_for_task
validate_plan
```

## Acceptance criteria

- `train --task "create component"` shows breaking component update before course steps.
- `validate-plan` detects deprecated commands from updates.
- Search can find update and course records.
- Existing `search --mode code|instruction|concept|reuse` behavior and output shape remain backward compatible.
- JSON outputs remain machine-readable.

## Codex prompt

```text
Integrate training courses and knowledge updates into greentic-coding-agent decision flows.

`train --task` and `course recommend --task` must show relevant important/breaking/critical knowledge updates before course steps. `validate-plan` must detect deprecated commands and replaced guidance from knowledge updates. Breaking or critical updates should cause plan validation to fail unless explicitly acknowledged.

Add search support for courses and updates, either as new search modes or as well-typed results in existing modes. Extend MCP request handling if present.

Add tests for task recommendation, plan validation warnings/failures, acknowledged updates, and JSON/Markdown output.

Run fmt, clippy, and tests.
```
