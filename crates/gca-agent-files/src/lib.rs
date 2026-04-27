use gca_core::{KnowledgeUpdateDescriptor, KnowledgeUpdateSeverity, RepoIndex};
use gca_query::command_catalog;
use std::fs;
use std::path::{Path, PathBuf};

const GENERATED_DIR: &str = ".greentic-agent/generated";
const GENERATOR_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedFile {
    pub file_name: String,
    pub content: String,
}

pub fn render_generated_files(repo_index: &RepoIndex) -> Vec<GeneratedFile> {
    vec![
        GeneratedFile {
            file_name: "AGENTS.md".to_string(),
            content: render_agents(repo_index),
        },
        GeneratedFile {
            file_name: "CLAUDE.md".to_string(),
            content: render_claude(repo_index),
        },
        GeneratedFile {
            file_name: "CODEX.md".to_string(),
            content: render_codex(repo_index),
        },
        GeneratedFile {
            file_name: "llms.txt".to_string(),
            content: render_llms(repo_index),
        },
    ]
}

pub fn write_generated_files(
    repo_root: &Path,
    files: &[GeneratedFile],
    write_root: bool,
) -> std::io::Result<Vec<PathBuf>> {
    let generated_dir = repo_root.join(GENERATED_DIR);
    fs::create_dir_all(&generated_dir)?;

    let mut written = Vec::new();
    for file in files {
        let generated_path = generated_dir.join(&file.file_name);
        fs::write(&generated_path, &file.content)?;
        written.push(generated_path);

        if write_root {
            let root_path = repo_root.join(&file.file_name);
            fs::write(&root_path, &file.content)?;
            written.push(root_path);
        }
    }

    Ok(written)
}

fn render_agents(repo_index: &RepoIndex) -> String {
    let mut out = String::new();
    out.push_str("# AGENTS.md\n\n");
    out.push_str(&generated_provenance());
    out.push_str(&format!(
        "This repository is `{}` with repo role `{}`.\n\n",
        repo_index.repo_name,
        repo_role_label(repo_index)
    ));
    out.push_str("## Repo Summary\n");
    out.push_str(&format!(
        "- Freshness: `{}`\n- Concepts indexed: `{}`\n- Workflows indexed: `{}`\n\n",
        freshness_label(repo_index),
        repo_index.concept_graph.len(),
        repo_index.workflow_graph.len()
    ));

    out.push_str("## Top Workflows\n");
    for workflow in repo_index.workflow_graph.iter().take(5) {
        out.push_str(&format!("- `{}`: {}\n", workflow.id, workflow.summary));
    }
    if repo_index.workflow_graph.is_empty() {
        out.push_str("- No workflows indexed yet.\n");
    }
    out.push('\n');

    out.push_str("## Reuse Warnings\n");
    for reuse in repo_index.reuse.iter().take(5) {
        out.push_str(&format!(
            "- `{}` owned by `{}`: {}\n",
            reuse.concept_id, reuse.owner_repo, reuse.rationale
        ));
    }
    if repo_index.reuse.is_empty() {
        out.push_str("- No reuse policy entries indexed yet.\n");
    }
    out.push('\n');

    push_recent_knowledge_updates(&mut out, repo_index);

    out.push_str("## Mandatory Validations\n");
    for validation in repo_index.validations.iter().take(5) {
        out.push_str(&format!("- `{}`: {}\n", validation.id, validation.summary));
    }
    if repo_index.validations.is_empty() {
        out.push_str("- No validation entries indexed yet.\n");
    }
    out.push('\n');

    out.push_str("## Command Cheat Sheet\n");
    for entry in command_catalog().into_iter().take(6) {
        out.push_str(&format!("- `{}`: {}\n", entry.command, entry.when_to_use));
    }
    out
}

fn render_claude(repo_index: &RepoIndex) -> String {
    let mut out = String::new();
    out.push_str("# CLAUDE.md\n\n");
    out.push_str(&generated_provenance());
    out.push_str("## First Calls\n");
    out.push_str("- `greentic-coding-agent describe --here --format json`\n");
    out.push_str("- `greentic-coding-agent concepts --format json`\n");
    out.push_str("- `greentic-coding-agent workflows --format json`\n\n");

    out.push_str("## Index Freshness\n");
    out.push_str(&format!(
        "- Current freshness: `{}`\n- Re-run `greentic-coding-agent analyze` after meaningful changes.\n\n",
        freshness_label(repo_index)
    ));

    out.push_str("## Editing Policy\n");
    out.push_str("- Check impact before editing shared concepts.\n");
    out.push_str("- Prefer seeded owner lookup before changing cross-repo contracts.\n\n");

    push_recent_knowledge_updates(&mut out, repo_index);

    out.push_str("## Validation Reminders\n");
    for validation in repo_index.validations.iter().take(4) {
        out.push_str(&format!(
            "- `{}`: {}\n",
            validation.id,
            validation.command_groups.join(", ")
        ));
    }
    if repo_index.validations.is_empty() {
        out.push_str("- No validation guidance indexed yet.\n");
    }
    out
}

fn render_codex(repo_index: &RepoIndex) -> String {
    let mut out = String::new();
    out.push_str("# CODEX.md\n\n");
    out.push_str(&generated_provenance());
    out.push_str("## Fast Orientation\n");
    out.push_str("- Start with `describe --here`, then inspect `concepts` and `workflows`.\n");
    out.push_str("- Use `search --mode instruction` for task guidance and `search --mode code` for implementation entrypoints.\n\n");

    out.push_str("## Execution Expectations\n");
    out.push_str("- Complete the requested task end-to-end when safe.\n");
    out.push_str("- Prefer deterministic local validation before finishing.\n\n");

    push_recent_knowledge_updates(&mut out, repo_index);

    out.push_str("## Required Checks\n");
    out.push_str("- `bash ci/local_check.sh`\n");
    out.push_str("- `greentic-dev coverage`\n\n");

    out.push_str("## Reuse-First Guidance\n");
    for reuse in repo_index.reuse.iter().take(5) {
        out.push_str(&format!(
            "- `{}` belongs in `{}`.\n",
            reuse.concept_id, reuse.owner_repo
        ));
    }
    if repo_index.reuse.is_empty() {
        out.push_str("- No reuse guidance indexed yet.\n");
    }
    out
}

fn render_llms(repo_index: &RepoIndex) -> String {
    let mut out = String::new();
    out.push_str("# llms.txt\n\n");
    out.push_str(&generated_provenance());
    out.push_str("Useful docs:\n");
    for path in repo_index.instruction_paths.iter().take(8) {
        out.push_str(&format!("- {}\n", path));
    }
    if repo_index.instruction_paths.is_empty() {
        out.push_str("- No instruction docs indexed yet.\n");
    }
    out.push_str("\nRecent knowledge updates:\n");
    let updates = recent_knowledge_updates(repo_index);
    for update in &updates {
        out.push_str(&format!(
            "- {}: {} ({})\n",
            update
                .effective_from
                .as_deref()
                .unwrap_or(&update.published_at),
            update.summary,
            update.severity.as_str()
        ));
    }
    if updates.is_empty() {
        out.push_str("- No high-signal knowledge updates indexed yet.\n");
    }
    out.push_str("\nUseful commands:\n");
    for entry in command_catalog().into_iter().take(8) {
        out.push_str(&format!("- {}\n", entry.command));
    }
    out
}

fn push_recent_knowledge_updates(out: &mut String, repo_index: &RepoIndex) {
    out.push_str("## Recent knowledge updates\n");
    let updates = recent_knowledge_updates(repo_index);
    for update in &updates {
        out.push_str(&format!(
            "- {}: {} [{}]\n",
            update
                .effective_from
                .as_deref()
                .unwrap_or(&update.published_at),
            update.summary,
            update.severity.as_str()
        ));
    }
    if updates.is_empty() {
        out.push_str("- No high-signal knowledge updates indexed yet.\n");
    }
    out.push('\n');
}

fn recent_knowledge_updates(repo_index: &RepoIndex) -> Vec<&KnowledgeUpdateDescriptor> {
    let concept_ids = repo_index
        .concept_graph
        .iter()
        .map(|concept| concept.id.as_str())
        .collect::<Vec<_>>();
    let course_ids = repo_index
        .training_courses
        .iter()
        .map(|course| course.id.as_str())
        .collect::<Vec<_>>();
    let mut updates = repo_index
        .knowledge_updates
        .iter()
        .filter(|update| high_signal_severity(update.severity))
        .filter(|update| !is_expired(update))
        .filter(|update| update_affects_repo(update, repo_index, &concept_ids, &course_ids))
        .collect::<Vec<_>>();
    updates.sort_by(|left, right| {
        right
            .effective_from
            .as_ref()
            .unwrap_or(&right.published_at)
            .cmp(left.effective_from.as_ref().unwrap_or(&left.published_at))
            .then(left.id.cmp(&right.id))
    });
    updates.truncate(5);
    updates
}

fn high_signal_severity(severity: KnowledgeUpdateSeverity) -> bool {
    matches!(
        severity,
        KnowledgeUpdateSeverity::Important
            | KnowledgeUpdateSeverity::Breaking
            | KnowledgeUpdateSeverity::Critical
    )
}

fn is_expired(update: &KnowledgeUpdateDescriptor) -> bool {
    update
        .expires_at
        .as_deref()
        .is_some_and(|expires_at| !expires_at.is_empty() && expires_at <= "2026-04-26")
}

fn update_affects_repo(
    update: &KnowledgeUpdateDescriptor,
    repo_index: &RepoIndex,
    concept_ids: &[&str],
    course_ids: &[&str],
) -> bool {
    update.owner_repo == repo_index.repo_name
        || update.owner_repo == repo_index.repo_id
        || update
            .affected_repos
            .iter()
            .any(|repo| repo == &repo_index.repo_name || repo == &repo_index.repo_id)
        || update
            .affected_concepts
            .iter()
            .any(|concept| concept_ids.iter().any(|candidate| candidate == concept))
        || update
            .affected_courses
            .iter()
            .any(|course| course_ids.iter().any(|candidate| candidate == course))
}

fn generated_provenance() -> String {
    format!(
        "<!-- generated by greentic-coding-agent {} -->\n\n",
        GENERATOR_VERSION
    )
}

fn freshness_label(repo_index: &RepoIndex) -> &'static str {
    match repo_index.freshness {
        gca_core::FreshnessStatus::Fresh => "fresh",
        gca_core::FreshnessStatus::Stale => "stale",
        gca_core::FreshnessStatus::Unknown => "unknown",
    }
}

fn repo_role_label(repo_index: &RepoIndex) -> &'static str {
    match repo_index.repo_role {
        gca_core::RepoRole::CoreContracts => "core_contracts",
        gca_core::RepoRole::CliLauncher => "cli_launcher",
        gca_core::RepoRole::ComponentAuthoring => "component_authoring",
        gca_core::RepoRole::FlowAuthoring => "flow_authoring",
        gca_core::RepoRole::PackAuthoring => "pack_authoring",
        gca_core::RepoRole::BundleAssembly => "bundle_assembly",
        gca_core::RepoRole::SolutionLayer => "solution_layer",
        gca_core::RepoRole::SorlaLayer => "sorla_layer",
        gca_core::RepoRole::ProviderFamily => "provider_family",
        gca_core::RepoRole::DemoApp => "demo_app",
        gca_core::RepoRole::CustomerSolution => "customer_solution",
        gca_core::RepoRole::ExamplesOnly => "examples_only",
    }
}

#[cfg(test)]
mod tests {
    use super::{render_generated_files, write_generated_files};
    use gca_core::{
        CapabilityAnnouncement, ConceptDescriptor, DeprecatedCommandDescriptor, FreshnessStatus,
        InstructionDescriptor, KnowledgeScope, KnowledgeUpdateDescriptor, KnowledgeUpdateSeverity,
        KnowledgeUpdateType, LifecyclePhase, MigrationStepDescriptor, ReplacedGuidanceDescriptor,
        RepoAgentManifest, RepoIndex, RepoRole, ReuseDescriptor, SourceStats, ValidationDescriptor,
        WorkflowDescriptor,
    };
    use tempfile::tempdir;

    #[test]
    fn generated_files_render_expected_sections() {
        let files = render_generated_files(&demo_repo_index());
        assert_eq!(files.len(), 4);
        let agents = files
            .iter()
            .find(|file| file.file_name == "AGENTS.md")
            .unwrap();
        assert!(agents.content.contains("## Top Workflows"));
        assert!(agents.content.contains("## Reuse Warnings"));
        assert!(agents.content.contains("## Recent knowledge updates"));
        assert!(
            agents
                .content
                .contains("Component creation must use answers")
        );
        let codex = files
            .iter()
            .find(|file| file.file_name == "CODEX.md")
            .unwrap();
        assert!(codex.content.contains("## Required Checks"));
        assert!(
            codex
                .content
                .contains("Component creation must use answers")
        );
        let llms = files
            .iter()
            .find(|file| file.file_name == "llms.txt")
            .unwrap();
        assert!(llms.content.contains("Useful commands"));
    }

    #[test]
    fn generated_files_have_missing_data_fallbacks() {
        let mut repo_index = demo_repo_index();
        repo_index.workflow_graph.clear();
        repo_index.reuse.clear();
        repo_index.validations.clear();
        repo_index.instruction_paths.clear();

        let files = render_generated_files(&repo_index);
        let agents = files
            .iter()
            .find(|file| file.file_name == "AGENTS.md")
            .unwrap();
        assert!(agents.content.contains("No workflows indexed yet."));
        assert!(
            agents
                .content
                .contains("No reuse policy entries indexed yet.")
        );
        assert!(
            agents
                .content
                .contains("No validation entries indexed yet.")
        );
        let llms = files
            .iter()
            .find(|file| file.file_name == "llms.txt")
            .unwrap();
        assert!(llms.content.contains("No instruction docs indexed yet."));
    }

    #[test]
    fn generated_files_write_to_generated_dir_and_root_optionally() {
        let temp = tempdir().unwrap();
        let files = render_generated_files(&demo_repo_index());

        let written = write_generated_files(temp.path(), &files, true).unwrap();

        assert!(written.iter().any(|path| path.ends_with("AGENTS.md")));
        assert!(
            temp.path()
                .join(".greentic-agent/generated/AGENTS.md")
                .exists()
        );
        assert!(temp.path().join("AGENTS.md").exists());
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

        RepoIndex {
            version: "v1".to_string(),
            repo_id: manifest.repo_id.clone(),
            repo_name: manifest.repo_name.clone(),
            repo_role: RepoRole::CliLauncher,
            generated_at: "unix:1".to_string(),
            freshness: FreshnessStatus::Fresh,
            manifest,
            concept_graph: vec![ConceptDescriptor {
                id: "digital_worker".to_string(),
                title: "Digital worker".to_string(),
                summary: "Digital worker runtime concept.".to_string(),
                scope: KnowledgeScope::LocalRepo,
                lifecycle_phase: LifecyclePhase::Runtime,
                owners: vec!["greentic-coding-agent".to_string()],
                related_paths: vec!["docs/architecture.md".to_string()],
            }],
            workflow_graph: vec![WorkflowDescriptor {
                id: "analyze_repo".to_string(),
                title: "Analyze repo".to_string(),
                summary: "Analyze the repo.".to_string(),
                phase: LifecyclePhase::Build,
                commands: vec!["gtc dev coding-agent analyze".to_string()],
                docs: vec!["README.md".to_string()],
                concept_ids: vec!["digital_worker".to_string()],
            }],
            validations: vec![ValidationDescriptor {
                id: "shared_schema_changed".to_string(),
                title: "Shared schema change".to_string(),
                summary: "Run full checks.".to_string(),
                phase: LifecyclePhase::Build,
                command_groups: vec!["bash ci/local_check.sh".to_string()],
                triggered_by: vec!["schema".to_string()],
            }],
            reuse: vec![ReuseDescriptor {
                id: "extension_pack_owner".to_string(),
                concept_id: "extension_pack".to_string(),
                owner_repo: "greentic-pack".to_string(),
                rationale: "Keep extension packs in greentic-pack.".to_string(),
                forbidden_locations: vec!["customer-solution".to_string()],
                required_validations: vec!["shared_schema_changed".to_string()],
            }],
            training_courses: Vec::new(),
            knowledge_updates: vec![KnowledgeUpdateDescriptor {
                version: "v1".to_string(),
                id: "component_answers".to_string(),
                title: "Component answers".to_string(),
                summary: "Component creation must use answers.".to_string(),
                owner_repo: "greentic-coding-agent".to_string(),
                update_type: KnowledgeUpdateType::DeprecatedCommand,
                published_at: "2026-04-26".to_string(),
                effective_from: Some("2026-04-26".to_string()),
                expires_at: None,
                affected_concepts: vec!["digital_worker".to_string()],
                affected_workflows: vec!["analyze_repo".to_string()],
                affected_courses: Vec::new(),
                affected_repos: vec!["greenticai/greentic-coding-agent".to_string()],
                agent_instruction: "Use answers.".to_string(),
                human_summary: Some("Use answers.".to_string()),
                new_capabilities: vec![CapabilityAnnouncement {
                    id: "answers".to_string(),
                    title: "Answers".to_string(),
                    summary: "Answers flow.".to_string(),
                    use_when: vec!["create component".to_string()],
                    owner_repo: "greentic-coding-agent".to_string(),
                    related_course: None,
                }],
                deprecated_commands: vec![DeprecatedCommandDescriptor {
                    command: "old create".to_string(),
                    reason: "Old".to_string(),
                    replacement: Some("new create".to_string()),
                }],
                replaced_guidance: vec![ReplacedGuidanceDescriptor {
                    old_guidance: "Old".to_string(),
                    replacement_guidance: "New".to_string(),
                    reason: "Changed".to_string(),
                }],
                migration_steps: vec![MigrationStepDescriptor {
                    order: 1,
                    instruction: "Migrate".to_string(),
                    command: Some("new create".to_string()),
                    validation: Some("test".to_string()),
                }],
                required_validations: vec!["bash ci/local_check.sh".to_string()],
                source_paths: vec![".greentic/updates/component.update.v1.json".to_string()],
                severity: KnowledgeUpdateSeverity::Breaking,
            }],
            instruction_graph: vec![InstructionDescriptor {
                id: "readme".to_string(),
                path: "README.md".to_string(),
                title: "README".to_string(),
                kind: "doc".to_string(),
                headings: vec!["Overview".to_string()],
                commands: vec!["greentic-coding-agent analyze".to_string()],
                concept_ids: vec!["digital_worker".to_string()],
            }],
            instruction_paths: vec!["README.md".to_string(), "docs/architecture.md".to_string()],
            source_stats: SourceStats {
                workspace_members: vec![],
                crate_names: vec![],
                modules: vec![],
                public_items: vec![],
                rust_symbols: vec![],
                test_targets: vec![],
                feature_names: vec![],
                dependencies: vec![],
                markdown_docs: vec!["README.md".to_string()],
                workflow_files: vec![],
                example_paths: vec![],
            },
        }
    }
}
