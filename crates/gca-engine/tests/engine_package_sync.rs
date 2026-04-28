use gca_engine::{
    AnalyzeOptions, CheckRefreshOptions, CodingAgentService, DispatchMcpRequestOptions,
    GenerateAgentFilesOptions, InstallGithubWorkflowOptions, ListRemoteReposOptions,
    McpSnapshotOptions, PackageIndexOptions, PublishIndexOptions, RebuildMergedIndexOptions,
    ShowCatalogOptions, SyncOptions,
};
use std::fs;
use tempfile::TempDir;

#[test]
fn engine_generates_files_packages_index_and_checks_refresh() {
    let repo = synthetic_repo();
    let home = TempDir::new().unwrap();
    let service = CodingAgentService::new(repo.path().to_path_buf(), home.path().to_path_buf());
    service.analyze(AnalyzeOptions).unwrap();

    let generated = service
        .generate_agent_files(GenerateAgentFilesOptions { write_root: false })
        .unwrap();
    let package = service
        .package_index(PackageIndexOptions {
            tags: vec!["test".to_string(), "sha-abc123".to_string()],
        })
        .unwrap();
    let refresh = service.check_refresh(CheckRefreshOptions).unwrap();

    assert_eq!(generated.generated_files.len(), 4);
    assert!(generated.written_paths.iter().all(|path| path.exists()));
    assert!(package.package.package_dir.exists());
    assert_eq!(package.packages.len(), 2);
    assert!(
        refresh
            .reasons
            .iter()
            .any(|reason| reason.contains("generator version changed"))
    );
}

#[test]
fn engine_syncs_package_and_rebuilds_merged_index() {
    let repo = synthetic_repo();
    let home = TempDir::new().unwrap();
    let service = CodingAgentService::new(repo.path().to_path_buf(), home.path().to_path_buf());
    let analyzed = service.analyze(AnalyzeOptions).unwrap();
    let repo_id = analyzed.repo_index.repo_id;
    service
        .package_index(PackageIndexOptions {
            tags: vec!["latest".to_string()],
        })
        .unwrap();
    service
        .publish_index(PublishIndexOptions {
            tags: vec!["latest".to_string()],
            remote_root: None,
        })
        .unwrap();
    let sync = service
        .sync(SyncOptions {
            repo_id: None,
            tag: None,
            channel: None,
            remote_root: None,
        })
        .unwrap();
    let second_sync = service
        .sync(SyncOptions {
            repo_id: None,
            tag: None,
            channel: None,
            remote_root: None,
        })
        .unwrap();
    let remote_repos = service
        .list_remote_repos(ListRemoteReposOptions { remote_root: None })
        .unwrap();
    let catalog = service
        .show_catalog(ShowCatalogOptions { remote_root: None })
        .unwrap();
    let merged = service
        .rebuild_merged_index(RebuildMergedIndexOptions { tenant: None })
        .unwrap();

    assert!(!sync.synced_paths.is_empty());
    assert_eq!(sync.report.downloaded.len(), 1);
    assert!(sync.report.failed.is_empty());
    assert!(second_sync.synced_paths.is_empty());
    assert_eq!(second_sync.report.skipped, vec![repo_id.clone()]);
    assert!(
        home.path()
            .join(".greentic-agent")
            .join("remote-oci")
            .exists()
    );
    assert!(
        home.path()
            .join(".greentic-agent")
            .join("cache-oci")
            .exists()
    );
    assert!(
        home.path()
            .join(".greentic-agent")
            .join("sync-state.json")
            .exists()
    );
    assert!(
        home.path()
            .join(".greentic-agent")
            .join("indexes")
            .join("public")
            .join(&repo_id)
            .join("latest")
            .join("repo-index.json")
            .exists()
    );
    assert!(
        home.path()
            .join(".greentic-agent")
            .join("indexes")
            .join("public")
            .join(&repo_id)
            .join("latest")
            .join("tantivy")
            .exists()
    );
    assert_eq!(remote_repos.repos.len(), 1);
    assert_eq!(catalog.catalog.repos.len(), 1);
    assert_eq!(merged.repos_indexed, 1);
    assert!(merged.documents_indexed > 0);
    assert!(merged.index_path.exists());
}

#[test]
fn engine_installs_workflow_and_dispatches_mcp_requests() {
    let repo = synthetic_repo();
    let home = TempDir::new().unwrap();
    let service = CodingAgentService::new(repo.path().to_path_buf(), home.path().to_path_buf());
    service.analyze(AnalyzeOptions).unwrap();

    let workflow = service
        .install_github_workflow(InstallGithubWorkflowOptions)
        .unwrap();
    let snapshot = service.mcp_snapshot(McpSnapshotOptions).unwrap();
    let response = service
        .dispatch_mcp_request(DispatchMcpRequestOptions {
            request: gca_mcp::McpRequest {
                id: Some("1".to_string()),
                tool: "describe_repo".to_string(),
                arguments: serde_json::json!({}),
            },
        })
        .unwrap();

    assert!(workflow.workflow_path.exists());
    assert!(
        snapshot
            .tools
            .iter()
            .any(|tool| tool.name == "describe_repo")
    );
    assert!(response.ok);
    assert!(response.result.is_some());
}

fn synthetic_repo() -> TempDir {
    let repo = TempDir::new().unwrap();
    fs::create_dir(repo.path().join(".git")).unwrap();
    fs::create_dir_all(repo.path().join("src")).unwrap();
    fs::write(
        repo.path().join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(repo.path().join("src/lib.rs"), "pub fn package_demo() {}\n").unwrap();
    repo
}
