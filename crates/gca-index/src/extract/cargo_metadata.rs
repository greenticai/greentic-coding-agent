use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CargoMetadataStats {
    pub workspace_members: Vec<String>,
    pub crate_names: Vec<String>,
    pub crate_root_paths: Vec<String>,
    pub test_targets: Vec<String>,
    pub feature_names: Vec<String>,
    pub dependencies: Vec<String>,
}

pub fn extract_cargo_metadata(repo_root: &Path) -> Option<CargoMetadataStats> {
    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--format-version")
        .arg("1")
        .arg("--no-deps")
        .current_dir(repo_root)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let metadata = serde_json::from_slice::<CargoMetadata>(&output.stdout).ok()?;
    let canonical_root = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    Some(metadata.into_stats(&canonical_root))
}

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
    workspace_members: Vec<String>,
}

impl CargoMetadata {
    fn into_stats(self, repo_root: &Path) -> CargoMetadataStats {
        let workspace_ids: BTreeSet<_> = self.workspace_members.into_iter().collect();
        let mut workspace_members = BTreeSet::new();
        let mut crate_names = BTreeSet::new();
        let mut crate_root_paths = BTreeSet::new();
        let mut test_targets = BTreeSet::new();
        let mut feature_names = BTreeSet::new();
        let mut dependencies = BTreeSet::new();

        for package in self.packages {
            if !workspace_ids.contains(&package.id) {
                continue;
            }

            crate_names.insert(package.name.clone());
            if let Some(relative) = relative_parent(repo_root, &package.manifest_path) {
                workspace_members.insert(relative);
            }

            for dependency in package.dependencies {
                dependencies.insert(dependency.name);
            }

            feature_names.extend(package.features.into_keys());

            for target in package.targets {
                if let Some(relative) = relative_path(repo_root, &target.src_path) {
                    if is_test_target(&target) {
                        test_targets.insert(relative.clone());
                    }
                    if is_crate_root_target(&target) {
                        crate_root_paths.insert(relative);
                    }
                }
            }
        }

        CargoMetadataStats {
            workspace_members: workspace_members.into_iter().collect(),
            crate_names: crate_names.into_iter().collect(),
            crate_root_paths: crate_root_paths.into_iter().collect(),
            test_targets: test_targets.into_iter().collect(),
            feature_names: feature_names.into_iter().collect(),
            dependencies: dependencies.into_iter().collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    id: String,
    name: String,
    manifest_path: PathBuf,
    targets: Vec<CargoTarget>,
    dependencies: Vec<CargoDependency>,
    features: std::collections::BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct CargoDependency {
    name: String,
}

#[derive(Debug, Deserialize)]
struct CargoTarget {
    src_path: PathBuf,
    kind: Vec<String>,
}

fn is_test_target(target: &CargoTarget) -> bool {
    target
        .kind
        .iter()
        .any(|kind| kind == "test" || kind == "bench")
}

fn is_crate_root_target(target: &CargoTarget) -> bool {
    target.kind.iter().any(|kind| {
        matches!(
            kind.as_str(),
            "lib" | "bin" | "proc-macro" | "test" | "bench"
        )
    })
}

fn relative_parent(repo_root: &Path, manifest_path: &Path) -> Option<String> {
    let parent = manifest_path.parent()?;
    relative_path(repo_root, parent)
}

fn relative_path(repo_root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(repo_root).ok()?;
    let display = relative.display().to_string();
    if display.is_empty() {
        Some(".".to_string())
    } else {
        Some(display)
    }
}
