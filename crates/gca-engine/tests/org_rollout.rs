use gca_engine::{
    ApplyIndexRolloutStatus, GitHubRepoClient, INDEX_WORKFLOW_PATH, IndexRolloutAction,
    RepoMetadata, apply_index_rollout, plan_index_rollout_for_repos,
};
use gca_oci::render_github_workflow;
use std::cell::RefCell;
use std::collections::BTreeMap;

#[test]
fn rollout_plan_detects_skip_current_missing_and_outdated_repos() {
    let client = FakeGitHubClient::new()
        .with_file(
            "greenticai/current",
            INDEX_WORKFLOW_PATH,
            &render_github_workflow(),
        )
        .with_file("greenticai/outdated", INDEX_WORKFLOW_PATH, "name: old\n");
    let repos = vec![
        repo("greenticai/missing", "main", false),
        repo("greenticai/current", "main", false),
        repo("greenticai/outdated", "main", false),
        repo("greenticai/archived", "main", true),
        repo("greenticai/.github", "main", false),
    ];

    let plan = plan_index_rollout_for_repos(
        &client,
        "greenticai",
        repos,
        false,
        Some("unix:123".to_string()),
    )
    .unwrap();

    assert_eq!(plan.version, "v1");
    assert_eq!(plan.generated_at, "unix:123");
    assert_eq!(
        action_for(&plan, "greenticai/archived"),
        IndexRolloutAction::Skip
    );
    assert_eq!(
        action_for(&plan, "greenticai/.github"),
        IndexRolloutAction::Skip
    );
    assert_eq!(
        action_for(&plan, "greenticai/current"),
        IndexRolloutAction::AlreadyEnabled
    );
    assert_eq!(
        action_for(&plan, "greenticai/missing"),
        IndexRolloutAction::CreatePr
    );
    assert_eq!(
        action_for(&plan, "greenticai/outdated"),
        IndexRolloutAction::UpdateExistingWorkflow
    );
    let missing = plan
        .repos
        .iter()
        .find(|repo| repo.repo_id == "greenticai/missing")
        .unwrap();
    assert_eq!(
        missing.branch_name.as_deref(),
        Some("greentic-agent-index/missing")
    );
    assert_eq!(missing.workflow_path, INDEX_WORKFLOW_PATH);
}

#[test]
fn rollout_apply_uses_fake_client_and_respects_dry_run() {
    let client = FakeGitHubClient::new();
    let repos = vec![repo("greenticai/missing", "main", false)];
    let plan = plan_index_rollout_for_repos(
        &client,
        "greenticai",
        repos,
        false,
        Some("unix:123".to_string()),
    )
    .unwrap();

    let dry_run = apply_index_rollout(&client, &plan, true, true).unwrap();

    assert_eq!(dry_run.results[0].status, ApplyIndexRolloutStatus::DryRun);
    assert!(client.calls.borrow().is_empty());

    let applied = apply_index_rollout(&client, &plan, false, true).unwrap();

    assert_eq!(applied.results[0].status, ApplyIndexRolloutStatus::PrOpened);
    assert_eq!(
        applied.results[0].pr_url.as_deref(),
        Some("https://github.com/greenticai/missing/pull/1")
    );
    assert_eq!(
        client.calls.borrow().as_slice(),
        &[
            "create_branch greenticai/missing greentic-agent-index/missing main".to_string(),
            "upsert_file greenticai/missing greentic-agent-index/missing .github/workflows/greentic-agent-index.yml".to_string(),
            "open_pr greenticai/missing greentic-agent-index/missing main".to_string()
        ]
    );
}

fn action_for(plan: &gca_engine::IndexRolloutPlan, repo_id: &str) -> IndexRolloutAction {
    plan.repos
        .iter()
        .find(|repo| repo.repo_id == repo_id)
        .unwrap()
        .action
}

fn repo(repo_id: &str, default_branch: &str, archived: bool) -> RepoMetadata {
    RepoMetadata {
        repo_id: repo_id.to_string(),
        default_branch: default_branch.to_string(),
        archived,
    }
}

#[derive(Default)]
struct FakeGitHubClient {
    files: BTreeMap<(String, String), String>,
    calls: RefCell<Vec<String>>,
}

impl FakeGitHubClient {
    fn new() -> Self {
        Self::default()
    }

    fn with_file(mut self, repo_id: &str, path: &str, content: &str) -> Self {
        self.files
            .insert((repo_id.to_string(), path.to_string()), content.to_string());
        self
    }
}

impl GitHubRepoClient for FakeGitHubClient {
    fn list_repos(&self, _org: &str) -> Result<Vec<RepoMetadata>, String> {
        Ok(Vec::new())
    }

    fn fetch_file(
        &self,
        repo_id: &str,
        path: &str,
        _ref_name: &str,
    ) -> Result<Option<String>, String> {
        Ok(self
            .files
            .get(&(repo_id.to_string(), path.to_string()))
            .cloned())
    }

    fn create_branch(&self, repo_id: &str, branch: &str, base: &str) -> Result<(), String> {
        self.calls
            .borrow_mut()
            .push(format!("create_branch {repo_id} {branch} {base}"));
        Ok(())
    }

    fn upsert_file(
        &self,
        repo_id: &str,
        branch: &str,
        path: &str,
        _content: &str,
        _message: &str,
    ) -> Result<(), String> {
        self.calls
            .borrow_mut()
            .push(format!("upsert_file {repo_id} {branch} {path}"));
        Ok(())
    }

    fn open_pr(
        &self,
        repo_id: &str,
        branch: &str,
        base: &str,
        _title: &str,
        _body: &str,
    ) -> Result<String, String> {
        self.calls
            .borrow_mut()
            .push(format!("open_pr {repo_id} {branch} {base}"));
        Ok(format!("https://github.com/{repo_id}/pull/1"))
    }
}
