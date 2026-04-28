pub mod oras;

use gca_agent_files::render_generated_files;
use gca_core::{AuthKind, Catalog, CatalogRepo, IndexVisibility, RepoIndex, SCHEMA_VERSION_V1};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

pub use oras::{OrasError, oras_login, oras_pull, oras_push};

const OCI_LAYOUT_VERSION: &str = "1.0.0";
const LOCAL_INDEX_DIR: &str = ".greentic-agent";
const GENERATOR_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DEFAULT_PUBLIC_CATALOG_REF: &str = "ghcr.io/greenticai/indexes/catalog:latest";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageMetadata {
    pub repo_id: String,
    pub repo_name: String,
    pub tag: String,
    pub reference: String,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub commit_sha: Option<String>,
    pub generated_at: String,
    #[serde(default)]
    pub indexed_at: Option<String>,
    #[serde(default)]
    pub index_schema_version: Option<String>,
    #[serde(default)]
    pub tool_version: Option<String>,
    #[serde(default)]
    pub source_tree_hash: Option<String>,
    pub compatibility: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageOutput {
    pub package_dir: PathBuf,
    pub reference: String,
    pub metadata: PackageMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteRepo {
    pub repo_id: String,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncState {
    pub version: String,
    pub updated_at: String,
    pub repos: Vec<SyncedRepoState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncedRepoState {
    pub repo_id: String,
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    pub tenant: Option<String>,
    pub visibility: IndexVisibility,
    pub package_ref: String,
    pub digest: Option<String>,
    pub source_commit: Option<String>,
    #[serde(default)]
    pub indexed_at: Option<String>,
    pub downloaded_at: String,
    pub local_index_path: PathBuf,
    pub local_tantivy_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncReport {
    #[serde(default)]
    pub channel: Option<String>,
    pub public_catalog: Option<String>,
    pub tenant_catalog: Option<String>,
    pub downloaded: Vec<PathBuf>,
    pub skipped: Vec<String>,
    pub failed: Vec<SyncFailure>,
    pub merged_index_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncFailure {
    pub repo_id: String,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedRepoIndex {
    pub repo_index: RepoIndex,
    pub state: SyncedRepoState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergedIndexReport {
    pub merged_index_path: PathBuf,
    pub repos_indexed: usize,
    pub documents_indexed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SyncCatalogOptions {
    pub channel: Option<String>,
    pub tenant: Option<String>,
    pub public_only: bool,
    pub private_only: bool,
    pub include_private: bool,
    pub prune_disabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteBackendKind {
    LocalFixture,
    GhcrOras,
}

impl RemoteBackendKind {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "local" | "local_fixture" => Ok(Self::LocalFixture),
            "ghcr" | "ghcr_oras" => Ok(Self::GhcrOras),
            other => Err(format!("unsupported remote backend: {other}")),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryAuth {
    pub registry: String,
    pub username: Option<String>,
    pub token: String,
}

impl fmt::Debug for RegistryAuth {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RegistryAuth")
            .field("registry", &self.registry)
            .field("username", &self.username)
            .field("token", &"[redacted]")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteConfig {
    pub backend: RemoteBackendKind,
    pub public_catalog_ref: String,
    pub tenant: Option<String>,
    pub tenant_catalog_ref: Option<String>,
    pub auth: Option<RegistryAuth>,
    pub strict: bool,
    pub public_only: bool,
    pub private_only: bool,
    pub include_private: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemoteConfigInput {
    pub backend: Option<RemoteBackendKind>,
    pub catalog: Option<String>,
    pub tenant: Option<String>,
    pub tenant_catalog: Option<String>,
    pub token: Option<String>,
    pub token_env: Option<String>,
    pub strict: bool,
    pub public_only: bool,
    pub private_only: bool,
    pub include_private: bool,
}

pub trait RemoteIndexBackend {
    fn pull(
        &self,
        reference: &str,
        out_dir: &Path,
        auth: Option<&RegistryAuth>,
    ) -> Result<(), String>;
    fn push(&self, reference: &str, dir: &Path, auth: Option<&RegistryAuth>) -> Result<(), String>;
}

#[derive(Debug, Clone)]
pub struct LocalFixtureBackend {
    pub remote_root: PathBuf,
}

impl RemoteIndexBackend for LocalFixtureBackend {
    fn pull(
        &self,
        reference: &str,
        out_dir: &Path,
        _auth: Option<&RegistryAuth>,
    ) -> Result<(), String> {
        copy_dir_all(&self.remote_root.join(reference), out_dir).map_err(|error| error.to_string())
    }

    fn push(
        &self,
        reference: &str,
        dir: &Path,
        _auth: Option<&RegistryAuth>,
    ) -> Result<(), String> {
        copy_dir_all(dir, &self.remote_root.join(reference)).map_err(|error| error.to_string())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GhcrOrasBackend;

impl RemoteIndexBackend for GhcrOrasBackend {
    fn pull(
        &self,
        reference: &str,
        out_dir: &Path,
        auth: Option<&RegistryAuth>,
    ) -> Result<(), String> {
        oras_pull(reference, out_dir, auth).map_err(|error| error.to_string())
    }

    fn push(&self, reference: &str, dir: &Path, auth: Option<&RegistryAuth>) -> Result<(), String> {
        oras_push(reference, dir, auth).map_err(|error| error.to_string())
    }
}

pub fn resolve_remote_config_from(
    input: RemoteConfigInput,
    env: &BTreeMap<String, String>,
) -> Result<RemoteConfig, String> {
    let tenant = input
        .tenant
        .or_else(|| env.get("GREENTIC_AGENT_TENANT").cloned());
    let public_catalog_ref = input
        .catalog
        .or_else(|| env.get("GREENTIC_AGENT_CATALOG").cloned())
        .unwrap_or_else(|| DEFAULT_PUBLIC_CATALOG_REF.to_string());
    let tenant_catalog_ref = input
        .tenant_catalog
        .or_else(|| env.get("GREENTIC_AGENT_TENANT_CATALOG").cloned())
        .or_else(|| {
            tenant
                .as_ref()
                .map(|tenant| default_tenant_catalog_ref(tenant))
        });
    let token = if let Some(token) = input.token {
        Some(token)
    } else if let Some(env_name) = input.token_env {
        env.get(&env_name).cloned()
    } else {
        env.get("GREENTIC_AGENT_TOKEN")
            .cloned()
            .or_else(|| env.get("GHCR_TOKEN").cloned())
    };

    Ok(RemoteConfig {
        backend: input.backend.unwrap_or(RemoteBackendKind::LocalFixture),
        public_catalog_ref,
        tenant,
        tenant_catalog_ref,
        auth: token.map(|token| RegistryAuth {
            registry: "ghcr.io".to_string(),
            username: Some("greentic-agent".to_string()),
            token,
        }),
        strict: input.strict,
        public_only: input.public_only,
        private_only: input.private_only,
        include_private: input.include_private,
    })
}

pub fn merge_catalogs(public_catalog: Catalog, tenant_catalog: Option<Catalog>) -> Catalog {
    let mut repos = BTreeMap::new();
    let mut change_log = public_catalog.change_log;
    for repo in public_catalog.repos {
        repos.insert(repo.repo_id.clone(), repo);
    }
    let generated_at = tenant_catalog
        .as_ref()
        .map(|catalog| catalog.generated_at.clone())
        .unwrap_or_else(|| public_catalog.generated_at.clone());
    if let Some(tenant_catalog) = tenant_catalog {
        change_log.extend(tenant_catalog.change_log);
        for repo in tenant_catalog.repos {
            repos.insert(repo.repo_id.clone(), repo);
        }
    }
    Catalog {
        version: public_catalog.version,
        generated_at,
        catalog_id: public_catalog.catalog_id,
        default_channel: public_catalog.default_channel,
        repos: repos.into_values().collect(),
        change_log,
    }
}

pub fn default_tenant_catalog_ref(tenant: &str) -> String {
    format!("ghcr.io/greenticai/indexes/tenants/{tenant}/catalog:latest")
}

pub fn default_remote_store_path(home: &Path) -> PathBuf {
    home.join(".greentic-agent").join("remote-oci")
}

pub fn default_sync_cache_path(home: &Path) -> PathBuf {
    home.join(".greentic-agent").join("cache-oci")
}

pub fn default_indexes_path(home: &Path) -> PathBuf {
    home.join(".greentic-agent").join("indexes")
}

pub fn sync_state_path(home: &Path) -> PathBuf {
    home.join(".greentic-agent").join("sync-state.json")
}

pub fn merged_tantivy_path(home: &Path) -> PathBuf {
    home.join(".greentic-agent").join("tantivy").join("merged")
}

pub fn merged_tantivy_next_path(home: &Path) -> PathBuf {
    home.join(".greentic-agent")
        .join("tantivy")
        .join("merged.next")
}

pub fn merged_tantivy_previous_path(home: &Path) -> PathBuf {
    home.join(".greentic-agent")
        .join("tantivy")
        .join("merged.previous")
}

pub fn package_index(
    repo_root: &Path,
    repo_index: &RepoIndex,
    tag: &str,
    output_root: &Path,
) -> std::io::Result<PackageOutput> {
    let package_dir = repo_id_path(output_root, &repo_index.repo_id).join(tag);
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

    let reference = format!("ghcr.io/greenticai/indexes/{}:{}", repo_index.repo_id, tag);
    let metadata = PackageMetadata {
        repo_id: repo_index.repo_id.clone(),
        repo_name: repo_index.repo_name.clone(),
        tag: tag.to_string(),
        reference: reference.clone(),
        branch: repo_index
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.branch.clone()),
        commit_sha: repo_index
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.commit_sha.clone()),
        generated_at: repo_index.generated_at.clone(),
        indexed_at: repo_index
            .metadata
            .as_ref()
            .map(|metadata| metadata.indexed_at.clone()),
        index_schema_version: repo_index
            .metadata
            .as_ref()
            .map(|metadata| metadata.index_schema_version.clone()),
        tool_version: repo_index
            .metadata
            .as_ref()
            .map(|metadata| metadata.tool_version.clone()),
        source_tree_hash: repo_index
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.source_tree_hash.clone()),
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
    let target = repo_id_path(remote_root, repo_name).join(tag);
    copy_dir_all(package_dir, &target)?;
    Ok(target)
}

pub fn sync_local_package(
    remote_root: &Path,
    cache_root: &Path,
    repo_name: &str,
    tag: &str,
) -> std::io::Result<PathBuf> {
    let source = repo_id_path(remote_root, repo_name).join(tag);
    let target = repo_id_path(cache_root, repo_name).join(tag);
    copy_dir_all(&source, &target)?;
    Ok(target)
}

pub fn list_remote_repos(remote_root: &Path) -> std::io::Result<Vec<RemoteRepo>> {
    if fs::read_dir(remote_root).is_err() {
        return Ok(Vec::new());
    }

    let mut repos = Vec::new();
    collect_remote_repos(remote_root, remote_root, &mut repos)?;
    repos.sort_by(|left, right| left.repo_id.cmp(&right.repo_id));
    Ok(repos)
}

pub fn build_catalog(remote_root: &Path) -> std::io::Result<Catalog> {
    let repos = list_remote_repos(remote_root)?;
    let mut catalog_repos = Vec::new();

    for repo in repos {
        let Some(latest_tag) = repo.tags.last().cloned() else {
            continue;
        };
        let mut branches = BTreeMap::new();
        let mut latest_repo_index = None;
        for tag in &repo.tags {
            let repo_index = load_repo_index(
                &remote_root
                    .join(&repo.repo_id)
                    .join(tag)
                    .join("artifacts")
                    .join("repo-index.json"),
            )?;
            let branch = if tag.starts_with("sha-") {
                tag.clone()
            } else {
                repo_index
                    .metadata
                    .as_ref()
                    .and_then(|metadata| metadata.branch.clone())
                    .unwrap_or_else(|| tag.clone())
            };
            let package_ref = format!("ghcr.io/greenticai/indexes/{}:{}", repo_index.repo_id, tag);
            branches.insert(
                branch,
                gca_core::CatalogBranchEntry {
                    index_uri: package_ref,
                    commit_sha: repo_index
                        .metadata
                        .as_ref()
                        .and_then(|metadata| metadata.commit_sha.clone()),
                    updated_at: Some(repo_index.generated_at.clone()),
                    digest: file_digest_hex(
                        &remote_root
                            .join(&repo.repo_id)
                            .join(tag)
                            .join("artifacts")
                            .join("repo-index.json"),
                    )
                    .ok(),
                },
            );
            if tag == &latest_tag {
                latest_repo_index = Some(repo_index);
            }
        }
        let repo_index = latest_repo_index.expect("latest tag should have been loaded");
        let branch = repo_index
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.branch.clone())
            .unwrap_or_else(|| latest_tag.clone());
        let package_ref = format!(
            "ghcr.io/greenticai/indexes/{}:{}",
            repo_index.repo_id, latest_tag
        );
        catalog_repos.push(CatalogRepo {
            repo_id: repo.repo_id,
            repo_name: repo.repo_name,
            repo_role: repo_index.repo_role,
            latest_tag: latest_tag.clone(),
            package_ref,
            updated_at: repo_index.generated_at.clone(),
            default_branch: Some(branch.clone()),
            preferred_branch: Some(branch),
            branches,
            visibility: gca_core::IndexVisibility::Public,
            tenant: None,
            required_auth: None,
            digest: None,
            source_commit: repo_index
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.commit_sha.clone()),
            enabled: true,
        });
    }

    catalog_repos.sort_by(|left, right| left.repo_id.cmp(&right.repo_id));
    Ok(Catalog {
        version: SCHEMA_VERSION_V1.to_string(),
        generated_at: timestamp_string(),
        catalog_id: Some("greenticai/public".to_string()),
        default_channel: Some("latest".to_string()),
        repos: catalog_repos,
        change_log: Vec::new(),
    })
}

pub fn sync_catalog(remote_root: &Path, cache_root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let catalog = match load_published_catalog(remote_root)? {
        Some(catalog) => catalog,
        None => build_catalog(remote_root)?,
    };
    let mut synced = Vec::new();
    for repo in catalog.repos {
        if !repo.enabled {
            continue;
        }
        synced.push(sync_local_package(
            remote_root,
            cache_root,
            &repo.repo_id,
            &repo.latest_tag,
        )?);
    }
    Ok(synced)
}

pub fn sync_catalog_with_state(
    remote_root: &Path,
    cache_root: &Path,
    indexes_root: &Path,
    home: &Path,
    options: &SyncCatalogOptions,
) -> Result<SyncReport, String> {
    let catalog = match load_published_catalog(remote_root).map_err(|error| error.to_string())? {
        Some(catalog) => catalog,
        None => build_catalog(remote_root).map_err(|error| error.to_string())?,
    };
    let mut state = load_sync_state(home).unwrap_or_else(empty_sync_state);
    let mut report = SyncReport {
        channel: options.channel.clone(),
        public_catalog: Some(
            options
                .channel
                .as_deref()
                .map(|channel| format!("ghcr.io/greenticai/indexes/catalog:{channel}"))
                .unwrap_or_else(|| DEFAULT_PUBLIC_CATALOG_REF.to_string()),
        ),
        tenant_catalog: options.tenant.as_deref().map(default_tenant_catalog_ref),
        downloaded: Vec::new(),
        skipped: Vec::new(),
        failed: Vec::new(),
        merged_index_path: merged_tantivy_path(home),
    };

    for repo in &catalog.repos {
        if !repo.enabled {
            if options.prune_disabled {
                prune_synced_repo(&mut state, repo);
            }
            report.skipped.push(repo.repo_id.clone());
            continue;
        }
        if !sync_options_include_repo(options, repo) {
            continue;
        }

        let Some((branch, branch_entry)) = repo.selected_branch(options.channel.as_deref()) else {
            report.skipped.push(repo.repo_id.clone());
            continue;
        };
        let tag =
            tag_from_index_uri(&branch_entry.index_uri).unwrap_or_else(|| repo.latest_tag.clone());
        let mut selected_repo = repo.clone();
        selected_repo.latest_tag = tag.clone();
        selected_repo.package_ref = branch_entry.index_uri.clone();
        selected_repo.preferred_branch = Some(branch.clone());
        selected_repo.source_commit = branch_entry
            .commit_sha
            .clone()
            .or_else(|| repo.source_commit.clone());
        selected_repo.digest = branch_entry.digest.clone().or_else(|| repo.digest.clone());
        selected_repo.updated_at = branch_entry
            .updated_at
            .clone()
            .unwrap_or_else(|| repo.updated_at.clone());

        let source = repo_id_path(remote_root, &repo.repo_id).join(&tag);
        let target = repo_id_path(cache_root, &repo.repo_id).join(&tag);
        let digest = selected_repo
            .digest
            .clone()
            .or_else(|| file_digest_hex(&source.join("artifacts").join("repo-index.json")).ok());
        let unchanged = state.repos.iter().any(|entry| {
            entry.repo_id == repo.repo_id
                && entry.branch.as_deref() == Some(branch.as_str())
                && entry.tenant == repo.tenant
                && entry.digest == digest
                && entry.source_commit == selected_repo.source_commit
                && entry.local_index_path.join("repo-index.json").exists()
        });
        if unchanged {
            report.skipped.push(repo.repo_id.clone());
            continue;
        }

        if let Err(error) = copy_dir_all(&source, &target) {
            report.failed.push(SyncFailure {
                repo_id: repo.repo_id.clone(),
                error: error.to_string(),
            });
            continue;
        }
        match sync_cached_index_from_package(
            &selected_repo,
            &source,
            indexes_root,
            digest,
            options.channel.clone(),
        ) {
            Ok(entry) => {
                upsert_synced_repo(&mut state, entry);
                report.downloaded.push(target);
            }
            Err(error) => report.failed.push(SyncFailure {
                repo_id: repo.repo_id.clone(),
                error,
            }),
        }
    }

    write_sync_state(home, &state)?;
    Ok(report)
}

pub fn sync_repo_with_state(
    remote_root: &Path,
    cache_root: &Path,
    indexes_root: &Path,
    home: &Path,
    repo_id: &str,
    tag: &str,
    tenant: Option<&str>,
) -> Result<SyncReport, String> {
    let source = repo_id_path(remote_root, repo_id).join(tag);
    let target = repo_id_path(cache_root, repo_id).join(tag);
    let repo = catalog_repo_from_package(&source, repo_id, tag, tenant)?;
    let digest = repo.digest.clone();
    let branch = repo
        .selected_branch(None)
        .map(|(branch, _)| branch)
        .unwrap_or_else(|| tag.to_string());
    let mut state = load_sync_state(home).unwrap_or_else(empty_sync_state);
    let mut report = SyncReport {
        channel: Some(branch.clone()),
        public_catalog: None,
        tenant_catalog: tenant.map(default_tenant_catalog_ref),
        downloaded: Vec::new(),
        skipped: Vec::new(),
        failed: Vec::new(),
        merged_index_path: merged_tantivy_path(home),
    };

    let unchanged = state.repos.iter().any(|entry| {
        entry.repo_id == repo.repo_id
            && entry.branch.as_deref() == Some(branch.as_str())
            && entry.tenant == repo.tenant
            && entry.digest == digest
            && entry.source_commit == repo.source_commit
            && entry.local_index_path.join("repo-index.json").exists()
    });
    if unchanged {
        report.skipped.push(repo.repo_id);
        return Ok(report);
    }

    copy_dir_all(&source, &target).map_err(|error| error.to_string())?;
    let entry = sync_cached_index_from_package(&repo, &source, indexes_root, digest, Some(branch))?;
    upsert_synced_repo(&mut state, entry);
    write_sync_state(home, &state)?;
    report.downloaded.push(target);
    Ok(report)
}

pub fn local_index_path_for(indexes_root: &Path, repo: &CatalogRepo) -> PathBuf {
    let branch = repo
        .selected_branch(None)
        .map(|(branch, _)| branch)
        .unwrap_or_else(|| repo.latest_tag.clone());
    match repo.visibility {
        IndexVisibility::Tenant | IndexVisibility::Private => indexes_root
            .join("tenants")
            .join(repo.tenant.as_deref().unwrap_or("default"))
            .join(repo_id_path(Path::new(""), &repo.repo_id))
            .join(branch),
        IndexVisibility::Public => indexes_root
            .join("public")
            .join(repo_id_path(Path::new(""), &repo.repo_id))
            .join(branch),
    }
}

pub fn empty_sync_state() -> SyncState {
    SyncState {
        version: SCHEMA_VERSION_V1.to_string(),
        updated_at: timestamp_string(),
        repos: Vec::new(),
    }
}

pub fn load_sync_state(home: &Path) -> Option<SyncState> {
    let raw = fs::read_to_string(sync_state_path(home)).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn write_sync_state(home: &Path, state: &SyncState) -> Result<(), String> {
    let path = sync_state_path(home);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let mut state = state.clone();
    state.updated_at = timestamp_string();
    state.repos.sort_by(|left, right| {
        left.repo_id
            .cmp(&right.repo_id)
            .then(left.tenant.cmp(&right.tenant))
            .then(left.branch.cmp(&right.branch))
            .then(left.channel.cmp(&right.channel))
    });
    let raw = serde_json::to_string_pretty(&state).expect("sync state should serialize as json");
    fs::write(&path, format!("{raw}\n"))
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

pub fn load_cached_repo_indexes(
    home: &Path,
    tenant: Option<&str>,
) -> Result<Vec<CachedRepoIndex>, String> {
    let state = load_sync_state(home).unwrap_or_else(|| recover_sync_state_from_cache(home));
    let mut cached = Vec::new();
    for entry in state.repos {
        if let Some(tenant) = tenant
            && entry.tenant.as_deref() != Some(tenant)
            && entry.visibility != IndexVisibility::Public
        {
            continue;
        }
        let path = entry.local_index_path.join("repo-index.json");
        if !path.exists() {
            continue;
        }
        let repo_index = load_repo_index(&path).map_err(|error| error.to_string())?;
        cached.push(CachedRepoIndex {
            repo_index,
            state: entry,
        });
    }
    cached.sort_by(|left, right| {
        left.state
            .repo_id
            .cmp(&right.state.repo_id)
            .then(left.state.branch.cmp(&right.state.branch))
    });
    Ok(cached)
}

pub fn recover_sync_state_from_cache(home: &Path) -> SyncState {
    let mut state = empty_sync_state();
    let indexes_root = default_indexes_path(home);
    recover_cached_indexes_under(
        &indexes_root.join("public"),
        None,
        IndexVisibility::Public,
        &mut state,
    );
    let tenants_root = indexes_root.join("tenants");
    if let Ok(tenants) = fs::read_dir(&tenants_root) {
        for tenant_entry in tenants.flatten() {
            let tenant_path = tenant_entry.path();
            if !tenant_path.is_dir() {
                continue;
            }
            let tenant = tenant_entry.file_name().to_string_lossy().to_string();
            recover_cached_indexes_under(
                &tenant_path,
                Some(tenant),
                IndexVisibility::Tenant,
                &mut state,
            );
        }
    }
    state
}

pub fn rebuild_merged_tantivy_index(
    home: &Path,
    tenant: Option<&str>,
) -> Result<MergedIndexReport, String> {
    let cached = load_cached_repo_indexes(home, tenant)?;
    let repo_indexes = cached
        .iter()
        .map(|entry| entry.repo_index.clone())
        .collect::<Vec<_>>();
    let merged_path = merged_tantivy_path(home);
    let next_path = merged_tantivy_next_path(home);
    let previous_path = merged_tantivy_previous_path(home);
    let build = gca_index::build_merged_tantivy_index(&repo_indexes, &next_path)
        .map_err(|error| error.to_string())?;
    fs::write(
        next_path.join("greentic-meta.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "version": SCHEMA_VERSION_V1,
            "generated_at": timestamp_string(),
            "repos": cached.iter().map(|entry| &entry.state.repo_id).collect::<Vec<_>>(),
            "documents_indexed": build.documents_indexed,
        }))
        .expect("merged metadata should serialize"),
    )
    .map_err(|error| format!("failed to write merged metadata: {error}"))?;

    if previous_path.exists() {
        fs::remove_dir_all(&previous_path)
            .map_err(|error| format!("failed to remove previous merged index: {error}"))?;
    }
    if merged_path.exists() {
        fs::rename(&merged_path, &previous_path)
            .map_err(|error| format!("failed to archive previous merged index: {error}"))?;
    }
    if let Err(error) = fs::rename(&next_path, &merged_path) {
        if previous_path.exists() && !merged_path.exists() {
            let _ = fs::rename(&previous_path, &merged_path);
        }
        return Err(format!("failed to activate merged index: {error}"));
    }

    Ok(MergedIndexReport {
        merged_index_path: merged_path,
        repos_indexed: cached.len(),
        documents_indexed: build.documents_indexed,
    })
}

fn load_published_catalog(remote_root: &Path) -> std::io::Result<Option<Catalog>> {
    let catalog_path = remote_root
        .join("catalogs")
        .join("public")
        .join("catalog.json");
    if !catalog_path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(catalog_path)?;
    let mut catalog: Catalog = serde_json::from_str(&raw)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    catalog
        .repos
        .sort_by(|left, right| left.repo_id.cmp(&right.repo_id));
    Ok(Some(catalog))
}

fn sync_options_include_repo(options: &SyncCatalogOptions, repo: &CatalogRepo) -> bool {
    if options.public_only && repo.visibility != IndexVisibility::Public {
        return false;
    }
    if options.private_only && repo.visibility == IndexVisibility::Public {
        return false;
    }
    if !options.include_private
        && options.tenant.is_none()
        && matches!(
            repo.visibility,
            IndexVisibility::Tenant | IndexVisibility::Private
        )
    {
        return false;
    }
    if let Some(tenant) = &options.tenant
        && let Some(repo_tenant) = &repo.tenant
    {
        return repo_tenant == tenant;
    }
    true
}

fn catalog_repo_from_package(
    package_dir: &Path,
    requested_repo: &str,
    tag: &str,
    tenant: Option<&str>,
) -> Result<CatalogRepo, String> {
    let repo_index = load_repo_index(&package_dir.join("artifacts").join("repo-index.json"))
        .map_err(|error| error.to_string())?;
    let branch = repo_index
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.branch.clone())
        .unwrap_or_else(|| tag.to_string());
    let package_ref = format!("ghcr.io/greenticai/indexes/{requested_repo}:{tag}");
    let digest = file_digest_hex(&package_dir.join("artifacts").join("repo-index.json")).ok();
    let mut branches = BTreeMap::new();
    branches.insert(
        branch.clone(),
        gca_core::CatalogBranchEntry {
            index_uri: package_ref.clone(),
            commit_sha: repo_index
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.commit_sha.clone()),
            updated_at: Some(repo_index.generated_at.clone()),
            digest: digest.clone(),
        },
    );
    Ok(CatalogRepo {
        repo_id: repo_index.repo_id.clone(),
        repo_name: repo_index.repo_name.clone(),
        repo_role: repo_index.repo_role,
        latest_tag: tag.to_string(),
        package_ref,
        updated_at: repo_index.generated_at.clone(),
        default_branch: Some(branch.clone()),
        preferred_branch: Some(branch),
        branches,
        visibility: if tenant.is_some() {
            IndexVisibility::Tenant
        } else {
            IndexVisibility::Public
        },
        tenant: tenant.map(ToString::to_string),
        required_auth: tenant.map(|_| AuthKind::GhcrToken),
        digest,
        source_commit: None,
        enabled: true,
    })
}

fn tag_from_index_uri(index_uri: &str) -> Option<String> {
    index_uri
        .rsplit_once(':')
        .map(|(_, tag)| tag.to_string())
        .filter(|tag| !tag.is_empty())
}

fn sync_cached_index_from_package(
    repo: &CatalogRepo,
    package_dir: &Path,
    indexes_root: &Path,
    digest: Option<String>,
    channel: Option<String>,
) -> Result<SyncedRepoState, String> {
    let target = local_index_path_for(indexes_root, repo);
    fs::create_dir_all(&target)
        .map_err(|error| format!("failed to create {}: {error}", target.display()))?;
    let artifacts = package_dir.join("artifacts");
    fs::copy(
        artifacts.join("repo-index.json"),
        target.join("repo-index.json"),
    )
    .map_err(|error| format!("failed to cache repo-index.json: {error}"))?;
    let manifest_source = artifacts.join("repo-manifest.json");
    if manifest_source.exists() {
        fs::copy(manifest_source, target.join("manifest.json"))
            .map_err(|error| format!("failed to cache manifest.json: {error}"))?;
    }
    let metadata_source = artifacts.join("package-metadata.json");
    if metadata_source.exists() {
        fs::copy(metadata_source, target.join("package-metadata.json"))
            .map_err(|error| format!("failed to cache package metadata: {error}"))?;
    }

    let repo_index =
        load_repo_index(&target.join("repo-index.json")).map_err(|error| error.to_string())?;
    let tantivy_path = target.join("tantivy");
    gca_index::build_local_tantivy_index(&repo_index, &tantivy_path)
        .map_err(|error| error.to_string())?;
    Ok(SyncedRepoState {
        repo_id: repo.repo_id.clone(),
        channel,
        branch: repo.selected_branch(None).map(|(branch, _)| branch),
        tenant: repo.tenant.clone(),
        visibility: repo.visibility,
        package_ref: repo.package_ref.clone(),
        digest,
        source_commit: repo.source_commit.clone(),
        indexed_at: Some(repo.updated_at.clone()).filter(|value| !value.is_empty()),
        downloaded_at: timestamp_string(),
        local_index_path: target,
        local_tantivy_path: Some(tantivy_path),
    })
}

fn upsert_synced_repo(state: &mut SyncState, entry: SyncedRepoState) {
    state.repos.retain(|repo| {
        repo.repo_id != entry.repo_id
            || repo.tenant != entry.tenant
            || repo.branch != entry.branch
            || repo.channel != entry.channel
    });
    state.repos.push(entry);
}

fn prune_synced_repo(state: &mut SyncState, repo: &CatalogRepo) {
    let mut retained = Vec::new();
    for entry in state.repos.drain(..) {
        if entry.repo_id == repo.repo_id && entry.tenant == repo.tenant {
            let _ = fs::remove_dir_all(&entry.local_index_path);
        } else {
            retained.push(entry);
        }
    }
    state.repos = retained;
}

fn recover_cached_indexes_under(
    root: &Path,
    tenant: Option<String>,
    visibility: IndexVisibility,
    state: &mut SyncState,
) {
    recover_cached_index_dirs(root, tenant, visibility, state);
}

fn recover_cached_index_dirs(
    root: &Path,
    tenant: Option<String>,
    visibility: IndexVisibility,
    state: &mut SyncState,
) {
    let repo_index_path = root.join("repo-index.json");
    if repo_index_path.exists() {
        if let Ok(repo_index) = load_repo_index(&repo_index_path) {
            let digest = file_digest_hex(&repo_index_path).ok();
            let branch = repo_index
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.branch.clone())
                .or_else(|| {
                    root.file_name()
                        .map(|name| name.to_string_lossy().to_string())
                });
            state.repos.push(SyncedRepoState {
                repo_id: repo_index.repo_id.clone(),
                channel: branch.clone(),
                branch,
                tenant,
                visibility,
                package_ref: format!("ghcr.io/greenticai/indexes/{}:latest", repo_index.repo_id),
                digest,
                source_commit: repo_index
                    .metadata
                    .as_ref()
                    .and_then(|metadata| metadata.commit_sha.clone()),
                indexed_at: repo_index
                    .metadata
                    .as_ref()
                    .map(|metadata| metadata.indexed_at.clone()),
                downloaded_at: timestamp_string(),
                local_index_path: root.to_path_buf(),
                local_tantivy_path: Some(root.join("tantivy")),
            });
        }
        return;
    }

    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        recover_cached_index_dirs(&path, tenant.clone(), visibility, state);
    }
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
    branches:
      - main
      - develop
  workflow_dispatch:

permissions:
  contents: read
  packages: write

jobs:
  index:
    runs-on: ubuntu-latest
    env:
      GHCR_TOKEN: ${{ secrets.GITHUB_TOKEN }}
    steps:
      - name: Checkout
        uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Install ORAS
        uses: oras-project/setup-oras@v1

      - name: Build greentic-coding-agent
        run: cargo build --release --package greentic-coding-agent

      - name: Analyze repo
        run: ./target/release/greentic-coding-agent analyze --print --format json | tee .greentic-agent-analyze.json

      - name: Check refresh
        run: ./target/release/greentic-coding-agent check-refresh --format json | tee .greentic-agent-refresh.json

      - name: Build local Tantivy index
        run: ./target/release/greentic-coding-agent search --engine auto --mode concept greentic --format json

      - name: Package index
        run: ./target/release/greentic-coding-agent package-index --tag "${{ github.ref_name }}" --tag "sha-${{ github.sha }}" --format json | tee .greentic-agent-package.json

      - name: Publish index to GHCR when refresh is needed
        shell: bash
        run: |
          if ./target/release/greentic-coding-agent check-refresh --format json | grep -q '"needs_refresh": true'; then
            ./target/release/greentic-coding-agent publish-index --tag "${{ github.ref_name }}" --tag "sha-${{ github.sha }}" --backend ghcr --token-env GHCR_TOKEN --format json | tee .greentic-agent-publish.json
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

fn repo_id_path(root: &Path, repo_id: &str) -> PathBuf {
    let mut path = root.to_path_buf();
    for part in repo_id.split('/') {
        path.push(part);
    }
    path
}

fn collect_remote_repos(
    remote_root: &Path,
    current: &Path,
    repos: &mut Vec<RemoteRepo>,
) -> std::io::Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        if path.join("artifacts").join("repo-index.json").exists() {
            let Ok(relative) = current.strip_prefix(remote_root) else {
                continue;
            };
            let repo_id = relative.display().to_string();
            let repo_name = repo_id.rsplit('/').next().unwrap_or(&repo_id).to_string();
            let tag = entry.file_name().to_string_lossy().to_string();
            if let Some(existing) = repos.iter_mut().find(|repo| repo.repo_id == repo_id) {
                existing.tags.push(tag);
                existing.tags.sort();
            } else {
                repos.push(RemoteRepo {
                    repo_id,
                    repo_name,
                    tags: vec![tag],
                });
            }
        } else {
            collect_remote_repos(remote_root, &path, repos)?;
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

fn file_digest_hex(path: &Path) -> std::io::Result<String> {
    fs::read(path).map(|bytes| digest_hex(&bytes))
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
        DEFAULT_PUBLIC_CATALOG_REF, RemoteBackendKind, RemoteConfigInput, build_catalog,
        check_refresh, default_indexes_path, default_tenant_catalog_ref, install_github_workflow,
        list_remote_repos, load_cached_repo_indexes, load_sync_state, merge_catalogs,
        package_index, publish_local_package, rebuild_merged_tantivy_index,
        recover_sync_state_from_cache, resolve_remote_config_from, sync_catalog,
        sync_catalog_with_state, sync_local_package, sync_state_path,
    };
    use gca_core::{
        AuthKind, Catalog, ConceptDescriptor, FreshnessStatus, IndexVisibility,
        InstructionDescriptor, KnowledgeScope, LifecyclePhase, RepoAgentManifest, RepoIndex,
        RepoIndexMetadata, RepoRole, ReuseDescriptor, SourceStats, ValidationDescriptor,
        WorkflowDescriptor,
    };
    use std::collections::BTreeMap;
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
        assert_eq!(output.metadata.branch.as_deref(), Some("main"));
        assert_eq!(output.metadata.commit_sha.as_deref(), Some("abc123"));
        assert_eq!(
            output.metadata.index_schema_version.as_deref(),
            Some("gca.repo_index.v1")
        );
        assert_eq!(
            output.metadata.tool_version.as_deref(),
            Some(env!("CARGO_PKG_VERSION"))
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
        assert!(catalog.repos[0].branches.contains_key("main"));
        assert_eq!(catalog.repos[1].repo_name, "beta-repo");
        assert!(catalog.repos[1].branches.contains_key("main"));
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
    fn sync_catalog_skips_disabled_published_entries() {
        let temp = tempdir().unwrap();
        let remote_root = temp.path().join("remote");
        let cache_root = temp.path().join("cache");

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
            publish_local_package(
                &output.package_dir,
                &remote_root,
                &repo_index.repo_id,
                "latest",
            )
            .unwrap();
        }

        let mut catalog = build_catalog(&remote_root).unwrap();
        catalog
            .repos
            .iter_mut()
            .find(|repo| repo.repo_name == "beta-repo")
            .unwrap()
            .enabled = false;
        let catalog_path = remote_root
            .join("catalogs")
            .join("public")
            .join("catalog.json");
        fs::create_dir_all(catalog_path.parent().unwrap()).unwrap();
        fs::write(
            &catalog_path,
            serde_json::to_string_pretty(&catalog).unwrap(),
        )
        .unwrap();

        let synced = sync_catalog(&remote_root, &cache_root).unwrap();
        assert_eq!(synced.len(), 1);
        assert!(
            cache_root
                .join("greenticai")
                .join("alpha-repo")
                .join("latest")
                .exists()
        );
        assert!(
            !cache_root
                .join("greenticai")
                .join("beta-repo")
                .join("latest")
                .exists()
        );
    }

    #[test]
    fn sync_catalog_with_state_writes_normalized_cache_and_skips_unchanged() {
        let temp = tempdir().unwrap();
        let repo_root = temp.path().join("repo");
        let remote_root = temp.path().join("remote");
        let cache_root = temp.path().join("cache");
        let home = temp.path().join("home");
        let indexes_root = default_indexes_path(&home);
        let repo_index = write_packaged_repo(&repo_root, &remote_root, "latest");

        let first = sync_catalog_with_state(
            &remote_root,
            &cache_root,
            &indexes_root,
            &home,
            &Default::default(),
        )
        .unwrap();
        let second = sync_catalog_with_state(
            &remote_root,
            &cache_root,
            &indexes_root,
            &home,
            &Default::default(),
        )
        .unwrap();

        let normalized_index = indexes_root
            .join("public")
            .join("greenticai")
            .join("greentic-coding-agent")
            .join("main")
            .join("repo-index.json");
        assert_eq!(first.downloaded.len(), 1);
        assert!(first.failed.is_empty());
        assert!(second.downloaded.is_empty());
        assert_eq!(second.skipped, vec![repo_index.repo_id.clone()]);
        assert!(sync_state_path(&home).exists());
        assert!(normalized_index.exists());
        assert!(normalized_index.with_file_name("manifest.json").exists());
        assert!(
            normalized_index
                .with_file_name("package-metadata.json")
                .exists()
        );
        assert!(normalized_index.with_file_name("tantivy").exists());

        let state = load_sync_state(&home).unwrap();
        assert_eq!(state.repos.len(), 1);
        assert_eq!(state.repos[0].repo_id, repo_index.repo_id);
        assert_eq!(state.repos[0].visibility, IndexVisibility::Public);
        assert!(state.repos[0].digest.is_some());

        let cached = load_cached_repo_indexes(&home, None).unwrap();
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].repo_index.repo_name, "greentic-coding-agent");
        let merged = rebuild_merged_tantivy_index(&home, None).unwrap();
        assert_eq!(merged.repos_indexed, 1);
        assert!(merged.documents_indexed > 0);
        assert!(merged.merged_index_path.join("greentic-meta.json").exists());

        fs::remove_file(sync_state_path(&home)).unwrap();
        let recovered = recover_sync_state_from_cache(&home);
        assert_eq!(recovered.repos.len(), 1);
        assert_eq!(
            recovered.repos[0].repo_id,
            "greenticai/greentic-coding-agent"
        );
    }

    #[test]
    fn sync_catalog_with_state_filters_and_caches_tenant_indexes() {
        let temp = tempdir().unwrap();
        let remote_root = temp.path().join("remote");
        let cache_root = temp.path().join("cache");
        let home = temp.path().join("home");
        let indexes_root = default_indexes_path(&home);

        let public_repo =
            write_packaged_repo_named(&temp.path().join("public-repo"), &remote_root, "shared");
        let tenant_repo =
            write_packaged_repo_named(&temp.path().join("tenant-repo"), &remote_root, "tenant");
        let mut catalog = build_catalog(&remote_root).unwrap();
        let tenant_entry = catalog
            .repos
            .iter_mut()
            .find(|repo| repo.repo_id == tenant_repo.repo_id)
            .unwrap();
        tenant_entry.visibility = IndexVisibility::Tenant;
        tenant_entry.tenant = Some("meeza".to_string());
        tenant_entry.required_auth = Some(AuthKind::GhcrToken);
        write_public_catalog(&remote_root, &catalog);

        let public_only = sync_catalog_with_state(
            &remote_root,
            &cache_root,
            &indexes_root,
            &home,
            &Default::default(),
        )
        .unwrap();
        assert_eq!(public_only.downloaded.len(), 1);
        assert_eq!(public_only.skipped, Vec::<String>::new());
        assert!(
            indexes_root
                .join("public")
                .join("greenticai")
                .join("shared")
                .join("main")
                .join("repo-index.json")
                .exists()
        );
        assert!(
            !indexes_root
                .join("tenants")
                .join("meeza")
                .join("greenticai")
                .join("tenant")
                .join("main")
                .join("repo-index.json")
                .exists()
        );

        let tenant_sync = sync_catalog_with_state(
            &remote_root,
            &cache_root,
            &indexes_root,
            &home,
            &super::SyncCatalogOptions {
                tenant: Some("meeza".to_string()),
                include_private: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(tenant_sync.downloaded.len(), 1);
        assert_eq!(tenant_sync.skipped, vec![public_repo.repo_id]);
        assert!(
            indexes_root
                .join("tenants")
                .join("meeza")
                .join("greenticai")
                .join("tenant")
                .join("main")
                .join("repo-index.json")
                .exists()
        );

        let cached = load_cached_repo_indexes(&home, Some("meeza")).unwrap();
        assert_eq!(cached.len(), 2);
        assert!(
            cached
                .iter()
                .any(|entry| entry.state.visibility == IndexVisibility::Public)
        );
        assert!(
            cached
                .iter()
                .any(|entry| entry.state.tenant.as_deref() == Some("meeza"))
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

    #[test]
    fn remote_config_resolves_cli_env_and_defaults() {
        let mut env = BTreeMap::new();
        env.insert(
            "GREENTIC_AGENT_CATALOG".to_string(),
            "ghcr.io/custom/catalog:latest".to_string(),
        );
        env.insert("GREENTIC_AGENT_TENANT".to_string(), "meeza".to_string());
        env.insert("GHCR_TOKEN".to_string(), "ghcr-secret".to_string());

        let config = resolve_remote_config_from(
            RemoteConfigInput {
                backend: Some(RemoteBackendKind::GhcrOras),
                token: Some("cli-secret".to_string()),
                strict: true,
                ..RemoteConfigInput::default()
            },
            &env,
        )
        .unwrap();

        assert_eq!(config.backend, RemoteBackendKind::GhcrOras);
        assert_eq!(config.public_catalog_ref, "ghcr.io/custom/catalog:latest");
        assert_eq!(config.tenant.as_deref(), Some("meeza"));
        assert_eq!(
            config.tenant_catalog_ref.as_deref(),
            Some("ghcr.io/greenticai/indexes/tenants/meeza/catalog:latest")
        );
        assert_eq!(config.auth.as_ref().unwrap().token, "cli-secret");
        assert!(!format!("{:?}", config.auth.as_ref().unwrap()).contains("cli-secret"));
        assert!(config.strict);
    }

    #[test]
    fn remote_config_uses_defaults_without_env() {
        let config =
            resolve_remote_config_from(RemoteConfigInput::default(), &BTreeMap::new()).unwrap();

        assert_eq!(config.backend, RemoteBackendKind::LocalFixture);
        assert_eq!(config.public_catalog_ref, DEFAULT_PUBLIC_CATALOG_REF);
        assert_eq!(
            default_tenant_catalog_ref("meeza"),
            "ghcr.io/greenticai/indexes/tenants/meeza/catalog:latest"
        );
        assert!(config.auth.is_none());
    }

    #[test]
    fn tenant_catalog_overrides_public_catalog_entries() {
        let mut public = build_catalog_fixture("unix:1");
        public.repos[0].repo_id = "greenticai/shared".to_string();
        public.repos[0].repo_name = "shared".to_string();
        public.repos[0].package_ref = "public".to_string();

        let mut tenant = build_catalog_fixture("unix:2");
        tenant.repos[0].repo_id = "greenticai/shared".to_string();
        tenant.repos[0].repo_name = "shared".to_string();
        tenant.repos[0].package_ref = "tenant".to_string();

        let merged = merge_catalogs(public, Some(tenant));

        assert_eq!(merged.generated_at, "unix:2");
        assert_eq!(merged.repos.len(), 1);
        assert_eq!(merged.repos[0].package_ref, "tenant");
    }

    fn demo_repo_index() -> RepoIndex {
        demo_repo_index_named("greentic-coding-agent")
    }

    fn write_packaged_repo(
        repo_root: &std::path::Path,
        remote_root: &std::path::Path,
        tag: &str,
    ) -> RepoIndex {
        write_packaged_repo_named_with_tag(repo_root, remote_root, "greentic-coding-agent", tag)
    }

    fn write_packaged_repo_named(
        repo_root: &std::path::Path,
        remote_root: &std::path::Path,
        repo_name: &str,
    ) -> RepoIndex {
        write_packaged_repo_named_with_tag(repo_root, remote_root, repo_name, "latest")
    }

    fn write_packaged_repo_named_with_tag(
        repo_root: &std::path::Path,
        remote_root: &std::path::Path,
        repo_name: &str,
        tag: &str,
    ) -> RepoIndex {
        fs::create_dir_all(repo_root.join(".greentic-agent")).unwrap();
        fs::write(repo_root.join(".greentic-agent/manifest.json"), "{}").unwrap();
        let repo_index = demo_repo_index_named(repo_name);
        fs::write(
            repo_root.join(".greentic-agent/repo-index.json"),
            serde_json::to_string_pretty(&repo_index).unwrap(),
        )
        .unwrap();
        let output = package_index(
            repo_root,
            &repo_index,
            tag,
            &repo_root.join(".greentic-agent/oci"),
        )
        .unwrap();
        publish_local_package(&output.package_dir, remote_root, &repo_index.repo_id, tag).unwrap();
        repo_index
    }

    fn write_public_catalog(remote_root: &std::path::Path, catalog: &Catalog) {
        let catalog_path = remote_root
            .join("catalogs")
            .join("public")
            .join("catalog.json");
        fs::create_dir_all(catalog_path.parent().unwrap()).unwrap();
        fs::write(catalog_path, serde_json::to_string_pretty(catalog).unwrap()).unwrap();
    }

    fn build_catalog_fixture(generated_at: &str) -> gca_core::Catalog {
        gca_core::Catalog {
            version: "v1".to_string(),
            generated_at: generated_at.to_string(),
            catalog_id: Some("greenticai/public".to_string()),
            default_channel: Some("latest".to_string()),
            repos: vec![gca_core::CatalogRepo {
                repo_id: "greenticai/demo".to_string(),
                repo_name: "demo".to_string(),
                repo_role: RepoRole::DemoApp,
                latest_tag: "latest".to_string(),
                package_ref: "ghcr.io/greenticai/indexes/greenticai/demo:latest".to_string(),
                updated_at: generated_at.to_string(),
                default_branch: None,
                preferred_branch: None,
                branches: BTreeMap::new(),
                visibility: gca_core::IndexVisibility::Public,
                tenant: None,
                required_auth: None,
                digest: None,
                source_commit: None,
                enabled: true,
            }],
            change_log: Vec::new(),
        }
    }

    fn demo_repo_index_named(repo_name: &str) -> RepoIndex {
        let manifest = RepoAgentManifest {
            version: "v1".to_string(),
            repo_id: format!("greenticai/{repo_name}"),
            repo_name: repo_name.to_string(),
            org: Some("greenticai".to_string()),
            repo_root: format!("/tmp/{repo_name}"),
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
            metadata: Some(RepoIndexMetadata {
                repo_id: manifest.repo_id.clone(),
                branch: Some("main".to_string()),
                commit_sha: Some("abc123".to_string()),
                commit_time: None,
                indexed_at: "unix:1".to_string(),
                index_schema_version: "gca.repo_index.v1".to_string(),
                tool_version: env!("CARGO_PKG_VERSION").to_string(),
                source_tree_hash: Some("fnv64:0000000000000001".to_string()),
            }),
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
            knowledge_updates: Vec::new(),
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
