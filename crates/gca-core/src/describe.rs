use serde::Serialize;
use std::env;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DescribeHere {
    pub version: String,
    pub repo_root: PathBuf,
    pub repo_name: String,
    pub manifest_path: PathBuf,
    pub has_git_dir: bool,
}

#[derive(Debug, Error)]
pub enum DescribeHereError {
    #[error("failed to determine current directory: {0}")]
    CurrentDir(std::io::Error),
    #[error("failed to detect repository root from {start_dir}")]
    RepoRootNotFound { start_dir: PathBuf },
}

pub fn describe_here() -> Result<DescribeHere, DescribeHereError> {
    let current_dir = env::current_dir().map_err(DescribeHereError::CurrentDir)?;
    let repo_root =
        find_repo_root(&current_dir).ok_or_else(|| DescribeHereError::RepoRootNotFound {
            start_dir: current_dir.clone(),
        })?;

    let repo_name = repo_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown-repo")
        .to_string();

    Ok(DescribeHere {
        version: env!("CARGO_PKG_VERSION").to_string(),
        manifest_path: repo_root.join("Cargo.toml"),
        has_git_dir: repo_root.join(".git").exists(),
        repo_name,
        repo_root,
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

#[cfg(test)]
mod tests {
    use super::describe_here;

    #[test]
    fn describe_here_returns_repo_root_metadata_for_this_workspace() {
        let description = describe_here().expect("current workspace should be discoverable");

        assert_eq!(description.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(description.repo_name, "greentic-coding-agent");
        assert!(description.repo_root.join(".git").exists());
        assert!(description.manifest_path.ends_with("Cargo.toml"));
        assert!(description.has_git_dir);
    }
}
