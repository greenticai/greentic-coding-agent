use gca_agent_files::GeneratedFile;
use gca_core::{
    AgentKnowledgeState, Catalog, ConceptDescriptor, KnowledgeUpdateDescriptor, RepoAgentManifest,
    RepoIndex, TrainingCourseDescriptor, ValidationDescriptor, WorkflowDescriptor,
};
use gca_index::Fingerprints;
use gca_mcp::{ChangeSummary, ImpactAnalysis, McpResponse, McpServerSnapshot};
use gca_oci::{PackageOutput, RefreshCheck, RemoteRepo, SyncReport};
use gca_query::{CommandCatalogEntry, OwnerLookup, RequiredValidationsResponse, SearchResponse};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyzeResponse {
    pub manifest: RepoAgentManifest,
    pub repo_index: RepoIndex,
    pub fingerprints: Fingerprints,
    pub manifest_path: PathBuf,
    pub repo_index_path: PathBuf,
    pub fingerprints_path: PathBuf,
    pub registry_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescribeResponse {
    pub repo_root: PathBuf,
    pub repo_index: RepoIndex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConceptsResponse {
    pub concepts: Vec<ConceptDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowsResponse {
    pub workflows: Vec<WorkflowDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandsResponse {
    pub commands: Vec<CommandCatalogEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoursesResponse {
    pub courses: Vec<TrainingCourseDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CourseResponse {
    pub course: TrainingCourseDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainingRecommendationsResponse {
    pub courses: Vec<TrainingCourseDescriptor>,
    pub updates: Vec<KnowledgeUpdateDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainResponse {
    pub task: String,
    pub courses: Vec<TrainingCourseDescriptor>,
    pub updates: Vec<KnowledgeUpdateDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdatesResponse {
    pub updates: Vec<KnowledgeUpdateDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateResponse {
    pub update: KnowledgeUpdateDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkKnowledgeUpdateSeenResponse {
    pub state_path: PathBuf,
    pub state: AgentKnowledgeState,
    pub marked_updates: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateAgentFilesResponse {
    pub generated_files: Vec<GeneratedFile>,
    pub written_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallGithubWorkflowResponse {
    pub workflow_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageIndexResponse {
    pub package: PackageOutput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishIndexResponse {
    pub published_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncResponse {
    pub synced_paths: Vec<PathBuf>,
    pub report: SyncReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListRemoteReposResponse {
    pub repos: Vec<RemoteRepo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShowCatalogResponse {
    pub catalog: Catalog,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebuildMergedIndexResponse {
    pub index_path: PathBuf,
    pub repos_indexed: usize,
    pub documents_indexed: usize,
}

pub type SearchEngineResponse = SearchResponse;
pub type OwnerLookupResponse = OwnerLookup;
pub type RequiredValidationsEngineResponse = RequiredValidationsResponse;
pub type CheckRefreshResponse = RefreshCheck;
pub type ImpactResponse = ImpactAnalysis;
pub type DetectChangesResponse = ChangeSummary;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatePlanResponse {
    pub plan_path: PathBuf,
    pub task_summary: String,
    pub owner_hints: Vec<OwnerLookup>,
    pub validations: Vec<ValidationDescriptor>,
    pub knowledge_updates: Vec<KnowledgeUpdateDescriptor>,
    pub acknowledged_updates: Vec<String>,
    pub freshness_warning: Option<String>,
    pub issues: Vec<String>,
}

pub type McpSnapshotResponse = McpServerSnapshot;
pub type DispatchMcpRequestResponse = McpResponse;
