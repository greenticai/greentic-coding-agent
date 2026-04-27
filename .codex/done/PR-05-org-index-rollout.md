# PR-05 — Org-wide Repository Index Rollout

## Objective

Add a deterministic plan/apply workflow that discovers Greentic repos and opens PRs to install the standard coding-agent index workflow in each repo.

## New concept

Add built-in concept ID:

```text
repository_index_rollout
```

## Commands

```bash
greentic-coding-agent org plan-index-rollout --org greenticai --format json
greentic-coding-agent org apply-index-rollout --plan rollout-plan.json --open-prs
```

Optional sources:

```bash
--repo-source github-org
--repo-source greenticai/.github
--repo-list-file repos.json
```

## Rollout plan model

Add to a suitable crate, preferably `gca-engine` or a new `gca-org` crate:

```rust
pub struct IndexRolloutPlan {
    pub version: String,
    pub org: String,
    pub generated_at: String,
    pub repos: Vec<IndexRolloutRepoAction>,
}

pub struct IndexRolloutRepoAction {
    pub repo_id: String,
    pub default_branch: String,
    pub archived: bool,
    pub action: IndexRolloutAction,
    pub reason: String,
    pub workflow_path: String,
    pub branch_name: Option<String>,
    pub pr_title: Option<String>,
    pub pr_body: Option<String>,
}

#[serde(rename_all = "snake_case")]
pub enum IndexRolloutAction {
    Skip,
    AlreadyEnabled,
    CreatePr,
    UpdateExistingWorkflow,
}
```

## Detection

For each repo:

1. Skip archived repos.
2. Skip `.github` unless explicitly included.
3. Check `.github/workflows/greentic-agent-index.yml`.
4. If missing, action = `create_pr`.
5. If present but outdated, action = `update_existing_workflow`.
6. If present and current, action = `already_enabled`.

## Apply behaviour

`apply-index-rollout` should:

1. Read plan JSON.
2. For each `create_pr` or `update_existing_workflow` action:
   - create branch
   - write generated workflow
   - commit
   - open PR
3. Never push or open PRs unless `apply-index-rollout` is used.
4. Support `--dry-run`.

The existing workflow renderer/installer lives in `gca-oci` and is already exposed through the CLI `install-github-workflow` flow. Reuse that renderer as the source of truth for current/outdated detection instead of embedding a second workflow template in the org rollout implementation.

## PR title

```text
Enable Greentic coding-agent index publishing
```

## PR body

```markdown
## Summary

This PR installs the standard Greentic coding-agent indexing workflow for this repository.

It enables:

- repo-local analysis
- generated `AGENTS.md`, `CODEX.md`, `CLAUDE.md`, and `llms.txt`
- GHCR-published repo index artifacts
- participation in the Greentic org-wide coding-agent catalog

## Validation

- Generated workflow using `greentic-coding-agent install-github-workflow --publish-ghcr`
- No source code changes
```

## GitHub API abstraction

Do not hard-wire live GitHub calls into core logic. Add trait:

```rust
pub trait GitHubRepoClient {
    fn list_repos(&self, org: &str) -> Result<Vec<RepoMetadata>>;
    fn fetch_file(&self, repo_id: &str, path: &str, ref_name: &str) -> Result<Option<String>>;
    fn create_branch(&self, repo_id: &str, branch: &str, base: &str) -> Result<()>;
    fn upsert_file(&self, repo_id: &str, branch: &str, path: &str, content: &str, message: &str) -> Result<()>;
    fn open_pr(&self, repo_id: &str, branch: &str, base: &str, title: &str, body: &str) -> Result<String>;
}
```

CLI can initially call `gh` or leave live implementation behind a feature flag.

## Acceptance criteria

- Plan generation works with mocked repo metadata.
- Apply logic can be tested with a fake GitHub client.
- Workflow generation reuses existing `install-github-workflow` template logic.
- No PRs are opened during tests.

## Codex prompt

```text
Add org-wide indexing rollout support to greenticai/greentic-coding-agent.

Add concept `repository_index_rollout`. Add commands `org plan-index-rollout` and `org apply-index-rollout`. The plan command should discover repos from GitHub org, greenticai/.github, or repo-list file; determine whether `.github/workflows/greentic-agent-index.yml` is missing/current/outdated; and emit deterministic JSON.

The apply command should read a plan and, only when explicitly invoked, create branches, write the standard generated indexing workflow, and open PRs. Use a GitHubRepoClient trait so tests can use a fake client. Reuse existing install-github-workflow template logic.

Add tests for plan generation, skip/already-enabled/create-pr/update-existing-workflow actions, and fake apply behaviour. Add docs/org-index-rollout.md.

Do not perform live GitHub writes in tests. Run fmt, clippy, and tests.
```
