# PR-03 — Add `knowledge_update` Concept

## Objective

Tell coding agents when new capabilities become available or when previous knowledge is stale, deprecated, replaced, or requires migration.

## New concept

Add built-in concept ID:

```text
knowledge_update
```

## Source-controlled update location

Scan:

```text
.greentic/updates/*.update.v1.json
```

## Core model additions

Add to `gca-core`:

```rust
pub struct KnowledgeUpdateDescriptor {
    pub version: String,
    pub id: String,
    pub title: String,
    pub summary: String,
    pub owner_repo: String,
    pub update_type: KnowledgeUpdateType,
    pub published_at: String,
    pub effective_from: Option<String>,
    pub expires_at: Option<String>,
    pub affected_concepts: Vec<String>,
    pub affected_workflows: Vec<String>,
    pub affected_courses: Vec<String>,
    pub affected_repos: Vec<String>,
    pub agent_instruction: String,
    pub human_summary: Option<String>,
    pub new_capabilities: Vec<CapabilityAnnouncement>,
    pub deprecated_commands: Vec<DeprecatedCommandDescriptor>,
    pub replaced_guidance: Vec<ReplacedGuidanceDescriptor>,
    pub migration_steps: Vec<MigrationStepDescriptor>,
    pub required_validations: Vec<String>,
    pub source_paths: Vec<String>,
    pub severity: KnowledgeUpdateSeverity,
}

#[serde(rename_all = "snake_case")]
pub enum KnowledgeUpdateType {
    NewCapability,
    BehaviourChange,
    DeprecatedWorkflow,
    DeprecatedCommand,
    MigrationRequired,
    ValidationChanged,
    OwnershipChanged,
    CourseUpdated,
    SecurityNotice,
    BreakingChange,
    DocumentationCorrection,
}

#[serde(rename_all = "snake_case")]
pub enum KnowledgeUpdateSeverity {
    Info,
    Recommended,
    Important,
    Breaking,
    Critical,
}

pub struct CapabilityAnnouncement {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub use_when: Vec<String>,
    pub owner_repo: String,
    pub related_course: Option<String>,
}

pub struct ReplacedGuidanceDescriptor {
    pub old_guidance: String,
    pub replacement_guidance: String,
    pub reason: String,
}

pub struct MigrationStepDescriptor {
    pub order: u32,
    pub instruction: String,
    pub command: Option<String>,
    pub validation: Option<String>,
}
```

Reuse `DeprecatedCommandDescriptor` from PR-02.

Add to `RepoIndex`:

```rust
#[serde(default)]
pub knowledge_updates: Vec<KnowledgeUpdateDescriptor>,
```

Also update the committed schema/example contract files that mirror `RepoIndex`, especially `schemas/*.cddl` and any `examples/*.json` fixtures loaded by `crates/gca-core/tests/examples.rs`, so old fixtures keep passing and new update fixtures are covered.

## Required validation fields

Each update must have:

```text
version
id
title
summary
owner_repo
update_type
published_at
severity
agent_instruction
```

## Query support

Add functions:

```rust
pub fn list_knowledge_updates(repo_index: &RepoIndex, filter: UpdateFilter) -> Vec<KnowledgeUpdateDescriptor>;
pub fn show_knowledge_update(repo_index: &RepoIndex, id: &str) -> Option<KnowledgeUpdateDescriptor>;
pub fn recommend_updates_for_task(repo_index: &RepoIndex, task: &str) -> Vec<KnowledgeUpdateDescriptor>;
pub fn recommend_updates_for_concept(repo_index: &RepoIndex, concept_id: &str) -> Vec<KnowledgeUpdateDescriptor>;
```

`UpdateFilter` should be a concrete serializable type in `gca-query` rather than an implied placeholder, because the CLI and MCP surfaces will need stable JSON-compatible filters.

Ranking:

1. critical
2. breaking
3. important
4. recommended
5. info
6. task/concept/course/workflow match
7. recency

## CLI commands

Add:

```bash
greentic-coding-agent updates
greentic-coding-agent updates show <update_id>
greentic-coding-agent updates --task "create a component"
greentic-coding-agent updates --concept component
greentic-coding-agent updates --severity breaking
```

Markdown output must show:

- severity
- update type
- agent instruction
- deprecated commands and replacements
- migration steps
- required validations

## Example update

Add:

```text
examples/updates/component-creation-uses-wizard-answers.update.v1.json
```

This should warn agents that component creation must use the current wizard schema and answers.json flow.

## Acceptance criteria

- Old repo-index JSON without `knowledge_updates` still deserializes.
- Existing committed examples and schema tests remain green after adding the optional field.
- Synthetic repo update files are indexed.
- CLI filters by task, concept, severity.
- Search finds update content.

## Codex prompt

```text
Add first-class `knowledge_update` support to greenticai/greentic-coding-agent.

Add core model types for KnowledgeUpdateDescriptor, KnowledgeUpdateType, KnowledgeUpdateSeverity, CapabilityAnnouncement, ReplacedGuidanceDescriptor, and MigrationStepDescriptor. Add `knowledge_updates` to RepoIndex with serde default compatibility.

Scan `.greentic/updates/*.update.v1.json`, validate required fields, index update content for search, and expose commands: `updates`, `updates show <id>`, `updates --task`, `updates --concept`, and `updates --severity`.

Add an example update warning that component creation must use the current wizard/answers flow. Add tests for parsing, indexing, filtering, backward compatibility, and CLI output.

Do not implement seen-state yet. Run fmt, clippy, and tests.
```
