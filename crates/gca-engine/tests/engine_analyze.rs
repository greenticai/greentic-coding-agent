use gca_engine::{AnalyzeOptions, CodingAgentService, ConceptsOptions, DescribeOptions};
use std::fs;
use tempfile::TempDir;

#[test]
fn engine_analyze_writes_index_outputs() {
    let repo = synthetic_repo();
    let home = TempDir::new().unwrap();
    let service = CodingAgentService::new(repo.path().to_path_buf(), home.path().to_path_buf());

    let response = service.analyze(AnalyzeOptions).unwrap();

    assert_eq!(
        response.repo_index.repo_name,
        repo.path().file_name().unwrap().to_string_lossy()
    );
    assert!(response.manifest_path.exists());
    assert!(response.repo_index_path.exists());
    assert!(response.fingerprints_path.exists());
    assert!(response.registry_path.exists());
}

#[test]
fn engine_describe_and_concepts_auto_analyze_when_missing() {
    let repo = synthetic_repo();
    let home = TempDir::new().unwrap();
    let service = CodingAgentService::new(repo.path().to_path_buf(), home.path().to_path_buf());

    let description = service.describe_here(DescribeOptions).unwrap();
    let concepts = service.concepts(ConceptsOptions).unwrap();

    assert_eq!(description.repo_root, repo.path());
    assert!(
        concepts
            .concepts
            .iter()
            .any(|concept| concept.id == "component")
    );
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
    fs::write(
        repo.path().join("src/lib.rs"),
        "pub fn create_component() {}\n",
    )
    .unwrap();
    fs::write(
        repo.path().join("README.md"),
        "# Demo\n\nRun `gtc wizard --schema` for component authoring.\n",
    )
    .unwrap();
    repo
}
