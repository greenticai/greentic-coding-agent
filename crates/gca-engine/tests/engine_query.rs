use gca_engine::{
    AnalyzeOptions, CodingAgentService, CommandsOptions, DetectChangesOptions, ImpactOptions,
    LocateOwnerOptions, MarkKnowledgeUpdateSeenOptions, RequiredValidationsOptions, SearchOptions,
    UpdatesOptions, ValidatePlanOptions,
};
use gca_query::{SearchEngineChoice, SearchMode};
use std::fs;
use tempfile::TempDir;

#[test]
fn engine_search_and_policy_queries_use_shared_crates() {
    let repo = synthetic_repo();
    let home = TempDir::new().unwrap();
    let service = CodingAgentService::new(repo.path().to_path_buf(), home.path().to_path_buf());
    service.analyze(AnalyzeOptions).unwrap();

    let search = service
        .search(SearchOptions {
            mode: SearchMode::Instruction,
            query: "wizard".to_string(),
            engine: SearchEngineChoice::Fallback,
        })
        .unwrap();
    let owner = service
        .locate_owner(LocateOwnerOptions {
            concept_id: "wizard".to_string(),
        })
        .unwrap();
    let validations = service
        .required_validations(RequiredValidationsOptions {
            task: "change component qa schema".to_string(),
        })
        .unwrap();

    assert!(!search.results.is_empty());
    assert_eq!(owner.owner_repo, "greentic-dev");
    assert!(
        validations
            .validations
            .iter()
            .any(|validation| validation.id == "component_qa_schema_change")
    );
}

#[test]
fn engine_lists_command_catalog() {
    let repo = synthetic_repo();
    let home = TempDir::new().unwrap();
    let service = CodingAgentService::new(repo.path().to_path_buf(), home.path().to_path_buf());

    let commands = service.commands(CommandsOptions).unwrap();

    assert!(
        commands
            .commands
            .iter()
            .any(|entry| entry.command == "greentic-coding-agent analyze")
    );
}

#[test]
fn engine_impact_detect_changes_and_validate_plan_are_structured() {
    let repo = synthetic_repo();
    let home = TempDir::new().unwrap();
    let service = CodingAgentService::new(repo.path().to_path_buf(), home.path().to_path_buf());
    service.analyze(AnalyzeOptions).unwrap();
    let plan_path = repo.path().join("plan.json");
    fs::write(
        &plan_path,
        r#"{"summary":"Change component QA schema and wizard setup flow"}"#,
    )
    .unwrap();

    let impact = service
        .impact(ImpactOptions {
            symbol: "component".to_string(),
        })
        .unwrap();
    let changes = service
        .detect_changes(DetectChangesOptions {
            changed_files: vec!["component/qa/schema.json".to_string()],
        })
        .unwrap();
    let plan = service
        .validate_plan(ValidatePlanOptions { plan_path })
        .unwrap();

    assert!(impact.concepts.iter().any(|concept| concept == "component"));
    assert!(
        changes
            .likely_concepts
            .iter()
            .any(|concept| concept == "component")
    );
    assert!(
        plan.owner_hints
            .iter()
            .any(|owner| owner.concept_id == "component")
    );
    assert!(
        plan.validations
            .iter()
            .any(|validation| validation.id == "component_qa_schema_change")
    );
}

#[test]
fn engine_tracks_unseen_updates_and_detects_digest_changes() {
    let repo = synthetic_repo();
    write_update(&repo, "Use answers flow.");
    let home = TempDir::new().unwrap();
    let service = CodingAgentService::new(repo.path().to_path_buf(), home.path().to_path_buf());
    service.analyze(AnalyzeOptions).unwrap();

    let first = service
        .updates(UpdatesOptions {
            new_only: true,
            ..UpdatesOptions::default()
        })
        .unwrap();
    let second_listing_does_not_mark_seen = service
        .updates(UpdatesOptions {
            new_only: true,
            ..UpdatesOptions::default()
        })
        .unwrap();

    assert_eq!(first.updates.len(), 1);
    assert_eq!(second_listing_does_not_mark_seen.updates.len(), 1);

    let marked = service
        .mark_knowledge_update_seen(MarkKnowledgeUpdateSeenOptions {
            update_id: Some("component_answers_flow".to_string()),
            all: false,
        })
        .unwrap();
    assert_eq!(
        marked.marked_updates,
        vec!["greentic-component::component_answers_flow"]
    );
    assert!(marked.state_path.exists());

    let after_seen = service
        .updates(UpdatesOptions {
            new_only: true,
            ..UpdatesOptions::default()
        })
        .unwrap();
    assert!(after_seen.updates.is_empty());

    write_update(&repo, "Use answers flow with a changed digest.");
    service.analyze(AnalyzeOptions).unwrap();
    let after_digest_change = service
        .updates(UpdatesOptions {
            new_only: true,
            ..UpdatesOptions::default()
        })
        .unwrap();
    assert_eq!(after_digest_change.updates.len(), 1);
}

fn synthetic_repo() -> TempDir {
    let repo = TempDir::new().unwrap();
    fs::create_dir(repo.path().join(".git")).unwrap();
    fs::write(
        repo.path().join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(
        repo.path().join("README.md"),
        "# Demo\n\nUse `gtc wizard --schema` for component work.\n",
    )
    .unwrap();
    repo
}

fn write_update(repo: &TempDir, summary: &str) {
    let updates_dir = repo.path().join(".greentic").join("updates");
    fs::create_dir_all(&updates_dir).unwrap();
    fs::write(
        updates_dir.join("component-answers-flow.update.v1.json"),
        format!(
            r#"{{
  "version": "v1",
  "id": "component_answers_flow",
  "title": "Component answers flow",
  "summary": "{summary}",
  "owner_repo": "greentic-component",
  "update_type": "deprecated_command",
  "published_at": "2026-04-26",
  "effective_from": "2026-04-26",
  "expires_at": null,
  "affected_concepts": ["component", "wizard"],
  "affected_workflows": ["component_creation"],
  "affected_courses": [],
  "affected_repos": ["unknown/demo"],
  "agent_instruction": "Use answers.",
  "human_summary": "Use answers.",
  "new_capabilities": [],
  "deprecated_commands": [
    {{
      "command": "gtc component new",
      "reason": "Old command.",
      "replacement": "greentic-flow wizard --answers answers.json"
    }}
  ],
  "replaced_guidance": [],
  "migration_steps": [],
  "required_validations": ["bash ci/local_check.sh"],
  "source_paths": [".greentic/updates/component-answers-flow.update.v1.json"],
  "severity": "important"
}}"#
        ),
    )
    .unwrap();
}
