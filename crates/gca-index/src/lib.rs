mod extract;
mod tantivy_index;

use extract::cargo_metadata::extract_cargo_metadata;
use extract::rust_symbols::extract_rust_symbols;
use gca_core::{
    FreshnessStatus, InstructionDescriptor, KnowledgeUpdateDescriptor, RegistryEntry,
    RepoAgentManifest, RepoId, RepoIndex, SourceStats, TrainingCourseDescriptor, builtin_concepts,
    load_registry, write_registry,
};
use gca_greentic::{
    EnrichmentInput, infer_concepts, infer_repo_role, infer_workflows, known_command_matches,
};
use gca_query::load_policy_bundle;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

const LOCAL_INDEX_DIR: &str = ".greentic-agent";
const TRAINING_DIR: &str = ".greentic/training";
const UPDATES_DIR: &str = ".greentic/updates";

pub use tantivy_index::{
    TantivyBuildReport, TantivyIndexError, build_local_tantivy_index, build_merged_tantivy_index,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fingerprints {
    pub version: String,
    pub head_sha: String,
    pub default_branch: Option<String>,
    pub tracked_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyzeOutputs {
    pub manifest: RepoAgentManifest,
    pub repo_index: RepoIndex,
    pub fingerprints: Fingerprints,
    pub manifest_path: PathBuf,
    pub repo_index_path: PathBuf,
    pub fingerprints_path: PathBuf,
    pub registry_path: PathBuf,
    pub tantivy_report: Option<TantivyBuildReport>,
}

#[derive(Debug, Error)]
pub enum AnalyzeError {
    #[error("failed to detect repository root from {start_dir}")]
    RepoRootNotFound { start_dir: PathBuf },
    #[error("failed to read current directory: {0}")]
    CurrentDir(std::io::Error),
    #[error("failed to create local index directory at {path}: {source}")]
    CreateDir {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to serialize {path}: {source}")]
    Serialize {
        path: String,
        source: serde_json::Error,
    },
    #[error("failed to write {path}: {source}")]
    Write {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to update registry: {0}")]
    Registry(#[from] gca_core::RegistryError),
    #[error("failed to build tantivy index: {0}")]
    Tantivy(#[from] TantivyIndexError),
}

#[derive(Debug, Clone)]
struct InstructionScan {
    descriptors: Vec<InstructionDescriptor>,
    paths: Vec<String>,
    commands: Vec<String>,
}

pub fn analyze_repo(
    start_dir: &Path,
    registry_path: &Path,
) -> Result<AnalyzeOutputs, AnalyzeError> {
    let repo_root = find_repo_root(start_dir).ok_or_else(|| AnalyzeError::RepoRootNotFound {
        start_dir: start_dir.to_path_buf(),
    })?;

    let repo_name = repo_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown-repo")
        .to_string();
    let repo_id = detect_repo_id(&repo_root, &repo_name);
    let org = RepoId::parse(&repo_id).ok().map(|repo_id| repo_id.org);
    let generated_at = timestamp_string();
    let head_sha = read_head_sha(&repo_root).unwrap_or_else(|| "unknown".to_string());
    let default_branch = read_default_branch(&repo_root);
    let candidate_docs = find_candidate_docs(&repo_root);
    let cargo_manifests = find_cargo_manifests(&repo_root);
    let tracked_files = find_tracked_files(&repo_root);
    let source_stats = build_source_stats(&repo_root, &cargo_manifests);
    let training_courses = load_training_courses(&repo_root);
    let knowledge_updates = load_knowledge_updates(&repo_root);
    let instruction_scan = build_instruction_graph(
        &repo_root,
        &source_stats,
        &training_courses,
        &knowledge_updates,
    );

    let enrichment = EnrichmentInput {
        repo_name: repo_name.clone(),
        markdown_docs: source_stats.markdown_docs.clone(),
        workflow_files: source_stats.workflow_files.clone(),
        example_paths: source_stats.example_paths.clone(),
        public_items: source_stats.public_items.clone(),
        commands: instruction_scan.commands.clone(),
    };
    let repo_role = infer_repo_role(&enrichment);

    let manifest = RepoAgentManifest {
        version: gca_core::SCHEMA_VERSION_V1.to_string(),
        repo_id: repo_id.clone(),
        repo_name: repo_name.clone(),
        org: org.clone(),
        repo_root: repo_root.display().to_string(),
        repo_role,
        primary_language: "rust".to_string(),
        generated_at: generated_at.clone(),
        candidate_docs,
        cargo_manifests,
    };

    let mut concept_graph = builtin_concepts();
    let inferred_concepts = infer_concepts(&enrichment);
    for concept in inferred_concepts {
        if !concept_graph
            .iter()
            .any(|existing| existing.id == concept.id)
        {
            concept_graph.push(concept);
        }
    }
    concept_graph.sort_by(|left, right| left.id.cmp(&right.id));

    let concept_ids = concept_graph
        .iter()
        .map(|concept| concept.id.clone())
        .collect::<Vec<_>>();
    let workflow_graph = infer_workflows(&enrichment, &concept_ids);

    let policy = load_policy_bundle(&repo_root);

    let repo_index = RepoIndex {
        version: gca_core::SCHEMA_VERSION_V1.to_string(),
        repo_id: repo_id.clone(),
        repo_name: repo_name.clone(),
        repo_role,
        generated_at: generated_at.clone(),
        freshness: FreshnessStatus::Fresh,
        manifest: manifest.clone(),
        concept_graph,
        workflow_graph,
        validations: policy.validations,
        reuse: policy.reuse,
        training_courses,
        knowledge_updates,
        instruction_graph: instruction_scan.descriptors,
        instruction_paths: instruction_scan.paths,
        source_stats,
    };
    let fingerprints = Fingerprints {
        version: gca_core::SCHEMA_VERSION_V1.to_string(),
        head_sha: head_sha.clone(),
        default_branch: default_branch.clone(),
        tracked_files,
    };

    let local_dir = repo_root.join(LOCAL_INDEX_DIR);
    fs::create_dir_all(&local_dir).map_err(|source| AnalyzeError::CreateDir {
        path: local_dir.display().to_string(),
        source,
    })?;
    let manifest_path = local_dir.join("manifest.json");
    let repo_index_path = local_dir.join("repo-index.json");
    let fingerprints_path = local_dir.join("fingerprints.json");

    write_json(&manifest_path, &manifest)?;
    write_json(&repo_index_path, &repo_index)?;
    write_json(&fingerprints_path, &fingerprints)?;
    let tantivy_report = Some(build_local_tantivy_index(
        &repo_index,
        &local_dir.join("tantivy").join("local"),
    )?);

    let mut registry = load_registry(registry_path)?;
    registry.upsert(RegistryEntry {
        repo_id,
        repo_name,
        org,
        repo_path: repo_root.display().to_string(),
        repo_role,
        last_analyzed_commit: head_sha,
        manifest_path: manifest_path.display().to_string(),
        repo_index_path: repo_index_path.display().to_string(),
        updated_at: generated_at,
    });
    write_registry(registry_path, &registry)?;

    Ok(AnalyzeOutputs {
        manifest,
        repo_index,
        fingerprints,
        manifest_path,
        repo_index_path,
        fingerprints_path,
        registry_path: registry_path.to_path_buf(),
        tantivy_report,
    })
}

pub fn default_registry_path(home_dir: &Path) -> PathBuf {
    home_dir.join(".greentic-agent").join("registry.json")
}

pub fn current_dir() -> Result<PathBuf, AnalyzeError> {
    std::env::current_dir().map_err(AnalyzeError::CurrentDir)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), AnalyzeError> {
    let raw = serde_json::to_string_pretty(value).map_err(|source| AnalyzeError::Serialize {
        path: path.display().to_string(),
        source,
    })?;

    fs::write(path, raw).map_err(|source| AnalyzeError::Write {
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

pub fn detect_repo_id(repo_root: &Path, repo_name: &str) -> String {
    read_origin_url(repo_root)
        .and_then(|url| parse_github_remote_url(&url))
        .unwrap_or_else(|| format!("unknown/{repo_name}"))
}

fn read_origin_url(repo_root: &Path) -> Option<String> {
    let config = fs::read_to_string(repo_root.join(".git").join("config")).ok()?;
    let mut in_origin = false;
    for line in config.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_origin = trimmed == r#"[remote "origin"]"#;
            continue;
        }
        if in_origin && let Some(url) = trimmed.strip_prefix("url =") {
            return Some(url.trim().to_string());
        }
    }
    None
}

pub fn parse_github_remote_url(value: &str) -> Option<String> {
    let value = value.trim().trim_end_matches(".git");
    if let Some(path) = value.strip_prefix("git@github.com:") {
        return RepoId::parse(path).ok().map(|repo_id| repo_id.as_str());
    }
    if let Some(path) = value.strip_prefix("https://github.com/") {
        return RepoId::parse(path).ok().map(|repo_id| repo_id.as_str());
    }
    if let Some(path) = value.strip_prefix("ssh://git@github.com/") {
        return RepoId::parse(path).ok().map(|repo_id| repo_id.as_str());
    }
    None
}

fn read_head_sha(repo_root: &Path) -> Option<String> {
    let head = fs::read_to_string(repo_root.join(".git").join("HEAD")).ok()?;
    let head = head.trim();

    if let Some(reference) = head.strip_prefix("ref: ") {
        let ref_path = repo_root.join(".git").join(reference);
        return fs::read_to_string(ref_path)
            .ok()
            .map(|value| value.trim().to_string());
    }

    Some(head.to_string())
}

fn read_default_branch(repo_root: &Path) -> Option<String> {
    let head = fs::read_to_string(repo_root.join(".git").join("HEAD")).ok()?;
    let head = head.trim();
    head.strip_prefix("ref: refs/heads/")
        .map(|branch| branch.to_string())
}

fn find_candidate_docs(repo_root: &Path) -> Vec<String> {
    let candidates = [
        "README.md",
        "ARCHITECTURE.md",
        "RUNBOOK.md",
        "TESTING.md",
        "CONTRIBUTING.md",
        "docs/architecture.md",
    ];

    candidates
        .iter()
        .filter(|relative| repo_root.join(relative).exists())
        .map(|relative| (*relative).to_string())
        .collect()
}

fn find_cargo_manifests(repo_root: &Path) -> Vec<String> {
    let mut manifests = Vec::new();
    gather_files_named(repo_root, repo_root, "Cargo.toml", &mut manifests);
    manifests.sort();
    manifests
}

fn find_tracked_files(repo_root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    gather_regular_files(repo_root, repo_root, &mut files);
    files.sort();
    files
}

fn build_source_stats(repo_root: &Path, cargo_manifests: &[String]) -> SourceStats {
    let cargo_metadata = extract_cargo_metadata(repo_root);
    let mut workspace_members = Vec::new();
    let mut crate_names = Vec::new();
    let mut feature_names = Vec::new();
    let mut dependencies = Vec::new();

    if let Some(metadata) = &cargo_metadata {
        workspace_members.extend(metadata.workspace_members.clone());
        crate_names.extend(metadata.crate_names.clone());
        feature_names.extend(metadata.feature_names.clone());
        dependencies.extend(metadata.dependencies.clone());
    } else {
        for manifest_path in cargo_manifests {
            let raw = fs::read_to_string(repo_root.join(manifest_path)).unwrap_or_default();
            if is_workspace_manifest(manifest_path) {
                workspace_members.extend(parse_workspace_members(&raw));
            }
            if let Some(crate_name) = parse_manifest_string(&raw, "name") {
                crate_names.push(crate_name);
            }
            feature_names.extend(parse_feature_names(&raw));
            dependencies.extend(parse_dependency_names(&raw));
        }
    }

    let rust_symbols = extract_rust_symbols(repo_root);
    let mut modules = rust_symbols.modules;
    let mut public_items = rust_symbols.public_items;
    let mut test_targets = rust_symbols.test_targets;
    let rust_symbols = rust_symbols.symbols;

    if let Some(metadata) = &cargo_metadata {
        modules.extend(metadata.crate_root_paths.clone());
        test_targets.extend(metadata.test_targets.clone());
    }

    let mut markdown_docs = Vec::new();
    gather_docs(repo_root, repo_root, &mut markdown_docs);

    let mut workflow_files = Vec::new();
    gather_workflows(repo_root, repo_root, &mut workflow_files);

    let mut example_paths = Vec::new();
    gather_examples(repo_root, repo_root, &mut example_paths);

    dedup_sorted(&mut workspace_members);
    dedup_sorted(&mut crate_names);
    dedup_sorted(&mut modules);
    dedup_sorted(&mut public_items);
    dedup_sorted(&mut test_targets);
    dedup_sorted(&mut feature_names);
    dedup_sorted(&mut dependencies);
    dedup_sorted(&mut markdown_docs);
    dedup_sorted(&mut workflow_files);
    dedup_sorted(&mut example_paths);

    SourceStats {
        workspace_members,
        crate_names,
        modules,
        public_items,
        rust_symbols,
        test_targets,
        feature_names,
        dependencies,
        markdown_docs,
        workflow_files,
        example_paths,
    }
}

fn build_instruction_graph(
    repo_root: &Path,
    source_stats: &SourceStats,
    training_courses: &[TrainingCourseDescriptor],
    knowledge_updates: &[KnowledgeUpdateDescriptor],
) -> InstructionScan {
    let mut descriptors = Vec::new();
    let mut commands = Vec::new();

    let mut doc_paths = source_stats.markdown_docs.clone();
    doc_paths.extend(source_stats.workflow_files.clone());
    dedup_sorted(&mut doc_paths);

    for path in &doc_paths {
        let raw = fs::read_to_string(repo_root.join(path)).unwrap_or_default();
        let matches = known_command_matches(&raw);
        let headings = collect_headings(&raw);
        let title = headings
            .first()
            .cloned()
            .unwrap_or_else(|| fallback_title(path));
        let lower_path = path.to_ascii_lowercase();
        let kind = if lower_path.ends_with(".yml") || lower_path.ends_with(".yaml") {
            "workflow"
        } else if lower_path.starts_with(".codex/") {
            "codex"
        } else {
            "doc"
        };
        let concept_ids = infer_instruction_concepts(path, &raw);

        commands.extend(matches.clone());
        descriptors.push(InstructionDescriptor {
            id: sanitize_id(path),
            path: path.clone(),
            title,
            kind: kind.to_string(),
            headings,
            commands: matches,
            concept_ids,
        });
    }

    for course in training_courses {
        commands.extend(course.canonical_commands.clone());
        commands.extend(
            course
                .modules
                .iter()
                .flat_map(|module| module.steps.iter())
                .filter_map(|step| step.command.clone()),
        );
        let mut headings = Vec::new();
        headings.push(course.summary.clone());
        headings.extend(course.tasks.clone());
        headings.extend(
            course
                .modules
                .iter()
                .map(|module| format!("{}: {}", module.title, module.objective)),
        );
        headings.extend(course.examples.clone());
        let mut concept_ids = course.teaches_concepts.clone();
        concept_ids.push("agent_training_course".to_string());
        dedup_sorted(&mut concept_ids);
        descriptors.push(InstructionDescriptor {
            id: format!("training_{}", sanitize_id(&course.id)),
            path: course
                .source_paths
                .first()
                .cloned()
                .unwrap_or_else(|| format!("{TRAINING_DIR}/{}.course.v1.json", course.id)),
            title: course.title.clone(),
            kind: "training_course".to_string(),
            headings,
            commands: course.canonical_commands.clone(),
            concept_ids,
        });
    }

    for update in knowledge_updates {
        commands.extend(
            update
                .deprecated_commands
                .iter()
                .map(|command| command.command.clone()),
        );
        commands.extend(
            update
                .deprecated_commands
                .iter()
                .filter_map(|command| command.replacement.clone()),
        );
        commands.extend(
            update
                .migration_steps
                .iter()
                .filter_map(|step| step.command.clone()),
        );
        let mut headings = vec![
            update.summary.clone(),
            update.agent_instruction.clone(),
            update.update_type.as_str().to_string(),
            update.severity.as_str().to_string(),
        ];
        if let Some(summary) = &update.human_summary {
            headings.push(summary.clone());
        }
        headings.extend(update.affected_workflows.clone());
        headings.extend(update.affected_courses.clone());
        headings.extend(
            update
                .new_capabilities
                .iter()
                .flat_map(|capability| {
                    [
                        capability.title.clone(),
                        capability.summary.clone(),
                        capability.use_when.join(" "),
                    ]
                })
                .collect::<Vec<_>>(),
        );
        headings.extend(
            update
                .replaced_guidance
                .iter()
                .flat_map(|guidance| {
                    [
                        guidance.old_guidance.clone(),
                        guidance.replacement_guidance.clone(),
                        guidance.reason.clone(),
                    ]
                })
                .collect::<Vec<_>>(),
        );
        headings.extend(
            update
                .migration_steps
                .iter()
                .map(|step| step.instruction.clone()),
        );
        let mut concept_ids = update.affected_concepts.clone();
        concept_ids.push("knowledge_update".to_string());
        dedup_sorted(&mut concept_ids);
        descriptors.push(InstructionDescriptor {
            id: format!("update_{}", sanitize_id(&update.id)),
            path: update
                .source_paths
                .first()
                .cloned()
                .unwrap_or_else(|| format!("{UPDATES_DIR}/{}.update.v1.json", update.id)),
            title: update.title.clone(),
            kind: "knowledge_update".to_string(),
            headings,
            commands: update
                .migration_steps
                .iter()
                .filter_map(|step| step.command.clone())
                .chain(
                    update
                        .deprecated_commands
                        .iter()
                        .map(|command| command.command.clone()),
                )
                .collect(),
            concept_ids,
        });
    }

    dedup_sorted(&mut commands);
    descriptors.sort_by(|left, right| left.path.cmp(&right.path));

    InstructionScan {
        paths: descriptors.iter().map(|entry| entry.path.clone()).collect(),
        descriptors,
        commands,
    }
}

fn load_training_courses(repo_root: &Path) -> Vec<TrainingCourseDescriptor> {
    let training_dir = repo_root.join(TRAINING_DIR);
    let Ok(entries) = fs::read_dir(&training_dir) else {
        return Vec::new();
    };

    let mut courses = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if !file_name.ends_with(".course.v1.json") {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(mut course) = serde_json::from_str::<TrainingCourseDescriptor>(&raw) else {
            continue;
        };
        if course.validate().is_err() {
            continue;
        }
        let relative = format!("{TRAINING_DIR}/{file_name}");
        if !course
            .source_paths
            .iter()
            .any(|existing| existing == &relative)
        {
            course.source_paths.push(relative);
        }
        course.source_paths.sort();
        courses.push(course);
    }
    courses.sort_by(|left, right| left.id.cmp(&right.id));
    courses
}

fn load_knowledge_updates(repo_root: &Path) -> Vec<KnowledgeUpdateDescriptor> {
    let updates_dir = repo_root.join(UPDATES_DIR);
    let Ok(entries) = fs::read_dir(&updates_dir) else {
        return Vec::new();
    };

    let mut updates = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if !file_name.ends_with(".update.v1.json") {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(mut update) = serde_json::from_str::<KnowledgeUpdateDescriptor>(&raw) else {
            continue;
        };
        if update.validate().is_err() {
            continue;
        }
        let relative = format!("{UPDATES_DIR}/{file_name}");
        if !update
            .source_paths
            .iter()
            .any(|existing| existing == &relative)
        {
            update.source_paths.push(relative);
        }
        update.source_paths.sort();
        updates.push(update);
    }
    updates.sort_by(|left, right| left.id.cmp(&right.id));
    updates
}

fn is_workspace_manifest(path: &str) -> bool {
    path == "Cargo.toml"
}

fn parse_workspace_members(raw: &str) -> Vec<String> {
    let mut members = Vec::new();
    let mut in_members = false;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("members") && trimmed.contains('[') {
            in_members = true;
            continue;
        }
        if in_members {
            if trimmed.starts_with(']') {
                break;
            }
            let value = trimmed.trim_end_matches(',').trim().trim_matches('"');
            if !value.is_empty() {
                members.push(value.to_string());
            }
        }
    }

    members
}

fn parse_manifest_string(raw: &str, key: &str) -> Option<String> {
    for line in raw.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with(key) || !trimmed.contains('=') {
            continue;
        }

        let (_, value) = trimmed.split_once('=')?;
        let value = value.trim().trim_matches('"');
        if !value.is_empty() && !value.contains('{') {
            return Some(value.to_string());
        }
    }

    None
}

fn parse_feature_names(raw: &str) -> Vec<String> {
    parse_table_keys(raw, "[features]")
}

fn parse_dependency_names(raw: &str) -> Vec<String> {
    let mut dependencies = parse_table_keys(raw, "[dependencies]");
    dependencies.extend(parse_table_keys(raw, "[dev-dependencies]"));
    dedup_sorted(&mut dependencies);
    dependencies
}

fn parse_table_keys(raw: &str, table_name: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut in_table = false;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_table = trimmed == table_name;
            continue;
        }
        if !in_table || trimmed.is_empty() || trimmed.starts_with('#') || !trimmed.contains('=') {
            continue;
        }

        if let Some((key, _)) = trimmed.split_once('=') {
            let key = key.trim();
            if !key.is_empty() {
                keys.push(key.to_string());
            }
        }
    }

    keys
}

fn gather_files_named(root: &Path, current: &Path, file_name: &str, output: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();

        if should_skip_dir(&name) {
            continue;
        }

        if path.is_dir() {
            gather_files_named(root, &path, file_name, output);
        } else if name == file_name
            && let Ok(relative) = path.strip_prefix(root)
        {
            output.push(relative.display().to_string());
        }
    }
}

fn gather_regular_files(root: &Path, current: &Path, output: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();

        if should_skip_dir(&name) {
            continue;
        }

        if path.is_dir() {
            gather_regular_files(root, &path, output);
        } else if let Ok(relative) = path.strip_prefix(root) {
            output.push(relative.display().to_string());
        }
    }
}

fn gather_docs(root: &Path, current: &Path, output: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();

        if should_skip_dir(&name) {
            continue;
        }

        if path.is_dir() {
            gather_docs(root, &path, output);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("md")
            && let Ok(relative) = path.strip_prefix(root)
        {
            output.push(relative.display().to_string());
        }
    }
}

fn gather_workflows(root: &Path, current: &Path, output: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();

        if should_skip_dir(&name) {
            continue;
        }

        if path.is_dir() {
            gather_workflows(root, &path, output);
        } else if (path.extension().and_then(|ext| ext.to_str()) == Some("yml")
            || path.extension().and_then(|ext| ext.to_str()) == Some("yaml"))
            && let Ok(relative) = path.strip_prefix(root)
        {
            let relative = relative.display().to_string();
            if relative.starts_with(".github/workflows/") {
                output.push(relative);
            }
        }
    }
}

fn gather_examples(root: &Path, current: &Path, output: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();

        if should_skip_dir(&name) {
            continue;
        }

        if path.is_dir() {
            gather_examples(root, &path, output);
        } else if let Ok(relative) = path.strip_prefix(root) {
            let relative = relative.display().to_string();
            if relative.starts_with("examples/") {
                output.push(relative);
            }
        }
    }
}

fn should_skip_dir(name: &str) -> bool {
    name == "target" || name == ".git" || name == LOCAL_INDEX_DIR
}

fn collect_headings(raw: &str) -> Vec<String> {
    raw.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            trimmed
                .strip_prefix('#')
                .map(|rest| rest.trim_start_matches('#').trim().to_string())
                .filter(|heading| !heading.is_empty())
        })
        .collect()
}

fn fallback_title(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_string()
}

fn infer_instruction_concepts(path: &str, raw: &str) -> Vec<String> {
    let lower = format!("{} {}", path.to_ascii_lowercase(), raw.to_ascii_lowercase());
    let mapping = [
        ("digital_worker", &["digital worker"][..]),
        ("application_pack", &["application pack", "pack"][..]),
        ("extension_pack", &["extension pack"][..]),
        ("wizard", &["wizard"][..]),
        ("setup", &["setup"][..]),
        ("start", &["start"][..]),
        ("greentic_x", &["greentic-x", "greentic x"][..]),
        ("greentic_sorla", &["greentic-sorla", "greentic sorla"][..]),
    ];

    let mut concepts = mapping
        .iter()
        .filter(|(_, needles)| needles.iter().any(|needle| lower.contains(needle)))
        .map(|(id, _)| (*id).to_string())
        .collect::<Vec<_>>();
    dedup_sorted(&mut concepts);
    concepts
}

fn sanitize_id(path: &str) -> String {
    path.chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' => character.to_ascii_lowercase(),
            _ => '_',
        })
        .collect()
}

fn dedup_sorted(values: &mut Vec<String>) {
    values.sort();
    values.dedup();
}

fn timestamp_string() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("unix:{seconds}")
}

#[cfg(test)]
mod tests {
    use super::{analyze_repo, default_registry_path, parse_github_remote_url};
    use gca_core::{SourceStats, load_registry};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn analyze_repo_creates_local_index_outputs_and_registry_entry() {
        let temp = tempdir().unwrap();
        let repo_root = temp.path().join("demo-repo");
        create_demo_repo(&repo_root);

        let registry_path = default_registry_path(temp.path());
        let outputs = analyze_repo(&repo_root, &registry_path).unwrap();

        assert!(outputs.manifest_path.exists());
        assert!(outputs.repo_index_path.exists());
        assert!(outputs.fingerprints_path.exists());
        let tantivy_report = outputs.tantivy_report.as_ref().unwrap();
        assert!(tantivy_report.index_path.exists());
        assert!(tantivy_report.documents_indexed > 0);
        assert_eq!(outputs.manifest.repo_name, "demo-repo");
        assert_eq!(outputs.manifest.repo_id, "greenticai/demo-repo");
        assert_eq!(outputs.manifest.org.as_deref(), Some("greenticai"));
        assert_eq!(outputs.repo_index.repo_id, "greenticai/demo-repo");
        assert_eq!(outputs.fingerprints.head_sha, "abc123");
        assert!(!outputs.repo_index.concept_graph.is_empty());
        assert!(
            outputs
                .repo_index
                .instruction_graph
                .iter()
                .any(|entry| entry.path == ".codex/PR-04.md")
        );
        assert!(
            outputs
                .repo_index
                .workflow_graph
                .iter()
                .any(|workflow| workflow.id == "wizard_bootstrap")
        );
        assert!(
            outputs
                .repo_index
                .reuse
                .iter()
                .any(|entry| entry.concept_id == "extension_pack")
        );
        assert!(
            outputs
                .repo_index
                .validations
                .iter()
                .any(|entry| entry.id == "shared_schema_changed")
        );
        assert!(
            outputs
                .repo_index
                .training_courses
                .iter()
                .any(|course| course.id == "create_demo_component")
        );
        assert!(
            outputs
                .repo_index
                .knowledge_updates
                .iter()
                .any(|update| update.id == "component_answers_flow")
        );
        assert!(outputs.repo_index.instruction_graph.iter().any(|entry| {
            entry.kind == "training_course"
                && entry
                    .commands
                    .iter()
                    .any(|command| command == "greentic-flow wizard --answers answers.json")
        }));

        let registry = load_registry(&registry_path).unwrap();
        assert_eq!(registry.repos.len(), 1);
        assert_eq!(registry.repos[0].repo_name, "demo-repo");
        assert_eq!(registry.repos[0].repo_id, "greenticai/demo-repo");
    }

    #[test]
    fn github_remote_urls_parse_to_repo_id() {
        assert_eq!(
            parse_github_remote_url("git@github.com:greenticai/greentic-coding-agent.git")
                .as_deref(),
            Some("greenticai/greentic-coding-agent")
        );
        assert_eq!(
            parse_github_remote_url("https://github.com/greentic-biz/meeza-store.git").as_deref(),
            Some("greentic-biz/meeza-store")
        );
        assert_eq!(
            parse_github_remote_url("ssh://git@github.com/greenticai/greentic-types.git")
                .as_deref(),
            Some("greenticai/greentic-types")
        );
        assert!(parse_github_remote_url("https://example.com/greenticai/nope.git").is_none());
    }

    #[test]
    fn analyze_repo_updates_existing_registry_entry_idempotently() {
        let temp = tempdir().unwrap();
        let repo_root = temp.path().join("demo-repo");
        create_demo_repo(&repo_root);

        let registry_path = default_registry_path(temp.path());
        analyze_repo(&repo_root, &registry_path).unwrap();
        analyze_repo(&repo_root, &registry_path).unwrap();

        let registry = load_registry(&registry_path).unwrap();
        assert_eq!(registry.repos.len(), 1);
    }

    #[test]
    fn analyze_repo_uses_cargo_metadata_and_structured_rust_symbols() {
        let temp = tempdir().unwrap();
        let repo_root = temp.path().join("metadata-repo");
        create_metadata_repo(&repo_root);

        let outputs = analyze_repo(&repo_root, &default_registry_path(temp.path())).unwrap();
        let stats = &outputs.repo_index.source_stats;

        assert!(stats.workspace_members.contains(&"crates/api".to_string()));
        assert!(
            stats
                .workspace_members
                .contains(&"crates/support".to_string())
        );
        assert!(stats.crate_names.contains(&"api".to_string()));
        assert!(stats.crate_names.contains(&"support".to_string()));
        assert!(stats.dependencies.contains(&"support".to_string()));
        assert!(stats.feature_names.contains(&"default".to_string()));
        assert!(stats.feature_names.contains(&"cli".to_string()));
        assert!(stats.modules.contains(&"crates/api/src/lib.rs".to_string()));
        assert!(
            stats
                .modules
                .contains(&"crates/api/src/main.rs".to_string())
        );
        assert!(
            stats
                .test_targets
                .contains(&"crates/api/tests/smoke.rs".to_string())
        );
        assert!(
            stats
                .test_targets
                .contains(&"crates/api/src/lib.rs::api_test".to_string())
        );

        assert_symbol(
            stats,
            "inner::ApiThing",
            "struct",
            "pub",
            "crates/api/src/lib.rs",
        );
        assert_symbol(
            stats,
            "internal_helper",
            "function",
            "pub(crate)",
            "crates/api/src/lib.rs",
        );
        assert_symbol(
            stats,
            "ApiThing::build",
            "function",
            "pub",
            "crates/api/src/lib.rs",
        );
        assert_symbol(
            stats,
            "inner::ApiMode",
            "enum",
            "pub",
            "crates/api/src/lib.rs",
        );
        assert_symbol(
            stats,
            "inner::ApiBehavior",
            "trait",
            "pub",
            "crates/api/src/lib.rs",
        );
        assert_symbol(
            stats,
            "inner::ApiThing",
            "use",
            "pub",
            "crates/api/src/lib.rs",
        );
        assert_symbol(
            stats,
            "api_test",
            "test",
            "private",
            "crates/api/src/lib.rs",
        );
    }

    fn create_demo_repo(repo_root: &std::path::Path) {
        fs::create_dir_all(repo_root.join(".git").join("refs").join("heads")).unwrap();
        fs::create_dir_all(repo_root.join("docs")).unwrap();
        fs::create_dir_all(repo_root.join(".codex")).unwrap();
        fs::create_dir_all(repo_root.join(".greentic").join("training")).unwrap();
        fs::create_dir_all(repo_root.join(".greentic").join("updates")).unwrap();
        fs::create_dir_all(repo_root.join("src")).unwrap();
        fs::create_dir_all(repo_root.join(".github").join("workflows")).unwrap();
        fs::create_dir_all(repo_root.join("examples")).unwrap();
        fs::write(
            repo_root.join("Cargo.toml"),
            "[workspace]\nmembers = [\n  \"crates/demo\"\n]\n\n[package]\nname = \"demo\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\n\n[features]\ndefault = []\ncli = []\n",
        )
        .unwrap();
        fs::write(
            repo_root.join("README.md"),
            "# Demo\n\nUse `gtc wizard --schema demo.json` and `gtc start demo-bundle`.\n",
        )
        .unwrap();
        fs::write(
            repo_root.join("docs").join("architecture.md"),
            "# Architecture\n\nGreentic-X digital worker setup guidance.\n",
        )
        .unwrap();
        fs::write(
            repo_root.join(".codex").join("PR-04.md"),
            "# PR-04\n\nRun `gtc setup demo --answers answers.json`.\n",
        )
        .unwrap();
        fs::write(
            repo_root.join("src").join("lib.rs"),
            "pub fn example_hot_path() {}\n\n#[test]\nfn demo_test() {}\n",
        )
        .unwrap();
        fs::write(
            repo_root.join(".github").join("workflows").join("perf.yml"),
            "name: Perf\nsteps:\n  - run: gtc wizard --answers answers.json\n",
        )
        .unwrap();
        fs::write(
            repo_root.join("examples").join("demo.md"),
            "# Example\n\nGreentic-sorla walkthrough.\n",
        )
        .unwrap();
        fs::write(
            repo_root
                .join(".greentic")
                .join("training")
                .join("create-demo-component.course.v1.json"),
            r#"{
              "version": "v1",
              "id": "create_demo_component",
              "title": "Create demo component",
              "summary": "Create a demo component through the current answers flow.",
              "owner_repo": "greentic-component",
              "teaches_concepts": ["component", "wizard"],
              "tasks": ["create a component"],
              "audience": ["coding_agent"],
              "lifecycle_phase": "build",
              "modules": [{
                "id": "answers",
                "title": "Answers flow",
                "objective": "Use answers rather than obsolete commands.",
                "steps": [{
                  "order": 1,
                  "instruction": "Apply answers through the wizard.",
                  "command": "greentic-flow wizard --answers answers.json",
                  "expected_output": "Component files are generated.",
                  "validation": "greentic-flow component-qa --answers answers.json"
                }]
              }],
              "canonical_commands": ["greentic-flow wizard --answers answers.json"],
              "deprecated_commands": [{
                "command": "gtc component new",
                "reason": "The current flow is schema and answers driven.",
                "replacement": "greentic-flow wizard --answers answers.json"
              }],
              "required_validations": ["greentic-flow component-qa --answers answers.json"],
              "examples": [],
              "source_paths": []
            }"#,
        )
        .unwrap();
        fs::write(
            repo_root
                .join(".greentic")
                .join("updates")
                .join("component-answers-flow.update.v1.json"),
            r#"{
              "version": "v1",
              "id": "component_answers_flow",
              "title": "Component creation uses wizard answers",
              "summary": "Agents must use the current wizard answers flow for component creation.",
              "owner_repo": "greentic-component",
              "update_type": "deprecated_command",
              "published_at": "2026-04-26",
              "effective_from": "2026-04-26",
              "expires_at": null,
              "affected_concepts": ["component", "wizard"],
              "affected_workflows": ["component_creation"],
              "affected_courses": ["create_demo_component"],
              "affected_repos": ["greentic-component"],
              "agent_instruction": "Use greentic-flow component-schema and greentic-flow wizard --answers answers.json.",
              "human_summary": "Old component creation commands are stale.",
              "new_capabilities": [{
                "id": "component_answers",
                "title": "Component answers flow",
                "summary": "Components are created through a schema and answers file.",
                "use_when": ["create a component"],
                "owner_repo": "greentic-component",
                "related_course": "create_demo_component"
              }],
              "deprecated_commands": [{
                "command": "gtc component new",
                "reason": "The current flow is schema and answers driven.",
                "replacement": "greentic-flow wizard --answers answers.json"
              }],
              "replaced_guidance": [{
                "old_guidance": "Run gtc component new.",
                "replacement_guidance": "Capture schema, write answers.json, then run greentic-flow wizard --answers answers.json.",
                "reason": "The wizard answers contract is now authoritative."
              }],
              "migration_steps": [{
                "order": 1,
                "instruction": "Replace old component creation commands with the answers flow.",
                "command": "greentic-flow component-schema",
                "validation": "greentic-flow component-qa --answers answers.json"
              }],
              "required_validations": ["greentic-flow component-qa --answers answers.json"],
              "source_paths": [],
              "severity": "breaking"
            }"#,
        )
        .unwrap();
        fs::write(
            repo_root.join(".git").join("HEAD"),
            "ref: refs/heads/main\n",
        )
        .unwrap();
        fs::write(
            repo_root.join(".git").join("config"),
            "[remote \"origin\"]\n    url = git@github.com:greenticai/demo-repo.git\n",
        )
        .unwrap();
        fs::write(
            repo_root
                .join(".git")
                .join("refs")
                .join("heads")
                .join("main"),
            "abc123\n",
        )
        .unwrap();
    }

    fn create_metadata_repo(repo_root: &std::path::Path) {
        fs::create_dir_all(repo_root.join(".git").join("refs").join("heads")).unwrap();
        fs::create_dir_all(repo_root.join("crates").join("api").join("src")).unwrap();
        fs::create_dir_all(repo_root.join("crates").join("api").join("tests")).unwrap();
        fs::create_dir_all(repo_root.join("crates").join("support").join("src")).unwrap();
        fs::write(
            repo_root.join(".git").join("HEAD"),
            "ref: refs/heads/main\n",
        )
        .unwrap();
        fs::write(
            repo_root
                .join(".git")
                .join("refs")
                .join("heads")
                .join("main"),
            "def456\n",
        )
        .unwrap();
        fs::write(
            repo_root.join("Cargo.toml"),
            r#"[workspace]
members = ["crates/api", "crates/support"]
resolver = "2"

[workspace.dependencies]
support = { path = "crates/support" }
"#,
        )
        .unwrap();
        fs::write(
            repo_root.join("crates").join("api").join("Cargo.toml"),
            r#"[package]
name = "api"
version = "0.1.0"
edition = "2024"

[dependencies]
support.workspace = true

[dev-dependencies]
support.workspace = true

[features]
default = ["cli"]
cli = []

[lib]
path = "src/lib.rs"

[[bin]]
name = "api-bin"
path = "src/main.rs"

[[test]]
name = "smoke"
path = "tests/smoke.rs"
"#,
        )
        .unwrap();
        fs::write(
            repo_root.join("crates").join("support").join("Cargo.toml"),
            r#"[package]
name = "support"
version = "0.1.0"
edition = "2024"

[lib]
path = "src/lib.rs"
"#,
        )
        .unwrap();
        fs::write(
            repo_root
                .join("crates")
                .join("api")
                .join("src")
                .join("lib.rs"),
            r#"pub use inner::ApiThing;

pub(crate) fn internal_helper() {}

pub mod inner {
    pub struct ApiThing {
        pub id: String,
    }

    pub enum ApiMode {
        Fast,
    }

    pub trait ApiBehavior {
        fn run(&self);
    }
}

impl inner::ApiThing {
    pub fn build() -> Self {
        Self { id: String::new() }
    }
}

#[test]
fn api_test() {}
"#,
        )
        .unwrap();
        fs::write(
            repo_root
                .join("crates")
                .join("api")
                .join("src")
                .join("main.rs"),
            "fn main() {}\n",
        )
        .unwrap();
        fs::write(
            repo_root
                .join("crates")
                .join("api")
                .join("tests")
                .join("smoke.rs"),
            "#[test]\nfn smoke_test() {}\n",
        )
        .unwrap();
        fs::write(
            repo_root
                .join("crates")
                .join("support")
                .join("src")
                .join("lib.rs"),
            "pub fn support_value() -> u32 { 1 }\n",
        )
        .unwrap();
    }

    fn assert_symbol(stats: &SourceStats, name: &str, kind: &str, visibility: &str, path: &str) {
        assert!(
            stats.rust_symbols.iter().any(|symbol| {
                symbol.name == name
                    && format!("{:?}", symbol.kind).eq_ignore_ascii_case(kind)
                    && symbol.visibility == visibility
                    && symbol.path.starts_with(path)
            }),
            "missing symbol {visibility} {kind} {name} in {path}: {:?}",
            stats.rust_symbols
        );
    }
}
