use crate::{
    AnalyzeOptions, AnalyzeResponse, CheckRefreshOptions, CheckRefreshResponse, CommandsOptions,
    CommandsResponse, ConceptsOptions, ConceptsResponse, CourseResponse, CoursesOptions,
    CoursesResponse, DescribeOptions, DescribeResponse, DetectChangesOptions,
    DetectChangesResponse, DispatchMcpRequestOptions, DispatchMcpRequestResponse,
    GenerateAgentFilesOptions, GenerateAgentFilesResponse, ImpactOptions, ImpactResponse,
    InstallGithubWorkflowOptions, InstallGithubWorkflowResponse, ListRemoteReposOptions,
    ListRemoteReposResponse, LocateOwnerOptions, MarkKnowledgeUpdateSeenOptions,
    MarkKnowledgeUpdateSeenResponse, McpSnapshotOptions, McpSnapshotResponse, OwnerLookupResponse,
    PackageIndexOptions, PackageIndexResponse, PublishIndexOptions, PublishIndexResponse,
    RebuildMergedIndexOptions, RebuildMergedIndexResponse, RecommendTrainingCoursesOptions,
    RequiredValidationsEngineResponse, RequiredValidationsOptions, SearchEngineResponse,
    SearchOptions, ShowCatalogOptions, ShowCatalogResponse, ShowCourseOptions, ShowUpdateOptions,
    SyncOptions, SyncResponse, TrainOptions, TrainResponse, TrainingRecommendationsResponse,
    UpdateResponse, UpdatesOptions, UpdatesResponse, ValidatePlanOptions, ValidatePlanResponse,
    WorkflowsOptions, WorkflowsResponse,
};
use gca_core::{
    AgentKnowledgeState, KnowledgeUpdateDescriptor, KnowledgeUpdateSeverity, RepoIndex,
    SeenKnowledgeUpdate, TrainingAudience,
};
use gca_index::{AnalyzeError, TantivyIndexError, default_registry_path};
use gca_query::{UpdateFilter, load_policy_bundle};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

const LOCAL_INDEX_DIR: &str = ".greentic-agent";
const AGENT_KNOWLEDGE_STATE_FILE: &str = "agent-knowledge-state.json";
const AGENT_KNOWLEDGE_STATE_VERSION: &str = "v1";

#[derive(Debug, Clone)]
pub struct CodingAgentService {
    pub cwd: PathBuf,
    pub home_dir: PathBuf,
}

#[derive(Debug, Error)]
pub enum EngineError {
    #[error(transparent)]
    Analyze(#[from] AnalyzeError),
    #[error(transparent)]
    Tantivy(#[from] TantivyIndexError),
    #[error("failed to read {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse {path}: {source}")]
    Parse {
        path: String,
        source: serde_json::Error,
    },
    #[error("failed to write {path}: {source}")]
    Write {
        path: String,
        source: std::io::Error,
    },
    #[error("io error at {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("owner not found for concept `{0}`")]
    OwnerNotFound(String),
    #[error("training course not found: `{0}`")]
    CourseNotFound(String),
    #[error("knowledge update not found: `{0}`")]
    UpdateNotFound(String),
    #[error("search failed: {0}")]
    Search(String),
    #[error("{0}")]
    InvalidInput(String),
    #[error("sync failed: {0}")]
    Sync(String),
    #[error("unsupported operation: {0}")]
    Unsupported(String),
}

impl CodingAgentService {
    pub fn new(cwd: PathBuf, home_dir: PathBuf) -> Self {
        Self { cwd, home_dir }
    }

    pub fn analyze(&self, _options: AnalyzeOptions) -> Result<AnalyzeResponse, EngineError> {
        let outputs = gca_index::analyze_repo(&self.cwd, &default_registry_path(&self.home_dir))?;
        Ok(AnalyzeResponse {
            manifest: outputs.manifest,
            repo_index: outputs.repo_index,
            fingerprints: outputs.fingerprints,
            manifest_path: outputs.manifest_path,
            repo_index_path: outputs.repo_index_path,
            fingerprints_path: outputs.fingerprints_path,
            registry_path: outputs.registry_path,
        })
    }

    pub fn describe_here(
        &self,
        _options: DescribeOptions,
    ) -> Result<DescribeResponse, EngineError> {
        let repo_index = self.ensure_repo_index()?;
        Ok(DescribeResponse {
            repo_root: self.repo_root(),
            repo_index,
        })
    }

    pub fn concepts(&self, _options: ConceptsOptions) -> Result<ConceptsResponse, EngineError> {
        Ok(ConceptsResponse {
            concepts: self.ensure_repo_index()?.concept_graph,
        })
    }

    pub fn workflows(&self, _options: WorkflowsOptions) -> Result<WorkflowsResponse, EngineError> {
        Ok(WorkflowsResponse {
            workflows: self.ensure_repo_index()?.workflow_graph,
        })
    }

    pub fn commands(&self, _options: CommandsOptions) -> Result<CommandsResponse, EngineError> {
        Ok(CommandsResponse {
            commands: gca_query::command_catalog(),
        })
    }

    pub fn courses(&self, _options: CoursesOptions) -> Result<CoursesResponse, EngineError> {
        let repo_index = self.ensure_repo_index()?;
        Ok(CoursesResponse {
            courses: gca_query::list_training_courses(&repo_index),
        })
    }

    pub fn show_course(&self, options: ShowCourseOptions) -> Result<CourseResponse, EngineError> {
        let repo_index = self.ensure_repo_index()?;
        let course = gca_query::show_training_course(&repo_index, &options.course_id)
            .ok_or(EngineError::CourseNotFound(options.course_id))?;
        Ok(CourseResponse { course })
    }

    pub fn recommend_training_courses(
        &self,
        options: RecommendTrainingCoursesOptions,
    ) -> Result<TrainingRecommendationsResponse, EngineError> {
        let repo_index = self.ensure_repo_index()?;
        let audience = parse_optional_audience(options.audience)?;
        Ok(TrainingRecommendationsResponse {
            courses: gca_query::recommend_training_courses(&repo_index, &options.task, audience),
            updates: gca_query::important_updates_for_task(&repo_index, &options.task),
        })
    }

    pub fn train(&self, options: TrainOptions) -> Result<TrainResponse, EngineError> {
        let repo_index = self.ensure_repo_index()?;
        let audience = parse_optional_audience(options.audience)?;
        let courses = gca_query::recommend_training_courses(&repo_index, &options.task, audience);
        let updates = gca_query::important_updates_for_task(&repo_index, &options.task);
        Ok(TrainResponse {
            task: options.task,
            courses,
            updates,
        })
    }

    pub fn updates(&self, options: UpdatesOptions) -> Result<UpdatesResponse, EngineError> {
        let repo_index = self.ensure_repo_index()?;
        let severity = options
            .severity
            .as_deref()
            .map(KnowledgeUpdateSeverity::parse)
            .transpose()
            .map_err(EngineError::InvalidInput)?;
        let mut updates = gca_query::list_knowledge_updates(
            &repo_index,
            UpdateFilter {
                task: options.task,
                concept: options.concept,
                severity,
                update_type: None,
            },
        );
        if options.new_only {
            let state = self.load_agent_knowledge_state()?;
            updates.retain(|update| is_update_unseen(&state, update));
        }
        Ok(UpdatesResponse { updates })
    }

    pub fn show_update(&self, options: ShowUpdateOptions) -> Result<UpdateResponse, EngineError> {
        let repo_index = self.ensure_repo_index()?;
        let update = gca_query::show_knowledge_update(&repo_index, &options.update_id)
            .ok_or(EngineError::UpdateNotFound(options.update_id))?;
        Ok(UpdateResponse { update })
    }

    pub fn mark_knowledge_update_seen(
        &self,
        options: MarkKnowledgeUpdateSeenOptions,
    ) -> Result<MarkKnowledgeUpdateSeenResponse, EngineError> {
        let repo_index = self.ensure_repo_index()?;
        let mut state = self.load_agent_knowledge_state()?;
        let mut marked_updates = Vec::new();
        let seen_at = current_timestamp();
        for update in &repo_index.knowledge_updates {
            let key = stable_update_key(update);
            let matches_target = options.all
                || options
                    .update_id
                    .as_deref()
                    .is_some_and(|id| id == update.id || id == key);
            if matches_target {
                state.seen_updates.insert(
                    key.clone(),
                    SeenKnowledgeUpdate {
                        seen_at: seen_at.clone(),
                        source_digest: Some(update_source_digest(update)),
                    },
                );
                marked_updates.push(key);
            }
        }
        if !options.all && marked_updates.is_empty() {
            return Err(EngineError::UpdateNotFound(
                options.update_id.unwrap_or_default(),
            ));
        }
        state.last_sync_at = Some(seen_at);
        let state_path = self.agent_knowledge_state_path();
        self.write_agent_knowledge_state(&state)?;
        Ok(MarkKnowledgeUpdateSeenResponse {
            state_path,
            state,
            marked_updates,
        })
    }

    pub fn search(&self, options: SearchOptions) -> Result<SearchEngineResponse, EngineError> {
        let repo_index = self.ensure_repo_index()?;
        let local_index_path = self
            .repo_root()
            .join(LOCAL_INDEX_DIR)
            .join("tantivy")
            .join("local");
        gca_query::search_repo_index_with_engine(
            &repo_index,
            Some(&local_index_path),
            options.mode,
            &options.query,
            options.engine,
        )
        .map_err(EngineError::Search)
    }

    pub fn locate_owner(
        &self,
        options: LocateOwnerOptions,
    ) -> Result<OwnerLookupResponse, EngineError> {
        let policy = load_policy_bundle(&self.repo_root());
        gca_query::locate_owner(&policy, &options.concept_id)
            .ok_or(EngineError::OwnerNotFound(options.concept_id))
    }

    pub fn required_validations(
        &self,
        options: RequiredValidationsOptions,
    ) -> Result<RequiredValidationsEngineResponse, EngineError> {
        let policy = load_policy_bundle(&self.repo_root());
        Ok(gca_query::required_validations(&policy, &options.task))
    }

    pub fn generate_agent_files(
        &self,
        options: GenerateAgentFilesOptions,
    ) -> Result<GenerateAgentFilesResponse, EngineError> {
        let repo_root = self.repo_root();
        let repo_index = self.ensure_repo_index()?;
        let generated_files = gca_agent_files::render_generated_files(&repo_index);
        let written_paths = gca_agent_files::write_generated_files(
            &repo_root,
            &generated_files,
            options.write_root,
        )
        .map_err(|source| EngineError::Io {
            path: repo_root.display().to_string(),
            source,
        })?;
        Ok(GenerateAgentFilesResponse {
            generated_files,
            written_paths,
        })
    }

    pub fn install_github_workflow(
        &self,
        _options: InstallGithubWorkflowOptions,
    ) -> Result<InstallGithubWorkflowResponse, EngineError> {
        let repo_root = self.repo_root();
        let workflow_path =
            gca_oci::install_github_workflow(&repo_root).map_err(|source| EngineError::Io {
                path: repo_root.display().to_string(),
                source,
            })?;
        Ok(InstallGithubWorkflowResponse { workflow_path })
    }

    pub fn package_index(
        &self,
        options: PackageIndexOptions,
    ) -> Result<PackageIndexResponse, EngineError> {
        let repo_root = self.repo_root();
        let repo_index = self.ensure_repo_index()?;
        let output_root = repo_root.join(LOCAL_INDEX_DIR).join("oci");
        let tags = normalized_tags(options.tags);
        let mut packages = Vec::new();
        for tag in tags {
            packages.push(
                gca_oci::package_index(&repo_root, &repo_index, &tag, &output_root).map_err(
                    |source| EngineError::Io {
                        path: output_root.display().to_string(),
                        source,
                    },
                )?,
            );
        }
        let package = packages
            .first()
            .cloned()
            .expect("normalized tags should produce at least one package");
        Ok(PackageIndexResponse { package, packages })
    }

    pub fn publish_index(
        &self,
        options: PublishIndexOptions,
    ) -> Result<PublishIndexResponse, EngineError> {
        let repo_index = self.ensure_repo_index()?;
        let remote_root = options
            .remote_root
            .unwrap_or_else(|| self.default_remote_root());
        let tags = normalized_tags(options.tags);
        let mut published_paths = Vec::new();
        for tag in tags {
            let package_dir = self
                .repo_root()
                .join(LOCAL_INDEX_DIR)
                .join("oci")
                .join(&repo_index.repo_id)
                .join(&tag);
            if !package_dir
                .join("artifacts")
                .join("repo-index.json")
                .exists()
            {
                self.package_index(PackageIndexOptions {
                    tags: vec![tag.clone()],
                })?;
            }
            published_paths.push(
                gca_oci::publish_local_package(
                    &package_dir,
                    &remote_root,
                    &repo_index.repo_id,
                    &tag,
                )
                .map_err(|source| EngineError::Io {
                    path: remote_root.display().to_string(),
                    source,
                })?,
            );
        }
        let published_path = published_paths
            .first()
            .cloned()
            .expect("normalized tags should produce at least one publish path");
        Ok(PublishIndexResponse {
            published_path,
            published_paths,
        })
    }

    pub fn sync(&self, options: SyncOptions) -> Result<SyncResponse, EngineError> {
        let remote_root = options
            .remote_root
            .unwrap_or_else(|| self.default_remote_root());
        let cache_root = self.default_sync_cache_root();
        let indexes_root = gca_oci::default_indexes_path(&self.home_dir);
        let report = if let Some(repo_id) = options.repo_id {
            gca_oci::sync_repo_with_state(
                &remote_root,
                &cache_root,
                &indexes_root,
                &self.home_dir,
                &repo_id,
                options
                    .tag
                    .as_deref()
                    .or(options.channel.as_deref())
                    .unwrap_or("latest"),
                None,
            )
            .map_err(EngineError::Sync)?
        } else {
            gca_oci::sync_catalog_with_state(
                &remote_root,
                &cache_root,
                &indexes_root,
                &self.home_dir,
                &gca_oci::SyncCatalogOptions {
                    channel: options.channel,
                    ..Default::default()
                },
            )
            .map_err(EngineError::Sync)?
        };
        Ok(SyncResponse {
            synced_paths: report.downloaded.clone(),
            report,
        })
    }

    pub fn list_remote_repos(
        &self,
        options: ListRemoteReposOptions,
    ) -> Result<ListRemoteReposResponse, EngineError> {
        let remote_root = options
            .remote_root
            .unwrap_or_else(|| self.default_remote_root());
        let repos = gca_oci::list_remote_repos(&remote_root).map_err(|source| EngineError::Io {
            path: remote_root.display().to_string(),
            source,
        })?;
        Ok(ListRemoteReposResponse { repos })
    }

    pub fn show_catalog(
        &self,
        options: ShowCatalogOptions,
    ) -> Result<ShowCatalogResponse, EngineError> {
        let remote_root = options
            .remote_root
            .unwrap_or_else(|| self.default_remote_root());
        let catalog = gca_oci::build_catalog(&remote_root).map_err(|source| EngineError::Io {
            path: remote_root.display().to_string(),
            source,
        })?;
        Ok(ShowCatalogResponse { catalog })
    }

    pub fn rebuild_merged_index(
        &self,
        options: RebuildMergedIndexOptions,
    ) -> Result<RebuildMergedIndexResponse, EngineError> {
        let report =
            gca_oci::rebuild_merged_tantivy_index(&self.home_dir, options.tenant.as_deref())
                .map_err(EngineError::Sync)?;
        Ok(RebuildMergedIndexResponse {
            index_path: report.merged_index_path,
            repos_indexed: report.repos_indexed,
            documents_indexed: report.documents_indexed,
        })
    }

    pub fn check_refresh(
        &self,
        _options: CheckRefreshOptions,
    ) -> Result<CheckRefreshResponse, EngineError> {
        gca_oci::check_refresh(&self.repo_root()).map_err(|source| EngineError::Io {
            path: self.repo_root().display().to_string(),
            source,
        })
    }

    pub fn impact(&self, options: ImpactOptions) -> Result<ImpactResponse, EngineError> {
        let repo_index = self.ensure_repo_index()?;
        let policy = load_policy_bundle(&self.repo_root());
        Ok(gca_mcp::impact_analysis(
            &repo_index,
            &policy,
            &options.symbol,
        ))
    }

    pub fn detect_changes(
        &self,
        options: DetectChangesOptions,
    ) -> Result<DetectChangesResponse, EngineError> {
        let repo_index = self.ensure_repo_index()?;
        Ok(gca_mcp::detect_changes(&repo_index, &options.changed_files))
    }

    pub fn validate_plan(
        &self,
        options: ValidatePlanOptions,
    ) -> Result<ValidatePlanResponse, EngineError> {
        let repo_index = self.ensure_repo_index()?;
        let policy = load_policy_bundle(&self.repo_root());
        let raw = fs::read_to_string(&options.plan_path).map_err(|source| EngineError::Read {
            path: options.plan_path.display().to_string(),
            source,
        })?;
        let task = extract_plan_task(&raw);
        let acknowledged_updates = extract_acknowledged_updates(&raw);
        let lower = task.to_ascii_lowercase();
        let mut owner_hints = repo_index
            .concept_graph
            .iter()
            .filter_map(|concept| {
                let concept_phrase = concept.id.replace('_', " ");
                if lower.contains(&concept.id.to_ascii_lowercase())
                    || lower.contains(&concept_phrase)
                {
                    gca_query::locate_owner(&policy, &concept.id)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        owner_hints.sort_by(|left, right| left.concept_id.cmp(&right.concept_id));
        owner_hints.dedup_by(|left, right| left.concept_id == right.concept_id);
        let freshness_warning = self.freshness_warning();
        let mut issues = Vec::new();
        if task.trim().is_empty() {
            issues.push("plan file did not contain any extractable task text".to_string());
        }
        if freshness_warning.is_some() {
            issues.push("local index appears stale relative to the current checkout".to_string());
        }
        let knowledge_updates = gca_query::recommend_updates_for_task(&repo_index, &task);
        issues.extend(plan_update_issues(
            &repo_index,
            &raw,
            &knowledge_updates,
            &acknowledged_updates,
        ));
        Ok(ValidatePlanResponse {
            plan_path: options.plan_path,
            task_summary: task.clone(),
            owner_hints,
            validations: gca_mcp::plan_change(&policy, &repo_index, &task),
            knowledge_updates,
            acknowledged_updates,
            freshness_warning,
            issues,
        })
    }

    pub fn mcp_snapshot(
        &self,
        _options: McpSnapshotOptions,
    ) -> Result<McpSnapshotResponse, EngineError> {
        Ok(gca_mcp::server_snapshot(self.freshness_warning()))
    }

    pub fn dispatch_mcp_request(
        &self,
        options: DispatchMcpRequestOptions,
    ) -> Result<DispatchMcpRequestResponse, EngineError> {
        let repo_index = self.ensure_repo_index()?;
        let policy = load_policy_bundle(&self.repo_root());
        let remote_repos = self
            .list_remote_repos(ListRemoteReposOptions { remote_root: None })?
            .repos
            .into_iter()
            .map(|repo| gca_mcp::RemoteRepoInfo {
                repo_name: repo.repo_name,
                tags: repo.tags,
            })
            .collect::<Vec<_>>();
        let context = gca_mcp::DispatchContext {
            repo_index: &repo_index,
            policy: &policy,
            freshness_warning: self.freshness_warning(),
            remote_repos,
        };
        Ok(gca_mcp::dispatch_request(&context, options.request))
    }

    fn ensure_repo_index(&self) -> Result<RepoIndex, EngineError> {
        let path = self.repo_index_path();
        if !path.exists() {
            return Ok(self.analyze(AnalyzeOptions)?.repo_index);
        }
        read_json(&path)
    }

    fn repo_index_path(&self) -> PathBuf {
        self.repo_root()
            .join(LOCAL_INDEX_DIR)
            .join("repo-index.json")
    }

    fn repo_root(&self) -> PathBuf {
        find_repo_root(&self.cwd).unwrap_or_else(|| self.cwd.clone())
    }

    fn default_remote_root(&self) -> PathBuf {
        gca_oci::default_remote_store_path(&self.home_dir)
    }

    fn default_sync_cache_root(&self) -> PathBuf {
        gca_oci::default_sync_cache_path(&self.home_dir)
    }

    fn agent_knowledge_state_path(&self) -> PathBuf {
        self.home_dir
            .join(LOCAL_INDEX_DIR)
            .join(AGENT_KNOWLEDGE_STATE_FILE)
    }

    fn load_agent_knowledge_state(&self) -> Result<AgentKnowledgeState, EngineError> {
        let path = self.agent_knowledge_state_path();
        if !path.exists() {
            return Ok(empty_agent_knowledge_state());
        }
        read_json(&path)
    }

    fn write_agent_knowledge_state(&self, state: &AgentKnowledgeState) -> Result<(), EngineError> {
        let path = self.agent_knowledge_state_path();
        write_json(&path, state)
    }

    fn freshness_warning(&self) -> Option<String> {
        match gca_oci::check_refresh(&self.repo_root()) {
            Ok(refresh) if refresh.needs_refresh => Some(refresh.reasons.join("; ")),
            _ => None,
        }
    }
}

fn parse_optional_audience(value: Option<String>) -> Result<Option<TrainingAudience>, EngineError> {
    value
        .as_deref()
        .map(TrainingAudience::parse)
        .transpose()
        .map_err(EngineError::InvalidInput)
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, EngineError> {
    let raw = fs::read_to_string(path).map_err(|source| EngineError::Read {
        path: path.display().to_string(),
        source,
    })?;
    serde_json::from_str(&raw).map_err(|source| EngineError::Parse {
        path: path.display().to_string(),
        source,
    })
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), EngineError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| EngineError::Write {
            path: parent.display().to_string(),
            source,
        })?;
    }
    let raw = serde_json::to_string_pretty(value).map_err(|source| EngineError::Parse {
        path: path.display().to_string(),
        source,
    })?;
    fs::write(path, raw).map_err(|source| EngineError::Write {
        path: path.display().to_string(),
        source,
    })
}

fn find_repo_root(start: &Path) -> Option<PathBuf> {
    for candidate in start.ancestors() {
        if candidate.join(".git").exists() && candidate.join("Cargo.toml").exists() {
            return Some(candidate.to_path_buf());
        }
    }
    None
}

fn extract_plan_task(raw: &str) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return raw.to_string();
    };
    value
        .get("task")
        .or_else(|| value.get("summary"))
        .or_else(|| value.get("description"))
        .and_then(|value| value.as_str())
        .unwrap_or(raw)
        .to_string()
}

fn empty_agent_knowledge_state() -> AgentKnowledgeState {
    AgentKnowledgeState {
        version: AGENT_KNOWLEDGE_STATE_VERSION.to_string(),
        last_sync_at: None,
        seen_updates: Default::default(),
    }
}

fn is_update_unseen(state: &AgentKnowledgeState, update: &KnowledgeUpdateDescriptor) -> bool {
    let key = stable_update_key(update);
    let digest = update_source_digest(update);
    state
        .seen_updates
        .get(&key)
        .and_then(|seen| seen.source_digest.as_deref())
        != Some(digest.as_str())
}

fn stable_update_key(update: &KnowledgeUpdateDescriptor) -> String {
    format!("{}::{}", update.owner_repo, update.id)
}

fn update_source_digest(update: &KnowledgeUpdateDescriptor) -> String {
    let raw = serde_json::to_string(update).unwrap_or_else(|_| update.id.clone());
    format!("fnv64:{:016x}", fnv64(raw.as_bytes()))
}

fn fnv64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn current_timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    format!("unix:{seconds}")
}

fn extract_acknowledged_updates(raw: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return Vec::new();
    };
    value
        .get("acknowledged_updates")
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn plan_update_issues(
    repo_index: &RepoIndex,
    raw_plan: &str,
    updates: &[KnowledgeUpdateDescriptor],
    acknowledged_updates: &[String],
) -> Vec<String> {
    let lower_plan = raw_plan.to_ascii_lowercase();
    let mut issues = Vec::new();
    for update in updates {
        let matched_deprecated = update.deprecated_commands.iter().any(|command| {
            lower_plan.contains(&command.command.to_ascii_lowercase())
                || command.replacement.as_ref().is_some_and(|replacement| {
                    lower_plan.contains(&replacement.to_ascii_lowercase())
                })
        });
        let matched_guidance = update.replaced_guidance.iter().any(|guidance| {
            lower_plan.contains(&guidance.old_guidance.to_ascii_lowercase())
                || lower_plan.contains(&guidance.replacement_guidance.to_ascii_lowercase())
        });
        if !matched_deprecated && !matched_guidance {
            continue;
        }
        let ack_key = format!("{}::{}", repo_index.repo_id, update.id);
        let acknowledged = acknowledged_updates
            .iter()
            .any(|value| value == &update.id || value == &ack_key);
        let label = update.severity.as_str();
        if update.severity.rank() >= KnowledgeUpdateSeverity::Breaking.rank() && !acknowledged {
            issues.push(format!(
                "{label} knowledge update `{}` affects this plan and must be acknowledged as `{ack_key}`",
                update.id
            ));
        } else if update.severity.rank() >= KnowledgeUpdateSeverity::Recommended.rank() {
            issues.push(format!(
                "{label} knowledge update `{}` affects this plan",
                update.id
            ));
        } else {
            issues.push(format!(
                "note: knowledge update `{}` affects this plan",
                update.id
            ));
        }
    }
    issues.sort();
    issues.dedup();
    issues
}

fn normalized_tags(tags: Vec<String>) -> Vec<String> {
    let mut tags = if tags.is_empty() {
        vec!["latest".to_string()]
    } else {
        tags
    };
    tags.retain(|tag| !tag.trim().is_empty());
    for tag in &mut tags {
        *tag = tag.trim().to_string();
    }
    tags.sort();
    tags.dedup();
    if tags.is_empty() {
        tags.push("latest".to_string());
    }
    tags
}
