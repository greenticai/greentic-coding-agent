use gca_core::{RepoIndex, TrainingAudience, ValidationDescriptor};
use gca_query::{
    PolicyBundle, SearchMode, list_training_courses, locate_owner, recommend_training_courses,
    recommend_updates_for_task, required_validations, search_repo_index, show_knowledge_update,
    show_training_course,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServerSnapshot {
    pub protocol: String,
    pub tools: Vec<McpTool>,
    pub freshness_warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactAnalysis {
    pub symbol: String,
    pub confidence: String,
    pub provenance: Vec<String>,
    pub concepts: Vec<String>,
    pub workflows: Vec<String>,
    pub validations: Vec<String>,
    pub owner_repos: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteRepoInfo {
    pub repo_name: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeSummary {
    pub changed_files: Vec<String>,
    pub likely_concepts: Vec<String>,
    pub likely_workflows: Vec<String>,
    pub suggested_validations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpRequest {
    pub id: Option<String>,
    pub tool: String,
    #[serde(default)]
    pub arguments: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpResponse {
    pub id: Option<String>,
    pub ok: bool,
    pub result: Option<Value>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DispatchContext<'a> {
    pub repo_index: &'a RepoIndex,
    pub policy: &'a PolicyBundle,
    pub freshness_warning: Option<String>,
    pub remote_repos: Vec<RemoteRepoInfo>,
}

pub fn tool_definitions() -> Vec<McpTool> {
    vec![
        tool("describe_repo", "Summarize the indexed repository state."),
        tool(
            "list_workflows",
            "List inferred workflows for the repository.",
        ),
        tool(
            "explain_concept",
            "Show details and ownership for a concept.",
        ),
        tool("search_code", "Search indexed code metadata."),
        tool(
            "search_instructions",
            "Search indexed docs and instructions.",
        ),
        tool("search_reuse", "Search seeded reuse and ownership policy."),
        tool("search_all", "Search all indexed local result types."),
        tool(
            "list_training_courses",
            "List repo-authored training courses.",
        ),
        tool("show_training_course", "Show a training course by ID."),
        tool(
            "recommend_training_courses",
            "Recommend training courses for a task.",
        ),
        tool(
            "list_knowledge_updates",
            "List repo-authored knowledge updates.",
        ),
        tool("show_knowledge_update", "Show a knowledge update by ID."),
        tool(
            "recommend_knowledge_updates",
            "Recommend knowledge updates for a task.",
        ),
        tool(
            "list_new_knowledge_updates",
            "List knowledge updates not yet marked seen by the transport-managed state.",
        ),
        tool(
            "mark_knowledge_update_seen",
            "Mark a knowledge update seen in transport-managed state.",
        ),
        tool("locate_owner", "Find the owner repo for a concept."),
        tool(
            "locate_extension_point",
            "Find likely extension points for a concept or task.",
        ),
        tool(
            "plan_change",
            "Validate a proposed plan against repo policy.",
        ),
        tool(
            "required_validations",
            "List validations implied by a task.",
        ),
        tool(
            "impact_analysis",
            "Estimate blast radius for a symbol or concept.",
        ),
        tool(
            "detect_changes",
            "Summarize changed files and likely affected areas.",
        ),
        tool("show_freshness", "Report whether the local index is stale."),
        tool(
            "list_indexed_repos",
            "List repos currently available in the local index cache.",
        ),
        tool("sync_indexes", "Refresh local index cache metadata."),
        tool("show_catalog", "Show catalog metadata."),
        tool("gca.search", "Search indexed knowledge."),
        tool("gca.agent_context", "Build task context for an agent."),
        tool("gca.find_owner", "Find owner candidates for a concept."),
        tool(
            "gca.required_validations",
            "List validations implied by a task.",
        ),
        tool("gca.recent_updates", "Find recent updates for a task."),
        tool("gca.branch_status", "Report indexed branch status."),
    ]
}

pub fn server_snapshot(freshness_warning: Option<String>) -> McpServerSnapshot {
    McpServerSnapshot {
        protocol: "mcp-lite-v1".to_string(),
        tools: tool_definitions(),
        freshness_warning,
    }
}

pub fn dispatch_request(context: &DispatchContext<'_>, request: McpRequest) -> McpResponse {
    let McpRequest {
        id,
        tool,
        arguments,
    } = request;

    let outcome = match tool.as_str() {
        "describe_repo" => {
            serde_json::to_value(context.repo_index).map_err(|error| error.to_string())
        }
        "list_workflows" => serde_json::to_value(&context.repo_index.workflow_graph)
            .map_err(|error| error.to_string()),
        "explain_concept" => {
            let Some(concept_id) = arguments.get("concept_id").and_then(Value::as_str) else {
                return error_response(id, "missing `concept_id` argument");
            };
            let concept = context
                .repo_index
                .concept_graph
                .iter()
                .find(|entry| entry.id == concept_id)
                .cloned();
            let owner = locate_owner(context.policy, concept_id);
            serde_json::to_value(serde_json::json!({
                "concept": concept,
                "owner": owner
            }))
            .map_err(|error| error.to_string())
        }
        "search_code" => search_value(context.repo_index, SearchMode::Code, &arguments),
        "gca.search" => {
            let Some(query) = arguments.get("query").and_then(Value::as_str) else {
                return error_response(id, "missing `query` argument");
            };
            let mode = match arguments.get("mode").and_then(Value::as_str) {
                Some("code") => SearchMode::Code,
                Some("instruction") | None => SearchMode::Instruction,
                Some("concept") => SearchMode::Concept,
                Some("reuse") => SearchMode::Reuse,
                Some("course") => SearchMode::Course,
                Some("update") => SearchMode::Update,
                Some(other) => {
                    return error_response(id, &format!("unsupported search mode: {other}"));
                }
            };
            serde_json::to_value(search_repo_index(context.repo_index, mode, query))
                .map_err(|error| error.to_string())
        }
        "search_instructions" => {
            search_value(context.repo_index, SearchMode::Instruction, &arguments)
        }
        "search_reuse" => search_value(context.repo_index, SearchMode::Reuse, &arguments),
        "search_all" => {
            let Some(query) = arguments.get("query").and_then(Value::as_str) else {
                return error_response(id, "missing `query` argument");
            };
            serde_json::to_value([
                search_repo_index(context.repo_index, SearchMode::Code, query),
                search_repo_index(context.repo_index, SearchMode::Instruction, query),
                search_repo_index(context.repo_index, SearchMode::Concept, query),
                search_repo_index(context.repo_index, SearchMode::Reuse, query),
                search_repo_index(context.repo_index, SearchMode::Course, query),
                search_repo_index(context.repo_index, SearchMode::Update, query),
            ])
            .map_err(|error| error.to_string())
        }
        "list_training_courses" => serde_json::to_value(list_training_courses(context.repo_index))
            .map_err(|error| error.to_string()),
        "show_training_course" => {
            let Some(course_id) = arguments.get("course_id").and_then(Value::as_str) else {
                return error_response(id, "missing `course_id` argument");
            };
            serde_json::to_value(show_training_course(context.repo_index, course_id))
                .map_err(|error| error.to_string())
        }
        "recommend_training_courses" => {
            let Some(task) = arguments.get("task").and_then(Value::as_str) else {
                return error_response(id, "missing `task` argument");
            };
            let audience = match arguments.get("audience").and_then(Value::as_str) {
                Some(value) => match TrainingAudience::parse(value) {
                    Ok(audience) => Some(audience),
                    Err(error) => return error_response(id, &error),
                },
                None => None,
            };
            serde_json::to_value(recommend_training_courses(
                context.repo_index,
                task,
                audience,
            ))
            .map_err(|error| error.to_string())
        }
        "list_knowledge_updates" => serde_json::to_value(&context.repo_index.knowledge_updates)
            .map_err(|error| error.to_string()),
        "show_knowledge_update" => {
            let Some(update_id) = arguments.get("update_id").and_then(Value::as_str) else {
                return error_response(id, "missing `update_id` argument");
            };
            serde_json::to_value(show_knowledge_update(context.repo_index, update_id))
                .map_err(|error| error.to_string())
        }
        "recommend_knowledge_updates" => {
            let Some(task) = arguments.get("task").and_then(Value::as_str) else {
                return error_response(id, "missing `task` argument");
            };
            serde_json::to_value(recommend_updates_for_task(context.repo_index, task))
                .map_err(|error| error.to_string())
        }
        "list_new_knowledge_updates" => serde_json::to_value(&context.repo_index.knowledge_updates)
            .map_err(|error| error.to_string()),
        "mark_knowledge_update_seen" => {
            let Some(update_id) = arguments.get("update_id").and_then(Value::as_str) else {
                return error_response(id, "missing `update_id` argument");
            };
            serde_json::to_value(serde_json::json!({
                "update_id": update_id,
                "message": "seen-state is managed by the host transport"
            }))
            .map_err(|error| error.to_string())
        }
        "locate_owner" => {
            let Some(concept_id) = arguments.get("concept_id").and_then(Value::as_str) else {
                return error_response(id, "missing `concept_id` argument");
            };
            serde_json::to_value(locate_owner(context.policy, concept_id))
                .map_err(|error| error.to_string())
        }
        "gca.agent_context" => {
            let Some(task) = arguments.get("task").and_then(Value::as_str) else {
                return error_response(id, "missing `task` argument");
            };
            serde_json::to_value(serde_json::json!({
                "task": task,
                "channel": arguments.get("channel").and_then(Value::as_str).unwrap_or("develop"),
                "relevant_repos": [{
                    "repo_id": context.repo_index.repo_id,
                    "branch": context.repo_index.metadata.as_ref().and_then(|metadata| metadata.branch.as_deref()),
                    "source": "local",
                    "matched": []
                }],
                "owner_candidates": [],
                "required_validations": required_validations(context.policy, task),
                "recent_updates": recommend_updates_for_task(context.repo_index, task),
                "tutorials": recommend_training_courses(context.repo_index, task, None),
                "warnings": []
            }))
            .map_err(|error| error.to_string())
        }
        "gca.find_owner" => {
            let Some(concept_id) = arguments
                .get("concept")
                .or_else(|| arguments.get("concept_id"))
                .and_then(Value::as_str)
            else {
                return error_response(id, "missing `concept` argument");
            };
            serde_json::to_value(serde_json::json!({
                "concept": concept_id,
                "channel": arguments.get("channel").and_then(Value::as_str).unwrap_or("develop"),
                "owner_candidates": locate_owner(context.policy, concept_id).into_iter().collect::<Vec<_>>(),
                "warnings": []
            }))
            .map_err(|error| error.to_string())
        }
        "locate_extension_point" => {
            search_value(context.repo_index, SearchMode::Instruction, &arguments)
        }
        "plan_change" => {
            let Some(task) = arguments.get("task").and_then(Value::as_str) else {
                return error_response(id, "missing `task` argument");
            };
            serde_json::to_value(plan_change(context.policy, context.repo_index, task))
                .map_err(|error| error.to_string())
        }
        "required_validations" => {
            let Some(task) = arguments.get("task").and_then(Value::as_str) else {
                return error_response(id, "missing `task` argument");
            };
            serde_json::to_value(required_validations(context.policy, task))
                .map_err(|error| error.to_string())
        }
        "gca.required_validations" => {
            let Some(task) = arguments.get("task").and_then(Value::as_str) else {
                return error_response(id, "missing `task` argument");
            };
            serde_json::to_value(serde_json::json!({
                "task": task,
                "channel": arguments.get("channel").and_then(Value::as_str).unwrap_or("develop"),
                "validations": required_validations(context.policy, task),
                "warnings": []
            }))
            .map_err(|error| error.to_string())
        }
        "gca.recent_updates" => {
            let Some(task) = arguments.get("task").and_then(Value::as_str) else {
                return error_response(id, "missing `task` argument");
            };
            serde_json::to_value(serde_json::json!({
                "task": task,
                "channel": arguments.get("channel").and_then(Value::as_str).unwrap_or("develop"),
                "updates": recommend_updates_for_task(context.repo_index, task),
                "warnings": []
            }))
            .map_err(|error| error.to_string())
        }
        "impact_analysis" => {
            let Some(symbol) = arguments.get("symbol").and_then(Value::as_str) else {
                return error_response(id, "missing `symbol` argument");
            };
            serde_json::to_value(impact_analysis(context.repo_index, context.policy, symbol))
                .map_err(|error| error.to_string())
        }
        "detect_changes" => {
            let changed_files = arguments
                .get("changed_files")
                .and_then(Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let summary = detect_changes(context.repo_index, &changed_files);
            serde_json::to_value(summary).map_err(|error| error.to_string())
        }
        "show_freshness" => serde_json::to_value(serde_json::json!({
            "freshness_warning": context.freshness_warning
        }))
        .map_err(|error| error.to_string()),
        "list_indexed_repos" | "list_remote_repos" => {
            serde_json::to_value(&context.remote_repos).map_err(|error| error.to_string())
        }
        "sync_indexes" => serde_json::to_value(serde_json::json!({
            "ok": true,
            "message": "sync is transport-managed"
        }))
        .map_err(|error| error.to_string()),
        "show_catalog" => serde_json::to_value(serde_json::json!({
            "repos": context.remote_repos
        }))
        .map_err(|error| error.to_string()),
        "gca.branch_status" => serde_json::to_value(serde_json::json!({
            "ok": true,
            "repos": context.remote_repos,
            "freshness_warning": context.freshness_warning
        }))
        .map_err(|error| error.to_string()),
        other => return error_response(id, &format!("unknown tool: {other}")),
    };

    match outcome {
        Ok(result) => McpResponse {
            id,
            ok: true,
            result: Some(result),
            error: None,
        },
        Err(error) => error_response(id, &error),
    }
}

pub fn impact_analysis(
    repo_index: &RepoIndex,
    policy: &PolicyBundle,
    symbol: &str,
) -> ImpactAnalysis {
    let query = symbol.trim();
    let lower = query.to_ascii_lowercase();
    let mut provenance = Vec::new();
    let mut concepts = Vec::new();
    let mut workflows = Vec::new();
    let mut validations = Vec::new();
    let mut owner_repos = Vec::new();

    for concept in &repo_index.concept_graph {
        let exact =
            concept.id.eq_ignore_ascii_case(query) || concept.title.eq_ignore_ascii_case(query);
        let fuzzy = concept.summary.to_ascii_lowercase().contains(&lower)
            || concept.id.to_ascii_lowercase().contains(&lower)
            || concept.title.to_ascii_lowercase().contains(&lower);
        if exact || fuzzy {
            concepts.push(concept.id.clone());
            provenance.push(format!("concept:{}", concept.id));
            if let Some(owner) = locate_owner(policy, &concept.id) {
                owner_repos.push(owner.owner_repo);
            }
        }
    }

    for workflow in &repo_index.workflow_graph {
        if workflow.id.to_ascii_lowercase().contains(&lower)
            || workflow.title.to_ascii_lowercase().contains(&lower)
            || workflow.summary.to_ascii_lowercase().contains(&lower)
            || workflow
                .commands
                .iter()
                .any(|command| command.to_ascii_lowercase().contains(&lower))
        {
            workflows.push(workflow.id.clone());
            provenance.push(format!("workflow:{}", workflow.id));
        }
    }

    for validation in &repo_index.validations {
        if validation.id.to_ascii_lowercase().contains(&lower)
            || validation.title.to_ascii_lowercase().contains(&lower)
            || validation.summary.to_ascii_lowercase().contains(&lower)
        {
            validations.push(validation.id.clone());
            provenance.push(format!("validation:{}", validation.id));
        }
    }

    for item in &repo_index.source_stats.public_items {
        if item.to_ascii_lowercase().contains(&lower) {
            provenance.push(format!("public_item:{item}"));
        }
    }
    for item in &repo_index.source_stats.modules {
        if item.to_ascii_lowercase().contains(&lower) {
            provenance.push(format!("module:{item}"));
        }
    }

    let confidence = if concepts.iter().any(|id| id.eq_ignore_ascii_case(query))
        || workflows.iter().any(|id| id.eq_ignore_ascii_case(query))
    {
        "high"
    } else if !provenance.is_empty() {
        "medium"
    } else {
        "low"
    };

    concepts.sort();
    concepts.dedup();
    workflows.sort();
    workflows.dedup();
    validations.sort();
    validations.dedup();
    owner_repos.sort();
    owner_repos.dedup();
    provenance.sort();
    provenance.dedup();

    ImpactAnalysis {
        symbol: query.to_string(),
        confidence: confidence.to_string(),
        provenance,
        concepts,
        workflows,
        validations,
        owner_repos,
    }
}

pub fn plan_change(
    policy: &PolicyBundle,
    repo_index: &RepoIndex,
    task: &str,
) -> Vec<ValidationDescriptor> {
    required_validations(policy, task)
        .validations
        .into_iter()
        .chain(
            search_repo_index(repo_index, SearchMode::Reuse, task)
                .results
                .into_iter()
                .filter_map(|result| {
                    repo_index
                        .validations
                        .iter()
                        .find(|entry| result.snippet.contains(&entry.id))
                        .cloned()
                }),
        )
        .collect()
}

pub fn detect_changes(repo_index: &RepoIndex, changed_files: &[String]) -> ChangeSummary {
    let mut likely_concepts = Vec::new();
    let mut likely_workflows = Vec::new();

    for path in changed_files {
        let lower = path.to_ascii_lowercase();
        for concept in &repo_index.concept_graph {
            if concept.related_paths.iter().any(|related| related == path)
                || lower.contains(&concept.id.to_ascii_lowercase())
            {
                likely_concepts.push(concept.id.clone());
            }
        }
        for workflow in &repo_index.workflow_graph {
            if workflow.docs.iter().any(|doc| doc == path)
                || lower.contains(&workflow.id.to_ascii_lowercase())
            {
                likely_workflows.push(workflow.id.clone());
            }
        }
    }

    likely_concepts.sort();
    likely_concepts.dedup();
    likely_workflows.sort();
    likely_workflows.dedup();

    let task = changed_files.join(" ");
    let suggested_validations = required_validations(
        &PolicyBundle {
            validations: repo_index.validations.clone(),
            reuse: repo_index.reuse.clone(),
        },
        &task,
    )
    .validations
    .into_iter()
    .map(|validation| validation.id)
    .collect();

    ChangeSummary {
        changed_files: changed_files.to_vec(),
        likely_concepts,
        likely_workflows,
        suggested_validations,
    }
}

fn search_value(
    repo_index: &RepoIndex,
    mode: SearchMode,
    arguments: &Value,
) -> Result<Value, String> {
    let Some(query) = arguments.get("query").and_then(Value::as_str) else {
        return Err("missing `query` argument".to_string());
    };
    serde_json::to_value(search_repo_index(repo_index, mode, query))
        .map_err(|error| error.to_string())
}

fn error_response(id: Option<String>, message: &str) -> McpResponse {
    McpResponse {
        id,
        ok: false,
        result: None,
        error: Some(message.to_string()),
    }
}

fn tool(name: &str, description: &str) -> McpTool {
    McpTool {
        name: name.to_string(),
        description: description.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DispatchContext, McpRequest, RemoteRepoInfo, detect_changes, dispatch_request,
        impact_analysis, plan_change, server_snapshot, tool_definitions,
    };
    use gca_core::{
        ConceptDescriptor, FreshnessStatus, KnowledgeScope, LifecyclePhase, RepoAgentManifest,
        RepoIndex, RepoRole, ReuseDescriptor, SourceStats, ValidationDescriptor,
        WorkflowDescriptor,
    };
    use gca_query::PolicyBundle;

    #[test]
    fn snapshot_lists_expected_tools() {
        let snapshot = server_snapshot(Some("index is stale".to_string()));
        assert_eq!(snapshot.protocol, "mcp-lite-v1");
        assert!(
            snapshot
                .tools
                .iter()
                .any(|tool| tool.name == "impact_analysis")
        );
        assert_eq!(
            snapshot.freshness_warning.as_deref(),
            Some("index is stale")
        );
        assert_eq!(tool_definitions().len(), 31);
        assert!(snapshot.tools.iter().any(|tool| tool.name == "search_all"));
        assert!(snapshot.tools.iter().any(|tool| tool.name == "gca.search"));
        assert!(
            snapshot
                .tools
                .iter()
                .any(|tool| tool.name == "gca.agent_context")
        );
        assert!(
            snapshot
                .tools
                .iter()
                .any(|tool| tool.name == "sync_indexes")
        );
    }

    #[test]
    fn impact_analysis_reports_related_areas() {
        let repo_index = demo_repo_index();
        let policy = demo_policy();

        let impact = impact_analysis(&repo_index, &policy, "wizard");
        assert_eq!(impact.confidence, "high");
        assert!(impact.concepts.contains(&"wizard".to_string()));
        assert!(impact.workflows.contains(&"wizard_bootstrap".to_string()));
        assert!(impact.owner_repos.contains(&"greentic-wizard".to_string()));
    }

    #[test]
    fn plan_change_uses_required_validations() {
        let repo_index = demo_repo_index();
        let policy = demo_policy();

        let validations = plan_change(&policy, &repo_index, "update wizard setup schema");
        assert!(
            validations
                .iter()
                .any(|validation| validation.id == "setup_runtime_schema_change")
        );
    }

    #[test]
    fn request_dispatch_searches_and_reports_freshness() {
        let repo_index = demo_repo_index();
        let policy = demo_policy();
        let context = DispatchContext {
            repo_index: &repo_index,
            policy: &policy,
            freshness_warning: Some("index is stale".to_string()),
            remote_repos: vec![],
        };

        let response = dispatch_request(
            &context,
            McpRequest {
                id: Some("1".to_string()),
                tool: "search_code".to_string(),
                arguments: serde_json::json!({ "query": "wizard" }),
            },
        );
        assert!(response.ok);
        assert!(
            response
                .result
                .unwrap()
                .to_string()
                .contains("\"mode\":\"code\"")
        );

        let freshness = dispatch_request(
            &context,
            McpRequest {
                id: Some("2".to_string()),
                tool: "show_freshness".to_string(),
                arguments: serde_json::json!({}),
            },
        );
        assert!(freshness.ok);
        assert!(
            freshness
                .result
                .unwrap()
                .to_string()
                .contains("index is stale")
        );
    }

    #[test]
    fn request_dispatch_accepts_stable_gca_tool_aliases() {
        let repo_index = demo_repo_index();
        let policy = demo_policy();
        let context = DispatchContext {
            repo_index: &repo_index,
            policy: &policy,
            freshness_warning: None,
            remote_repos: vec![RemoteRepoInfo {
                repo_name: "greenticai/demo-repo".to_string(),
                tags: vec!["develop".to_string()],
            }],
        };

        let search = dispatch_request(
            &context,
            McpRequest {
                id: Some("search".to_string()),
                tool: "gca.search".to_string(),
                arguments: serde_json::json!({ "query": "wizard", "mode": "concept" }),
            },
        );
        assert!(search.ok);
        assert!(search.result.unwrap().to_string().contains("\"wizard\""));

        let context_response = dispatch_request(
            &context,
            McpRequest {
                id: Some("context".to_string()),
                tool: "gca.agent_context".to_string(),
                arguments: serde_json::json!({ "task": "update wizard setup schema" }),
            },
        );
        assert!(context_response.ok);
        let context_json = context_response.result.unwrap().to_string();
        assert!(context_json.contains("\"channel\":\"develop\""));
        assert!(context_json.contains("setup_runtime_schema_change"));

        let branch_status = dispatch_request(
            &context,
            McpRequest {
                id: Some("status".to_string()),
                tool: "gca.branch_status".to_string(),
                arguments: serde_json::json!({}),
            },
        );
        assert!(branch_status.ok);
        assert!(
            branch_status
                .result
                .unwrap()
                .to_string()
                .contains("develop")
        );
    }

    #[test]
    fn detect_changes_maps_files_to_workflows() {
        let summary = detect_changes(
            &demo_repo_index(),
            &[
                "README.md".to_string(),
                ".github/workflows/ci.yml".to_string(),
            ],
        );
        assert!(summary.likely_concepts.contains(&"wizard".to_string()));
        assert!(
            summary
                .likely_workflows
                .contains(&"wizard_bootstrap".to_string())
        );
        assert_eq!(summary.changed_files.len(), 2);
    }

    fn demo_repo_index() -> RepoIndex {
        RepoIndex {
            version: "v1".to_string(),
            repo_id: "greenticai/demo-repo".to_string(),
            repo_name: "demo-repo".to_string(),
            repo_role: RepoRole::CliLauncher,
            generated_at: "2026-04-15T00:00:00Z".to_string(),
            metadata: None,
            freshness: FreshnessStatus::Fresh,
            manifest: RepoAgentManifest {
                version: "v1".to_string(),
                repo_id: "greenticai/demo-repo".to_string(),
                repo_name: "demo-repo".to_string(),
                org: Some("greenticai".to_string()),
                repo_root: "/tmp/demo-repo".to_string(),
                repo_role: RepoRole::CliLauncher,
                primary_language: "rust".to_string(),
                generated_at: "2026-04-15T00:00:00Z".to_string(),
                candidate_docs: vec!["README.md".to_string()],
                cargo_manifests: vec!["Cargo.toml".to_string()],
            },
            concept_graph: vec![ConceptDescriptor {
                id: "wizard".to_string(),
                title: "Wizard".to_string(),
                summary: "Wizard setup flow.".to_string(),
                scope: KnowledgeScope::CrossRepo,
                lifecycle_phase: LifecyclePhase::Setup,
                owners: vec!["greentic-wizard".to_string()],
                related_paths: vec!["README.md".to_string()],
            }],
            workflow_graph: vec![WorkflowDescriptor {
                id: "wizard_bootstrap".to_string(),
                title: "Wizard bootstrap".to_string(),
                summary: "Bootstrap repo setup through wizard commands.".to_string(),
                phase: LifecyclePhase::Setup,
                commands: vec!["gtc wizard --schema".to_string()],
                docs: vec!["README.md".to_string()],
                concept_ids: vec!["wizard".to_string()],
            }],
            validations: vec![ValidationDescriptor {
                id: "setup_runtime_schema_change".to_string(),
                title: "Setup runtime schema change".to_string(),
                summary: "Run setup validations.".to_string(),
                phase: LifecyclePhase::Setup,
                command_groups: vec!["bash ci/local_check.sh".to_string()],
                triggered_by: vec!["setup".to_string(), "schema".to_string()],
            }],
            reuse: vec![ReuseDescriptor {
                id: "wizard_owner".to_string(),
                concept_id: "wizard".to_string(),
                owner_repo: "greentic-wizard".to_string(),
                rationale: "Wizard contracts belong in greentic-wizard.".to_string(),
                forbidden_locations: vec!["customer-solution".to_string()],
                required_validations: vec!["setup_runtime_schema_change".to_string()],
            }],
            training_courses: Vec::new(),
            knowledge_updates: Vec::new(),
            instruction_graph: Vec::new(),
            instruction_paths: Vec::new(),
            source_stats: SourceStats {
                public_items: vec!["wizard::bootstrap".to_string()],
                modules: vec!["wizard".to_string()],
                ..SourceStats::default()
            },
        }
    }

    fn demo_policy() -> PolicyBundle {
        PolicyBundle {
            validations: vec![ValidationDescriptor {
                id: "setup_runtime_schema_change".to_string(),
                title: "Setup runtime schema change".to_string(),
                summary: "Run setup validations.".to_string(),
                phase: LifecyclePhase::Setup,
                command_groups: vec!["bash ci/local_check.sh".to_string()],
                triggered_by: vec!["setup".to_string(), "schema".to_string()],
            }],
            reuse: vec![ReuseDescriptor {
                id: "wizard_owner".to_string(),
                concept_id: "wizard".to_string(),
                owner_repo: "greentic-wizard".to_string(),
                rationale: "Wizard contracts belong in greentic-wizard.".to_string(),
                forbidden_locations: vec!["customer-solution".to_string()],
                required_validations: vec!["setup_runtime_schema_change".to_string()],
            }],
        }
    }
}
