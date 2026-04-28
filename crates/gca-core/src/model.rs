use crate::{FreshnessStatus, KnowledgeScope, LifecyclePhase, RepoRole};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCHEMA_VERSION_V1: &str = "v1";

pub const BUILTIN_CONCEPT_IDS: &[&str] = &[
    "agent_training_course",
    "knowledge_update",
    "repository_index_rollout",
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
pub struct RepoId {
    pub org: String,
    pub name: String,
}

impl RepoId {
    pub fn parse(value: &str) -> Result<Self, String> {
        let value = value.trim().trim_end_matches(".git");
        let mut parts = value.split('/');
        let Some(org) = parts.next() else {
            return Err("repo id must use org/repo form".to_string());
        };
        let Some(name) = parts.next() else {
            return Err("repo id must use org/repo form".to_string());
        };
        if parts.next().is_some() || org.is_empty() || name.is_empty() {
            return Err("repo id must use org/repo form".to_string());
        }
        Ok(Self {
            org: org.to_string(),
            name: name.to_string(),
        })
    }

    pub fn as_str(&self) -> String {
        format!("{}/{}", self.org, self.name)
    }

    pub fn ghcr_path(&self) -> String {
        self.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoAgentManifest {
    pub version: String,
    #[serde(default = "default_repo_id")]
    pub repo_id: String,
    pub repo_name: String,
    #[serde(default)]
    pub org: Option<String>,
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
        if self.repo_id.is_empty() {
            return Err("repo manifest repo_id must not be empty".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoIndex {
    pub version: String,
    #[serde(default = "default_repo_id")]
    pub repo_id: String,
    pub repo_name: String,
    pub repo_role: RepoRole,
    pub generated_at: String,
    #[serde(default)]
    pub metadata: Option<RepoIndexMetadata>,
    pub freshness: FreshnessStatus,
    pub manifest: RepoAgentManifest,
    pub concept_graph: Vec<ConceptDescriptor>,
    pub workflow_graph: Vec<WorkflowDescriptor>,
    pub validations: Vec<ValidationDescriptor>,
    pub reuse: Vec<ReuseDescriptor>,
    #[serde(default)]
    pub training_courses: Vec<TrainingCourseDescriptor>,
    #[serde(default)]
    pub knowledge_updates: Vec<KnowledgeUpdateDescriptor>,
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
pub struct RepoIndexMetadata {
    pub repo_id: String,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub commit_sha: Option<String>,
    #[serde(default)]
    pub commit_time: Option<String>,
    pub indexed_at: String,
    pub index_schema_version: String,
    pub tool_version: String,
    #[serde(default)]
    pub source_tree_hash: Option<String>,
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
    #[serde(default)]
    pub rust_symbols: Vec<RustSymbolDescriptor>,
    pub test_targets: Vec<String>,
    pub feature_names: Vec<String>,
    pub dependencies: Vec<String>,
    pub markdown_docs: Vec<String>,
    pub workflow_files: Vec<String>,
    pub example_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RustSymbolDescriptor {
    pub name: String,
    pub kind: RustSymbolKind,
    pub visibility: String,
    pub path: String,
    pub line: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RustSymbolKind {
    Function,
    Struct,
    Enum,
    Trait,
    Impl,
    Module,
    Use,
    Test,
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
pub struct TrainingCourseDescriptor {
    pub version: String,
    pub id: String,
    pub title: String,
    pub summary: String,
    pub owner_repo: String,
    pub teaches_concepts: Vec<String>,
    pub tasks: Vec<String>,
    pub audience: Vec<TrainingAudience>,
    pub lifecycle_phase: LifecyclePhase,
    pub modules: Vec<TrainingModuleDescriptor>,
    pub canonical_commands: Vec<String>,
    pub deprecated_commands: Vec<DeprecatedCommandDescriptor>,
    pub required_validations: Vec<String>,
    pub examples: Vec<String>,
    pub source_paths: Vec<String>,
}

impl TrainingCourseDescriptor {
    pub fn validate(&self) -> Result<(), String> {
        if self.version.is_empty() {
            return Err("training course version must not be empty".to_string());
        }
        if self.id.is_empty() {
            return Err("training course id must not be empty".to_string());
        }
        if self.title.is_empty() {
            return Err("training course title must not be empty".to_string());
        }
        if self.owner_repo.is_empty() {
            return Err("training course owner_repo must not be empty".to_string());
        }
        if self.modules.is_empty() {
            return Err("training course modules must not be empty".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingModuleDescriptor {
    pub id: String,
    pub title: String,
    pub objective: String,
    pub steps: Vec<TrainingStepDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingStepDescriptor {
    pub order: u32,
    pub instruction: String,
    pub command: Option<String>,
    pub expected_output: Option<String>,
    pub validation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeprecatedCommandDescriptor {
    pub command: String,
    pub reason: String,
    pub replacement: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingAudience {
    CodingAgent,
    HumanDeveloper,
    CiAutomation,
    RepoMaintainer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeUpdateDescriptor {
    pub version: String,
    pub id: String,
    pub title: String,
    pub summary: String,
    pub owner_repo: String,
    pub update_type: KnowledgeUpdateType,
    pub published_at: String,
    pub effective_from: Option<String>,
    pub expires_at: Option<String>,
    pub affected_concepts: Vec<String>,
    pub affected_workflows: Vec<String>,
    pub affected_courses: Vec<String>,
    pub affected_repos: Vec<String>,
    pub agent_instruction: String,
    pub human_summary: Option<String>,
    pub new_capabilities: Vec<CapabilityAnnouncement>,
    pub deprecated_commands: Vec<DeprecatedCommandDescriptor>,
    pub replaced_guidance: Vec<ReplacedGuidanceDescriptor>,
    pub migration_steps: Vec<MigrationStepDescriptor>,
    pub required_validations: Vec<String>,
    pub source_paths: Vec<String>,
    pub severity: KnowledgeUpdateSeverity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentKnowledgeState {
    pub version: String,
    pub last_sync_at: Option<String>,
    #[serde(default)]
    pub seen_updates: BTreeMap<String, SeenKnowledgeUpdate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeenKnowledgeUpdate {
    pub seen_at: String,
    pub source_digest: Option<String>,
}

impl KnowledgeUpdateDescriptor {
    pub fn validate(&self) -> Result<(), String> {
        if self.version.is_empty() {
            return Err("knowledge update version must not be empty".to_string());
        }
        if self.id.is_empty() {
            return Err("knowledge update id must not be empty".to_string());
        }
        if self.title.is_empty() {
            return Err("knowledge update title must not be empty".to_string());
        }
        if self.summary.is_empty() {
            return Err("knowledge update summary must not be empty".to_string());
        }
        if self.owner_repo.is_empty() {
            return Err("knowledge update owner_repo must not be empty".to_string());
        }
        if self.published_at.is_empty() {
            return Err("knowledge update published_at must not be empty".to_string());
        }
        if self.agent_instruction.is_empty() {
            return Err("knowledge update agent_instruction must not be empty".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeUpdateType {
    NewCapability,
    BehaviourChange,
    DeprecatedWorkflow,
    DeprecatedCommand,
    MigrationRequired,
    ValidationChanged,
    OwnershipChanged,
    CourseUpdated,
    SecurityNotice,
    BreakingChange,
    DocumentationCorrection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeUpdateSeverity {
    Info,
    Recommended,
    Important,
    Breaking,
    Critical,
}

impl KnowledgeUpdateSeverity {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim() {
            "info" => Ok(Self::Info),
            "recommended" => Ok(Self::Recommended),
            "important" => Ok(Self::Important),
            "breaking" => Ok(Self::Breaking),
            "critical" => Ok(Self::Critical),
            other => Err(format!("unsupported knowledge update severity: {other}")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Recommended => "recommended",
            Self::Important => "important",
            Self::Breaking => "breaking",
            Self::Critical => "critical",
        }
    }

    pub fn rank(self) -> u8 {
        match self {
            Self::Critical => 5,
            Self::Breaking => 4,
            Self::Important => 3,
            Self::Recommended => 2,
            Self::Info => 1,
        }
    }
}

impl KnowledgeUpdateType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NewCapability => "new_capability",
            Self::BehaviourChange => "behaviour_change",
            Self::DeprecatedWorkflow => "deprecated_workflow",
            Self::DeprecatedCommand => "deprecated_command",
            Self::MigrationRequired => "migration_required",
            Self::ValidationChanged => "validation_changed",
            Self::OwnershipChanged => "ownership_changed",
            Self::CourseUpdated => "course_updated",
            Self::SecurityNotice => "security_notice",
            Self::BreakingChange => "breaking_change",
            Self::DocumentationCorrection => "documentation_correction",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityAnnouncement {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub use_when: Vec<String>,
    pub owner_repo: String,
    pub related_course: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplacedGuidanceDescriptor {
    pub old_guidance: String,
    pub replacement_guidance: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationStepDescriptor {
    pub order: u32,
    pub instruction: String,
    pub command: Option<String>,
    pub validation: Option<String>,
}

impl TrainingAudience {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim() {
            "coding_agent" => Ok(Self::CodingAgent),
            "human_developer" => Ok(Self::HumanDeveloper),
            "ci_automation" => Ok(Self::CiAutomation),
            "repo_maintainer" => Ok(Self::RepoMaintainer),
            other => Err(format!("unsupported training audience: {other}")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::CodingAgent => "coding_agent",
            Self::HumanDeveloper => "human_developer",
            Self::CiAutomation => "ci_automation",
            Self::RepoMaintainer => "repo_maintainer",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogRepo {
    #[serde(default = "default_repo_id")]
    pub repo_id: String,
    #[serde(default)]
    pub repo_name: String,
    #[serde(alias = "role", default = "default_repo_role")]
    pub repo_role: RepoRole,
    #[serde(default = "default_latest_tag")]
    pub latest_tag: String,
    #[serde(default)]
    pub package_ref: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub default_branch: Option<String>,
    #[serde(default)]
    pub preferred_branch: Option<String>,
    #[serde(default)]
    pub branches: BTreeMap<String, CatalogBranchEntry>,
    #[serde(default)]
    pub visibility: IndexVisibility,
    #[serde(default)]
    pub tenant: Option<String>,
    #[serde(default)]
    pub required_auth: Option<AuthKind>,
    #[serde(default)]
    pub digest: Option<String>,
    #[serde(default)]
    pub source_commit: Option<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogBranchEntry {
    pub index_uri: String,
    #[serde(default)]
    pub commit_sha: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub digest: Option<String>,
}

impl CatalogRepo {
    pub fn validate(&self) -> Result<(), String> {
        RepoId::parse(&self.repo_id)
            .map_err(|error| format!("catalog repo_id `{}` is invalid: {error}", self.repo_id))?;

        for branch in self
            .default_branch
            .iter()
            .chain(self.preferred_branch.iter())
            .chain(self.branches.keys())
        {
            if !is_valid_branch_name(branch) {
                return Err(format!("catalog branch `{branch}` is invalid"));
            }
        }

        for (branch, entry) in &self.branches {
            if entry.index_uri.is_empty() {
                return Err(format!(
                    "catalog branch `{branch}` index_uri must not be empty"
                ));
            }
        }

        Ok(())
    }

    pub fn selected_branch(&self, channel: Option<&str>) -> Option<(String, CatalogBranchEntry)> {
        let branch = channel
            .or(self.preferred_branch.as_deref())
            .or(self.default_branch.as_deref());

        if let Some(branch) = branch
            && let Some(entry) = self.branches.get(branch)
        {
            return Some((branch.to_string(), entry.clone()));
        }

        if let Some(default_branch) = self.default_branch.as_deref()
            && let Some(entry) = self.branches.get(default_branch)
        {
            return Some((default_branch.to_string(), entry.clone()));
        }

        if let Some((branch, entry)) = self.branches.iter().next() {
            return Some((branch.clone(), entry.clone()));
        }

        if !self.package_ref.is_empty() {
            return Some((
                self.latest_tag.clone(),
                CatalogBranchEntry {
                    index_uri: self.package_ref.clone(),
                    commit_sha: self.source_commit.clone(),
                    updated_at: Some(self.updated_at.clone()).filter(|value| !value.is_empty()),
                    digest: self.digest.clone(),
                },
            ));
        }

        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IndexVisibility {
    #[default]
    Public,
    Tenant,
    Private,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthKind {
    GhcrToken,
    BearerToken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalog {
    #[serde(alias = "schema_version")]
    pub version: String,
    pub generated_at: String,
    #[serde(default)]
    pub catalog_id: Option<String>,
    #[serde(default)]
    pub default_channel: Option<String>,
    pub repos: Vec<CatalogRepo>,
    #[serde(default)]
    pub change_log: Vec<CatalogChange>,
}

impl Catalog {
    pub fn validate(&self) -> Result<(), String> {
        if self.version.is_empty() {
            return Err("catalog version must not be empty".to_string());
        }
        for repo in &self.repos {
            repo.validate()?;
        }
        Ok(())
    }
}

fn is_valid_branch_name(branch: &str) -> bool {
    !branch.is_empty()
        && !branch.contains(char::is_whitespace)
        && !branch.contains("..")
        && !branch.starts_with('/')
        && !branch.ends_with('/')
        && !branch.ends_with(".lock")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogChange {
    pub action: CatalogAction,
    pub repo_id: String,
    pub tenant: Option<String>,
    pub at: String,
    pub by: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogAction {
    AddRepo,
    RemoveRepo,
    EnableRepo,
    DisableRepo,
    Publish,
}

fn default_repo_id() -> String {
    "unknown/unknown-repo".to_string()
}

fn default_repo_role() -> RepoRole {
    RepoRole::DemoApp
}

fn default_latest_tag() -> String {
    "latest".to_string()
}

fn default_enabled() -> bool {
    true
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
        BUILTIN_CONCEPT_IDS, CapabilityAnnouncement, Catalog, ConceptDescriptor,
        InstructionDescriptor, KnowledgeUpdateDescriptor, KnowledgeUpdateSeverity,
        KnowledgeUpdateType, MigrationStepDescriptor, RepoAgentManifest, RepoId, RepoIndex,
        SCHEMA_VERSION_V1, SourceStats, TrainingAudience, TrainingCourseDescriptor,
        TrainingModuleDescriptor, TrainingStepDescriptor, builtin_concepts,
    };
    use crate::{
        FreshnessStatus, KnowledgeScope, LifecyclePhase, RepoRole, ReuseDescriptor,
        ValidationDescriptor, WorkflowDescriptor,
    };
    use std::collections::BTreeMap;

    #[test]
    fn builtin_concept_ids_cover_required_seed_values() {
        let concepts = builtin_concepts();
        assert_eq!(concepts.len(), BUILTIN_CONCEPT_IDS.len());
        assert!(BUILTIN_CONCEPT_IDS.contains(&"digital_worker"));
        assert!(BUILTIN_CONCEPT_IDS.contains(&"agent_training_course"));
        assert!(BUILTIN_CONCEPT_IDS.contains(&"knowledge_update"));
        assert!(BUILTIN_CONCEPT_IDS.contains(&"repository_index_rollout"));
        assert!(BUILTIN_CONCEPT_IDS.contains(&"greentic_sorla"));
        assert!(BUILTIN_CONCEPT_IDS.contains(&"static_route"));
    }

    #[test]
    fn top_level_models_round_trip_through_json() {
        let manifest = RepoAgentManifest {
            version: SCHEMA_VERSION_V1.to_string(),
            repo_id: "greenticai/greentic-coding-agent".to_string(),
            repo_name: "greentic-coding-agent".to_string(),
            org: Some("greenticai".to_string()),
            repo_root: "/workspace/greentic-coding-agent".to_string(),
            repo_role: RepoRole::CliLauncher,
            primary_language: "rust".to_string(),
            generated_at: "2026-04-15T00:00:00Z".to_string(),
            candidate_docs: vec!["README.md".to_string()],
            cargo_manifests: vec!["Cargo.toml".to_string()],
        };
        let repo_index = RepoIndex {
            version: SCHEMA_VERSION_V1.to_string(),
            repo_id: manifest.repo_id.clone(),
            repo_name: manifest.repo_name.clone(),
            repo_role: manifest.repo_role,
            generated_at: "2026-04-15T00:00:00Z".to_string(),
            metadata: None,
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
            training_courses: vec![TrainingCourseDescriptor {
                version: SCHEMA_VERSION_V1.to_string(),
                id: "create_component".to_string(),
                title: "Create a component".to_string(),
                summary: "Teach agents the current component authoring flow.".to_string(),
                owner_repo: "greentic-component".to_string(),
                teaches_concepts: vec!["component".to_string(), "wizard".to_string()],
                tasks: vec!["create a component".to_string()],
                audience: vec![TrainingAudience::CodingAgent],
                lifecycle_phase: LifecyclePhase::Build,
                modules: vec![TrainingModuleDescriptor {
                    id: "wizard_flow".to_string(),
                    title: "Wizard flow".to_string(),
                    objective: "Use schema and answers instead of obsolete commands.".to_string(),
                    steps: vec![TrainingStepDescriptor {
                        order: 1,
                        instruction: "Generate the wizard schema.".to_string(),
                        command: Some("greentic-flow component-schema".to_string()),
                        expected_output: Some("answers schema".to_string()),
                        validation: Some("component_qa_schema_change".to_string()),
                    }],
                }],
                canonical_commands: vec!["greentic-flow component-schema".to_string()],
                deprecated_commands: Vec::new(),
                required_validations: vec!["component_qa_schema_change".to_string()],
                examples: vec!["examples/training/create-component.course.v1.json".to_string()],
                source_paths: vec![
                    ".greentic/training/create-component.course.v1.json".to_string(),
                ],
            }],
            knowledge_updates: vec![KnowledgeUpdateDescriptor {
                version: SCHEMA_VERSION_V1.to_string(),
                id: "component_answers_flow".to_string(),
                title: "Component creation uses answers flow".to_string(),
                summary: "Component creation must use the current wizard answers flow.".to_string(),
                owner_repo: "greentic-component".to_string(),
                update_type: KnowledgeUpdateType::DeprecatedCommand,
                published_at: "2026-04-26".to_string(),
                effective_from: Some("2026-04-26".to_string()),
                expires_at: None,
                affected_concepts: vec!["component".to_string(), "wizard".to_string()],
                affected_workflows: vec!["component_creation".to_string()],
                affected_courses: vec!["create_component".to_string()],
                affected_repos: vec!["greentic-component".to_string()],
                agent_instruction:
                    "Use the wizard schema and answers.json flow for component creation."
                        .to_string(),
                human_summary: Some("Old component creation commands are stale.".to_string()),
                new_capabilities: vec![CapabilityAnnouncement {
                    id: "component_answers".to_string(),
                    title: "Component answers flow".to_string(),
                    summary: "Create components from a checked-in answers file.".to_string(),
                    use_when: vec!["create a component".to_string()],
                    owner_repo: "greentic-component".to_string(),
                    related_course: Some("create_component".to_string()),
                }],
                deprecated_commands: Vec::new(),
                replaced_guidance: vec![super::ReplacedGuidanceDescriptor {
                    old_guidance: "Run old component create commands.".to_string(),
                    replacement_guidance: "Use greentic-flow wizard --answers answers.json."
                        .to_string(),
                    reason: "The current flow is schema driven.".to_string(),
                }],
                migration_steps: vec![MigrationStepDescriptor {
                    order: 1,
                    instruction: "Generate and review answers.json.".to_string(),
                    command: Some("greentic-flow component-schema".to_string()),
                    validation: Some(
                        "greentic-flow component-qa --answers answers.json".to_string(),
                    ),
                }],
                required_validations: vec![
                    "greentic-flow component-qa --answers answers.json".to_string(),
                ],
                source_paths: vec![
                    ".greentic/updates/component-creation-uses-wizard-answers.update.v1.json"
                        .to_string(),
                ],
                severity: KnowledgeUpdateSeverity::Important,
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
                rust_symbols: vec![],
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
            catalog_id: None,
            default_channel: None,
            repos: vec![super::CatalogRepo {
                repo_id: "greenticai/greentic-coding-agent".to_string(),
                repo_name: "greentic-coding-agent".to_string(),
                repo_role: RepoRole::CliLauncher,
                latest_tag: "v0.1.0".to_string(),
                package_ref: "ghcr.io/greenticai/indexes/greentic-coding-agent:v0.1.0".to_string(),
                updated_at: "2026-04-15T00:00:00Z".to_string(),
                default_branch: None,
                preferred_branch: None,
                branches: BTreeMap::new(),
                visibility: super::IndexVisibility::Public,
                tenant: None,
                required_auth: None,
                digest: None,
                source_commit: None,
                enabled: true,
            }],
            change_log: vec![super::CatalogChange {
                action: super::CatalogAction::AddRepo,
                repo_id: "greenticai/greentic-coding-agent".to_string(),
                tenant: None,
                at: "2026-04-15T00:00:00Z".to_string(),
                by: None,
                reason: None,
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
    fn catalog_v2_branch_entries_select_requested_or_default_channel() {
        let catalog = serde_json::from_str::<Catalog>(
            r#"{
              "schema_version": "gca.catalog.v2",
              "catalog_id": "greenticai/public",
              "default_channel": "develop",
              "generated_at": "2026-04-27T00:00:00Z",
              "repos": [{
                "repo_id": "greenticai/greentic-coding-agent",
                "repo_name": "greentic-coding-agent",
                "role": "pack_authoring",
                "default_branch": "main",
                "preferred_branch": "develop",
                "branches": {
                  "main": {
                    "index_uri": "ghcr.io/greenticai/indexes/greentic-coding-agent:main",
                    "commit_sha": "abc123",
                    "updated_at": "2026-04-27T00:00:00Z"
                  },
                  "develop": {
                    "index_uri": "ghcr.io/greenticai/indexes/greentic-coding-agent:develop",
                    "commit_sha": "def456",
                    "updated_at": "2026-04-27T00:00:00Z",
                    "digest": "sha256:1234"
                  }
                }
              }]
            }"#,
        )
        .unwrap();

        assert_eq!(catalog.version, "gca.catalog.v2");
        assert_eq!(catalog.default_channel.as_deref(), Some("develop"));
        let repo = catalog.repos.first().unwrap();
        assert_eq!(repo.repo_role, RepoRole::PackAuthoring);
        assert_eq!(repo.latest_tag, "latest");

        let (branch, entry) = repo.selected_branch(Some("main")).unwrap();
        assert_eq!(branch, "main");
        assert_eq!(
            entry.index_uri,
            "ghcr.io/greenticai/indexes/greentic-coding-agent:main"
        );

        let (branch, entry) = repo.selected_branch(None).unwrap();
        assert_eq!(branch, "develop");
        assert_eq!(entry.commit_sha.as_deref(), Some("def456"));
    }

    #[test]
    fn legacy_catalog_entries_remain_selectable_without_branch_map() {
        let catalog = serde_json::from_str::<Catalog>(
            r#"{
              "version": "v1",
              "generated_at": "2026-04-27T00:00:00Z",
              "repos": [{
                "repo_id": "greenticai/greentic-coding-agent",
                "repo_name": "greentic-coding-agent",
                "repo_role": "cli_launcher",
                "latest_tag": "v0.1.2",
                "package_ref": "ghcr.io/greenticai/indexes/greentic-coding-agent:v0.1.2",
                "updated_at": "2026-04-27T00:00:00Z",
                "source_commit": "abc123"
              }]
            }"#,
        )
        .unwrap();

        let repo = catalog.repos.first().unwrap();
        let (branch, entry) = repo.selected_branch(None).unwrap();
        assert_eq!(branch, "v0.1.2");
        assert_eq!(
            entry.index_uri,
            "ghcr.io/greenticai/indexes/greentic-coding-agent:v0.1.2"
        );
        assert_eq!(entry.commit_sha.as_deref(), Some("abc123"));
        assert_eq!(entry.updated_at.as_deref(), Some("2026-04-27T00:00:00Z"));
    }

    #[test]
    fn catalog_validation_rejects_bad_repo_id_and_branch_names() {
        let bad_repo_id = serde_json::from_str::<Catalog>(
            r#"{
              "version": "gca.catalog.v2",
              "generated_at": "2026-04-27T00:00:00Z",
              "repos": [{
                "repo_id": "not-a-repo-id",
                "repo_name": "bad",
                "repo_role": "cli_launcher",
                "branches": {
                  "main": {
                    "index_uri": "ghcr.io/greenticai/indexes/bad:main"
                  }
                }
              }]
            }"#,
        )
        .unwrap();
        assert!(bad_repo_id.validate().unwrap_err().contains("repo_id"));

        let bad_branch = serde_json::from_str::<Catalog>(
            r#"{
              "version": "gca.catalog.v2",
              "generated_at": "2026-04-27T00:00:00Z",
              "repos": [{
                "repo_id": "greenticai/bad",
                "repo_name": "bad",
                "repo_role": "cli_launcher",
                "branches": {
                  "feature branch": {
                    "index_uri": "ghcr.io/greenticai/indexes/bad:feature-branch"
                  }
                }
              }]
            }"#,
        )
        .unwrap();
        assert!(bad_branch.validate().unwrap_err().contains("branch"));
    }

    #[test]
    fn repo_id_parses_org_repo_form() {
        let repo_id = RepoId::parse("greenticai/greentic-types").unwrap();

        assert_eq!(repo_id.org, "greenticai");
        assert_eq!(repo_id.name, "greentic-types");
        assert_eq!(repo_id.as_str(), "greenticai/greentic-types");
        assert_eq!(repo_id.ghcr_path(), "greenticai/greentic-types");
        assert!(RepoId::parse("greentic-types").is_err());
    }

    #[test]
    fn old_repo_name_only_json_remains_readable() {
        let manifest = serde_json::from_str::<RepoAgentManifest>(
            r#"{
              "version": "v1",
              "repo_name": "legacy",
              "repo_root": "/tmp/legacy",
              "repo_role": "cli_launcher",
              "primary_language": "rust",
              "generated_at": "2026-04-15T00:00:00Z",
              "candidate_docs": [],
              "cargo_manifests": []
            }"#,
        )
        .unwrap();

        assert_eq!(manifest.repo_id, "unknown/unknown-repo");
        assert_eq!(manifest.repo_name, "legacy");
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
