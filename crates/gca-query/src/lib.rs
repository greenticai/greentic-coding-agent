mod tantivy_search;

use gca_core::{
    FreshnessStatus, KnowledgeUpdateDescriptor, KnowledgeUpdateSeverity, KnowledgeUpdateType,
    LifecyclePhase, RepoIndex, ReuseDescriptor, TrainingAudience, TrainingCourseDescriptor,
    ValidationDescriptor,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub use tantivy_search::{SearchEngineChoice, search_tantivy_index};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchMode {
    Code,
    Instruction,
    Concept,
    Reuse,
    Course,
    Update,
}

impl SearchMode {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "code" => Ok(Self::Code),
            "instruction" => Ok(Self::Instruction),
            "concept" => Ok(Self::Concept),
            "reuse" => Ok(Self::Reuse),
            "course" => Ok(Self::Course),
            "update" => Ok(Self::Update),
            other => Err(format!("unsupported search mode: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchResultType {
    Code,
    Instruction,
    Concept,
    Reuse,
    Course,
    Update,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResult {
    pub repo_id: String,
    pub id: String,
    pub title: String,
    pub result_type: SearchResultType,
    pub locator: String,
    pub snippet: String,
    pub provenance: String,
    pub freshness: FreshnessStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResponse {
    pub mode: SearchMode,
    pub query: String,
    pub results: Vec<SearchResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandCatalogEntry {
    pub command: String,
    pub purpose: String,
    pub phase: LifecyclePhase,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub when_to_use: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PolicyBundle {
    pub validations: Vec<ValidationDescriptor>,
    pub reuse: Vec<ReuseDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerLookup {
    pub concept_id: String,
    pub owner_repo: String,
    pub rationale: String,
    pub forbidden_locations: Vec<String>,
    pub required_validations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequiredValidationsResponse {
    pub task: String,
    pub validations: Vec<ValidationDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingRecommendation {
    pub course: TrainingCourseDescriptor,
    pub score: u32,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UpdateFilter {
    pub task: Option<String>,
    pub concept: Option<String>,
    pub severity: Option<KnowledgeUpdateSeverity>,
    pub update_type: Option<KnowledgeUpdateType>,
}

pub trait SearchEngine {
    fn search(&self, mode: SearchMode, query: &str) -> Result<SearchResponse, String>;
}

pub struct FallbackSearchEngine<'a> {
    pub repo_index: &'a RepoIndex,
}

impl SearchEngine for FallbackSearchEngine<'_> {
    fn search(&self, mode: SearchMode, query: &str) -> Result<SearchResponse, String> {
        Ok(search_repo_index(self.repo_index, mode, query))
    }
}

pub struct TantivySearchEngine<'a> {
    pub index_dir: &'a Path,
}

impl SearchEngine for TantivySearchEngine<'_> {
    fn search(&self, mode: SearchMode, query: &str) -> Result<SearchResponse, String> {
        search_tantivy_index(self.index_dir, mode, query)
    }
}

pub fn search_repo_index_with_engine(
    repo_index: &RepoIndex,
    tantivy_index_dir: Option<&Path>,
    mode: SearchMode,
    query: &str,
    engine: SearchEngineChoice,
) -> Result<SearchResponse, String> {
    match engine {
        SearchEngineChoice::Fallback => FallbackSearchEngine { repo_index }.search(mode, query),
        SearchEngineChoice::Tantivy => {
            let Some(index_dir) = tantivy_index_dir else {
                return Err("tantivy index path was not provided".to_string());
            };
            TantivySearchEngine { index_dir }.search(mode, query)
        }
        SearchEngineChoice::Auto => {
            if let Some(index_dir) = tantivy_index_dir
                && index_dir.exists()
                && let Ok(response) = (TantivySearchEngine { index_dir }).search(mode, query)
            {
                return Ok(response);
            }
            FallbackSearchEngine { repo_index }.search(mode, query)
        }
    }
}

pub fn command_catalog() -> Vec<CommandCatalogEntry> {
    vec![
        CommandCatalogEntry {
            command: "greentic-coding-agent analyze".to_string(),
            purpose:
                "Generate repo-local coding-agent metadata and refresh the global registry entry."
                    .to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Current repository checkout".to_string()],
            outputs: vec![
                ".greentic-agent/manifest.json".to_string(),
                ".greentic-agent/repo-index.json".to_string(),
                ".greentic-agent/fingerprints.json".to_string(),
            ],
            when_to_use: "Before querying a repo or after meaningful code changes.".to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent describe --here".to_string(),
            purpose: "Summarize the current repo and local index state.".to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Current working directory".to_string()],
            outputs: vec!["Repo summary".to_string()],
            when_to_use: "When an agent needs quick repo orientation.".to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent concepts".to_string(),
            purpose: "List inferred Greentic concepts for the current repo.".to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Local repo index".to_string()],
            outputs: vec!["Concept graph".to_string()],
            when_to_use: "When choosing domain areas or concepts to inspect first.".to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent workflows".to_string(),
            purpose: "List inferred workflows and known command flows for the current repo."
                .to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Local repo index".to_string()],
            outputs: vec!["Workflow graph".to_string()],
            when_to_use: "When understanding common task flows in a repo.".to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent commands".to_string(),
            purpose: "Show the built-in command catalog for the CLI.".to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Static command catalog".to_string()],
            outputs: vec!["Command catalog".to_string()],
            when_to_use: "When deciding which coding-agent command to use next.".to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent courses".to_string(),
            purpose: "List repo-authored training courses for coding agents.".to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Local repo index".to_string()],
            outputs: vec!["Training course list".to_string()],
            when_to_use: "When a repo may contain authoritative task instructions.".to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent train --task <task> --audience coding_agent"
                .to_string(),
            purpose: "Render agent-ready instructions from matching repo-authored training courses."
                .to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Task description".to_string(), "Audience".to_string()],
            outputs: vec!["Training instructions".to_string()],
            when_to_use: "Before performing a task whose process may differ from remembered commands."
                .to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent updates --task <task>".to_string(),
            purpose: "List knowledge updates that may change or invalidate remembered guidance."
                .to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec![
                "Local repo index".to_string(),
                "Optional task, concept, or severity filter".to_string(),
            ],
            outputs: vec!["Knowledge update list".to_string()],
            when_to_use: "Before performing a task where new capabilities, deprecations, or migrations may apply.".to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent search --mode <mode> --engine auto <query>"
                .to_string(),
            purpose: "Search code, instructions, concepts, reuse policy, courses, or knowledge updates using the local repo index."
                .to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Search mode".to_string(), "Query string".to_string()],
            outputs: vec!["Structured search results".to_string()],
            when_to_use: "When an agent needs deterministic discovery instead of free-form grep."
                .to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent locate-owner --concept <id>".to_string(),
            purpose: "Find the owner repo and reuse policy for a seeded concept.".to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Concept id".to_string()],
            outputs: vec!["Owner lookup".to_string()],
            when_to_use: "When deciding where a cross-repo change should live.".to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent required-validations --task <task>".to_string(),
            purpose: "Suggest required validation commands for a described task or change."
                .to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Task description".to_string()],
            outputs: vec!["Validation descriptors".to_string()],
            when_to_use: "When finishing work and deciding what checks must run.".to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent package-index --tag <tag>".to_string(),
            purpose: "Build a local OCI-style package for the current repo index and generated agent docs.".to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Tag".to_string()],
            outputs: vec![".greentic-agent/oci/<repo>/<tag>".to_string()],
            when_to_use: "Before publishing or inspecting a distributable repo index artifact.".to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent publish-index --tag <tag>".to_string(),
            purpose: "Publish the local OCI-style package into the configured remote store.".to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Tag".to_string()],
            outputs: vec!["~/.greentic-agent/remote-oci/<repo>/<tag>".to_string()],
            when_to_use: "When sharing a packaged repo index for later sync or inspection.".to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent sync --repo <repo> --tag <tag>".to_string(),
            purpose: "Copy a published OCI-style package into the local cache.".to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Repo name".to_string(), "Tag".to_string()],
            outputs: vec!["~/.greentic-agent/cache-oci/<repo>/<tag>".to_string()],
            when_to_use: "When pulling a packaged repo index into the local machine cache.".to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent list-remote-repos".to_string(),
            purpose: "List repos and tags currently available in the configured remote store.".to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Remote store".to_string()],
            outputs: vec!["Remote repo list".to_string()],
            when_to_use: "When discovering which packaged repo indexes are available to sync.".to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent show-catalog".to_string(),
            purpose: "Build a discovery catalog from the currently published remote repo indexes.".to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Remote store".to_string()],
            outputs: vec!["Catalog".to_string()],
            when_to_use: "When discovering multiple published repos and their latest tags.".to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent check-refresh".to_string(),
            purpose: "Explain whether the local repo index should be regenerated and republished.".to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Current checkout".to_string(), "Local index outputs".to_string()],
            outputs: vec!["Refresh decision".to_string()],
            when_to_use: "Before publishing or in CI when deciding whether refresh is needed.".to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent install-github-workflow".to_string(),
            purpose: "Generate the GitHub workflow that analyzes, checks refresh, packages, and publishes repo indexes.".to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Current repository".to_string()],
            outputs: vec![".github/workflows/greentic-agent-index.yml".to_string()],
            when_to_use: "When enabling per-repo self-refresh automation.".to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent org plan-index-rollout --org <org>".to_string(),
            purpose: "Create a deterministic org-wide plan for enabling the standard Greentic coding-agent index workflow.".to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec![
                "GitHub org".to_string(),
                "Repo source or repo-list file".to_string(),
            ],
            outputs: vec!["Index rollout plan JSON".to_string()],
            when_to_use: "Before applying indexing automation across multiple Greentic repositories.".to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent org apply-index-rollout --plan <plan.json> --open-prs".to_string(),
            purpose: "Apply an org-wide index rollout plan by writing workflow branches and opening pull requests.".to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Index rollout plan JSON".to_string()],
            outputs: vec!["Rollout apply report".to_string()],
            when_to_use: "After reviewing a rollout plan and deciding to create repository PRs.".to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent impact --symbol <id>".to_string(),
            purpose: "Estimate likely blast radius for a symbol, concept, or workflow identifier.".to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Symbol or concept id".to_string()],
            outputs: vec!["Impact analysis".to_string()],
            when_to_use: "Before editing shared code or contracts and you want a quick impact estimate.".to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent detect-changes".to_string(),
            purpose: "Compare the working tree to the indexed snapshot and suggest affected areas.".to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Current checkout".to_string(), "Local index outputs".to_string()],
            outputs: vec!["Changed files and suggested validations".to_string()],
            when_to_use: "When checking what your current unrefreshed changes are likely to affect.".to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent validate-plan <plan.json>".to_string(),
            purpose: "Validate a proposed change plan against repo ownership and validation hints.".to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Plan JSON file".to_string()],
            outputs: vec!["Plan validation".to_string()],
            when_to_use: "Before implementation when you want a structured sanity check of planned work.".to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent serve".to_string(),
            purpose: "Emit the current MCP-style tool surface and freshness state.".to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Local index state".to_string()],
            outputs: vec!["MCP server snapshot".to_string()],
            when_to_use: "When an agent host needs the available tool surface in machine-readable form.".to_string(),
        },
    ]
}

pub fn built_in_policy_bundle() -> PolicyBundle {
    PolicyBundle {
        validations: vec![
            ValidationDescriptor {
                id: "shared_schema_changed".to_string(),
                title: "Shared schema change".to_string(),
                summary: "Shared schema changes should run workspace checks plus downstream fixture coverage.".to_string(),
                phase: LifecyclePhase::Build,
                command_groups: vec![
                    "cargo test --workspace --all-features".to_string(),
                    "bash ci/local_check.sh".to_string(),
                ],
                triggered_by: vec![
                    "shared schema".to_string(),
                    "schema change".to_string(),
                    "application pack schema".to_string(),
                    "extension pack schema".to_string(),
                ],
            },
            ValidationDescriptor {
                id: "docs_only_change".to_string(),
                title: "Docs-only change".to_string(),
                summary: "Documentation changes should still run documentation-oriented validation.".to_string(),
                phase: LifecyclePhase::Build,
                command_groups: vec![
                    "cargo doc --no-deps --workspace".to_string(),
                    "bash ci/local_check.sh".to_string(),
                ],
                triggered_by: vec!["docs".to_string(), "readme".to_string(), "architecture".to_string()],
            },
            ValidationDescriptor {
                id: "new_workflow_added".to_string(),
                title: "New workflow added".to_string(),
                summary: "Workflow changes should refresh command intelligence and keep CI green.".to_string(),
                phase: LifecyclePhase::Build,
                command_groups: vec![
                    "bash ci/local_check.sh".to_string(),
                    "greentic-dev coverage".to_string(),
                ],
                triggered_by: vec!["workflow".to_string(), "github action".to_string(), "ci".to_string()],
            },
            ValidationDescriptor {
                id: "setup_runtime_schema_change".to_string(),
                title: "Setup/runtime schema change".to_string(),
                summary: "Setup and runtime contract changes should run full build and test checks.".to_string(),
                phase: LifecyclePhase::Setup,
                command_groups: vec![
                    "cargo build --workspace --all-features".to_string(),
                    "cargo test --workspace --all-features".to_string(),
                    "bash ci/local_check.sh".to_string(),
                ],
                triggered_by: vec!["setup".to_string(), "runtime".to_string(), "start".to_string()],
            },
            ValidationDescriptor {
                id: "component_qa_schema_change".to_string(),
                title: "Component QA schema change".to_string(),
                summary: "Component QA and validation schema changes should keep workspace tests and docs healthy.".to_string(),
                phase: LifecyclePhase::Build,
                command_groups: vec![
                    "cargo clippy --workspace --all-targets --all-features -- -D warnings".to_string(),
                    "cargo test --workspace --all-features".to_string(),
                ],
                triggered_by: vec!["component qa".to_string(), "qa schema".to_string(), "component".to_string()],
            },
        ],
        reuse: vec![
            ReuseDescriptor {
                id: "extension_pack_owner".to_string(),
                concept_id: "extension_pack".to_string(),
                owner_repo: "greentic-pack".to_string(),
                rationale: "Pack-level extension schema changes should live with pack contracts instead of being duplicated elsewhere.".to_string(),
                forbidden_locations: vec!["customer-solution".to_string(), "demo-app".to_string()],
                required_validations: vec!["shared_schema_changed".to_string()],
            },
            ReuseDescriptor {
                id: "application_pack_owner".to_string(),
                concept_id: "application_pack".to_string(),
                owner_repo: "greentic-pack".to_string(),
                rationale: "Application-pack schema and packaging concerns belong with the pack contracts and toolchain.".to_string(),
                forbidden_locations: vec!["customer-solution".to_string(), "examples-only".to_string()],
                required_validations: vec!["shared_schema_changed".to_string()],
            },
            ReuseDescriptor {
                id: "setup_runtime_owner".to_string(),
                concept_id: "setup".to_string(),
                owner_repo: "greentic-setup".to_string(),
                rationale: "Setup schema and runtime bootstrapping should live in the setup-owning repo instead of being redefined locally.".to_string(),
                forbidden_locations: vec!["customer-solution".to_string()],
                required_validations: vec!["setup_runtime_schema_change".to_string()],
            },
            ReuseDescriptor {
                id: "bundle_start_owner".to_string(),
                concept_id: "start".to_string(),
                owner_repo: "greentic-runner".to_string(),
                rationale: "Bundle activation and start-time behavior belongs with the runtime/runner layer.".to_string(),
                forbidden_locations: vec!["docs-only".to_string()],
                required_validations: vec!["setup_runtime_schema_change".to_string()],
            },
            ReuseDescriptor {
                id: "component_qa_owner".to_string(),
                concept_id: "component".to_string(),
                owner_repo: "greentic-component".to_string(),
                rationale: "Component QA schema and generation concerns should stay with the component authoring contracts.".to_string(),
                forbidden_locations: vec!["customer-solution".to_string()],
                required_validations: vec!["component_qa_schema_change".to_string()],
            },
            ReuseDescriptor {
                id: "greentic_x_owner".to_string(),
                concept_id: "greentic_x".to_string(),
                owner_repo: "greentic-x".to_string(),
                rationale: "Greentic-X catalog and product-specific schema changes should be owned by the Greentic-X repo.".to_string(),
                forbidden_locations: vec!["examples-only".to_string()],
                required_validations: vec!["shared_schema_changed".to_string(), "new_workflow_added".to_string()],
            },
            ReuseDescriptor {
                id: "greentic_sorla_owner".to_string(),
                concept_id: "greentic_sorla".to_string(),
                owner_repo: "greentic-sorla".to_string(),
                rationale: "Greentic-sorla provider/schema changes should live in the Greentic-sorla repo.".to_string(),
                forbidden_locations: vec!["customer-solution".to_string()],
                required_validations: vec!["shared_schema_changed".to_string(), "component_qa_schema_change".to_string()],
            },
            ReuseDescriptor {
                id: "bundle_contract_owner".to_string(),
                concept_id: "bundle".to_string(),
                owner_repo: "greentic-bundle".to_string(),
                rationale: "Bundle assembly and bundle contract changes should live in greentic-bundle.".to_string(),
                forbidden_locations: vec!["examples-only".to_string(), "customer-solution".to_string()],
                required_validations: vec!["setup_runtime_schema_change".to_string()],
            },
            ReuseDescriptor {
                id: "capability_contract_owner".to_string(),
                concept_id: "capability".to_string(),
                owner_repo: "greentic-types".to_string(),
                rationale: "Capability contracts are shared core types and should live in greentic-types.".to_string(),
                forbidden_locations: vec!["customer-solution".to_string()],
                required_validations: vec!["shared_schema_changed".to_string()],
            },
            ReuseDescriptor {
                id: "provider_contract_owner".to_string(),
                concept_id: "provider".to_string(),
                owner_repo: "greentic-types".to_string(),
                rationale: "Provider contracts should be shared from greentic-types instead of being duplicated locally.".to_string(),
                forbidden_locations: vec!["customer-solution".to_string()],
                required_validations: vec!["shared_schema_changed".to_string()],
            },
            ReuseDescriptor {
                id: "hook_contract_owner".to_string(),
                concept_id: "hook".to_string(),
                owner_repo: "greentic-types".to_string(),
                rationale: "Hook contracts belong with the shared core types in greentic-types.".to_string(),
                forbidden_locations: vec!["examples-only".to_string()],
                required_validations: vec!["shared_schema_changed".to_string()],
            },
            ReuseDescriptor {
                id: "observer_contract_owner".to_string(),
                concept_id: "observer".to_string(),
                owner_repo: "greentic-types".to_string(),
                rationale: "Observer contracts belong with the shared core types in greentic-types.".to_string(),
                forbidden_locations: vec!["examples-only".to_string()],
                required_validations: vec!["shared_schema_changed".to_string()],
            },
            ReuseDescriptor {
                id: "static_route_contract_owner".to_string(),
                concept_id: "static_route".to_string(),
                owner_repo: "greentic-types".to_string(),
                rationale: "Static-route contracts should stay centralized in greentic-types.".to_string(),
                forbidden_locations: vec!["customer-solution".to_string()],
                required_validations: vec!["shared_schema_changed".to_string()],
            },
            ReuseDescriptor {
                id: "wizard_launcher_owner".to_string(),
                concept_id: "wizard".to_string(),
                owner_repo: "greentic-dev".to_string(),
                rationale: "Wizard launcher behavior is a cross-repo dev-tool concern and should live in greentic-dev.".to_string(),
                forbidden_locations: vec!["customer-solution".to_string()],
                required_validations: vec!["new_workflow_added".to_string()],
            },
        ],
    }
}

pub fn load_policy_bundle(repo_root: &Path) -> PolicyBundle {
    let mut bundle = built_in_policy_bundle();
    bundle.reuse.extend(load_repo_local_reuse(repo_root));
    bundle
        .validations
        .extend(load_repo_local_validations(repo_root));
    bundle.reuse.sort_by(|left, right| left.id.cmp(&right.id));
    bundle.reuse.dedup_by(|left, right| left.id == right.id);
    bundle
        .validations
        .sort_by(|left, right| left.id.cmp(&right.id));
    bundle
        .validations
        .dedup_by(|left, right| left.id == right.id);
    bundle
}

pub fn locate_owner(policy: &PolicyBundle, concept_id: &str) -> Option<OwnerLookup> {
    policy
        .reuse
        .iter()
        .find(|entry| entry.concept_id == concept_id)
        .map(|entry| OwnerLookup {
            concept_id: entry.concept_id.clone(),
            owner_repo: entry.owner_repo.clone(),
            rationale: entry.rationale.clone(),
            forbidden_locations: entry.forbidden_locations.clone(),
            required_validations: entry.required_validations.clone(),
        })
}

pub fn required_validations(policy: &PolicyBundle, task: &str) -> RequiredValidationsResponse {
    let lower = task.to_ascii_lowercase();
    let mut validations = policy
        .validations
        .iter()
        .filter(|validation| {
            validation
                .triggered_by
                .iter()
                .any(|trigger| lower.contains(&trigger.to_ascii_lowercase()))
        })
        .cloned()
        .collect::<Vec<_>>();

    for reuse in &policy.reuse {
        if lower.contains(&reuse.concept_id.replace('_', " "))
            || lower.contains(&reuse.concept_id.to_ascii_lowercase())
        {
            for validation_id in &reuse.required_validations {
                if let Some(validation) = policy
                    .validations
                    .iter()
                    .find(|entry| &entry.id == validation_id)
                    && !validations
                        .iter()
                        .any(|existing| existing.id == validation.id)
                {
                    validations.push(validation.clone());
                }
            }
        }
    }

    validations.sort_by(|left, right| left.id.cmp(&right.id));
    RequiredValidationsResponse {
        task: task.to_string(),
        validations,
    }
}

pub fn list_training_courses(repo_index: &RepoIndex) -> Vec<TrainingCourseDescriptor> {
    let mut courses = repo_index.training_courses.clone();
    courses.sort_by(|left, right| left.id.cmp(&right.id));
    courses
}

pub fn show_training_course(repo_index: &RepoIndex, id: &str) -> Option<TrainingCourseDescriptor> {
    repo_index
        .training_courses
        .iter()
        .find(|course| course.id == id)
        .cloned()
}

pub fn recommend_training_courses(
    repo_index: &RepoIndex,
    task: &str,
    audience: Option<TrainingAudience>,
) -> Vec<TrainingCourseDescriptor> {
    recommend_training_courses_with_reasons(repo_index, task, audience)
        .into_iter()
        .map(|entry| entry.course)
        .collect()
}

pub fn recommend_training_courses_with_reasons(
    repo_index: &RepoIndex,
    task: &str,
    audience: Option<TrainingAudience>,
) -> Vec<TrainingRecommendation> {
    let lower_task = task.trim().to_ascii_lowercase();
    let task_terms = query_terms(&lower_task);
    let mut recommendations = repo_index
        .training_courses
        .iter()
        .filter(|course| audience.is_none_or(|audience| course.audience.contains(&audience)))
        .filter_map(|course| {
            let mut score = 0;
            let mut reasons = Vec::new();
            if course
                .tasks
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(task.trim()))
            {
                score += 100;
                reasons.push("exact task match".to_string());
            }
            if contains_any_term(&course.teaches_concepts, &task_terms) {
                score += 40;
                reasons.push("concept match".to_string());
            }
            if course.title.to_ascii_lowercase().contains(&lower_task)
                || course.summary.to_ascii_lowercase().contains(&lower_task)
                || task_terms
                    .iter()
                    .any(|term| course.title.to_ascii_lowercase().contains(term))
                || task_terms
                    .iter()
                    .any(|term| course.summary.to_ascii_lowercase().contains(term))
            {
                score += 25;
                reasons.push("title or summary match".to_string());
            }
            if contains_any_term(&course.canonical_commands, &task_terms) {
                score += 15;
                reasons.push("canonical command match".to_string());
            }
            if contains_any_term(&course.source_paths, &task_terms) {
                score += 5;
                reasons.push("source path match".to_string());
            }
            if score == 0 {
                None
            } else {
                Some(TrainingRecommendation {
                    course: course.clone(),
                    score,
                    reasons,
                })
            }
        })
        .collect::<Vec<_>>();
    recommendations.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then(left.course.id.cmp(&right.course.id))
    });
    recommendations
}

pub fn list_knowledge_updates(
    repo_index: &RepoIndex,
    filter: UpdateFilter,
) -> Vec<KnowledgeUpdateDescriptor> {
    let task_terms = filter
        .task
        .as_ref()
        .map(|task| query_terms(&task.to_ascii_lowercase()))
        .unwrap_or_default();
    let mut updates = repo_index
        .knowledge_updates
        .iter()
        .filter(|update| {
            filter
                .severity
                .is_none_or(|severity| update.severity == severity)
        })
        .filter(|update| {
            filter
                .update_type
                .is_none_or(|update_type| update.update_type == update_type)
        })
        .filter(|update| {
            filter.concept.as_ref().is_none_or(|concept| {
                update
                    .affected_concepts
                    .iter()
                    .any(|candidate| candidate == concept)
            })
        })
        .filter(|update| {
            filter.task.is_none() || knowledge_update_matches_terms(update, &task_terms)
        })
        .cloned()
        .collect::<Vec<_>>();
    sort_knowledge_updates(&mut updates);
    updates
}

pub fn show_knowledge_update(
    repo_index: &RepoIndex,
    id: &str,
) -> Option<KnowledgeUpdateDescriptor> {
    repo_index
        .knowledge_updates
        .iter()
        .find(|update| update.id == id)
        .cloned()
}

pub fn recommend_updates_for_task(
    repo_index: &RepoIndex,
    task: &str,
) -> Vec<KnowledgeUpdateDescriptor> {
    list_knowledge_updates(
        repo_index,
        UpdateFilter {
            task: Some(task.to_string()),
            ..UpdateFilter::default()
        },
    )
}

pub fn important_updates_for_task(
    repo_index: &RepoIndex,
    task: &str,
) -> Vec<KnowledgeUpdateDescriptor> {
    recommend_updates_for_task(repo_index, task)
        .into_iter()
        .filter(|update| update.severity.rank() >= KnowledgeUpdateSeverity::Important.rank())
        .collect()
}

pub fn recommend_updates_for_concept(
    repo_index: &RepoIndex,
    concept_id: &str,
) -> Vec<KnowledgeUpdateDescriptor> {
    list_knowledge_updates(
        repo_index,
        UpdateFilter {
            concept: Some(concept_id.to_string()),
            ..UpdateFilter::default()
        },
    )
}

pub fn search_repo_index(repo_index: &RepoIndex, mode: SearchMode, query: &str) -> SearchResponse {
    let query = query.trim().to_string();
    let mut results = match mode {
        SearchMode::Code => search_code(repo_index, &query),
        SearchMode::Instruction => search_instruction(repo_index, &query),
        SearchMode::Concept => search_concept(repo_index, &query),
        SearchMode::Reuse => search_reuse(repo_index, &query),
        SearchMode::Course => search_course(repo_index, &query),
        SearchMode::Update => search_update(repo_index, &query),
    };
    results.sort_by(|left, right| left.id.cmp(&right.id));

    SearchResponse {
        mode,
        query,
        results,
    }
}

fn search_code(repo_index: &RepoIndex, query: &str) -> Vec<SearchResult> {
    let query = query.to_ascii_lowercase();
    let mut results = Vec::new();

    for symbol in &repo_index.source_stats.rust_symbols {
        let haystack = format!(
            "{} {:?} {} {}",
            symbol.name, symbol.kind, symbol.visibility, symbol.path
        )
        .to_ascii_lowercase();
        if haystack.contains(&query) {
            results.push(SearchResult {
                repo_id: repo_index.repo_id.clone(),
                id: format!("code:symbol:{}", sanitize_id(&symbol.path)),
                title: symbol.name.clone(),
                result_type: SearchResultType::Code,
                locator: symbol.path.clone(),
                snippet: format!("{} {:?} {}", symbol.visibility, symbol.kind, symbol.name),
                provenance: "source_stats.rust_symbols".to_string(),
                freshness: repo_index.freshness,
            });
        }
    }

    for module in &repo_index.source_stats.modules {
        if module.to_ascii_lowercase().contains(&query) {
            results.push(SearchResult {
                repo_id: repo_index.repo_id.clone(),
                id: format!("code:module:{module}"),
                title: module.clone(),
                result_type: SearchResultType::Code,
                locator: module.clone(),
                snippet: "Rust module path indexed from source tree.".to_string(),
                provenance: "source_stats.modules".to_string(),
                freshness: repo_index.freshness,
            });
        }
    }

    for item in &repo_index.source_stats.public_items {
        if item.to_ascii_lowercase().contains(&query) {
            results.push(SearchResult {
                repo_id: repo_index.repo_id.clone(),
                id: format!("code:public:{}", sanitize_id(item)),
                title: item.clone(),
                result_type: SearchResultType::Code,
                locator: repo_index
                    .source_stats
                    .modules
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "source_stats.public_items".to_string()),
                snippet: item.clone(),
                provenance: "source_stats.public_items".to_string(),
                freshness: repo_index.freshness,
            });
        }
    }

    for dependency in &repo_index.source_stats.dependencies {
        if dependency.to_ascii_lowercase().contains(&query) {
            results.push(SearchResult {
                repo_id: repo_index.repo_id.clone(),
                id: format!("code:dependency:{dependency}"),
                title: dependency.clone(),
                result_type: SearchResultType::Code,
                locator: "Cargo.toml".to_string(),
                snippet: "Dependency indexed from Cargo manifests.".to_string(),
                provenance: "source_stats.dependencies".to_string(),
                freshness: repo_index.freshness,
            });
        }
    }

    results
}

fn search_instruction(repo_index: &RepoIndex, query: &str) -> Vec<SearchResult> {
    let query = query.to_ascii_lowercase();
    let mut results = repo_index
        .instruction_graph
        .iter()
        .filter(|entry| {
            entry.path.to_ascii_lowercase().contains(&query)
                || entry.title.to_ascii_lowercase().contains(&query)
                || entry
                    .headings
                    .iter()
                    .any(|heading| heading.to_ascii_lowercase().contains(&query))
                || entry
                    .commands
                    .iter()
                    .any(|command| command.to_ascii_lowercase().contains(&query))
                || entry
                    .concept_ids
                    .iter()
                    .any(|concept| concept.to_ascii_lowercase().contains(&query))
        })
        .map(|entry| SearchResult {
            repo_id: repo_index.repo_id.clone(),
            id: format!("instruction:{}", entry.id),
            title: entry.title.clone(),
            result_type: SearchResultType::Instruction,
            locator: entry.path.clone(),
            snippet: if let Some(command) = entry.commands.first() {
                format!("Known command: {command}")
            } else if let Some(heading) = entry.headings.first() {
                format!("Heading: {heading}")
            } else {
                format!("{} entry indexed from {}", entry.kind, entry.path)
            },
            provenance: format!("instruction_graph:{}", entry.kind),
            freshness: repo_index.freshness,
        })
        .collect::<Vec<_>>();

    for course in &repo_index.training_courses {
        if training_course_matches(course, &query) {
            results.push(SearchResult {
                repo_id: repo_index.repo_id.clone(),
                id: format!("instruction:training:{}", course.id),
                title: course.title.clone(),
                result_type: SearchResultType::Instruction,
                locator: course
                    .source_paths
                    .first()
                    .cloned()
                    .unwrap_or_else(|| course.id.clone()),
                snippet: course.summary.clone(),
                provenance: "training_courses".to_string(),
                freshness: repo_index.freshness,
            });
        }
    }
    for update in &repo_index.knowledge_updates {
        if knowledge_update_matches_query(update, &query) {
            results.push(SearchResult {
                repo_id: repo_index.repo_id.clone(),
                id: format!("instruction:update:{}", update.id),
                title: update.title.clone(),
                result_type: SearchResultType::Instruction,
                locator: update
                    .source_paths
                    .first()
                    .cloned()
                    .unwrap_or_else(|| update.id.clone()),
                snippet: update.agent_instruction.clone(),
                provenance: "knowledge_updates".to_string(),
                freshness: repo_index.freshness,
            });
        }
    }
    results
}

fn search_concept(repo_index: &RepoIndex, query: &str) -> Vec<SearchResult> {
    let query = query.to_ascii_lowercase();
    repo_index
        .concept_graph
        .iter()
        .filter(|concept| {
            concept.id.to_ascii_lowercase().contains(&query)
                || concept.title.to_ascii_lowercase().contains(&query)
                || concept.summary.to_ascii_lowercase().contains(&query)
                || concept
                    .related_paths
                    .iter()
                    .any(|path| path.to_ascii_lowercase().contains(&query))
        })
        .map(|concept| SearchResult {
            repo_id: repo_index.repo_id.clone(),
            id: format!("concept:{}", concept.id),
            title: concept.title.clone(),
            result_type: SearchResultType::Concept,
            locator: concept.id.clone(),
            snippet: concept.summary.clone(),
            provenance: "concept_graph".to_string(),
            freshness: repo_index.freshness,
        })
        .collect()
}

fn search_reuse(repo_index: &RepoIndex, query: &str) -> Vec<SearchResult> {
    let query = query.to_ascii_lowercase();
    repo_index
        .reuse
        .iter()
        .filter(|reuse| {
            reuse.id.to_ascii_lowercase().contains(&query)
                || reuse.concept_id.to_ascii_lowercase().contains(&query)
                || reuse.owner_repo.to_ascii_lowercase().contains(&query)
                || reuse.rationale.to_ascii_lowercase().contains(&query)
                || reuse
                    .forbidden_locations
                    .iter()
                    .any(|entry| entry.to_ascii_lowercase().contains(&query))
        })
        .map(|reuse| SearchResult {
            repo_id: repo_index.repo_id.clone(),
            id: format!("reuse:{}", reuse.id),
            title: format!("{} owned by {}", reuse.concept_id, reuse.owner_repo),
            result_type: SearchResultType::Reuse,
            locator: reuse.concept_id.clone(),
            snippet: reuse.rationale.clone(),
            provenance: "reuse_policy".to_string(),
            freshness: repo_index.freshness,
        })
        .collect()
}

fn search_course(repo_index: &RepoIndex, query: &str) -> Vec<SearchResult> {
    let query = query.to_ascii_lowercase();
    repo_index
        .training_courses
        .iter()
        .filter(|course| training_course_matches(course, &query))
        .map(|course| SearchResult {
            repo_id: repo_index.repo_id.clone(),
            id: format!("course:{}", course.id),
            title: course.title.clone(),
            result_type: SearchResultType::Course,
            locator: course
                .source_paths
                .first()
                .cloned()
                .unwrap_or_else(|| course.id.clone()),
            snippet: course.summary.clone(),
            provenance: "training_courses".to_string(),
            freshness: repo_index.freshness,
        })
        .collect()
}

fn search_update(repo_index: &RepoIndex, query: &str) -> Vec<SearchResult> {
    let query = query.to_ascii_lowercase();
    repo_index
        .knowledge_updates
        .iter()
        .filter(|update| knowledge_update_matches_query(update, &query))
        .map(|update| SearchResult {
            repo_id: repo_index.repo_id.clone(),
            id: format!("update:{}", update.id),
            title: update.title.clone(),
            result_type: SearchResultType::Update,
            locator: update
                .source_paths
                .first()
                .cloned()
                .unwrap_or_else(|| update.id.clone()),
            snippet: update.agent_instruction.clone(),
            provenance: "knowledge_updates".to_string(),
            freshness: repo_index.freshness,
        })
        .collect()
}

fn load_repo_local_reuse(repo_root: &Path) -> Vec<ReuseDescriptor> {
    let mut entries = Vec::new();
    for relative in [".codex/policy/reuse", ".greentic-agent/policy/reuse"] {
        entries.extend(load_json_descriptors::<ReuseDescriptor>(
            &repo_root.join(relative),
        ));
    }
    entries
}

fn load_repo_local_validations(repo_root: &Path) -> Vec<ValidationDescriptor> {
    let mut entries = Vec::new();
    for relative in [
        ".codex/policy/validations",
        ".greentic-agent/policy/validations",
    ] {
        entries.extend(load_json_descriptors::<ValidationDescriptor>(
            &repo_root.join(relative),
        ));
    }
    entries
}

fn load_json_descriptors<T>(path: &Path) -> Vec<T>
where
    T: for<'de> Deserialize<'de>,
{
    if path.is_file() {
        return parse_json_descriptor_file(path);
    }
    let Ok(entries) = fs::read_dir(path) else {
        return Vec::new();
    };
    let mut values = Vec::new();
    for entry in entries.flatten() {
        let candidate = entry.path();
        if candidate.extension().and_then(|ext| ext.to_str()) == Some("json") {
            values.extend(parse_json_descriptor_file(&candidate));
        }
    }
    values
}

fn parse_json_descriptor_file<T>(path: &Path) -> Vec<T>
where
    T: for<'de> Deserialize<'de>,
{
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };
    if let Ok(single) = serde_json::from_str::<T>(&raw) {
        return vec![single];
    }
    serde_json::from_str::<Vec<T>>(&raw).unwrap_or_default()
}

fn sanitize_id(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' => character.to_ascii_lowercase(),
            _ => '_',
        })
        .collect()
}

fn query_terms(query: &str) -> Vec<String> {
    query
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|term| term.len() > 2)
        .map(str::to_ascii_lowercase)
        .collect()
}

fn contains_any_term(values: &[String], terms: &[String]) -> bool {
    values.iter().any(|value| {
        let lower = value.to_ascii_lowercase();
        terms.iter().any(|term| lower.contains(term))
    })
}

fn training_course_matches(course: &TrainingCourseDescriptor, query: &str) -> bool {
    course.id.to_ascii_lowercase().contains(query)
        || course.title.to_ascii_lowercase().contains(query)
        || course.summary.to_ascii_lowercase().contains(query)
        || course
            .teaches_concepts
            .iter()
            .any(|concept| concept.to_ascii_lowercase().contains(query))
        || course
            .tasks
            .iter()
            .any(|task| task.to_ascii_lowercase().contains(query))
        || course
            .canonical_commands
            .iter()
            .any(|command| command.to_ascii_lowercase().contains(query))
        || course
            .deprecated_commands
            .iter()
            .any(|command| command.command.to_ascii_lowercase().contains(query))
        || course.modules.iter().any(|module| {
            module.title.to_ascii_lowercase().contains(query)
                || module.objective.to_ascii_lowercase().contains(query)
                || module.steps.iter().any(|step| {
                    step.instruction.to_ascii_lowercase().contains(query)
                        || step
                            .command
                            .as_ref()
                            .is_some_and(|command| command.to_ascii_lowercase().contains(query))
                        || step.validation.as_ref().is_some_and(|validation| {
                            validation.to_ascii_lowercase().contains(query)
                        })
                })
        })
}

fn sort_knowledge_updates(updates: &mut [KnowledgeUpdateDescriptor]) {
    updates.sort_by(|left, right| {
        right
            .severity
            .rank()
            .cmp(&left.severity.rank())
            .then(right.published_at.cmp(&left.published_at))
            .then(left.id.cmp(&right.id))
    });
}

fn knowledge_update_matches_terms(update: &KnowledgeUpdateDescriptor, terms: &[String]) -> bool {
    contains_any_term(&update.affected_concepts, terms)
        || contains_any_term(&update.affected_workflows, terms)
        || contains_any_term(&update.affected_courses, terms)
        || contains_any_term(&update.affected_repos, terms)
        || terms.iter().any(|term| {
            update.title.to_ascii_lowercase().contains(term)
                || update.summary.to_ascii_lowercase().contains(term)
                || update.agent_instruction.to_ascii_lowercase().contains(term)
        })
        || contains_any_term(&update.required_validations, terms)
        || update.new_capabilities.iter().any(|capability| {
            terms.iter().any(|term| {
                capability.title.to_ascii_lowercase().contains(term)
                    || capability.summary.to_ascii_lowercase().contains(term)
                    || capability
                        .use_when
                        .iter()
                        .any(|entry| entry.to_ascii_lowercase().contains(term))
            })
        })
        || update.migration_steps.iter().any(|step| {
            terms.iter().any(|term| {
                step.instruction.to_ascii_lowercase().contains(term)
                    || step
                        .command
                        .as_ref()
                        .is_some_and(|command| command.to_ascii_lowercase().contains(term))
            })
        })
}

fn knowledge_update_matches_query(update: &KnowledgeUpdateDescriptor, query: &str) -> bool {
    update.id.to_ascii_lowercase().contains(query)
        || update.title.to_ascii_lowercase().contains(query)
        || update.summary.to_ascii_lowercase().contains(query)
        || update
            .agent_instruction
            .to_ascii_lowercase()
            .contains(query)
        || update.update_type.as_str().contains(query)
        || update.severity.as_str().contains(query)
        || update
            .affected_concepts
            .iter()
            .any(|concept| concept.to_ascii_lowercase().contains(query))
        || update
            .affected_workflows
            .iter()
            .any(|workflow| workflow.to_ascii_lowercase().contains(query))
        || update
            .affected_courses
            .iter()
            .any(|course| course.to_ascii_lowercase().contains(query))
        || update
            .deprecated_commands
            .iter()
            .any(|command| command.command.to_ascii_lowercase().contains(query))
        || update
            .migration_steps
            .iter()
            .any(|step| step.instruction.to_ascii_lowercase().contains(query))
}

#[cfg(test)]
mod tests {
    use super::{
        SearchMode, SearchResultType, UpdateFilter, built_in_policy_bundle, command_catalog,
        list_knowledge_updates, load_policy_bundle, locate_owner, recommend_training_courses,
        recommend_updates_for_concept, required_validations, search_repo_index,
    };
    use gca_core::{
        CapabilityAnnouncement, ConceptDescriptor, DeprecatedCommandDescriptor, FreshnessStatus,
        InstructionDescriptor, KnowledgeScope, KnowledgeUpdateDescriptor, KnowledgeUpdateSeverity,
        KnowledgeUpdateType, LifecyclePhase, MigrationStepDescriptor, RepoAgentManifest, RepoIndex,
        RepoRole, RustSymbolDescriptor, RustSymbolKind, SourceStats, TrainingAudience,
        TrainingCourseDescriptor, TrainingModuleDescriptor, TrainingStepDescriptor,
        WorkflowDescriptor,
    };
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn command_catalog_lists_expected_commands() {
        let entries = command_catalog();

        assert!(
            entries
                .iter()
                .any(|entry| entry.command.contains("analyze"))
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry.command.contains("locate-owner"))
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry.command.contains("required-validations"))
        );
    }

    #[test]
    fn built_in_policy_answers_seeded_owner_lookup() {
        let owner = locate_owner(&built_in_policy_bundle(), "extension_pack").unwrap();
        assert_eq!(owner.owner_repo, "greentic-pack");
    }

    #[test]
    fn seeded_type_family_owner_lookup_is_stable() {
        let provider = locate_owner(&built_in_policy_bundle(), "provider").unwrap();
        assert_eq!(provider.owner_repo, "greentic-types");

        let wizard = locate_owner(&built_in_policy_bundle(), "wizard").unwrap();
        assert_eq!(wizard.owner_repo, "greentic-dev");
    }

    #[test]
    fn required_validations_match_task_keywords() {
        let response = required_validations(&built_in_policy_bundle(), "modify setup schema");
        assert!(
            response
                .validations
                .iter()
                .any(|validation| validation.id == "setup_runtime_schema_change")
        );
    }

    #[test]
    fn repo_local_policy_is_loaded_from_json_files() {
        let temp = tempdir().unwrap();
        let policy_dir = temp.path().join(".codex").join("policy").join("reuse");
        fs::create_dir_all(&policy_dir).unwrap();
        fs::write(
            policy_dir.join("custom.json"),
            r#"{
              "id": "custom_owner",
              "concept_id": "custom_concept",
              "owner_repo": "greentic-custom",
              "rationale": "Custom policy.",
              "forbidden_locations": ["demo-app"],
              "required_validations": ["docs_only_change"]
            }"#,
        )
        .unwrap();

        let bundle = load_policy_bundle(temp.path());
        assert!(bundle.reuse.iter().any(|entry| entry.id == "custom_owner"));
    }

    #[test]
    fn code_search_matches_modules_and_public_items() {
        let repo_index = demo_repo_index();
        let response = search_repo_index(&repo_index, SearchMode::Code, "analyze");

        assert!(!response.results.is_empty());
        assert!(
            response
                .results
                .iter()
                .any(|result| result.locator.contains("gca-index"))
        );
    }

    #[test]
    fn code_search_matches_structured_rust_symbols() {
        let mut repo_index = demo_repo_index();
        repo_index.source_stats.rust_symbols = vec![RustSymbolDescriptor {
            name: "Analyzer::from_workspace".to_string(),
            kind: RustSymbolKind::Function,
            visibility: "pub(crate)".to_string(),
            path: "crates/gca-index/src/extract/rust_symbols.rs:42".to_string(),
            line: Some(42),
        }];

        let response = search_repo_index(&repo_index, SearchMode::Code, "from_workspace");

        assert!(response.results.iter().any(|result| {
            result.provenance == "source_stats.rust_symbols"
                && result.locator == "crates/gca-index/src/extract/rust_symbols.rs:42"
        }));
    }

    #[test]
    fn instruction_search_matches_commands() {
        let repo_index = demo_repo_index();
        let response = search_repo_index(&repo_index, SearchMode::Instruction, "wizard");

        assert!(
            response
                .results
                .iter()
                .any(|result| result.locator == "docs/architecture.md")
        );
    }

    #[test]
    fn concept_search_matches_summary_and_id() {
        let repo_index = demo_repo_index();
        let response = search_repo_index(&repo_index, SearchMode::Concept, "sorla");

        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].locator, "greentic_sorla");
    }

    #[test]
    fn reuse_search_matches_owner_and_rationale() {
        let repo_index = demo_repo_index();
        let response = search_repo_index(&repo_index, SearchMode::Reuse, "pack");

        assert!(
            response
                .results
                .iter()
                .any(|result| result.locator == "extension_pack")
        );
    }

    #[test]
    fn recommend_training_courses_matches_task_and_audience() {
        let repo_index = demo_repo_index();
        let courses = recommend_training_courses(
            &repo_index,
            "create a component",
            Some(TrainingAudience::CodingAgent),
        );

        assert_eq!(courses[0].id, "create_component");
    }

    #[test]
    fn instruction_search_matches_training_course_content() {
        let repo_index = demo_repo_index();
        let response = search_repo_index(&repo_index, SearchMode::Instruction, "component-qa");

        assert!(
            response
                .results
                .iter()
                .any(|result| result.provenance == "training_courses")
        );
    }

    #[test]
    fn list_knowledge_updates_filters_and_sorts() {
        let repo_index = demo_repo_index();
        let updates = list_knowledge_updates(
            &repo_index,
            UpdateFilter {
                task: Some("create a component".to_string()),
                severity: Some(KnowledgeUpdateSeverity::Important),
                ..UpdateFilter::default()
            },
        );

        assert_eq!(updates[0].id, "component_answers_flow");
    }

    #[test]
    fn recommend_updates_matches_concepts_and_search() {
        let repo_index = demo_repo_index();
        let updates = recommend_updates_for_concept(&repo_index, "component");
        assert_eq!(updates[0].id, "component_answers_flow");

        let response = search_repo_index(&repo_index, SearchMode::Instruction, "answers flow");
        assert!(
            response
                .results
                .iter()
                .any(|result| result.provenance == "knowledge_updates")
        );

        let course = search_repo_index(&repo_index, SearchMode::Course, "component");
        assert_eq!(course.results[0].result_type, SearchResultType::Course);

        let update = search_repo_index(&repo_index, SearchMode::Update, "answers");
        assert_eq!(update.results[0].result_type, SearchResultType::Update);
    }

    fn demo_repo_index() -> RepoIndex {
        let manifest = RepoAgentManifest {
            version: "v1".to_string(),
            repo_id: "greenticai/greentic-coding-agent".to_string(),
            repo_name: "greentic-coding-agent".to_string(),
            org: Some("greenticai".to_string()),
            repo_root: "/tmp/demo".to_string(),
            repo_role: RepoRole::CliLauncher,
            primary_language: "rust".to_string(),
            generated_at: "unix:1".to_string(),
            candidate_docs: vec!["README.md".to_string()],
            cargo_manifests: vec!["Cargo.toml".to_string()],
        };

        let policy = built_in_policy_bundle();
        RepoIndex {
            version: "v1".to_string(),
            repo_id: manifest.repo_id.clone(),
            repo_name: manifest.repo_name.clone(),
            repo_role: RepoRole::CliLauncher,
            generated_at: "unix:1".to_string(),
            metadata: None,
            freshness: FreshnessStatus::Fresh,
            manifest,
            concept_graph: vec![
                ConceptDescriptor {
                    id: "digital_worker".to_string(),
                    title: "Digital worker".to_string(),
                    summary: "Digital worker runtime concept.".to_string(),
                    scope: KnowledgeScope::LocalRepo,
                    lifecycle_phase: LifecyclePhase::Runtime,
                    owners: vec!["greentic-coding-agent".to_string()],
                    related_paths: vec!["docs/architecture.md".to_string()],
                },
                ConceptDescriptor {
                    id: "greentic_sorla".to_string(),
                    title: "Greentic sorla".to_string(),
                    summary: "Sorla provider workflow concept.".to_string(),
                    scope: KnowledgeScope::LocalRepo,
                    lifecycle_phase: LifecyclePhase::Build,
                    owners: vec!["greentic-coding-agent".to_string()],
                    related_paths: vec!["examples/sorla.md".to_string()],
                },
            ],
            workflow_graph: vec![WorkflowDescriptor {
                id: "analyze_repo".to_string(),
                title: "Analyze repo".to_string(),
                summary: "Analyze the repo.".to_string(),
                phase: LifecyclePhase::Build,
                commands: vec!["gtc dev coding-agent analyze".to_string()],
                docs: vec!["README.md".to_string()],
                concept_ids: vec!["digital_worker".to_string()],
            }],
            validations: policy.validations,
            reuse: policy.reuse,
            training_courses: vec![TrainingCourseDescriptor {
                version: "v1".to_string(),
                id: "create_component".to_string(),
                title: "Create component".to_string(),
                summary: "Create a component with the current wizard answers flow.".to_string(),
                owner_repo: "greentic-component".to_string(),
                teaches_concepts: vec!["component".to_string(), "wizard".to_string()],
                tasks: vec!["create a component".to_string()],
                audience: vec![TrainingAudience::CodingAgent],
                lifecycle_phase: LifecyclePhase::Build,
                modules: vec![TrainingModuleDescriptor {
                    id: "qa".to_string(),
                    title: "QA".to_string(),
                    objective: "Validate component output.".to_string(),
                    steps: vec![TrainingStepDescriptor {
                        order: 1,
                        instruction: "Run component QA.".to_string(),
                        command: Some(
                            "greentic-flow component-qa --answers answers.json".to_string(),
                        ),
                        expected_output: Some("QA passes.".to_string()),
                        validation: Some(
                            "greentic-flow component-qa --answers answers.json".to_string(),
                        ),
                    }],
                }],
                canonical_commands: vec![
                    "greentic-flow wizard --answers answers.json".to_string(),
                    "greentic-flow component-qa --answers answers.json".to_string(),
                ],
                deprecated_commands: vec![DeprecatedCommandDescriptor {
                    command: "gtc component new".to_string(),
                    reason: "Use the answers flow.".to_string(),
                    replacement: Some("greentic-flow wizard --answers answers.json".to_string()),
                }],
                required_validations: vec![
                    "greentic-flow component-qa --answers answers.json".to_string(),
                ],
                examples: vec!["examples/training/create-component.course.v1.json".to_string()],
                source_paths: vec![
                    ".greentic/training/create-component.course.v1.json".to_string(),
                ],
            }],
            knowledge_updates: vec![KnowledgeUpdateDescriptor {
                version: "v1".to_string(),
                id: "component_answers_flow".to_string(),
                title: "Component creation uses wizard answers".to_string(),
                summary: "Use the current wizard answers flow for component creation.".to_string(),
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
                    "Use greentic-flow wizard --answers answers.json instead of old commands."
                        .to_string(),
                human_summary: Some("Old component creation commands are stale.".to_string()),
                new_capabilities: vec![CapabilityAnnouncement {
                    id: "component_answers".to_string(),
                    title: "Component answers flow".to_string(),
                    summary: "Create components from schema-backed answers.".to_string(),
                    use_when: vec!["create a component".to_string()],
                    owner_repo: "greentic-component".to_string(),
                    related_course: Some("create_component".to_string()),
                }],
                deprecated_commands: vec![DeprecatedCommandDescriptor {
                    command: "gtc component new".to_string(),
                    reason: "Use the answers flow.".to_string(),
                    replacement: Some("greentic-flow wizard --answers answers.json".to_string()),
                }],
                replaced_guidance: Vec::new(),
                migration_steps: vec![MigrationStepDescriptor {
                    order: 1,
                    instruction: "Capture schema and apply answers.".to_string(),
                    command: Some("greentic-flow component-schema".to_string()),
                    validation: Some(
                        "greentic-flow component-qa --answers answers.json".to_string(),
                    ),
                }],
                required_validations: vec![
                    "greentic-flow component-qa --answers answers.json".to_string(),
                ],
                source_paths: vec![
                    ".greentic/updates/component-answers-flow.update.v1.json".to_string(),
                ],
                severity: KnowledgeUpdateSeverity::Important,
            }],
            instruction_graph: vec![InstructionDescriptor {
                id: "docs_architecture_md".to_string(),
                path: "docs/architecture.md".to_string(),
                title: "Architecture".to_string(),
                kind: "doc".to_string(),
                headings: vec!["Wizard bootstrap".to_string()],
                commands: vec!["gtc wizard --schema".to_string()],
                concept_ids: vec!["digital_worker".to_string()],
            }],
            instruction_paths: vec!["docs/architecture.md".to_string()],
            source_stats: SourceStats {
                workspace_members: vec!["crates/gca-index".to_string()],
                crate_names: vec!["gca-index".to_string()],
                modules: vec!["crates/gca-index/src/lib.rs".to_string()],
                public_items: vec!["pub fn analyze_repo".to_string()],
                rust_symbols: vec![],
                test_targets: vec!["crates/gca-index/tests/tmp.rs".to_string()],
                feature_names: vec![],
                dependencies: vec!["serde".to_string()],
                markdown_docs: vec!["docs/architecture.md".to_string()],
                workflow_files: vec![".github/workflows/ci.yml".to_string()],
                example_paths: vec!["examples/sorla.md".to_string()],
            },
        }
    }
}
