use gca_core::{
    Catalog, ConceptDescriptor, KnowledgeUpdateDescriptor, RepoAgentManifest, RepoIndex,
    ReuseDescriptor, TrainingCourseDescriptor, ValidationDescriptor, WorkflowDescriptor,
};
use std::path::{Path, PathBuf};

#[test]
fn example_repo_manifest_fixture_loads() {
    let raw = include_str!("../../../examples/repo-manifest.v1.json");
    let manifest: RepoAgentManifest = serde_json::from_str(raw).unwrap();

    manifest.validate().unwrap();
    assert_eq!(manifest.version, "v1");
    assert_eq!(manifest.repo_name, "greentic-coding-agent");
}

#[test]
fn example_repo_index_fixture_loads() {
    let raw = include_str!("../../../examples/repo-index.v1.json");
    let repo_index: RepoIndex = serde_json::from_str(raw).unwrap();

    repo_index.validate().unwrap();
    assert_eq!(repo_index.version, "v1");
    assert!(!repo_index.concept_graph.is_empty());
}

#[test]
fn old_repo_index_without_training_courses_still_loads() {
    let raw = include_str!("../../../examples/repo-index.v1.json");
    let mut value: serde_json::Value = serde_json::from_str(raw).unwrap();
    value.as_object_mut().unwrap().remove("training_courses");
    value.as_object_mut().unwrap().remove("knowledge_updates");
    let repo_index: RepoIndex = serde_json::from_value(value).unwrap();

    repo_index.validate().unwrap();
    assert!(repo_index.training_courses.is_empty());
    assert!(repo_index.knowledge_updates.is_empty());
}

#[test]
fn example_catalog_fixture_loads() {
    let raw = include_str!("../../../examples/catalog.v1.json");
    let public_raw = include_str!("../../../examples/catalog.public.v1.json");
    let tenant_raw = include_str!("../../../examples/catalog.tenant.meeza.v1.json");
    let catalog: Catalog = serde_json::from_str(raw).unwrap();
    let public_catalog: Catalog = serde_json::from_str(public_raw).unwrap();
    let tenant_catalog: Catalog = serde_json::from_str(tenant_raw).unwrap();

    catalog.validate().unwrap();
    public_catalog.validate().unwrap();
    tenant_catalog.validate().unwrap();
    assert_eq!(catalog.version, "v1");
    assert_eq!(catalog.repos.len(), 1);
    assert_eq!(public_catalog.repos[0].repo_id, "greenticai/greentic-types");
    assert_eq!(tenant_catalog.repos[0].tenant.as_deref(), Some("meeza"));
}

#[test]
fn example_request_fixtures_are_valid_json() {
    let describe_raw = include_str!("../../../examples/mcp-request.describe-repo.json");
    let search_raw = include_str!("../../../examples/mcp-request.search-all.json");
    let server_search_raw = include_str!("../../../examples/server-search-request.json");

    let describe: serde_json::Value = serde_json::from_str(describe_raw).unwrap();
    let search: serde_json::Value = serde_json::from_str(search_raw).unwrap();
    let server_search: serde_json::Value = serde_json::from_str(server_search_raw).unwrap();

    assert_eq!(describe["tool"], "describe_repo");
    assert_eq!(search["tool"], "search_all");
    assert_eq!(server_search["query"], "wizard");
}

#[test]
fn example_descriptor_fixtures_load() {
    let concept_raw = include_str!("../../../examples/concept.v1.json");
    let workflow_raw = include_str!("../../../examples/workflow.v1.json");
    let validation_raw = include_str!("../../../examples/validation.v1.json");
    let reuse_raw = include_str!("../../../examples/reuse.v1.json");
    let course_raw = include_str!("../../../examples/training/create-component.course.v1.json");
    let update_raw = include_str!(
        "../../../examples/updates/component-creation-uses-wizard-answers.update.v1.json"
    );

    let concept: ConceptDescriptor = serde_json::from_str(concept_raw).unwrap();
    let workflow: WorkflowDescriptor = serde_json::from_str(workflow_raw).unwrap();
    let validation: ValidationDescriptor = serde_json::from_str(validation_raw).unwrap();
    let reuse: ReuseDescriptor = serde_json::from_str(reuse_raw).unwrap();
    let course: TrainingCourseDescriptor = serde_json::from_str(course_raw).unwrap();
    let update: KnowledgeUpdateDescriptor = serde_json::from_str(update_raw).unwrap();

    concept.validate().unwrap();
    workflow.validate().unwrap();
    validation.validate().unwrap();
    reuse.validate().unwrap();
    course.validate().unwrap();
    update.validate().unwrap();
}

#[test]
fn all_training_course_examples_load_and_validate() {
    let files = collect_example_files(Path::new("../../examples/training"), ".course.v1.json");
    assert!(
        files.len() >= 11,
        "expected seeded training course examples, found {}",
        files.len()
    );

    for file in files {
        let raw = std::fs::read_to_string(&file).unwrap();
        let course: TrainingCourseDescriptor = serde_json::from_str(&raw)
            .unwrap_or_else(|error| panic!("failed to parse {}: {error}", file.display()));
        course
            .validate()
            .unwrap_or_else(|error| panic!("failed to validate {}: {error}", file.display()));
        assert!(
            !course.canonical_commands.is_empty(),
            "{} must include canonical commands",
            file.display()
        );
        assert!(
            !course.required_validations.is_empty(),
            "{} must include required validations",
            file.display()
        );
        assert!(
            !course.source_paths.is_empty(),
            "{} must include source paths",
            file.display()
        );
    }
}

#[test]
fn all_knowledge_update_examples_load_and_validate() {
    let files = collect_example_files(Path::new("../../examples/updates"), ".update.v1.json");
    assert!(
        files.len() >= 3,
        "expected seeded knowledge update examples, found {}",
        files.len()
    );

    for file in files {
        let raw = std::fs::read_to_string(&file).unwrap();
        let update: KnowledgeUpdateDescriptor = serde_json::from_str(&raw)
            .unwrap_or_else(|error| panic!("failed to parse {}: {error}", file.display()));
        update
            .validate()
            .unwrap_or_else(|error| panic!("failed to validate {}: {error}", file.display()));
        assert!(
            !update.affected_concepts.is_empty(),
            "{} must include affected concepts",
            file.display()
        );
        assert!(
            !update.required_validations.is_empty(),
            "{} must include required validations",
            file.display()
        );
    }
}

fn collect_example_files(relative_root: &Path, suffix: &str) -> Vec<PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.join(relative_root);
    let mut files = Vec::new();
    collect_example_files_inner(&root, suffix, &mut files);
    files.sort();
    files
}

fn collect_example_files_inner(root: &Path, suffix: &str, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            collect_example_files_inner(&path, suffix, files);
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(suffix))
        {
            files.push(path);
        }
    }
}
