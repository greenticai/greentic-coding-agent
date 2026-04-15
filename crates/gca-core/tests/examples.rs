use gca_core::{
    Catalog, ConceptDescriptor, RepoAgentManifest, RepoIndex, ReuseDescriptor,
    ValidationDescriptor, WorkflowDescriptor,
};

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
fn example_catalog_fixture_loads() {
    let raw = include_str!("../../../examples/catalog.v1.json");
    let catalog: Catalog = serde_json::from_str(raw).unwrap();

    catalog.validate().unwrap();
    assert_eq!(catalog.version, "v1");
    assert_eq!(catalog.repos.len(), 1);
}

#[test]
fn example_descriptor_fixtures_load() {
    let concept_raw = include_str!("../../../examples/concept.v1.json");
    let workflow_raw = include_str!("../../../examples/workflow.v1.json");
    let validation_raw = include_str!("../../../examples/validation.v1.json");
    let reuse_raw = include_str!("../../../examples/reuse.v1.json");

    let concept: ConceptDescriptor = serde_json::from_str(concept_raw).unwrap();
    let workflow: WorkflowDescriptor = serde_json::from_str(workflow_raw).unwrap();
    let validation: ValidationDescriptor = serde_json::from_str(validation_raw).unwrap();
    let reuse: ReuseDescriptor = serde_json::from_str(reuse_raw).unwrap();

    concept.validate().unwrap();
    workflow.validate().unwrap();
    validation.validate().unwrap();
    reuse.validate().unwrap();
}
