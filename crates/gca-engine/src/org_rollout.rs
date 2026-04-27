use gca_oci::render_github_workflow;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

pub const INDEX_ROLLOUT_PLAN_VERSION: &str = "v1";
pub const INDEX_WORKFLOW_PATH: &str = ".github/workflows/greentic-agent-index.yml";
pub const INDEX_ROLLOUT_PR_TITLE: &str = "Enable Greentic coding-agent index publishing";
pub const INDEX_ROLLOUT_COMMIT_MESSAGE: &str = "Install Greentic coding-agent index workflow";

pub const INDEX_ROLLOUT_PR_BODY: &str = r#"## Summary

This PR installs the standard Greentic coding-agent indexing workflow for this repository.

It enables:

- repo-local analysis
- generated `AGENTS.md`, `CODEX.md`, `CLAUDE.md`, and `llms.txt`
- GHCR-published repo index artifacts
- participation in the Greentic org-wide coding-agent catalog

## Validation

- Generated workflow using `greentic-coding-agent install-github-workflow --publish-ghcr`
- No source code changes
"#;

pub trait GitHubRepoClient {
    fn list_repos(&self, org: &str) -> Result<Vec<RepoMetadata>, String>;
    fn fetch_file(
        &self,
        repo_id: &str,
        path: &str,
        ref_name: &str,
    ) -> Result<Option<String>, String>;
    fn create_branch(&self, repo_id: &str, branch: &str, base: &str) -> Result<(), String>;
    fn upsert_file(
        &self,
        repo_id: &str,
        branch: &str,
        path: &str,
        content: &str,
        message: &str,
    ) -> Result<(), String>;
    fn open_pr(
        &self,
        repo_id: &str,
        branch: &str,
        base: &str,
        title: &str,
        body: &str,
    ) -> Result<String, String>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoMetadata {
    pub repo_id: String,
    pub default_branch: String,
    #[serde(default)]
    pub archived: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexRolloutPlan {
    pub version: String,
    pub org: String,
    pub generated_at: String,
    pub repos: Vec<IndexRolloutRepoAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexRolloutAction {
    Skip,
    AlreadyEnabled,
    CreatePr,
    UpdateExistingWorkflow,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyIndexRolloutReport {
    pub dry_run: bool,
    pub open_prs: bool,
    pub results: Vec<ApplyIndexRolloutResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyIndexRolloutResult {
    pub repo_id: String,
    pub action: IndexRolloutAction,
    pub status: ApplyIndexRolloutStatus,
    pub branch_name: Option<String>,
    pub pr_url: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyIndexRolloutStatus {
    Skipped,
    DryRun,
    BranchUpdated,
    PrOpened,
}

pub fn plan_index_rollout<C: GitHubRepoClient>(
    client: &C,
    org: &str,
    repos: Option<Vec<RepoMetadata>>,
    include_dot_github: bool,
    generated_at: Option<String>,
) -> Result<IndexRolloutPlan, String> {
    let repos = match repos {
        Some(repos) => repos,
        None => client.list_repos(org)?,
    };
    plan_index_rollout_for_repos(client, org, repos, include_dot_github, generated_at)
}

pub fn plan_index_rollout_for_repos<C: GitHubRepoClient>(
    client: &C,
    org: &str,
    mut repos: Vec<RepoMetadata>,
    include_dot_github: bool,
    generated_at: Option<String>,
) -> Result<IndexRolloutPlan, String> {
    repos.sort_by(|left, right| left.repo_id.cmp(&right.repo_id));
    repos.dedup_by(|left, right| left.repo_id == right.repo_id);

    let expected = render_github_workflow();
    let mut actions = Vec::with_capacity(repos.len());
    for repo in repos {
        actions.push(plan_repo_action(
            client,
            repo,
            include_dot_github,
            &expected,
        )?);
    }

    Ok(IndexRolloutPlan {
        version: INDEX_ROLLOUT_PLAN_VERSION.to_string(),
        org: org.to_string(),
        generated_at: generated_at.unwrap_or_else(current_timestamp),
        repos: actions,
    })
}

pub fn apply_index_rollout<C: GitHubRepoClient>(
    client: &C,
    plan: &IndexRolloutPlan,
    dry_run: bool,
    open_prs: bool,
) -> Result<ApplyIndexRolloutReport, String> {
    let workflow = render_github_workflow();
    let mut results = Vec::with_capacity(plan.repos.len());
    for repo in &plan.repos {
        match repo.action {
            IndexRolloutAction::Skip | IndexRolloutAction::AlreadyEnabled => {
                results.push(ApplyIndexRolloutResult {
                    repo_id: repo.repo_id.clone(),
                    action: repo.action,
                    status: ApplyIndexRolloutStatus::Skipped,
                    branch_name: repo.branch_name.clone(),
                    pr_url: None,
                    message: repo.reason.clone(),
                });
            }
            IndexRolloutAction::CreatePr | IndexRolloutAction::UpdateExistingWorkflow => {
                let Some(branch) = &repo.branch_name else {
                    return Err(format!(
                        "rollout action for `{}` is missing branch_name",
                        repo.repo_id
                    ));
                };
                if dry_run {
                    results.push(ApplyIndexRolloutResult {
                        repo_id: repo.repo_id.clone(),
                        action: repo.action,
                        status: ApplyIndexRolloutStatus::DryRun,
                        branch_name: Some(branch.clone()),
                        pr_url: None,
                        message: "would create branch, write workflow, and optionally open PR"
                            .to_string(),
                    });
                    continue;
                }

                client.create_branch(&repo.repo_id, branch, &repo.default_branch)?;
                client.upsert_file(
                    &repo.repo_id,
                    branch,
                    &repo.workflow_path,
                    &workflow,
                    INDEX_ROLLOUT_COMMIT_MESSAGE,
                )?;
                let pr_url = if open_prs {
                    Some(client.open_pr(
                        &repo.repo_id,
                        branch,
                        &repo.default_branch,
                        repo.pr_title.as_deref().unwrap_or(INDEX_ROLLOUT_PR_TITLE),
                        repo.pr_body.as_deref().unwrap_or(INDEX_ROLLOUT_PR_BODY),
                    )?)
                } else {
                    None
                };
                results.push(ApplyIndexRolloutResult {
                    repo_id: repo.repo_id.clone(),
                    action: repo.action,
                    status: if pr_url.is_some() {
                        ApplyIndexRolloutStatus::PrOpened
                    } else {
                        ApplyIndexRolloutStatus::BranchUpdated
                    },
                    branch_name: Some(branch.clone()),
                    pr_url,
                    message: if open_prs {
                        "opened rollout pull request".to_string()
                    } else {
                        "updated rollout branch without opening a pull request".to_string()
                    },
                });
            }
        }
    }

    Ok(ApplyIndexRolloutReport {
        dry_run,
        open_prs,
        results,
    })
}

fn plan_repo_action<C: GitHubRepoClient>(
    client: &C,
    repo: RepoMetadata,
    include_dot_github: bool,
    expected_workflow: &str,
) -> Result<IndexRolloutRepoAction, String> {
    if repo.archived {
        return Ok(base_action(
            repo,
            IndexRolloutAction::Skip,
            "repository is archived",
        ));
    }
    if repo.repo_id.ends_with("/.github") && !include_dot_github {
        return Ok(base_action(
            repo,
            IndexRolloutAction::Skip,
            ".github repository is skipped by default",
        ));
    }

    let current = client.fetch_file(&repo.repo_id, INDEX_WORKFLOW_PATH, &repo.default_branch)?;
    let (action, reason) = match current {
        None => (
            IndexRolloutAction::CreatePr,
            "index workflow is missing".to_string(),
        ),
        Some(current) if workflows_match(&current, expected_workflow) => (
            IndexRolloutAction::AlreadyEnabled,
            "index workflow is current".to_string(),
        ),
        Some(_) => (
            IndexRolloutAction::UpdateExistingWorkflow,
            "index workflow is present but differs from the current template".to_string(),
        ),
    };

    let mut action = base_action(repo, action, &reason);
    if matches!(
        action.action,
        IndexRolloutAction::CreatePr | IndexRolloutAction::UpdateExistingWorkflow
    ) {
        action.branch_name = Some(format!(
            "greentic-agent-index/{}",
            repo_name(&action.repo_id)
        ));
        action.pr_title = Some(INDEX_ROLLOUT_PR_TITLE.to_string());
        action.pr_body = Some(INDEX_ROLLOUT_PR_BODY.to_string());
    }
    Ok(action)
}

fn base_action(
    repo: RepoMetadata,
    action: IndexRolloutAction,
    reason: &str,
) -> IndexRolloutRepoAction {
    IndexRolloutRepoAction {
        repo_id: repo.repo_id,
        default_branch: repo.default_branch,
        archived: repo.archived,
        action,
        reason: reason.to_string(),
        workflow_path: INDEX_WORKFLOW_PATH.to_string(),
        branch_name: None,
        pr_title: None,
        pr_body: None,
    }
}

fn workflows_match(current: &str, expected: &str) -> bool {
    normalize_workflow(current) == normalize_workflow(expected)
}

fn normalize_workflow(raw: &str) -> String {
    raw.replace("\r\n", "\n").trim().to_string()
}

fn repo_name(repo_id: &str) -> &str {
    repo_id.rsplit('/').next().unwrap_or(repo_id)
}

fn current_timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    format!("unix:{seconds}")
}

pub fn validate_rollout_plan(plan: &IndexRolloutPlan) -> Result<(), String> {
    if plan.version != INDEX_ROLLOUT_PLAN_VERSION {
        return Err(format!(
            "unsupported rollout plan version `{}`",
            plan.version
        ));
    }
    let mut seen = BTreeSet::new();
    for repo in &plan.repos {
        if repo.repo_id.trim().is_empty() {
            return Err("rollout plan contains an empty repo_id".to_string());
        }
        if !seen.insert(repo.repo_id.clone()) {
            return Err(format!(
                "rollout plan contains duplicate repo `{}`",
                repo.repo_id
            ));
        }
        if matches!(
            repo.action,
            IndexRolloutAction::CreatePr | IndexRolloutAction::UpdateExistingWorkflow
        ) && repo.branch_name.is_none()
        {
            return Err(format!(
                "rollout plan action for `{}` requires branch_name",
                repo.repo_id
            ));
        }
    }
    Ok(())
}
