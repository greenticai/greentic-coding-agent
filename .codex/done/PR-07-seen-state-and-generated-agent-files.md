# PR-07 — Seen-State for Knowledge Updates and Generated Agent File Integration

## Objective

Let agents know which knowledge updates are new/unseen and include high-signal updates in generated `AGENTS.md`, `CODEX.md`, `CLAUDE.md`, and `llms.txt`.

## Local state file

Add:

```text
~/.greentic-agent/agent-knowledge-state.json
```

Model:

```rust
pub struct AgentKnowledgeState {
    pub version: String,
    pub last_sync_at: Option<String>,
    pub seen_updates: BTreeMap<String, SeenKnowledgeUpdate>,
}

pub struct SeenKnowledgeUpdate {
    pub seen_at: String,
    pub source_digest: Option<String>,
}
```

Stable update key:

```text
<owner_repo>::<update_id>
```

If digest changes, treat update as new again.

## Commands

```bash
greentic-coding-agent updates --new
greentic-coding-agent updates mark-seen <update_id>
greentic-coding-agent updates mark-seen --all
```

Do not automatically mark updates as seen when listing them.

## Generated files

Update generated files to include:

```markdown
## Recent knowledge updates

- 2026-04-26: Component creation must use the current wizard/answers flow.
- 2026-04-26: Extension packs can now declare control hooks.
```

Rules:

- Include severity `important`, `breaking`, `critical`.
- Include only updates affecting this repo, this repo's concepts, or known courses.
- Exclude expired updates.
- Keep output concise.

## MCP

Add:

```text
list_new_knowledge_updates
mark_knowledge_update_seen
```

## Acceptance criteria

- `updates --new` shows unseen updates.
- `mark-seen` suppresses update until digest changes.
- Generated files include relevant important/breaking/critical updates.
- Listing updates does not mark them seen.

## Codex prompt

```text
Add local seen-state support for knowledge updates and include important updates in generated agent files.

Create `~/.greentic-agent/agent-knowledge-state.json`, track seen updates by stable ID and source digest, and add commands `updates --new`, `updates mark-seen <id>`, and `updates mark-seen --all`. Do not automatically mark updates as seen when listing them.

Update AGENTS.md, CODEX.md, CLAUDE.md, and llms.txt generation so they include concise recent knowledge updates with severity important/breaking/critical that affect the repo or its concepts/courses.

Add tests for new/seen/digest-changed behaviour and generated file content. Run fmt, clippy, and tests.
```
