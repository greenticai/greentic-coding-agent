use gca_core::{ConceptDescriptor, KnowledgeScope, LifecyclePhase, RepoRole, WorkflowDescriptor};

const KNOWN_COMMANDS: &[&str] = &[
    "gtc wizard --schema",
    "gtc wizard --answers",
    "gtc setup --schema",
    "gtc setup",
    "gtc start",
    "gtc dev coding-agent analyze",
    "gtc dev coding-agent concepts",
    "gtc dev coding-agent workflows",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnrichmentInput {
    pub repo_name: String,
    pub markdown_docs: Vec<String>,
    pub workflow_files: Vec<String>,
    pub example_paths: Vec<String>,
    pub public_items: Vec<String>,
    pub commands: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeededAdapter {
    pub repo_name: &'static str,
    pub repo_role: RepoRole,
    pub docs_of_interest: &'static [&'static str],
    pub concepts: &'static [&'static str],
    pub workflows: &'static [SeededWorkflow],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeededWorkflow {
    pub id: &'static str,
    pub title: &'static str,
    pub summary: &'static str,
    pub phase: LifecyclePhase,
    pub commands: &'static [&'static str],
    pub concepts: &'static [&'static str],
}

const SEEDED_ADAPTERS: &[SeededAdapter] = &[
    SeededAdapter {
        repo_name: "greentic-types",
        repo_role: RepoRole::CoreContracts,
        docs_of_interest: &["README.md", "docs/architecture.md", "schemas/"],
        concepts: &["capability", "provider", "hook", "observer", "static_route"],
        workflows: &[SeededWorkflow {
            id: "validate_shared_types",
            title: "Validate shared types",
            summary: "Shared types and contracts should be validated before downstream repos consume them.",
            phase: LifecyclePhase::Build,
            commands: &[
                "bash ci/local_check.sh",
                "cargo test --workspace --all-features",
            ],
            concepts: &["capability", "provider", "hook", "observer", "static_route"],
        }],
    },
    SeededAdapter {
        repo_name: "greentic-pack",
        repo_role: RepoRole::PackAuthoring,
        docs_of_interest: &["README.md", "docs/architecture.md", "schemas/"],
        concepts: &["application_pack", "extension_pack", "setup"],
        workflows: &[
            SeededWorkflow {
                id: "pack_resolution",
                title: "Pack resolution",
                summary: "Resolve and validate application-pack and extension-pack metadata.",
                phase: LifecyclePhase::Build,
                commands: &["greentic-dev pack doctor", "bash ci/local_check.sh"],
                concepts: &["application_pack", "extension_pack"],
            },
            SeededWorkflow {
                id: "pack_setup",
                title: "Pack setup",
                summary: "Generate or validate pack setup flows and manifests.",
                phase: LifecyclePhase::Setup,
                commands: &["gtc setup --schema", "gtc setup"],
                concepts: &["setup", "application_pack"],
            },
        ],
    },
    SeededAdapter {
        repo_name: "greentic-bundle",
        repo_role: RepoRole::BundleAssembly,
        docs_of_interest: &["README.md", "docs/architecture.md"],
        concepts: &["bundle", "start", "digital_worker"],
        workflows: &[SeededWorkflow {
            id: "bundle_assembly",
            title: "Bundle assembly",
            summary: "Assemble and start runtime bundles for Greentic applications.",
            phase: LifecyclePhase::Start,
            commands: &["gtc start", "bash ci/local_check.sh"],
            concepts: &["bundle", "start", "digital_worker"],
        }],
    },
    SeededAdapter {
        repo_name: "greentic-dev",
        repo_role: RepoRole::CliLauncher,
        docs_of_interest: &["README.md", "docs/architecture.md", "ci/"],
        concepts: &["wizard", "setup", "start"],
        workflows: &[
            SeededWorkflow {
                id: "launcher_resolution",
                title: "Launcher resolution",
                summary: "Resolve and dispatch Greentic CLI launcher commands.",
                phase: LifecyclePhase::Build,
                commands: &["gtc dev coding-agent analyze", "bash ci/local_check.sh"],
                concepts: &["wizard", "setup", "start"],
            },
            SeededWorkflow {
                id: "coverage_policy",
                title: "Coverage policy",
                summary: "Run Greentic coverage policy checks across the repo.",
                phase: LifecyclePhase::Build,
                commands: &["greentic-dev coverage"],
                concepts: &["setup"],
            },
        ],
    },
    SeededAdapter {
        repo_name: "greentic-x",
        repo_role: RepoRole::SolutionLayer,
        docs_of_interest: &["README.md", "docs/architecture.md", "catalog/"],
        concepts: &["greentic_x", "digital_worker", "component"],
        workflows: &[SeededWorkflow {
            id: "greentic_x_catalog",
            title: "Greentic-X catalog",
            summary: "Refresh and validate Greentic-X product-specific catalog content.",
            phase: LifecyclePhase::Build,
            commands: &["bash ci/local_check.sh", "greentic-dev coverage"],
            concepts: &["greentic_x", "digital_worker"],
        }],
    },
    SeededAdapter {
        repo_name: "greentic-sorla",
        repo_role: RepoRole::SorlaLayer,
        docs_of_interest: &["README.md", "docs/architecture.md", "providers/"],
        concepts: &["greentic_sorla", "provider", "component"],
        workflows: &[SeededWorkflow {
            id: "sorla_provider_refresh",
            title: "Sorla provider refresh",
            summary: "Validate provider and component behavior for Greentic-sorla integrations.",
            phase: LifecyclePhase::Build,
            commands: &[
                "bash ci/local_check.sh",
                "cargo test --workspace --all-features",
            ],
            concepts: &["greentic_sorla", "provider", "component"],
        }],
    },
];

pub fn adapter_registry() -> Vec<&'static str> {
    SEEDED_ADAPTERS
        .iter()
        .map(|adapter| adapter.repo_name)
        .collect()
}

pub fn docs_of_interest(input: &EnrichmentInput) -> Vec<String> {
    let Some(adapter) = find_adapter(&input.repo_name) else {
        return Vec::new();
    };
    adapter
        .docs_of_interest
        .iter()
        .map(|value| (*value).to_string())
        .collect()
}

pub fn infer_repo_role(input: &EnrichmentInput) -> RepoRole {
    let repo_name = input.repo_name.to_ascii_lowercase();

    if let Some(adapter) = find_adapter(&input.repo_name) {
        return adapter.repo_role;
    }

    if repo_name.contains("coding-agent") {
        return RepoRole::CliLauncher;
    }
    if repo_name.contains("flow") {
        return RepoRole::FlowAuthoring;
    }
    if repo_name.contains("component") {
        return RepoRole::ComponentAuthoring;
    }
    if repo_name.contains("pack") {
        return RepoRole::PackAuthoring;
    }
    if repo_name.contains("bundle") {
        return RepoRole::BundleAssembly;
    }
    if !input.example_paths.is_empty() && input.markdown_docs.len() <= 2 {
        return RepoRole::ExamplesOnly;
    }

    RepoRole::CliLauncher
}

pub fn infer_concepts(input: &EnrichmentInput) -> Vec<ConceptDescriptor> {
    let mut concepts = Vec::new();
    if let Some(adapter) = find_adapter(&input.repo_name) {
        for concept_id in adapter.concepts {
            if let Some(concept) = seeded_concept(input, concept_id) {
                concepts.push(concept);
            }
        }
    }
    add_concept_if_detected(
        &mut concepts,
        input,
        "digital_worker",
        "Digital worker",
        "Greentic digital worker orchestration appears in repo docs or commands.",
        &["digital worker", "worker"],
    );
    add_concept_if_detected(
        &mut concepts,
        input,
        "application_pack",
        "Application pack",
        "Pack authoring or application packaging terminology appears in the repo.",
        &["application pack", "pack"],
    );
    add_concept_if_detected(
        &mut concepts,
        input,
        "extension_pack",
        "Extension pack",
        "Extension-pack terminology appears in docs or workflows.",
        &["extension pack"],
    );
    add_concept_if_detected(
        &mut concepts,
        input,
        "greentic_x",
        "Greentic X",
        "Greentic-X terminology appears in repo knowledge sources.",
        &["greentic-x", "greentic x"],
    );
    add_concept_if_detected(
        &mut concepts,
        input,
        "greentic_sorla",
        "Greentic sorla",
        "Greentic-sorla terminology appears in repo knowledge sources.",
        &["greentic-sorla", "greentic sorla"],
    );
    add_concept_if_detected(
        &mut concepts,
        input,
        "wizard",
        "Wizard",
        "Wizard-driven setup flows are referenced in repo docs or workflows.",
        &["wizard"],
    );
    add_concept_if_detected(
        &mut concepts,
        input,
        "setup",
        "Setup",
        "Setup commands or setup guidance appear in the repo.",
        &["setup"],
    );
    add_concept_if_detected(
        &mut concepts,
        input,
        "start",
        "Start",
        "Start commands appear in repo docs or workflows.",
        &["start"],
    );

    concepts.sort_by(|left, right| left.id.cmp(&right.id));
    concepts.dedup_by(|left, right| left.id == right.id);
    concepts
}

pub fn infer_workflows(input: &EnrichmentInput, concept_ids: &[String]) -> Vec<WorkflowDescriptor> {
    let mut workflows = Vec::new();
    if let Some(adapter) = find_adapter(&input.repo_name) {
        for workflow in adapter.workflows {
            workflows.push(WorkflowDescriptor {
                id: workflow.id.to_string(),
                title: workflow.title.to_string(),
                summary: workflow.summary.to_string(),
                phase: workflow.phase,
                commands: workflow
                    .commands
                    .iter()
                    .map(|command| (*command).to_string())
                    .collect(),
                docs: adapter
                    .docs_of_interest
                    .iter()
                    .map(|doc| (*doc).to_string())
                    .collect(),
                concept_ids: relevant_concepts(concept_ids, workflow.concepts),
            });
        }
    }

    if contains_command(input, "gtc dev coding-agent analyze") || !input.markdown_docs.is_empty() {
        workflows.push(WorkflowDescriptor {
            id: "analyze_repo".to_string(),
            title: "Analyze repo".to_string(),
            summary: "Generate repo-local Greentic coding-agent metadata for the current checkout."
                .to_string(),
            phase: LifecyclePhase::Build,
            commands: collect_matching_commands(input, &["gtc dev coding-agent analyze"]),
            docs: input.markdown_docs.clone(),
            concept_ids: relevant_concepts(concept_ids, &["digital_worker", "setup"]),
        });
    }

    if contains_command(input, "gtc wizard --schema")
        || contains_command(input, "gtc wizard --answers")
    {
        workflows.push(WorkflowDescriptor {
            id: "wizard_bootstrap".to_string(),
            title: "Wizard bootstrap".to_string(),
            summary: "Wizard-driven bootstrapping commands are referenced in repo materials."
                .to_string(),
            phase: LifecyclePhase::Setup,
            commands: collect_matching_commands(
                input,
                &["gtc wizard --schema", "gtc wizard --answers"],
            ),
            docs: input.markdown_docs.clone(),
            concept_ids: relevant_concepts(concept_ids, &["wizard", "setup"]),
        });
    }

    if contains_command(input, "gtc setup") {
        workflows.push(WorkflowDescriptor {
            id: "setup_bundle".to_string(),
            title: "Setup bundle".to_string(),
            summary: "A `gtc setup` flow is described in repo docs or workflows.".to_string(),
            phase: LifecyclePhase::Setup,
            commands: collect_matching_commands(input, &["gtc setup --schema", "gtc setup"]),
            docs: input.markdown_docs.clone(),
            concept_ids: relevant_concepts(concept_ids, &["setup", "application_pack"]),
        });
    }

    if contains_command(input, "gtc start") {
        workflows.push(WorkflowDescriptor {
            id: "start_bundle".to_string(),
            title: "Start bundle".to_string(),
            summary: "A `gtc start` flow is described in repo docs or workflows.".to_string(),
            phase: LifecyclePhase::Start,
            commands: collect_matching_commands(input, &["gtc start"]),
            docs: input.markdown_docs.clone(),
            concept_ids: relevant_concepts(concept_ids, &["start", "digital_worker"]),
        });
    }

    workflows.sort_by(|left, right| left.id.cmp(&right.id));
    workflows.dedup_by(|left, right| left.id == right.id);
    workflows
}

pub fn known_command_matches(raw: &str) -> Vec<String> {
    let lower = raw.to_ascii_lowercase();
    let mut matches = KNOWN_COMMANDS
        .iter()
        .filter(|command| lower.contains(&command.to_ascii_lowercase()))
        .map(|command| (*command).to_string())
        .collect::<Vec<_>>();
    matches.sort();
    matches.dedup();
    matches
}

fn add_concept_if_detected(
    concepts: &mut Vec<ConceptDescriptor>,
    input: &EnrichmentInput,
    id: &str,
    title: &str,
    summary: &str,
    needles: &[&str],
) {
    if !contains_any(input, needles) {
        return;
    }

    concepts.push(ConceptDescriptor {
        id: id.to_string(),
        title: title.to_string(),
        summary: summary.to_string(),
        scope: KnowledgeScope::LocalRepo,
        lifecycle_phase: LifecyclePhase::Build,
        owners: concept_owners(id),
        related_paths: related_paths(input),
    });
}

fn seeded_concept(input: &EnrichmentInput, id: &str) -> Option<ConceptDescriptor> {
    let (title, summary, phase) = match id {
        "application_pack" => (
            "Application pack",
            "Seeded Greentic knowledge says this repo is authoritative for application-pack behavior.",
            LifecyclePhase::Build,
        ),
        "extension_pack" => (
            "Extension pack",
            "Seeded Greentic knowledge says this repo is authoritative for extension-pack behavior.",
            LifecyclePhase::Build,
        ),
        "bundle" => (
            "Bundle",
            "Seeded Greentic knowledge says this repo centers bundle assembly and runtime packaging.",
            LifecyclePhase::Start,
        ),
        "wizard" => (
            "Wizard",
            "Seeded Greentic knowledge says this repo participates in wizard-driven setup flows.",
            LifecyclePhase::Setup,
        ),
        "setup" => (
            "Setup",
            "Seeded Greentic knowledge says this repo is involved in setup-time flows or launcher behavior.",
            LifecyclePhase::Setup,
        ),
        "start" => (
            "Start",
            "Seeded Greentic knowledge says this repo is involved in runtime start flows.",
            LifecyclePhase::Start,
        ),
        "greentic_x" => (
            "Greentic X",
            "Seeded Greentic knowledge says this repo owns Greentic-X specific behavior.",
            LifecyclePhase::Build,
        ),
        "greentic_sorla" => (
            "Greentic sorla",
            "Seeded Greentic knowledge says this repo owns Greentic-sorla specific behavior.",
            LifecyclePhase::Build,
        ),
        "digital_worker" => (
            "Digital worker",
            "Seeded Greentic knowledge says this repo participates in digital-worker execution paths.",
            LifecyclePhase::Runtime,
        ),
        "component" => (
            "Component",
            "Seeded Greentic knowledge says this repo participates in component authoring or validation.",
            LifecyclePhase::Build,
        ),
        "capability" => (
            "Capability",
            "Seeded Greentic knowledge says this repo defines capability-level contracts.",
            LifecyclePhase::Build,
        ),
        "provider" => (
            "Provider",
            "Seeded Greentic knowledge says this repo defines provider contracts or implementations.",
            LifecyclePhase::Build,
        ),
        "hook" => (
            "Hook",
            "Seeded Greentic knowledge says this repo defines hook contracts.",
            LifecyclePhase::Build,
        ),
        "observer" => (
            "Observer",
            "Seeded Greentic knowledge says this repo defines observer contracts.",
            LifecyclePhase::Build,
        ),
        "static_route" => (
            "Static route",
            "Seeded Greentic knowledge says this repo defines static-route contracts.",
            LifecyclePhase::Build,
        ),
        _ => return None,
    };

    Some(ConceptDescriptor {
        id: id.to_string(),
        title: title.to_string(),
        summary: summary.to_string(),
        scope: KnowledgeScope::CrossRepo,
        lifecycle_phase: phase,
        owners: concept_owners(id),
        related_paths: related_paths(input),
    })
}

fn concept_owners(id: &str) -> Vec<String> {
    match id {
        "application_pack" | "extension_pack" => vec!["greentic-pack".to_string()],
        "bundle" => vec!["greentic-bundle".to_string()],
        "wizard" => vec!["greentic-dev".to_string()],
        "setup" | "start" => vec!["greentic-dev".to_string(), "greentic-bundle".to_string()],
        "greentic_x" => vec!["greentic-x".to_string()],
        "greentic_sorla" => vec!["greentic-sorla".to_string()],
        "capability" | "provider" | "hook" | "observer" | "static_route" => {
            vec!["greentic-types".to_string()]
        }
        "component" => vec!["greentic-component".to_string()],
        "digital_worker" => vec!["greentic-bundle".to_string()],
        _ => vec!["greentic-coding-agent".to_string()],
    }
}

fn find_adapter(repo_name: &str) -> Option<&'static SeededAdapter> {
    let repo_name = repo_name.to_ascii_lowercase();
    SEEDED_ADAPTERS
        .iter()
        .find(|adapter| repo_name == adapter.repo_name)
}

fn contains_any(input: &EnrichmentInput, needles: &[&str]) -> bool {
    let mut corpus = Vec::new();
    corpus.push(input.repo_name.to_ascii_lowercase());
    corpus.extend(
        input
            .markdown_docs
            .iter()
            .map(|value| value.to_ascii_lowercase()),
    );
    corpus.extend(
        input
            .workflow_files
            .iter()
            .map(|value| value.to_ascii_lowercase()),
    );
    corpus.extend(
        input
            .example_paths
            .iter()
            .map(|value| value.to_ascii_lowercase()),
    );
    corpus.extend(
        input
            .public_items
            .iter()
            .map(|value| value.to_ascii_lowercase()),
    );
    corpus.extend(
        input
            .commands
            .iter()
            .map(|value| value.to_ascii_lowercase()),
    );

    needles.iter().any(|needle| {
        let needle = needle.to_ascii_lowercase();
        corpus.iter().any(|value| value.contains(&needle))
    })
}

fn related_paths(input: &EnrichmentInput) -> Vec<String> {
    let mut paths = input.markdown_docs.clone();
    paths.extend(input.workflow_files.clone());
    paths.sort();
    paths.dedup();
    paths
}

fn contains_command(input: &EnrichmentInput, needle: &str) -> bool {
    let needle = needle.to_ascii_lowercase();
    input
        .commands
        .iter()
        .any(|command| command.to_ascii_lowercase().contains(&needle))
}

fn collect_matching_commands(input: &EnrichmentInput, patterns: &[&str]) -> Vec<String> {
    let mut commands = input
        .commands
        .iter()
        .filter(|command| {
            let lower = command.to_ascii_lowercase();
            patterns
                .iter()
                .any(|pattern| lower.contains(&pattern.to_ascii_lowercase()))
        })
        .cloned()
        .collect::<Vec<_>>();
    commands.sort();
    commands.dedup();
    commands
}

fn relevant_concepts(concept_ids: &[String], desired: &[&str]) -> Vec<String> {
    concept_ids
        .iter()
        .filter(|id| desired.iter().any(|desired| id == desired))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        EnrichmentInput, adapter_registry, docs_of_interest, infer_concepts, infer_repo_role,
        infer_workflows, known_command_matches,
    };
    use gca_core::RepoRole;

    #[test]
    fn greentic_repo_role_and_concepts_are_inferred_from_docs() {
        let input = EnrichmentInput {
            repo_name: "greentic-flow".to_string(),
            markdown_docs: vec!["docs/greentic-x.md".to_string()],
            workflow_files: vec![".github/workflows/ci.yml".to_string()],
            example_paths: vec![],
            public_items: vec!["pub fn start_flow()".to_string()],
            commands: vec!["gtc start bundle".to_string()],
        };

        assert_eq!(infer_repo_role(&input), RepoRole::FlowAuthoring);

        let concepts = infer_concepts(&input);
        assert!(concepts.iter().any(|concept| concept.id == "greentic_x"));
        assert!(concepts.iter().any(|concept| concept.id == "start"));
    }

    #[test]
    fn workflows_capture_known_gtc_commands() {
        let input = EnrichmentInput {
            repo_name: "greentic-demo".to_string(),
            markdown_docs: vec!["README.md".to_string()],
            workflow_files: vec![".github/workflows/perf.yml".to_string()],
            example_paths: vec!["examples/demo".to_string()],
            public_items: vec![],
            commands: vec![
                "gtc wizard --schema bundle/schema.json".to_string(),
                "gtc setup my-bundle --answers answers.json".to_string(),
                "gtc start my-bundle".to_string(),
            ],
        };

        let concepts = infer_concepts(&input);
        let concept_ids = concepts
            .iter()
            .map(|concept| concept.id.clone())
            .collect::<Vec<_>>();
        let workflows = infer_workflows(&input, &concept_ids);

        assert!(
            workflows
                .iter()
                .any(|workflow| workflow.id == "wizard_bootstrap")
        );
        assert!(
            workflows
                .iter()
                .any(|workflow| workflow.id == "setup_bundle")
        );
        assert!(
            workflows
                .iter()
                .any(|workflow| workflow.id == "start_bundle")
        );
    }

    #[test]
    fn command_matcher_finds_requested_strings() {
        let matches = known_command_matches("Run gtc wizard --schema and then gtc start bundle");

        assert!(matches.contains(&"gtc wizard --schema".to_string()));
        assert!(matches.contains(&"gtc start".to_string()));
    }

    #[test]
    fn adapter_registry_lists_seeded_repos() {
        let repos = adapter_registry();

        assert!(repos.contains(&"greentic-types"));
        assert!(repos.contains(&"greentic-pack"));
        assert!(repos.contains(&"greentic-bundle"));
        assert!(repos.contains(&"greentic-dev"));
        assert!(repos.contains(&"greentic-x"));
        assert!(repos.contains(&"greentic-sorla"));
    }

    #[test]
    fn seeded_pack_repo_enrichment_is_stronger_than_generic_mode() {
        let input = EnrichmentInput {
            repo_name: "greentic-pack".to_string(),
            markdown_docs: vec!["README.md".to_string()],
            workflow_files: vec![],
            example_paths: vec![],
            public_items: vec!["pub fn resolve_pack()".to_string()],
            commands: vec!["greentic-dev pack doctor".to_string()],
        };

        assert_eq!(infer_repo_role(&input), RepoRole::PackAuthoring);
        let concepts = infer_concepts(&input);
        assert!(
            concepts
                .iter()
                .any(|concept| concept.id == "application_pack")
        );
        assert!(
            concepts
                .iter()
                .any(|concept| concept.id == "extension_pack")
        );
        assert!(
            concepts
                .iter()
                .find(|concept| concept.id == "application_pack")
                .unwrap()
                .owners
                .contains(&"greentic-pack".to_string())
        );

        let workflows = infer_workflows(
            &input,
            &concepts
                .iter()
                .map(|concept| concept.id.clone())
                .collect::<Vec<_>>(),
        );
        assert!(
            workflows
                .iter()
                .any(|workflow| workflow.id == "pack_resolution")
        );
    }

    #[test]
    fn seeded_dev_repo_docs_and_workflows_are_available() {
        let input = EnrichmentInput {
            repo_name: "greentic-dev".to_string(),
            markdown_docs: vec!["README.md".to_string()],
            workflow_files: vec![],
            example_paths: vec![],
            public_items: vec!["pub fn dispatch()".to_string()],
            commands: vec!["greentic-dev coverage".to_string()],
        };

        let docs = docs_of_interest(&input);
        assert!(docs.contains(&"README.md".to_string()));
        assert!(docs.contains(&"ci/".to_string()));

        let concepts = infer_concepts(&input);
        let workflows = infer_workflows(
            &input,
            &concepts
                .iter()
                .map(|concept| concept.id.clone())
                .collect::<Vec<_>>(),
        );
        assert!(
            workflows
                .iter()
                .any(|workflow| workflow.id == "coverage_policy")
        );
        assert!(
            workflows
                .iter()
                .find(|workflow| workflow.id == "coverage_policy")
                .unwrap()
                .commands
                .contains(&"greentic-dev coverage".to_string())
        );
    }
}
