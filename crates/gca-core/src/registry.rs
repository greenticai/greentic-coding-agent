use crate::RepoRole;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub repo_name: String,
    pub repo_path: String,
    pub repo_role: RepoRole,
    pub last_analyzed_commit: String,
    pub manifest_path: String,
    pub repo_index_path: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registry {
    pub version: String,
    pub repos: Vec<RegistryEntry>,
}

impl Registry {
    pub fn empty() -> Self {
        Self {
            version: crate::SCHEMA_VERSION_V1.to_string(),
            repos: Vec::new(),
        }
    }

    pub fn upsert(&mut self, entry: RegistryEntry) {
        if let Some(existing) = self
            .repos
            .iter_mut()
            .find(|existing| existing.repo_path == entry.repo_path)
        {
            *existing = entry;
            return;
        }

        self.repos.push(entry);
        self.repos
            .sort_by(|left, right| left.repo_path.cmp(&right.repo_path));
    }
}

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("failed to read registry at {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse registry at {path}: {source}")]
    Parse {
        path: String,
        source: serde_json::Error,
    },
    #[error("failed to create registry directory for {path}: {source}")]
    CreateDir {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to serialize registry for {path}: {source}")]
    Serialize {
        path: String,
        source: serde_json::Error,
    },
    #[error("failed to write registry at {path}: {source}")]
    Write {
        path: String,
        source: std::io::Error,
    },
}

pub fn load_registry(path: &Path) -> Result<Registry, RegistryError> {
    match fs::read_to_string(path) {
        Ok(raw) => serde_json::from_str(&raw).map_err(|source| RegistryError::Parse {
            path: path.display().to_string(),
            source,
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Registry::empty()),
        Err(source) => Err(RegistryError::Read {
            path: path.display().to_string(),
            source,
        }),
    }
}

pub fn write_registry(path: &Path, registry: &Registry) -> Result<(), RegistryError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| RegistryError::CreateDir {
            path: path.display().to_string(),
            source,
        })?;
    }

    let raw =
        serde_json::to_string_pretty(registry).map_err(|source| RegistryError::Serialize {
            path: path.display().to_string(),
            source,
        })?;

    fs::write(path, raw).map_err(|source| RegistryError::Write {
        path: path.display().to_string(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::{Registry, RegistryEntry, load_registry, write_registry};
    use crate::RepoRole;
    use tempfile::tempdir;

    #[test]
    fn load_registry_returns_empty_when_file_is_missing() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("registry.json");

        let registry = load_registry(&path).unwrap();

        assert_eq!(registry, Registry::empty());
    }

    #[test]
    fn upsert_replaces_existing_repo_entry_instead_of_duplicating() {
        let mut registry = Registry::empty();

        registry.upsert(RegistryEntry {
            repo_name: "repo".to_string(),
            repo_path: "/tmp/repo".to_string(),
            repo_role: RepoRole::CliLauncher,
            last_analyzed_commit: "abc".to_string(),
            manifest_path: "/tmp/repo/.greentic-agent/manifest.json".to_string(),
            repo_index_path: "/tmp/repo/.greentic-agent/repo-index.json".to_string(),
            updated_at: "2026-04-15T00:00:00Z".to_string(),
        });
        registry.upsert(RegistryEntry {
            repo_name: "repo".to_string(),
            repo_path: "/tmp/repo".to_string(),
            repo_role: RepoRole::CliLauncher,
            last_analyzed_commit: "def".to_string(),
            manifest_path: "/tmp/repo/.greentic-agent/manifest.json".to_string(),
            repo_index_path: "/tmp/repo/.greentic-agent/repo-index.json".to_string(),
            updated_at: "2026-04-16T00:00:00Z".to_string(),
        });

        assert_eq!(registry.repos.len(), 1);
        assert_eq!(registry.repos[0].last_analyzed_commit, "def");
    }

    #[test]
    fn registry_round_trips_through_disk() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("registry.json");
        let mut registry = Registry::empty();
        registry.upsert(RegistryEntry {
            repo_name: "repo".to_string(),
            repo_path: "/tmp/repo".to_string(),
            repo_role: RepoRole::CliLauncher,
            last_analyzed_commit: "abc".to_string(),
            manifest_path: "/tmp/repo/.greentic-agent/manifest.json".to_string(),
            repo_index_path: "/tmp/repo/.greentic-agent/repo-index.json".to_string(),
            updated_at: "2026-04-15T00:00:00Z".to_string(),
        });

        write_registry(&path, &registry).unwrap();
        let loaded = load_registry(&path).unwrap();

        assert_eq!(loaded, registry);
    }
}
