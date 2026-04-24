mod tantivy_index;

use gca_core::{
    FreshnessStatus, InstructionDescriptor, RegistryEntry, RepoAgentManifest, RepoId, RepoIndex,
    SourceStats, builtin_concepts, load_registry, write_registry,
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

pub use tantivy_index::{TantivyBuildReport, TantivyIndexError, build_local_tantivy_index};

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
    let instruction_scan = build_instruction_graph(&repo_root, &source_stats);

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
    let mut workspace_members = Vec::new();
    let mut crate_names = Vec::new();
    let mut feature_names = Vec::new();
    let mut dependencies = Vec::new();

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

    let mut modules = Vec::new();
    let mut public_items = Vec::new();
    let mut test_targets = Vec::new();
    gather_rust_sources(
        repo_root,
        repo_root,
        &mut modules,
        &mut public_items,
        &mut test_targets,
    );

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
        test_targets,
        feature_names,
        dependencies,
        markdown_docs,
        workflow_files,
        example_paths,
    }
}

fn build_instruction_graph(repo_root: &Path, source_stats: &SourceStats) -> InstructionScan {
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

    dedup_sorted(&mut commands);
    descriptors.sort_by(|left, right| left.path.cmp(&right.path));

    InstructionScan {
        paths: descriptors.iter().map(|entry| entry.path.clone()).collect(),
        descriptors,
        commands,
    }
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

fn gather_rust_sources(
    root: &Path,
    current: &Path,
    modules: &mut Vec<String>,
    public_items: &mut Vec<String>,
    test_targets: &mut Vec<String>,
) {
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
            gather_rust_sources(root, &path, modules, public_items, test_targets);
            continue;
        }

        let Some(extension) = path.extension().and_then(|ext| ext.to_str()) else {
            continue;
        };
        if extension != "rs" {
            continue;
        }

        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        let relative = relative.display().to_string();
        modules.push(relative.clone());
        if relative.contains("/tests/") || relative.starts_with("tests/") {
            test_targets.push(relative.clone());
        }

        let raw = fs::read_to_string(&path).unwrap_or_default();
        for line in raw.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("#[test]") || trimmed.starts_with("#[tokio::test]") {
                test_targets.push(relative.clone());
            }
            if trimmed.starts_with("pub ") {
                public_items.push(trimmed.to_string());
            }
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
    use gca_core::load_registry;
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

    fn create_demo_repo(repo_root: &std::path::Path) {
        fs::create_dir_all(repo_root.join(".git").join("refs").join("heads")).unwrap();
        fs::create_dir_all(repo_root.join("docs")).unwrap();
        fs::create_dir_all(repo_root.join(".codex")).unwrap();
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
}
