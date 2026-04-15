use crate::{FreshnessStatus, KnowledgeScope, LifecyclePhase, RepoRole};
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION_V1: &str = "v1";

pub const BUILTIN_CONCEPT_IDS: &[&str] = &[
    "digital_worker",
    "application_pack",
    "extension_pack",
    "bundle",
    "flow",
    "component",
    "wizard",
    "setup",
    "start",
    "greentic_x",
    "greentic_sorla",
    "capability",
    "provider",
    "hook",
    "observer",
    "static_route",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoAgentManifest {
    pub version: String,
    pub repo_name: String,
    pub repo_root: String,
    pub repo_role: RepoRole,
    pub primary_language: String,
    pub generated_at: String,
    pub candidate_docs: Vec<String>,
    pub cargo_manifests: Vec<String>,
}

impl RepoAgentManifest {
    pub fn validate(&self) -> Result<(), String> {
        if self.version.is_empty() {
            return Err("repo manifest version must not be empty".to_string());
        }
        if self.repo_name.is_empty() {
            return Err("repo manifest repo_name must not be empty".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoIndex {
    pub version: String,
    pub repo_name: String,
    pub repo_role: RepoRole,
    pub generated_at: String,
    pub freshness: FreshnessStatus,
    pub manifest: RepoAgentManifest,
    pub concept_graph: Vec<ConceptDescriptor>,
    pub workflow_graph: Vec<WorkflowDescriptor>,
    pub validations: Vec<ValidationDescriptor>,
    pub reuse: Vec<ReuseDescriptor>,
    pub instruction_graph: Vec<InstructionDescriptor>,
    pub instruction_paths: Vec<String>,
    pub source_stats: SourceStats,
}

impl RepoIndex {
    pub fn validate(&self) -> Result<(), String> {
        if self.version.is_empty() {
            return Err("repo index version must not be empty".to_string());
        }
        self.manifest.validate()?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstructionDescriptor {
    pub id: String,
    pub path: String,
    pub title: String,
    pub kind: String,
    pub headings: Vec<String>,
    pub commands: Vec<String>,
    pub concept_ids: Vec<String>,
}

impl InstructionDescriptor {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("instruction id must not be empty".to_string());
        }
        if self.path.is_empty() {
            return Err("instruction path must not be empty".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SourceStats {
    pub workspace_members: Vec<String>,
    pub crate_names: Vec<String>,
    pub modules: Vec<String>,
    pub public_items: Vec<String>,
    pub test_targets: Vec<String>,
    pub feature_names: Vec<String>,
    pub dependencies: Vec<String>,
    pub markdown_docs: Vec<String>,
    pub workflow_files: Vec<String>,
    pub example_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConceptDescriptor {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub scope: KnowledgeScope,
    pub lifecycle_phase: LifecyclePhase,
    pub owners: Vec<String>,
    pub related_paths: Vec<String>,
}

impl ConceptDescriptor {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("concept id must not be empty".to_string());
        }
        if self.title.is_empty() {
            return Err("concept title must not be empty".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowDescriptor {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub phase: LifecyclePhase,
    pub commands: Vec<String>,
    pub docs: Vec<String>,
    pub concept_ids: Vec<String>,
}

impl WorkflowDescriptor {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("workflow id must not be empty".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationDescriptor {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub phase: LifecyclePhase,
    pub command_groups: Vec<String>,
    pub triggered_by: Vec<String>,
}

impl ValidationDescriptor {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("validation id must not be empty".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReuseDescriptor {
    pub id: String,
    pub concept_id: String,
    pub owner_repo: String,
    pub rationale: String,
    pub forbidden_locations: Vec<String>,
    pub required_validations: Vec<String>,
}

impl ReuseDescriptor {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("reuse id must not be empty".to_string());
        }
        if self.concept_id.is_empty() {
            return Err("reuse concept_id must not be empty".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogRepo {
    pub repo_name: String,
    pub repo_role: RepoRole,
    pub latest_tag: String,
    pub package_ref: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalog {
    pub version: String,
    pub generated_at: String,
    pub repos: Vec<CatalogRepo>,
}

impl Catalog {
    pub fn validate(&self) -> Result<(), String> {
        if self.version.is_empty() {
            return Err("catalog version must not be empty".to_string());
        }
        Ok(())
    }
}

pub fn builtin_concepts() -> Vec<ConceptDescriptor> {
    BUILTIN_CONCEPT_IDS
        .iter()
        .map(|id| ConceptDescriptor {
            id: (*id).to_string(),
            title: id.replace('_', " "),
            summary: format!("Built-in Greentic concept `{id}`."),
            scope: KnowledgeScope::CrossRepo,
            lifecycle_phase: LifecyclePhase::Design,
            owners: vec!["greentic-coding-agent".to_string()],
            related_paths: Vec::new(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        BUILTIN_CONCEPT_IDS, Catalog, ConceptDescriptor, InstructionDescriptor, RepoAgentManifest,
        RepoIndex, SCHEMA_VERSION_V1, SourceStats, builtin_concepts,
    };
    use crate::{
        FreshnessStatus, KnowledgeScope, LifecyclePhase, RepoRole, ReuseDescriptor,
        ValidationDescriptor, WorkflowDescriptor,
    };

    #[test]
    fn builtin_concept_ids_cover_required_seed_values() {
        let concepts = builtin_concepts();
        assert_eq!(concepts.len(), BUILTIN_CONCEPT_IDS.len());
        assert!(BUILTIN_CONCEPT_IDS.contains(&"digital_worker"));
        assert!(BUILTIN_CONCEPT_IDS.contains(&"greentic_sorla"));
        assert!(BUILTIN_CONCEPT_IDS.contains(&"static_route"));
    }

    #[test]
    fn top_level_models_round_trip_through_json() {
        let manifest = RepoAgentManifest {
            version: SCHEMA_VERSION_V1.to_string(),
            repo_name: "greentic-coding-agent".to_string(),
            repo_root: "/workspace/greentic-coding-agent".to_string(),
            repo_role: RepoRole::CliLauncher,
            primary_language: "rust".to_string(),
            generated_at: "2026-04-15T00:00:00Z".to_string(),
            candidate_docs: vec!["README.md".to_string()],
            cargo_manifests: vec!["Cargo.toml".to_string()],
        };
        let repo_index = RepoIndex {
            version: SCHEMA_VERSION_V1.to_string(),
            repo_name: manifest.repo_name.clone(),
            repo_role: manifest.repo_role,
            generated_at: "2026-04-15T00:00:00Z".to_string(),
            freshness: FreshnessStatus::Fresh,
            manifest: manifest.clone(),
            concept_graph: vec![ConceptDescriptor {
                id: "digital_worker".to_string(),
                title: "Digital worker".to_string(),
                summary: "Core runtime concept".to_string(),
                scope: KnowledgeScope::CrossRepo,
                lifecycle_phase: LifecyclePhase::Runtime,
                owners: vec!["greentic-types".to_string()],
                related_paths: vec!["docs/architecture.md".to_string()],
            }],
            workflow_graph: vec![WorkflowDescriptor {
                id: "analyze_repo".to_string(),
                title: "Analyze repo".to_string(),
                summary: "Bootstrap local repo intelligence".to_string(),
                phase: LifecyclePhase::Build,
                commands: vec!["gtc dev coding-agent analyze".to_string()],
                docs: vec!["README.md".to_string()],
                concept_ids: vec!["digital_worker".to_string()],
            }],
            validations: vec![ValidationDescriptor {
                id: "cargo_test_workspace".to_string(),
                title: "Workspace tests".to_string(),
                summary: "Run workspace tests after contract changes".to_string(),
                phase: LifecyclePhase::Build,
                command_groups: vec!["cargo test --workspace --all-features".to_string()],
                triggered_by: vec!["shared_schema_change".to_string()],
            }],
            reuse: vec![ReuseDescriptor {
                id: "extension_pack_owner".to_string(),
                concept_id: "extension_pack".to_string(),
                owner_repo: "greentic-pack".to_string(),
                rationale: "Extension pack schemas should live with pack contracts".to_string(),
                forbidden_locations: vec!["customer-solution".to_string()],
                required_validations: vec!["cargo test --workspace --all-features".to_string()],
            }],
            instruction_graph: vec![InstructionDescriptor {
                id: "architecture".to_string(),
                path: "docs/architecture.md".to_string(),
                title: "Architecture".to_string(),
                kind: "doc".to_string(),
                headings: vec!["Repository goals".to_string()],
                commands: vec!["gtc dev coding-agent analyze".to_string()],
                concept_ids: vec!["digital_worker".to_string()],
            }],
            instruction_paths: vec![
                ".codex/PR-02-canonical-schemas-and-domain-types.md".to_string(),
            ],
            source_stats: SourceStats {
                workspace_members: vec!["crates/gca-cli".to_string()],
                crate_names: vec!["greentic-coding-agent".to_string()],
                modules: vec!["src/main.rs".to_string()],
                public_items: vec!["pub fn describe_here".to_string()],
                test_targets: vec!["crates/gca-cli/tests/cli.rs".to_string()],
                feature_names: vec![],
                dependencies: vec!["clap".to_string()],
                markdown_docs: vec!["README.md".to_string()],
                workflow_files: vec![".github/workflows/ci.yml".to_string()],
                example_paths: vec!["examples/repo-index.v1.json".to_string()],
            },
        };
        let catalog = Catalog {
            version: SCHEMA_VERSION_V1.to_string(),
            generated_at: "2026-04-15T00:00:00Z".to_string(),
            repos: vec![super::CatalogRepo {
                repo_name: "greentic-coding-agent".to_string(),
                repo_role: RepoRole::CliLauncher,
                latest_tag: "v0.1.0".to_string(),
                package_ref: "ghcr.io/greenticai/indexes/greentic-coding-agent:v0.1.0".to_string(),
                updated_at: "2026-04-15T00:00:00Z".to_string(),
            }],
        };

        let manifest_json = serde_json::to_string_pretty(&manifest).unwrap();
        let repo_index_json = serde_json::to_string_pretty(&repo_index).unwrap();
        let catalog_json = serde_json::to_string_pretty(&catalog).unwrap();

        assert_eq!(
            serde_json::from_str::<RepoAgentManifest>(&manifest_json).unwrap(),
            manifest
        );
        assert_eq!(
            serde_json::from_str::<RepoIndex>(&repo_index_json).unwrap(),
            repo_index
        );
        assert_eq!(
            serde_json::from_str::<Catalog>(&catalog_json).unwrap(),
            catalog
        );
    }

    #[test]
    fn validation_helpers_reject_empty_required_fields() {
        let invalid = ConceptDescriptor {
            id: String::new(),
            title: String::new(),
            summary: "missing contract fields".to_string(),
            scope: KnowledgeScope::LocalRepo,
            lifecycle_phase: LifecyclePhase::Build,
            owners: Vec::new(),
            related_paths: Vec::new(),
        };

        assert!(invalid.validate().is_err());
    }

    #[test]
    fn instruction_descriptor_rejects_empty_id() {
        let invalid = InstructionDescriptor {
            id: String::new(),
            path: "README.md".to_string(),
            title: "Readme".to_string(),
            kind: "doc".to_string(),
            headings: vec![],
            commands: vec![],
            concept_ids: vec![],
        };

        assert!(invalid.validate().is_err());
    }
}
