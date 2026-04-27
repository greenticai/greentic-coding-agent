use gca_query::{SearchEngineChoice, SearchMode};
use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct AnalyzeOptions;

#[derive(Debug, Clone, Default)]
pub struct DescribeOptions;

#[derive(Debug, Clone, Default)]
pub struct ConceptsOptions;

#[derive(Debug, Clone, Default)]
pub struct WorkflowsOptions;

#[derive(Debug, Clone, Default)]
pub struct CommandsOptions;

#[derive(Debug, Clone, Default)]
pub struct CoursesOptions;

#[derive(Debug, Clone)]
pub struct ShowCourseOptions {
    pub course_id: String,
}

#[derive(Debug, Clone)]
pub struct RecommendTrainingCoursesOptions {
    pub task: String,
    pub audience: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TrainOptions {
    pub task: String,
    pub audience: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdatesOptions {
    pub task: Option<String>,
    pub concept: Option<String>,
    pub severity: Option<String>,
    pub new_only: bool,
}

#[derive(Debug, Clone)]
pub struct ShowUpdateOptions {
    pub update_id: String,
}

#[derive(Debug, Clone, Default)]
pub struct MarkKnowledgeUpdateSeenOptions {
    pub update_id: Option<String>,
    pub all: bool,
}

#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub mode: SearchMode,
    pub query: String,
    pub engine: SearchEngineChoice,
}

#[derive(Debug, Clone)]
pub struct LocateOwnerOptions {
    pub concept_id: String,
}

#[derive(Debug, Clone)]
pub struct RequiredValidationsOptions {
    pub task: String,
}

#[derive(Debug, Clone)]
pub struct GenerateAgentFilesOptions {
    pub write_root: bool,
}

#[derive(Debug, Clone, Default)]
pub struct InstallGithubWorkflowOptions;

#[derive(Debug, Clone)]
pub struct PackageIndexOptions {
    pub tag: String,
}

#[derive(Debug, Clone)]
pub struct PublishIndexOptions {
    pub tag: String,
    pub remote_root: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct SyncOptions {
    pub repo_id: Option<String>,
    pub tag: Option<String>,
    pub remote_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct ListRemoteReposOptions {
    pub remote_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct ShowCatalogOptions {
    pub remote_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct RebuildMergedIndexOptions {
    pub tenant: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CheckRefreshOptions;

#[derive(Debug, Clone)]
pub struct ImpactOptions {
    pub symbol: String,
}

#[derive(Debug, Clone)]
pub struct DetectChangesOptions {
    pub changed_files: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ValidatePlanOptions {
    pub plan_path: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct McpSnapshotOptions;

#[derive(Debug, Clone)]
pub struct DispatchMcpRequestOptions {
    pub request: gca_mcp::McpRequest,
}
