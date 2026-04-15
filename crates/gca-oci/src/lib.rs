use gca_agent_files::render_generated_files;
use gca_core::{Catalog, CatalogRepo, RepoIndex, SCHEMA_VERSION_V1};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

const OCI_LAYOUT_VERSION: &str = "1.0.0";
const LOCAL_INDEX_DIR: &str = ".greentic-agent";
const GENERATOR_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageMetadata {
    pub repo_name: String,
    pub tag: String,
    pub reference: String,
    pub generated_at: String,
    pub compatibility: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageOutput {
    pub package_dir: PathBuf,
    pub reference: String,
    pub metadata: PackageMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteRepo {
    pub repo_name: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshCheck {
    pub needs_refresh: bool,
    pub reasons: Vec<String>,
    pub current_head_sha: String,
    pub indexed_head_sha: Option<String>,
    pub current_tracked_files: Vec<String>,
    pub indexed_tracked_files: Vec<String>,
    pub current_generator_version: String,
    pub indexed_generator_version: Option<String>,
    pub current_schema_version: String,
    pub indexed_schema_version: Option<String>,
}

pub fn package_index(
    repo_root: &Path,
    repo_index: &RepoIndex,
    tag: &str,
    output_root: &Path,
) -> std::io::Result<PackageOutput> {
    let package_dir = output_root.join(&repo_index.repo_name).join(tag);
    let artifacts_dir = package_dir.join("artifacts");
    let agents_dir = artifacts_dir.join("agents");
    let blobs_dir = package_dir.join("blobs").join("sha256");
    fs::create_dir_all(&agents_dir)?;
    fs::create_dir_all(&blobs_dir)?;

    let manifest_path = repo_root.join(".greentic-agent").join("manifest.json");
    let index_path = repo_root.join(".greentic-agent").join("repo-index.json");
    let manifest_bytes = fs::read(&manifest_path)?;
    let index_bytes = fs::read(&index_path)?;

    let manifest_out = artifacts_dir.join("repo-manifest.json");
    let index_out = artifacts_dir.join("repo-index.json");
    fs::write(&manifest_out, &manifest_bytes)?;
    fs::write(&index_out, &index_bytes)?;

    let generated = render_generated_files(repo_index);
    let mut files = vec![
        "repo-manifest.json".to_string(),
        "repo-index.json".to_string(),
    ];
    for file in &generated {
        let path = agents_dir.join(&file.file_name);
        fs::write(&path, &file.content)?;
        files.push(format!("agents/{}", file.file_name));
    }

    let reference = format!(
        "ghcr.io/greenticai/indexes/{}:{}",
        repo_index.repo_name, tag
    );
    let metadata = PackageMetadata {
        repo_name: repo_index.repo_name.clone(),
        tag: tag.to_string(),
        reference: reference.clone(),
        generated_at: repo_index.generated_at.clone(),
        compatibility: repo_index.version.clone(),
        files: files.clone(),
    };
    let metadata_path = artifacts_dir.join("package-metadata.json");
    fs::write(
        &metadata_path,
        serde_json::to_string_pretty(&metadata).expect("metadata should serialize"),
    )?;
    files.push("package-metadata.json".to_string());

    let mut descriptors = Vec::new();
    for relative in &files {
        let path = artifacts_dir.join(relative);
        let bytes = fs::read(&path)?;
        let digest = digest_hex(&bytes);
        let blob_path = blobs_dir.join(&digest);
        fs::write(&blob_path, &bytes)?;
        descriptors.push(serde_json::json!({
            "mediaType": media_type_for(relative),
            "digest": format!("sha256:{digest}"),
            "size": bytes.len(),
            "annotations": {
                "org.opencontainers.image.title": relative
            }
        }));
    }

    fs::write(
        package_dir.join("oci-layout"),
        serde_json::to_string_pretty(&serde_json::json!({
            "imageLayoutVersion": OCI_LAYOUT_VERSION
        }))
        .expect("oci layout should serialize"),
    )?;
    fs::write(
        package_dir.join("index.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "schemaVersion": 2,
            "manifests": descriptors,
            "annotations": {
                "org.opencontainers.image.ref.name": reference
            }
        }))
        .expect("oci index should serialize"),
    )?;

    Ok(PackageOutput {
        package_dir,
        reference,
        metadata,
    })
}

pub fn publish_local_package(
    package_dir: &Path,
    remote_root: &Path,
    repo_name: &str,
    tag: &str,
) -> std::io::Result<PathBuf> {
    let target = remote_root.join(repo_name).join(tag);
    copy_dir_all(package_dir, &target)?;
    Ok(target)
}

pub fn sync_local_package(
    remote_root: &Path,
    cache_root: &Path,
    repo_name: &str,
    tag: &str,
) -> std::io::Result<PathBuf> {
    let source = remote_root.join(repo_name).join(tag);
    let target = cache_root.join(repo_name).join(tag);
    copy_dir_all(&source, &target)?;
    Ok(target)
}

pub fn list_remote_repos(remote_root: &Path) -> std::io::Result<Vec<RemoteRepo>> {
    let Ok(entries) = fs::read_dir(remote_root) else {
        return Ok(Vec::new());
    };

    let mut repos = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let repo_name = entry.file_name().to_string_lossy().to_string();
        let mut tags = Vec::new();
        if let Ok(tag_entries) = fs::read_dir(&path) {
            for tag_entry in tag_entries.flatten() {
                if tag_entry.path().is_dir() {
                    tags.push(tag_entry.file_name().to_string_lossy().to_string());
                }
            }
        }
        tags.sort();
        repos.push(RemoteRepo { repo_name, tags });
    }
    repos.sort_by(|left, right| left.repo_name.cmp(&right.repo_name));
    Ok(repos)
}

pub fn build_catalog(remote_root: &Path) -> std::io::Result<Catalog> {
    let repos = list_remote_repos(remote_root)?;
    let mut catalog_repos = Vec::new();

    for repo in repos {
        let Some(latest_tag) = repo.tags.last().cloned() else {
            continue;
        };
        let repo_index = load_repo_index(
            &remote_root
                .join(&repo.repo_name)
                .join(&latest_tag)
                .join("artifacts")
                .join("repo-index.json"),
        )?;
        catalog_repos.push(CatalogRepo {
            repo_name: repo.repo_name,
            repo_role: repo_index.repo_role,
            latest_tag: latest_tag.clone(),
            package_ref: format!(
                "ghcr.io/greenticai/indexes/{}:{}",
                repo_index.repo_name, latest_tag
            ),
            updated_at: repo_index.generated_at,
        });
    }

    catalog_repos.sort_by(|left, right| left.repo_name.cmp(&right.repo_name));
    Ok(Catalog {
        version: SCHEMA_VERSION_V1.to_string(),
        generated_at: timestamp_string(),
        repos: catalog_repos,
    })
}

pub fn sync_catalog(remote_root: &Path, cache_root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let catalog = build_catalog(remote_root)?;
    let mut synced = Vec::new();
    for repo in catalog.repos {
        synced.push(sync_local_package(
            remote_root,
            cache_root,
            &repo.repo_name,
            &repo.latest_tag,
        )?);
    }
    Ok(synced)
}

pub fn check_refresh(repo_root: &Path) -> std::io::Result<RefreshCheck> {
    let fingerprints_path = repo_root.join(LOCAL_INDEX_DIR).join("fingerprints.json");
    let repo_index_path = repo_root.join(LOCAL_INDEX_DIR).join("repo-index.json");
    let current_head_sha = read_head_sha(repo_root).unwrap_or_else(|| "unknown".to_string());
    let current_tracked_files = find_tracked_files(repo_root);
    let indexed_fingerprints = load_optional_fingerprints(&fingerprints_path)?;
    let indexed_repo_index = load_optional_repo_index(&repo_index_path)?;
    let indexed_head_sha = indexed_fingerprints
        .as_ref()
        .map(|value| value.head_sha.clone());
    let indexed_tracked_files = indexed_fingerprints
        .as_ref()
        .map(|value| value.tracked_files.clone())
        .unwrap_or_default();
    let indexed_generator_version = indexed_fingerprints
        .as_ref()
        .and_then(|value| value.generator_version.clone());
    let indexed_schema_version = indexed_fingerprints
        .as_ref()
        .map(|value| value.version.clone());

    let mut reasons = Vec::new();
    if indexed_fingerprints.is_none() {
        reasons.push("missing fingerprints.json".to_string());
    }
    if indexed_repo_index.is_none() {
        reasons.push("missing repo-index.json".to_string());
    }
    if let Some(indexed_head_sha) = &indexed_head_sha
        && indexed_head_sha != &current_head_sha
    {
        reasons.push(format!(
            "source commit changed: indexed={}, current={}",
            indexed_head_sha, current_head_sha
        ));
    }
    if indexed_tracked_files != current_tracked_files {
        reasons.push("indexed file fingerprint changed".to_string());
    }
    if indexed_generator_version.as_deref() != Some(GENERATOR_VERSION) {
        reasons.push(format!(
            "generator version changed: indexed={}, current={}",
            indexed_generator_version
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            GENERATOR_VERSION
        ));
    }
    if indexed_schema_version.as_deref() != Some(SCHEMA_VERSION_V1) {
        reasons.push(format!(
            "schema version changed: indexed={}, current={}",
            indexed_schema_version
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            SCHEMA_VERSION_V1
        ));
    }

    Ok(RefreshCheck {
        needs_refresh: !reasons.is_empty(),
        reasons,
        current_head_sha,
        indexed_head_sha,
        current_tracked_files,
        indexed_tracked_files,
        current_generator_version: GENERATOR_VERSION.to_string(),
        indexed_generator_version,
        current_schema_version: SCHEMA_VERSION_V1.to_string(),
        indexed_schema_version,
    })
}

pub fn install_github_workflow(repo_root: &Path) -> std::io::Result<PathBuf> {
    let workflow_path = repo_root
        .join(".github")
        .join("workflows")
        .join("greentic-agent-index.yml");
    if let Some(parent) = workflow_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&workflow_path, render_github_workflow())?;
    Ok(workflow_path)
}

pub fn render_github_workflow() -> String {
    r#"name: Greentic Agent Index

on:
  push:
    branches: [main]
  schedule:
    - cron: "17 2 * * *"
  workflow_dispatch:

permissions:
  contents: read
  packages: write

jobs:
  index:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Analyze repo
        run: cargo run --package greentic-coding-agent -- analyze --print --format json | tee .greentic-agent-analyze.json

      - name: Check refresh
        run: cargo run --package greentic-coding-agent -- check-refresh --format json | tee .greentic-agent-refresh.json

      - name: Package index
        run: cargo run --package greentic-coding-agent -- package-index --tag latest --format json | tee .greentic-agent-package.json

      - name: Publish index when refresh is needed
        shell: bash
        run: |
          if cargo run --package greentic-coding-agent -- check-refresh --format json | grep -q '"needs_refresh": true'; then
            cargo run --package greentic-coding-agent -- publish-index --tag latest --format json | tee .greentic-agent-publish.json
          else
            echo '{"published": false, "reason": "refresh not required"}' | tee .greentic-agent-publish.json
          fi

      - name: Upload summaries
        uses: actions/upload-artifact@v4
        with:
          name: greentic-agent-index-summary
          path: |
            .greentic-agent-analyze.json
            .greentic-agent-refresh.json
            .greentic-agent-package.json
            .greentic-agent-publish.json
"#
    .to_string()
}

fn copy_dir_all(source: &Path, target: &Path) -> std::io::Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_all(&source_path, &target_path)?;
        } else {
            fs::copy(&source_path, &target_path)?;
        }
    }
    Ok(())
}

fn digest_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn media_type_for(path: &str) -> &'static str {
    if path.ends_with(".json") {
        "application/json"
    } else {
        "text/markdown"
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct IndexedFingerprints {
    version: String,
    head_sha: String,
    default_branch: Option<String>,
    tracked_files: Vec<String>,
    #[serde(default)]
    generator_version: Option<String>,
}

fn load_repo_index(path: &Path) -> std::io::Result<RepoIndex> {
    let raw = fs::read_to_string(path)?;
    serde_json::from_str(&raw)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

fn load_optional_repo_index(path: &Path) -> std::io::Result<Option<RepoIndex>> {
    if !path.exists() {
        return Ok(None);
    }
    load_repo_index(path).map(Some)
}

fn load_optional_fingerprints(path: &Path) -> std::io::Result<Option<IndexedFingerprints>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)?;
    let parsed = serde_json::from_str(&raw)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    Ok(Some(parsed))
}

fn read_head_sha(repo_root: &Path) -> Option<String> {
    let head = fs::read_to_string(repo_root.join(".git").join("HEAD")).ok()?;
    let head = head.trim();
    if let Some(reference) = head.strip_prefix("ref: ") {
        let reference_path = repo_root.join(".git").join(reference);
        fs::read_to_string(reference_path)
            .ok()
            .map(|value| value.trim().to_string())
    } else {
        Some(head.to_string())
    }
}

fn find_tracked_files(repo_root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    collect_tracked_files(repo_root, repo_root, &mut files);
    files.sort();
    files
}

fn collect_tracked_files(repo_root: &Path, current: &Path, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == "target" {
            continue;
        }
        if current == repo_root && (name == ".git" || name == LOCAL_INDEX_DIR) {
            continue;
        }
        if path.is_dir() {
            collect_tracked_files(repo_root, &path, out);
        } else if let Ok(relative) = path.strip_prefix(repo_root) {
            out.push(relative.display().to_string());
        }
    }
}

fn timestamp_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => format!("{}Z", duration.as_secs()),
        Err(_) => "0Z".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_catalog, check_refresh, install_github_workflow, list_remote_repos, package_index,
        publish_local_package, sync_catalog, sync_local_package,
    };
    use gca_core::{
        ConceptDescriptor, FreshnessStatus, InstructionDescriptor, KnowledgeScope, LifecyclePhase,
        RepoAgentManifest, RepoIndex, RepoRole, ReuseDescriptor, SourceStats, ValidationDescriptor,
        WorkflowDescriptor,
    };
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn package_index_writes_oci_like_layout() {
        let temp = tempdir().unwrap();
        let repo_root = temp.path();
        fs::create_dir_all(repo_root.join(".greentic-agent")).unwrap();
        fs::write(repo_root.join(".greentic-agent/manifest.json"), "{}").unwrap();
        fs::write(repo_root.join(".greentic-agent/repo-index.json"), "{}").unwrap();

        let output = package_index(
            repo_root,
            &demo_repo_index(),
            "latest",
            &repo_root.join(".greentic-agent/oci"),
        )
        .unwrap();

        assert!(output.package_dir.join("oci-layout").exists());
        assert!(output.package_dir.join("index.json").exists());
        assert!(
            output
                .package_dir
                .join("artifacts/agents/AGENTS.md")
                .exists()
        );
    }

    #[test]
    fn publish_and_sync_round_trip_through_local_remote_store() {
        let temp = tempdir().unwrap();
        let repo_root = temp.path().join("repo");
        fs::create_dir_all(repo_root.join(".greentic-agent")).unwrap();
        fs::write(repo_root.join(".greentic-agent/manifest.json"), "{}").unwrap();
        fs::write(repo_root.join(".greentic-agent/repo-index.json"), "{}").unwrap();

        let output = package_index(
            &repo_root,
            &demo_repo_index(),
            "latest",
            &repo_root.join(".greentic-agent/oci"),
        )
        .unwrap();
        let remote_root = temp.path().join("remote");
        let cache_root = temp.path().join("cache");
        let published = publish_local_package(
            &output.package_dir,
            &remote_root,
            "greentic-coding-agent",
            "latest",
        )
        .unwrap();
        assert!(published.join("artifacts/repo-index.json").exists());

        let synced =
            sync_local_package(&remote_root, &cache_root, "greentic-coding-agent", "latest")
                .unwrap();
        assert!(synced.join("artifacts/agents/CODEX.md").exists());

        let repos = list_remote_repos(&remote_root).unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].repo_name, "greentic-coding-agent");
        assert_eq!(repos[0].tags, vec!["latest".to_string()]);
    }

    #[test]
    fn build_catalog_discovers_multiple_repos() {
        let temp = tempdir().unwrap();
        let remote_root = temp.path().join("remote");

        for repo_name in ["alpha-repo", "beta-repo"] {
            let repo_root = temp.path().join(repo_name);
            fs::create_dir_all(repo_root.join(".greentic-agent")).unwrap();
            fs::write(repo_root.join(".greentic-agent/manifest.json"), "{}").unwrap();
            let repo_index = demo_repo_index_named(repo_name);
            fs::write(
                repo_root.join(".greentic-agent/repo-index.json"),
                serde_json::to_string_pretty(&repo_index).unwrap(),
            )
            .unwrap();
            let output = package_index(
                &repo_root,
                &repo_index,
                "latest",
                &repo_root.join(".greentic-agent/oci"),
            )
            .unwrap();
            publish_local_package(&output.package_dir, &remote_root, repo_name, "latest").unwrap();
        }

        let catalog = build_catalog(&remote_root).unwrap();
        assert_eq!(catalog.repos.len(), 2);
        assert_eq!(catalog.repos[0].repo_name, "alpha-repo");
        assert_eq!(catalog.repos[1].repo_name, "beta-repo");
    }

    #[test]
    fn sync_catalog_uses_catalog_latest_tags() {
        let temp = tempdir().unwrap();
        let repo_root = temp.path().join("repo");
        let remote_root = temp.path().join("remote");
        let cache_root = temp.path().join("cache");
        fs::create_dir_all(repo_root.join(".greentic-agent")).unwrap();
        fs::write(repo_root.join(".greentic-agent/manifest.json"), "{}").unwrap();
        let repo_index = demo_repo_index();
        fs::write(
            repo_root.join(".greentic-agent/repo-index.json"),
            serde_json::to_string_pretty(&repo_index).unwrap(),
        )
        .unwrap();
        let output = package_index(
            &repo_root,
            &repo_index,
            "latest",
            &repo_root.join(".greentic-agent/oci"),
        )
        .unwrap();
        publish_local_package(
            &output.package_dir,
            &remote_root,
            "greentic-coding-agent",
            "latest",
        )
        .unwrap();

        let synced = sync_catalog(&remote_root, &cache_root).unwrap();
        assert_eq!(synced.len(), 1);
        assert!(
            cache_root
                .join("greentic-coding-agent")
                .join("latest")
                .exists()
        );
    }

    #[test]
    fn check_refresh_reports_explicit_reasons() {
        let temp = tempdir().unwrap();
        let repo_root = temp.path().join("repo");
        fs::create_dir_all(repo_root.join(".git").join("refs").join("heads")).unwrap();
        fs::create_dir_all(repo_root.join(".greentic-agent")).unwrap();
        fs::write(
            repo_root.join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::write(repo_root.join("README.md"), "# Demo\n").unwrap();
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
            repo_root.join(".greentic-agent").join("fingerprints.json"),
            serde_json::json!({
                "version": "v0",
                "head_sha": "abc123",
                "default_branch": "main",
                "tracked_files": ["Cargo.toml"],
                "generator_version": "0.0.1"
            })
            .to_string(),
        )
        .unwrap();
        fs::write(
            repo_root.join(".greentic-agent").join("repo-index.json"),
            serde_json::to_string_pretty(&demo_repo_index()).unwrap(),
        )
        .unwrap();

        let refresh = check_refresh(&repo_root).unwrap();
        assert!(refresh.needs_refresh);
        assert!(
            refresh
                .reasons
                .iter()
                .any(|reason| reason.contains("source commit changed"))
        );
        assert!(
            refresh
                .reasons
                .iter()
                .any(|reason| reason.contains("indexed file fingerprint changed"))
        );
        assert!(
            refresh
                .reasons
                .iter()
                .any(|reason| reason.contains("generator version changed"))
        );
        assert!(
            refresh
                .reasons
                .iter()
                .any(|reason| reason.contains("schema version changed"))
        );
    }

    #[test]
    fn install_github_workflow_is_idempotent() {
        let temp = tempdir().unwrap();
        let repo_root = temp.path().join("repo");
        fs::create_dir_all(&repo_root).unwrap();

        let first = install_github_workflow(&repo_root).unwrap();
        let before = fs::read_to_string(&first).unwrap();
        let second = install_github_workflow(&repo_root).unwrap();
        let after = fs::read_to_string(&second).unwrap();

        assert_eq!(before, after);
        assert!(after.contains("check-refresh"));
        assert!(after.contains("publish-index"));
    }

    #[test]
    fn committed_workflow_example_matches_renderer() {
        let expected = include_str!("../../../examples/greentic-agent-index.workflow.yml");
        assert_eq!(expected, super::render_github_workflow());
    }

    fn demo_repo_index() -> RepoIndex {
        demo_repo_index_named("greentic-coding-agent")
    }

    fn demo_repo_index_named(repo_name: &str) -> RepoIndex {
        let manifest = RepoAgentManifest {
            version: "v1".to_string(),
            repo_name: repo_name.to_string(),
            repo_root: format!("/tmp/{repo_name}"),
            repo_role: RepoRole::CliLauncher,
            primary_language: "rust".to_string(),
            generated_at: "unix:1".to_string(),
            candidate_docs: vec!["README.md".to_string()],
            cargo_manifests: vec!["Cargo.toml".to_string()],
        };

        RepoIndex {
            version: "v1".to_string(),
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
            instruction_graph: vec![InstructionDescriptor {
                id: "readme".to_string(),
                path: "README.md".to_string(),
                title: "README".to_string(),
                kind: "doc".to_string(),
                headings: vec!["Overview".to_string()],
                commands: vec!["greentic-coding-agent analyze".to_string()],
                concept_ids: vec!["digital_worker".to_string()],
            }],
            instruction_paths: vec!["README.md".to_string()],
            source_stats: SourceStats::default(),
        }
    }
}
