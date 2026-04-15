use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn help_lists_scaffolded_commands() {
    let mut command = Command::cargo_bin("greentic-coding-agent").unwrap();

    command.arg("--help");

    command.assert().success().stdout(
        predicate::str::contains("analyze")
            .and(predicate::str::contains("check-refresh"))
            .and(predicate::str::contains("commands"))
            .and(predicate::str::contains("concepts"))
            .and(predicate::str::contains("detect-changes"))
            .and(predicate::str::contains("describe"))
            .and(predicate::str::contains("generate-agent-files"))
            .and(predicate::str::contains("impact"))
            .and(predicate::str::contains("install-github-workflow"))
            .and(predicate::str::contains("list-remote-repos"))
            .and(predicate::str::contains("locate-owner"))
            .and(predicate::str::contains("package-index"))
            .and(predicate::str::contains("publish-index"))
            .and(predicate::str::contains("required-validations"))
            .and(predicate::str::contains("search"))
            .and(predicate::str::contains("serve"))
            .and(predicate::str::contains("show-catalog"))
            .and(predicate::str::contains("sync"))
            .and(predicate::str::contains("validate-plan"))
            .and(predicate::str::contains("workflows")),
    );
}

#[test]
fn describe_here_json_returns_minimal_repo_metadata() {
    let mut command = Command::cargo_bin("greentic-coding-agent").unwrap();

    command.args(["describe", "--here", "--format", "json"]);

    command.assert().success().stdout(
        predicate::str::contains("\"repo_name\": \"greentic-coding-agent\"")
            .and(predicate::str::contains("\"version\": \"0.1.0\""))
            .and(predicate::str::contains("\"has_git_dir\": true")),
    );
}

#[test]
fn analyze_creates_local_outputs_and_updates_registry() {
    let temp_root = unique_temp_dir("gca-cli-analyze");
    let repo_root = temp_root.join("demo-repo");
    let fake_home = temp_root.join("home");
    create_demo_repo(&repo_root);
    fs::create_dir_all(&fake_home).unwrap();

    let mut command = Command::cargo_bin("greentic-coding-agent").unwrap();
    command
        .current_dir(&repo_root)
        .env("HOME", &fake_home)
        .args(["analyze", "--print", "--format", "json"]);

    command
        .assert()
        .success()
        .stdout(predicate::str::contains("\"repo_name\": \"demo-repo\""));

    assert!(
        repo_root
            .join(".greentic-agent")
            .join("manifest.json")
            .exists()
    );
    assert!(
        repo_root
            .join(".greentic-agent")
            .join("repo-index.json")
            .exists()
    );
    assert!(
        repo_root
            .join(".greentic-agent")
            .join("fingerprints.json")
            .exists()
    );

    let registry =
        fs::read_to_string(fake_home.join(".greentic-agent").join("registry.json")).unwrap();
    assert!(registry.contains("\"repo_name\": \"demo-repo\""));
}

#[test]
fn repeated_analyze_updates_same_registry_entry() {
    let temp_root = unique_temp_dir("gca-cli-analyze-repeat");
    let repo_root = temp_root.join("demo-repo");
    let fake_home = temp_root.join("home");
    create_demo_repo(&repo_root);
    fs::create_dir_all(&fake_home).unwrap();

    for _ in 0..2 {
        let mut command = Command::cargo_bin("greentic-coding-agent").unwrap();
        command
            .current_dir(&repo_root)
            .env("HOME", &fake_home)
            .arg("analyze");
        command.assert().success();
    }

    let registry =
        fs::read_to_string(fake_home.join(".greentic-agent").join("registry.json")).unwrap();

    let occurrences = registry.matches("\"repo_name\": \"demo-repo\"").count();
    assert_eq!(occurrences, 1);
}

#[test]
fn concepts_command_reports_inferred_concepts() {
    let temp_root = unique_temp_dir("gca-cli-concepts");
    let repo_root = temp_root.join("demo-repo");
    let fake_home = temp_root.join("home");
    create_demo_repo(&repo_root);
    fs::create_dir_all(&fake_home).unwrap();

    let mut command = Command::cargo_bin("greentic-coding-agent").unwrap();
    command
        .current_dir(&repo_root)
        .env("HOME", &fake_home)
        .args(["concepts", "--format", "json"]);

    command.assert().success().stdout(
        predicate::str::contains("\"id\": \"digital_worker\"")
            .and(predicate::str::contains("\"id\": \"greentic_x\""))
            .and(predicate::str::contains("\"id\": \"wizard\"")),
    );
}

#[test]
fn workflows_command_reports_gtc_flows() {
    let temp_root = unique_temp_dir("gca-cli-workflows");
    let repo_root = temp_root.join("demo-repo");
    let fake_home = temp_root.join("home");
    create_demo_repo(&repo_root);
    fs::create_dir_all(&fake_home).unwrap();

    let mut command = Command::cargo_bin("greentic-coding-agent").unwrap();
    command
        .current_dir(&repo_root)
        .env("HOME", &fake_home)
        .args(["workflows", "--format", "json"]);

    command.assert().success().stdout(
        predicate::str::contains("\"id\": \"wizard_bootstrap\"")
            .and(predicate::str::contains("\"id\": \"setup_bundle\""))
            .and(predicate::str::contains("\"id\": \"start_bundle\"")),
    );
}

#[test]
fn commands_command_lists_catalog_entries() {
    let mut command = Command::cargo_bin("greentic-coding-agent").unwrap();

    command.args(["commands", "--format", "json"]);

    command.assert().success().stdout(
        predicate::str::contains("\"command\": \"greentic-coding-agent analyze\"").and(
            predicate::str::contains(
                "\"command\": \"greentic-coding-agent search --mode <mode> <query>\"",
            ),
        ),
    );
}

#[test]
fn search_instruction_mode_returns_structured_results() {
    let temp_root = unique_temp_dir("gca-cli-search-instruction");
    let repo_root = temp_root.join("demo-repo");
    let fake_home = temp_root.join("home");
    create_demo_repo(&repo_root);
    fs::create_dir_all(&fake_home).unwrap();

    let mut command = Command::cargo_bin("greentic-coding-agent").unwrap();
    command
        .current_dir(&repo_root)
        .env("HOME", &fake_home)
        .args([
            "search",
            "--mode",
            "instruction",
            "wizard",
            "--format",
            "json",
        ]);

    command.assert().success().stdout(
        predicate::str::contains("\"mode\": \"instruction\"")
            .and(predicate::str::contains("\"result_type\": \"instruction\""))
            .and(predicate::str::contains("\"locator\": \"README.md\"")),
    );
}

#[test]
fn search_code_mode_returns_empty_results_explicitly() {
    let temp_root = unique_temp_dir("gca-cli-search-empty");
    let repo_root = temp_root.join("demo-repo");
    let fake_home = temp_root.join("home");
    create_demo_repo(&repo_root);
    fs::create_dir_all(&fake_home).unwrap();

    let mut command = Command::cargo_bin("greentic-coding-agent").unwrap();
    command
        .current_dir(&repo_root)
        .env("HOME", &fake_home)
        .args([
            "search",
            "--mode",
            "code",
            "no-such-symbol",
            "--format",
            "json",
        ]);

    command.assert().success().stdout(
        predicate::str::contains("\"mode\": \"code\"")
            .and(predicate::str::contains("\"query\": \"no-such-symbol\""))
            .and(predicate::str::contains("\"results\": []")),
    );
}

#[test]
fn locate_owner_returns_seeded_policy() {
    let temp_root = unique_temp_dir("gca-cli-locate-owner");
    let repo_root = temp_root.join("demo-repo");
    let fake_home = temp_root.join("home");
    create_demo_repo(&repo_root);
    fs::create_dir_all(&fake_home).unwrap();

    let mut command = Command::cargo_bin("greentic-coding-agent").unwrap();
    command
        .current_dir(&repo_root)
        .env("HOME", &fake_home)
        .args([
            "locate-owner",
            "--concept",
            "extension_pack",
            "--format",
            "json",
        ]);

    command.assert().success().stdout(
        predicate::str::contains("\"owner_repo\": \"greentic-pack\"").and(
            predicate::str::contains("\"concept_id\": \"extension_pack\""),
        ),
    );
}

#[test]
fn required_validations_matches_task_keywords() {
    let temp_root = unique_temp_dir("gca-cli-required-validations");
    let repo_root = temp_root.join("demo-repo");
    let fake_home = temp_root.join("home");
    create_demo_repo(&repo_root);
    fs::create_dir_all(&fake_home).unwrap();

    let mut command = Command::cargo_bin("greentic-coding-agent").unwrap();
    command
        .current_dir(&repo_root)
        .env("HOME", &fake_home)
        .args([
            "required-validations",
            "--task",
            "modify setup schema",
            "--format",
            "json",
        ]);

    command.assert().success().stdout(
        predicate::str::contains("\"task\": \"modify setup schema\"").and(
            predicate::str::contains("\"id\": \"setup_runtime_schema_change\""),
        ),
    );
}

#[test]
fn search_reuse_mode_returns_policy_results() {
    let temp_root = unique_temp_dir("gca-cli-search-reuse");
    let repo_root = temp_root.join("demo-repo");
    let fake_home = temp_root.join("home");
    create_demo_repo(&repo_root);
    fs::create_dir_all(&fake_home).unwrap();

    let mut command = Command::cargo_bin("greentic-coding-agent").unwrap();
    command
        .current_dir(&repo_root)
        .env("HOME", &fake_home)
        .args(["search", "--mode", "reuse", "pack", "--format", "json"]);

    command.assert().success().stdout(
        predicate::str::contains("\"mode\": \"reuse\"")
            .and(predicate::str::contains("\"result_type\": \"reuse\""))
            .and(predicate::str::contains("\"locator\": \"extension_pack\"")),
    );
}

#[test]
fn generate_agent_files_writes_generated_outputs() {
    let temp_root = unique_temp_dir("gca-cli-generate-agent-files");
    let repo_root = temp_root.join("demo-repo");
    let fake_home = temp_root.join("home");
    create_demo_repo(&repo_root);
    fs::create_dir_all(&fake_home).unwrap();

    let mut command = Command::cargo_bin("greentic-coding-agent").unwrap();
    command
        .current_dir(&repo_root)
        .env("HOME", &fake_home)
        .args(["generate-agent-files", "--format", "json"]);

    command.assert().success().stdout(
        predicate::str::contains(".greentic-agent/generated/AGENTS.md").and(
            predicate::str::contains(".greentic-agent/generated/CODEX.md"),
        ),
    );

    assert!(
        repo_root
            .join(".greentic-agent/generated/AGENTS.md")
            .exists()
    );
    assert!(
        repo_root
            .join(".greentic-agent/generated/CLAUDE.md")
            .exists()
    );
    assert!(
        repo_root
            .join(".greentic-agent/generated/CODEX.md")
            .exists()
    );
    assert!(
        repo_root
            .join(".greentic-agent/generated/llms.txt")
            .exists()
    );
}

#[test]
fn package_publish_list_and_sync_index_workflow() {
    let temp_root = unique_temp_dir("gca-cli-oci-workflow");
    let repo_root = temp_root.join("demo-repo");
    let fake_home = temp_root.join("home");
    create_demo_repo(&repo_root);
    fs::create_dir_all(&fake_home).unwrap();

    let mut package = Command::cargo_bin("greentic-coding-agent").unwrap();
    package
        .current_dir(&repo_root)
        .env("HOME", &fake_home)
        .args(["package-index", "--tag", "vtest", "--format", "json"]);
    package.assert().success().stdout(
        predicate::str::contains(".greentic-agent/oci/demo-repo/vtest").and(
            predicate::str::contains("ghcr.io/greenticai/indexes/demo-repo:vtest"),
        ),
    );
    assert!(
        repo_root
            .join(".greentic-agent/oci/demo-repo/vtest/oci-layout")
            .exists()
    );

    let mut publish = Command::cargo_bin("greentic-coding-agent").unwrap();
    publish
        .current_dir(&repo_root)
        .env("HOME", &fake_home)
        .args(["publish-index", "--tag", "vtest", "--format", "json"]);
    publish.assert().success().stdout(predicate::str::contains(
        ".greentic-agent/remote-oci/demo-repo/vtest",
    ));

    let mut list = Command::cargo_bin("greentic-coding-agent").unwrap();
    list.current_dir(&repo_root).env("HOME", &fake_home).args([
        "list-remote-repos",
        "--format",
        "json",
    ]);
    list.assert().success().stdout(
        predicate::str::contains("\"repo_name\": \"demo-repo\"")
            .and(predicate::str::contains("\"vtest\"")),
    );

    let mut sync = Command::cargo_bin("greentic-coding-agent").unwrap();
    sync.current_dir(&repo_root).env("HOME", &fake_home).args([
        "sync",
        "--repo",
        "demo-repo",
        "--tag",
        "vtest",
        "--format",
        "json",
    ]);
    sync.assert().success().stdout(predicate::str::contains(
        ".greentic-agent/cache-oci/demo-repo/vtest",
    ));
    assert!(
        fake_home
            .join(".greentic-agent/cache-oci/demo-repo/vtest/artifacts/repo-index.json")
            .exists()
    );
}

#[test]
fn show_catalog_and_sync_without_repo_use_discovery_catalog() {
    let temp_root = unique_temp_dir("gca-cli-catalog-sync");
    let fake_home = temp_root.join("home");
    fs::create_dir_all(&fake_home).unwrap();

    let alpha_repo = temp_root.join("alpha-repo");
    create_demo_repo(&alpha_repo);
    let beta_repo = temp_root.join("beta-repo");
    create_demo_repo(&beta_repo);

    for repo_root in [&alpha_repo, &beta_repo] {
        let mut publish = Command::cargo_bin("greentic-coding-agent").unwrap();
        publish
            .current_dir(repo_root)
            .env("HOME", &fake_home)
            .args(["publish-index", "--tag", "latest", "--format", "json"]);
        publish.assert().success();
    }

    let mut catalog = Command::cargo_bin("greentic-coding-agent").unwrap();
    catalog
        .current_dir(&alpha_repo)
        .env("HOME", &fake_home)
        .args(["show-catalog", "--format", "json"]);
    catalog.assert().success().stdout(
        predicate::str::contains("\"repo_name\": \"alpha-repo\"")
            .and(predicate::str::contains("\"repo_name\": \"beta-repo\"")),
    );

    let mut sync = Command::cargo_bin("greentic-coding-agent").unwrap();
    sync.current_dir(&alpha_repo)
        .env("HOME", &fake_home)
        .args(["sync", "--format", "json"]);
    sync.assert().success().stdout(
        predicate::str::contains(".greentic-agent/cache-oci/alpha-repo/latest").and(
            predicate::str::contains(".greentic-agent/cache-oci/beta-repo/latest"),
        ),
    );
}

#[test]
fn check_refresh_reports_explicit_reasons_after_repo_changes() {
    let temp_root = unique_temp_dir("gca-cli-check-refresh");
    let repo_root = temp_root.join("demo-repo");
    let fake_home = temp_root.join("home");
    create_demo_repo(&repo_root);
    fs::create_dir_all(&fake_home).unwrap();

    let mut analyze = Command::cargo_bin("greentic-coding-agent").unwrap();
    analyze
        .current_dir(&repo_root)
        .env("HOME", &fake_home)
        .arg("analyze");
    analyze.assert().success();

    fs::write(
        repo_root
            .join(".git")
            .join("refs")
            .join("heads")
            .join("main"),
        "def456\n",
    )
    .unwrap();
    fs::write(repo_root.join("docs").join("new-guide.md"), "# New Guide\n").unwrap();

    let mut check = Command::cargo_bin("greentic-coding-agent").unwrap();
    check.current_dir(&repo_root).env("HOME", &fake_home).args([
        "check-refresh",
        "--format",
        "json",
    ]);
    check.assert().success().stdout(
        predicate::str::contains("\"needs_refresh\": true")
            .and(predicate::str::contains("source commit changed"))
            .and(predicate::str::contains("indexed file fingerprint changed")),
    );
}

#[test]
fn install_github_workflow_writes_expected_file() {
    let temp_root = unique_temp_dir("gca-cli-install-workflow");
    let repo_root = temp_root.join("demo-repo");
    let fake_home = temp_root.join("home");
    create_demo_repo(&repo_root);
    fs::create_dir_all(&fake_home).unwrap();

    let mut install = Command::cargo_bin("greentic-coding-agent").unwrap();
    install
        .current_dir(&repo_root)
        .env("HOME", &fake_home)
        .args(["install-github-workflow", "--format", "json"]);
    install.assert().success().stdout(predicate::str::contains(
        ".github/workflows/greentic-agent-index.yml",
    ));

    let workflow = fs::read_to_string(
        repo_root
            .join(".github")
            .join("workflows")
            .join("greentic-agent-index.yml"),
    )
    .unwrap();
    assert!(workflow.contains("check-refresh"));
    assert!(workflow.contains("publish-index"));
    assert!(workflow.contains("packages: write"));
}

#[test]
fn impact_reports_blast_radius_with_freshness_warning() {
    let temp_root = unique_temp_dir("gca-cli-impact");
    let repo_root = temp_root.join("demo-repo");
    let fake_home = temp_root.join("home");
    create_demo_repo(&repo_root);
    fs::create_dir_all(&fake_home).unwrap();

    let mut analyze = Command::cargo_bin("greentic-coding-agent").unwrap();
    analyze
        .current_dir(&repo_root)
        .env("HOME", &fake_home)
        .arg("analyze");
    analyze.assert().success();

    fs::write(
        repo_root
            .join(".git")
            .join("refs")
            .join("heads")
            .join("main"),
        "def456\n",
    )
    .unwrap();

    let mut impact = Command::cargo_bin("greentic-coding-agent").unwrap();
    impact
        .current_dir(&repo_root)
        .env("HOME", &fake_home)
        .args(["impact", "--symbol", "wizard", "--format", "json"]);
    impact.assert().success().stdout(
        predicate::str::contains("\"symbol\": \"wizard\"")
            .and(predicate::str::contains("\"confidence\": \"high\""))
            .and(predicate::str::contains("\"concepts\": ["))
            .and(predicate::str::contains(
                "\"freshness_warning\": \"index may be stale",
            )),
    );
}

#[test]
fn detect_changes_reports_changed_files_and_validations() {
    let temp_root = unique_temp_dir("gca-cli-detect-changes");
    let repo_root = temp_root.join("demo-repo");
    let fake_home = temp_root.join("home");
    create_demo_repo(&repo_root);
    fs::create_dir_all(&fake_home).unwrap();

    let mut analyze = Command::cargo_bin("greentic-coding-agent").unwrap();
    analyze
        .current_dir(&repo_root)
        .env("HOME", &fake_home)
        .arg("analyze");
    analyze.assert().success();

    fs::write(repo_root.join("docs").join("setup-notes.md"), "# setup\n").unwrap();

    let mut detect = Command::cargo_bin("greentic-coding-agent").unwrap();
    detect
        .current_dir(&repo_root)
        .env("HOME", &fake_home)
        .args(["detect-changes", "--format", "json"]);
    detect.assert().success().stdout(
        predicate::str::contains("\"changed_files\": [")
            .and(predicate::str::contains("docs/setup-notes.md"))
            .and(predicate::str::contains("\"suggested_validations\": [")),
    );
}

#[test]
fn validate_plan_reports_owner_hints_and_validations() {
    let temp_root = unique_temp_dir("gca-cli-validate-plan");
    let repo_root = temp_root.join("demo-repo");
    let fake_home = temp_root.join("home");
    create_demo_repo(&repo_root);
    fs::create_dir_all(&fake_home).unwrap();

    let plan_path = repo_root.join("plan.json");
    fs::write(
        &plan_path,
        r#"{"summary":"Update wizard setup schema and bundle start flow"}"#,
    )
    .unwrap();

    let mut analyze = Command::cargo_bin("greentic-coding-agent").unwrap();
    analyze
        .current_dir(&repo_root)
        .env("HOME", &fake_home)
        .arg("analyze");
    analyze.assert().success();

    let mut validate = Command::cargo_bin("greentic-coding-agent").unwrap();
    validate
        .current_dir(&repo_root)
        .env("HOME", &fake_home)
        .args([
            "validate-plan",
            plan_path.to_str().unwrap(),
            "--format",
            "json",
        ]);
    validate.assert().success().stdout(
        predicate::str::contains("\"task_summary\":")
            .and(predicate::str::contains("\"owner_repo\":"))
            .and(predicate::str::contains("\"required_validations\": [")),
    );
}

#[test]
fn serve_outputs_mcp_tool_surface() {
    let temp_root = unique_temp_dir("gca-cli-serve");
    let repo_root = temp_root.join("demo-repo");
    let fake_home = temp_root.join("home");
    create_demo_repo(&repo_root);
    fs::create_dir_all(&fake_home).unwrap();

    let mut analyze = Command::cargo_bin("greentic-coding-agent").unwrap();
    analyze
        .current_dir(&repo_root)
        .env("HOME", &fake_home)
        .arg("analyze");
    analyze.assert().success();

    let mut serve = Command::cargo_bin("greentic-coding-agent").unwrap();
    serve
        .current_dir(&repo_root)
        .env("HOME", &fake_home)
        .args(["serve", "--format", "json"]);
    serve.assert().success().stdout(
        predicate::str::contains("\"protocol\": \"mcp-lite-v1\"")
            .and(predicate::str::contains("\"name\": \"impact_analysis\""))
            .and(predicate::str::contains("\"name\": \"detect_changes\"")),
    );
}

#[test]
fn serve_request_file_dispatches_tool_calls() {
    let temp_root = unique_temp_dir("gca-cli-serve-request");
    let repo_root = temp_root.join("demo-repo");
    let fake_home = temp_root.join("home");
    create_demo_repo(&repo_root);
    fs::create_dir_all(&fake_home).unwrap();

    let mut analyze = Command::cargo_bin("greentic-coding-agent").unwrap();
    analyze
        .current_dir(&repo_root)
        .env("HOME", &fake_home)
        .arg("analyze");
    analyze.assert().success();

    let request_path = repo_root.join("describe-request.json");
    fs::write(
        &request_path,
        r#"{"id":"req-1","tool":"describe_repo","arguments":{}}"#,
    )
    .unwrap();

    let mut serve = Command::cargo_bin("greentic-coding-agent").unwrap();
    serve.current_dir(&repo_root).env("HOME", &fake_home).args([
        "serve",
        "--request-file",
        request_path.to_str().unwrap(),
        "--format",
        "json",
    ]);
    serve.assert().success().stdout(
        predicate::str::contains("\"id\": \"req-1\"")
            .and(predicate::str::contains("\"ok\": true"))
            .and(predicate::str::contains("\"repo_name\": \"demo-repo\"")),
    );
}

#[test]
fn serve_request_file_detect_changes_reports_real_areas() {
    let temp_root = unique_temp_dir("gca-cli-serve-detect-changes");
    let repo_root = temp_root.join("demo-repo");
    let fake_home = temp_root.join("home");
    create_demo_repo(&repo_root);
    fs::create_dir_all(&fake_home).unwrap();

    let mut analyze = Command::cargo_bin("greentic-coding-agent").unwrap();
    analyze
        .current_dir(&repo_root)
        .env("HOME", &fake_home)
        .arg("analyze");
    analyze.assert().success();

    let request_path = repo_root.join("detect-request.json");
    fs::write(
        &request_path,
        r#"{"id":"req-2","tool":"detect_changes","arguments":{"changed_files":["README.md",".github/workflows/perf.yml"]}}"#,
    )
    .unwrap();

    let mut serve = Command::cargo_bin("greentic-coding-agent").unwrap();
    serve.current_dir(&repo_root).env("HOME", &fake_home).args([
        "serve",
        "--request-file",
        request_path.to_str().unwrap(),
        "--format",
        "json",
    ]);
    serve.assert().success().stdout(
        predicate::str::contains("\"id\": \"req-2\"")
            .and(predicate::str::contains("\"ok\": true"))
            .and(predicate::str::contains("\"likely_concepts\": ["))
            .and(predicate::str::contains("\"wizard\""))
            .and(predicate::str::contains("\"likely_workflows\": ["))
            .and(predicate::str::contains("\"wizard_bootstrap\"")),
    );
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("{prefix}-{nanos}"));
    fs::create_dir_all(&path).unwrap();
    path
}

fn create_demo_repo(repo_root: &Path) {
    fs::create_dir_all(repo_root.join(".git").join("refs").join("heads")).unwrap();
    fs::create_dir_all(repo_root.join("docs")).unwrap();
    fs::create_dir_all(repo_root.join(".codex")).unwrap();
    fs::create_dir_all(repo_root.join(".github").join("workflows")).unwrap();
    fs::create_dir_all(repo_root.join("src")).unwrap();
    fs::create_dir_all(repo_root.join("examples")).unwrap();
    fs::write(
        repo_root.join("Cargo.toml"),
        "[workspace]\nmembers = [\n  \"crates/demo\"\n]\n\n[package]\nname = \"demo\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\n\n[features]\ndefault = []\ncli = []\n",
    )
    .unwrap();
    fs::write(
        repo_root.join("README.md"),
        "# Demo\n\nUse `gtc wizard --schema demo.json` and `gtc start demo-bundle`.\n",
    )
    .unwrap();
    fs::write(
        repo_root.join("docs").join("architecture.md"),
        "# Architecture\n\nGreentic-X digital worker setup guidance.\n",
    )
    .unwrap();
    fs::write(
        repo_root.join(".codex").join("PR-04.md"),
        "# PR-04\n\nRun `gtc setup demo --answers answers.json`.\n",
    )
    .unwrap();
    fs::write(
        repo_root.join(".github").join("workflows").join("perf.yml"),
        "name: Perf\nsteps:\n  - run: gtc wizard --answers answers.json\n",
    )
    .unwrap();
    fs::write(
        repo_root.join("src").join("lib.rs"),
        "pub fn example_hot_path() {}\n\n#[test]\nfn smoke() {}\n",
    )
    .unwrap();
    fs::write(
        repo_root.join("examples").join("demo.md"),
        "# Example\n\nGreentic-sorla walkthrough.\n",
    )
    .unwrap();
    fs::write(
        repo_root.join(".git").join("HEAD"),
        "ref: refs/heads/main\n",
    )
    .unwrap();
    fs::write(
        repo_root
            .join(".git")
            .join("refs")
            .join("heads")
            .join("main"),
        "abc123\n",
    )
    .unwrap();
}
