# PR-02 — Add `agent_training_course` Concept

## Objective

Allow authoritative Greentic repos to teach coding agents how to perform tasks correctly.

Example: `greentic-component` should teach agents how to create a component using the current wizard/answers flow, not obsolete CLI commands.

## New concept

Add built-in concept ID:

```text
agent_training_course
```

## Source-controlled course location

Scan:

```text
.greentic/training/*.course.v1.json
```

Do not use `.greentic-agent/` for authored courses because that directory is generated/cache-like.

## Core model additions

Add to `gca-core`:

```rust
pub struct TrainingCourseDescriptor {
    pub version: String,
    pub id: String,
    pub title: String,
    pub summary: String,
    pub owner_repo: String,
    pub teaches_concepts: Vec<String>,
    pub tasks: Vec<String>,
    pub audience: Vec<TrainingAudience>,
    pub lifecycle_phase: LifecyclePhase,
    pub modules: Vec<TrainingModuleDescriptor>,
    pub canonical_commands: Vec<String>,
    pub deprecated_commands: Vec<DeprecatedCommandDescriptor>,
    pub required_validations: Vec<String>,
    pub examples: Vec<String>,
    pub source_paths: Vec<String>,
}

pub struct TrainingModuleDescriptor {
    pub id: String,
    pub title: String,
    pub objective: String,
    pub steps: Vec<TrainingStepDescriptor>,
}

pub struct TrainingStepDescriptor {
    pub order: u32,
    pub instruction: String,
    pub command: Option<String>,
    pub expected_output: Option<String>,
    pub validation: Option<String>,
}

pub struct DeprecatedCommandDescriptor {
    pub command: String,
    pub reason: String,
    pub replacement: Option<String>,
}

#[serde(rename_all = "snake_case")]
pub enum TrainingAudience {
    CodingAgent,
    HumanDeveloper,
    CiAutomation,
    RepoMaintainer,
}
```

Add to `RepoIndex`:

```rust
#[serde(default)]
pub training_courses: Vec<TrainingCourseDescriptor>,
```

Use `#[serde(default)]` for backward compatibility.

Also update the committed schema/example contract files that mirror `RepoIndex`, especially `schemas/*.cddl` and any `examples/*.json` fixtures loaded by `crates/gca-core/tests/examples.rs`, so old fixtures keep passing and new course fixtures are covered.

## Indexing

`gca-index` should:

1. Discover `.greentic/training/*.course.v1.json`.
2. Parse and validate course descriptors.
3. Add them to `RepoIndex.training_courses`.
4. Add source paths to instruction/search metadata where useful.

The current indexer already builds `instruction_graph`, `instruction_paths`, and `SourceStats`; extend those existing structures/search inputs instead of adding a parallel instruction metadata path.

## Query support

Add functions in `gca-query`:

```rust
pub fn list_training_courses(repo_index: &RepoIndex) -> Vec<TrainingCourseDescriptor>;
pub fn show_training_course(repo_index: &RepoIndex, id: &str) -> Option<TrainingCourseDescriptor>;
pub fn recommend_training_courses(repo_index: &RepoIndex, task: &str, audience: Option<TrainingAudience>) -> Vec<TrainingCourseDescriptor>;
```

Ranking:

1. Exact task match.
2. Concept match.
3. Title/summary match.
4. Canonical command match.
5. Source path match.

## CLI commands

Add:

```bash
greentic-coding-agent courses
greentic-coding-agent course show <course_id>
greentic-coding-agent course recommend --task "create a component"
greentic-coding-agent train --task "create a component" --audience coding_agent
```

`train` should produce agent-ready instructions, not just raw course JSON.

## Example course

Add:

```text
examples/training/create-component.course.v1.json
```

It should teach:

- component concept
- current wizard schema flow
- answers.json flow
- component QA
- wasm32-wasip2 build
- deprecated old commands

## Acceptance criteria

- Old repo-index JSON without `training_courses` still deserializes.
- Existing committed examples and schema tests remain green after adding the optional field.
- A synthetic repo with `.greentic/training/create-component.course.v1.json` indexes the course.
- `courses --format json` returns it.
- `course recommend --task "create a component"` returns it.
- Search can find course content.

## Codex prompt

```text
Add first-class `agent_training_course` support to greenticai/greentic-coding-agent.

Add TrainingCourseDescriptor and related types to gca-core, add `training_courses` to RepoIndex with serde default compatibility, scan `.greentic/training/*.course.v1.json`, validate required fields, and expose course listing/recommendation through gca-query, gca-engine, CLI, and MCP if MCP has a tool surface.

Add commands: `courses`, `course show <course_id>`, `course recommend --task <task>`, and `train --task <task> --audience coding_agent`.

Add an example course for creating a Greentic component using the current wizard/answers flow and warning against obsolete commands.

Add tests for parsing, indexing, recommendation, backward compatibility, and CLI JSON/Markdown output.

Run fmt, clippy, and tests. Complete as much as possible without repeatedly asking permission.
```
