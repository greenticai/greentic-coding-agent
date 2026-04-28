#![allow(dead_code)]

use clap::{Parser, Subcommand};
use gca_core::{
    KnowledgeUpdateDescriptor as EngineKnowledgeUpdateDescriptor,
    TrainingCourseDescriptor as EngineTrainingCourseDescriptor,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fmt;
use std::fs;
use std::io::{BufRead, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const LOCAL_INDEX_DIR: &str = ".greentic-agent";
const TRAINING_DIR: &str = ".greentic/training";
const UPDATES_DIR: &str = ".greentic/updates";
const SCHEMA_VERSION_V1: &str = "v1";
const BUILTIN_CONCEPT_IDS: &[&str] = &[
    "agent_training_course",
    "knowledge_update",
    "repository_index_rollout",
    "digital_worker",
    "application_pack",
    "extension_pack",
    "bundle",
    "flow",
    "component",
    "wizard",
    "setup",
    "start",
    "greentic_x",
    "greentic_sorla",
    "capability",
    "provider",
    "hook",
    "observer",
    "static_route",
];
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
const DEFAULT_PUBLIC_CATALOG_REF: &str = "ghcr.io/greenticai/indexes/catalog:latest";
const DEFAULT_REPO_ID: &str = "unknown/unknown-repo";
const LEGACY_REPO_NAME_ONLY_WARNING: &str = "legacy repo_name-only input: repo_id missing; using inferred repo_id unknown/<repo_name> for this version";
const BOOTSTRAP_TEMPLATE: &str = include_str!("../templates/CODEX_BOOTSTRAP.md.hbs");

#[derive(Debug, Parser)]
#[command(
    name = "greentic-coding-agent",
    version,
    about = "Analyze Greentic repositories, query indexed knowledge, and generate agent-facing outputs.",
    long_about = "Analyze the current repository into .greentic-agent metadata, inspect inferred concepts and workflows, search indexed docs/code/reuse policy, generate agent-facing files, package the index into an OCI-style layout, and serve MCP-style helper responses.\n\nTypical flow:\n  1. greentic-coding-agent analyze --print --format json\n  2. greentic-coding-agent describe --here --format markdown\n  3. greentic-coding-agent search --mode instruction wizard --format json\n  4. greentic-coding-agent generate-agent-files --write-root\n  5. greentic-coding-agent serve --request-file examples/mcp-request.describe-repo.json --format json",
    after_help = "Formats:\n  markdown  Human-readable output for local terminal use.\n  json      Machine-readable output for scripts, CI, or MCP-style hosts.\n\nMost commands analyze the current repository automatically if no local index exists yet."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Analyze the current repository and write local index files under `.greentic-agent/`.
    Analyze {
        /// Print the generated analyze summary to stdout after writing local files.
        #[arg(long)]
        print: bool,
        /// Print first-run bootstrap instructions even if the repo was already initialized.
        #[arg(long)]
        show_bootstrap: bool,
        /// Output format for the printed analyze summary.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Print first-run setup instructions for Codex/Claude.
    BootstrapInstructions {
        /// Output format for the bootstrap guidance.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// List inferred Greentic concepts for the current repository.
    Concepts {
        /// Output format for the concept list.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// List inferred workflows and known command flows for the current repository.
    Workflows {
        /// Output format for the workflow list.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Show the built-in command catalog this tool knows how to describe.
    #[command(name = "commands")]
    CommandList {
        /// Output format for the command catalog.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// List training courses authored for the current repository.
    Courses {
        /// Output format for the training course list.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Inspect or recommend a repository-authored training course.
    Course {
        #[command(subcommand)]
        command: CourseCommands,
    },
    /// Print agent-ready task instructions from matching training courses.
    Train {
        /// Task description used to select training courses.
        #[arg(long)]
        task: String,
        /// Target audience, for example `coding_agent`.
        #[arg(long, default_value = "coding_agent")]
        audience: String,
        /// Output format for the generated instructions.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// List or inspect knowledge updates for the current repository.
    Updates {
        #[command(subcommand)]
        command: Option<UpdatesCommands>,
        /// Update scope: `repo` for current checkout updates or `org` for synced notification feed.
        #[arg(long, default_value = "repo")]
        scope: String,
        /// Filter updates by task text.
        #[arg(long)]
        task: Option<String>,
        /// Filter updates by affected concept.
        #[arg(long)]
        concept: Option<String>,
        /// Filter updates by severity: info, recommended, important, breaking, or critical.
        #[arg(long)]
        severity: Option<String>,
        /// Show only unseen knowledge updates, including updates whose source digest changed.
        #[arg(long = "new")]
        new_only: bool,
        /// Output format for the update list.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Manage public or tenant Greentic index catalogs.
    Catalog {
        #[command(subcommand)]
        command: CatalogCommands,
    },
    /// Plan or apply org-wide coding-agent index workflow rollout.
    Org {
        #[command(subcommand)]
        command: OrgCommands,
    },
    /// Agent-facing helpers backed by the global merged index.
    Agent {
        #[command(subcommand)]
        command: AgentCommands,
    },
    /// Search indexed code, instructions, concepts, reuse policy, courses, or updates.
    Search {
        /// Search domain: `code`, `instruction`, `concept`, `reuse`, `course`, or `update`.
        #[arg(long)]
        mode: String,
        /// Search engine: `auto`, `tantivy`, or `fallback`.
        #[arg(long, default_value = "auto")]
        engine: String,
        /// Search scope: `local`, `merged`, or `all`.
        #[arg(long, default_value = "all")]
        scope: String,
        /// Restrict merged results to one repo ID.
        #[arg(long)]
        repo: Option<String>,
        /// Restrict merged results to one tenant.
        #[arg(long)]
        tenant: Option<String>,
        /// Query text to search for in the selected mode.
        query: String,
        /// Output format for the search response.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Show the seeded owner repository for a concept ID.
    LocateOwner {
        /// Concept ID to resolve, for example `wizard` or `extension_pack`.
        #[arg(long)]
        concept: String,
        /// Output format for the owner lookup.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// List validations implied by a task description.
    RequiredValidations {
        /// Free-form task text used to infer required validations.
        #[arg(long)]
        task: String,
        /// Output format for the validation response.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Package the local index and generated agent files into an OCI-style layout.
    PackageIndex {
        /// Package tag to write under `.greentic-agent/oci/<repo>/<tag>`.
        #[arg(long = "tag", default_value = "latest")]
        tag: Vec<String>,
        /// Output format for the package result.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Copy the packaged local index into the simulated remote OCI store.
    PublishIndex {
        /// Tag to publish from the local OCI-style package output.
        #[arg(long = "tag", default_value = "latest")]
        tag: Vec<String>,
        /// Publish to GHCR. Alias for `--backend ghcr`.
        #[arg(long)]
        ghcr: bool,
        /// Remote backend: `local` or `ghcr`.
        #[arg(long, default_value = "local")]
        backend: String,
        /// Registry token value. Prefer `--token-env` for local shells.
        #[arg(long)]
        token: Option<String>,
        /// Environment variable containing the registry token.
        #[arg(long)]
        token_env: Option<String>,
        /// Output format for the publish result.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// List repository packages currently present in the simulated remote OCI store.
    ListRemoteRepos {
        /// Output format for the remote repo list.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Build a discovery catalog from the simulated remote OCI store.
    ShowCatalog {
        /// Output format for the catalog response.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Explain whether the local index needs to be refreshed.
    CheckRefresh {
        /// Output format for the refresh check.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Watch catalogs/indexes and rebuild the merged query index when they change.
    WatchIndexes {
        /// Branch/channel to sync from the catalog, for example `main` or `develop`.
        #[arg(long)]
        channel: Option<String>,
        /// Tenant name for tenant-aware sync.
        #[arg(long)]
        tenant: Option<String>,
        /// Registry token value. Prefer `--token-env`.
        #[arg(long)]
        token: Option<String>,
        /// Environment variable containing the registry token.
        #[arg(long)]
        token_env: Option<String>,
        /// Poll interval in seconds.
        #[arg(long = "sync-interval", default_value_t = 300)]
        sync_interval_seconds: u64,
        /// Treat private sync failures as errors.
        #[arg(long)]
        strict_sync: bool,
        /// Remove disabled repos from the local cache.
        #[arg(long)]
        prune_disabled: bool,
        /// Run one watcher tick and exit.
        #[arg(long)]
        once: bool,
        /// Output format for watcher events.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Watch catalogs/indexes and write org notification feed items.
    Watch {
        /// Branch/channel to sync from the catalog, for example `main` or `develop`.
        #[arg(long)]
        channel: Option<String>,
        /// Poll interval such as `30s`, `10m`, or seconds.
        #[arg(long, default_value = "5m")]
        poll: String,
        /// Tenant name for tenant-aware sync.
        #[arg(long)]
        tenant: Option<String>,
        /// Registry token value. Prefer `--token-env`.
        #[arg(long)]
        token: Option<String>,
        /// Environment variable containing the registry token.
        #[arg(long)]
        token_env: Option<String>,
        /// Treat private sync failures as errors.
        #[arg(long)]
        strict_sync: bool,
        /// Remove disabled repos from the local cache.
        #[arg(long)]
        prune_disabled: bool,
        /// Run one watcher tick and exit.
        #[arg(long)]
        once: bool,
        /// Output format for watcher events.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Foreground daemon loop for keeping the global cache current.
    Daemon {
        /// Branch/channel to sync from the catalog, for example `main` or `develop`.
        #[arg(long)]
        channel: Option<String>,
        /// Poll interval such as `30s`, `10m`, or seconds.
        #[arg(long, default_value = "5m")]
        poll: String,
        /// Tenant name for tenant-aware sync.
        #[arg(long)]
        tenant: Option<String>,
        /// Registry token value. Prefer `--token-env`.
        #[arg(long)]
        token: Option<String>,
        /// Environment variable containing the registry token.
        #[arg(long)]
        token_env: Option<String>,
        /// Treat private sync failures as errors.
        #[arg(long)]
        strict_sync: bool,
        /// Remove disabled repos from the local cache.
        #[arg(long)]
        prune_disabled: bool,
        /// Run one daemon tick and exit.
        #[arg(long)]
        once: bool,
        /// Output format for daemon events.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Estimate likely blast radius for a symbol or concept.
    Impact {
        /// Symbol, concept ID, or keyword to analyze.
        #[arg(long)]
        symbol: String,
        /// Output format for the impact response.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Compare the current checkout to the indexed snapshot and suggest affected areas.
    DetectChanges {
        /// Output format for the change-detection response.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Validate a plan file against seeded ownership and validation hints.
    ValidatePlan {
        /// Path to the plan JSON file to validate.
        plan_path: String,
        /// Output format for the plan validation response.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Reserved placeholder for future plan-generation behavior.
    Plan,
    /// Emit the MCP-style tool surface or dispatch a single request file.
    Serve {
        /// Run MCP mode. This is the default server protocol.
        #[arg(long)]
        mcp: bool,
        /// Run the local HTTP JSON API.
        #[arg(long)]
        http: bool,
        /// Run MCP over newline-delimited stdio.
        #[arg(long)]
        stdio: bool,
        /// Local bind host for HTTP mode.
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// Local bind port for HTTP mode.
        #[arg(long, default_value_t = 7757)]
        port: u16,
        /// Watch for catalog/index updates.
        #[arg(long)]
        watch: bool,
        /// Poll interval for watch mode.
        #[arg(long = "sync-interval", default_value_t = 300)]
        sync_interval_seconds: u64,
        /// Treat sync failures as errors in watch mode.
        #[arg(long)]
        strict_sync: bool,
        /// Remove disabled repos from the local cache in watch mode.
        #[arg(long)]
        prune_disabled: bool,
        /// Tenant name for tenant-aware search/sync.
        #[arg(long)]
        tenant: Option<String>,
        /// Registry token value. Prefer `--token-env`.
        #[arg(long)]
        token: Option<String>,
        /// Environment variable containing the registry token.
        #[arg(long)]
        token_env: Option<String>,
        /// Public catalog reference.
        #[arg(long)]
        catalog_ref: Option<String>,
        /// Tenant catalog reference.
        #[arg(long)]
        tenant_catalog_ref: Option<String>,
        /// Optional JSON file describing one MCP-style tool call to execute.
        #[arg(long)]
        request_file: Option<String>,
        /// Output format for the snapshot or request response.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Generate `AGENTS.md`, `CLAUDE.md`, `CODEX.md`, and `llms.txt` from indexed knowledge.
    GenerateAgentFiles {
        /// Also copy generated files into the repository root in addition to `.greentic-agent/generated/`.
        #[arg(long)]
        write_root: bool,
        /// Output format for the generation result.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Install the generated GitHub workflow that analyzes and packages this repo.
    InstallGithubWorkflow {
        /// Generate the production GHCR publishing workflow.
        #[arg(long)]
        publish_ghcr: bool,
        /// Generate a catalog workflow: `public` or `tenant`.
        #[arg(long)]
        catalog: Option<String>,
        /// Tenant name for tenant catalog/index workflow generation.
        #[arg(long)]
        tenant: Option<String>,
        /// Output format for the install result.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Sync a published package from the simulated remote store into the local cache.
    Sync {
        /// Optional repository name to sync. If omitted, sync the full discovered catalog.
        #[arg(long)]
        repo: Option<String>,
        /// Optional tag to sync when `--repo` is provided. Defaults to `latest`.
        #[arg(long)]
        tag: Option<String>,
        /// Branch/channel to sync from the catalog, for example `main` or `develop`.
        #[arg(long)]
        channel: Option<String>,
        /// Public catalog OCI reference.
        #[arg(long)]
        catalog: Option<String>,
        /// Tenant name for private/tenant catalogs.
        #[arg(long)]
        tenant: Option<String>,
        /// Tenant catalog OCI reference.
        #[arg(long)]
        tenant_catalog: Option<String>,
        /// Registry token value. Prefer `--token-env` for local shells.
        #[arg(long)]
        token: Option<String>,
        /// Environment variable containing the registry token.
        #[arg(long)]
        token_env: Option<String>,
        /// Remote backend: `local` or `ghcr`.
        #[arg(long, default_value = "local")]
        backend: String,
        /// Treat missing private indexes as errors.
        #[arg(long)]
        strict: bool,
        /// Sync only public entries.
        #[arg(long)]
        public_only: bool,
        /// Sync only private/tenant entries.
        #[arg(long)]
        private_only: bool,
        /// Include private entries when tenant/auth are configured.
        #[arg(long)]
        include_private: bool,
        /// Remove disabled repos from the local index cache.
        #[arg(long)]
        prune_disabled: bool,
        /// Output format for the sync result.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Prepare the local Greentic Coding Agent cache directories.
    Init {
        /// Default channel to write into the local config.
        #[arg(long, default_value = "main")]
        channel: String,
        /// Output format for the init result.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Show local sync/cache status.
    Status {
        /// Channel to report, for example `main` or `develop`.
        #[arg(long)]
        channel: Option<String>,
        /// Output format for the status result.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Rebuild the merged Tantivy index from locally cached repo indexes.
    RebuildMergedIndex {
        /// Optional tenant whose cached indexes should be included.
        #[arg(long)]
        tenant: Option<String>,
        /// Output format for the rebuild result.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Describe the current repository or local indexed state.
    Describe {
        /// Describe the current repository checkout.
        #[arg(long)]
        here: bool,
        /// Output format for the repo description.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
}

#[derive(Debug, Subcommand)]
enum CatalogCommands {
    /// Show the editable local catalog file.
    Show {
        #[arg(long)]
        tenant: Option<String>,
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Add or update a repo entry in the local catalog.
    AddRepo {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        index_uri: String,
        #[arg(long)]
        tenant: Option<String>,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long)]
        publish: bool,
        #[arg(long, default_value = "local")]
        backend: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long)]
        token_env: Option<String>,
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Remove a repo entry from the local catalog.
    RemoveRepo {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        tenant: Option<String>,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Enable a disabled repo entry.
    EnableRepo {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        tenant: Option<String>,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Disable a repo entry while preserving history.
    DisableRepo {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        tenant: Option<String>,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Validate the editable local catalog.
    Validate {
        #[arg(long)]
        tenant: Option<String>,
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Publish the editable local catalog.
    Publish {
        #[arg(long)]
        tenant: Option<String>,
        /// Branch/channel tag for the published catalog.
        #[arg(long)]
        channel: Option<String>,
        #[arg(long)]
        expected_digest: Option<String>,
        #[arg(long, default_value = "local")]
        backend: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long)]
        token_env: Option<String>,
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Rebuild the editable catalog from published repo index packages.
    RebuildFromGhcr {
        /// GitHub organization to include, for example `greenticai`.
        #[arg(long)]
        org: String,
        /// Preferred/default channel for the rebuilt catalog.
        #[arg(long)]
        channel: String,
        /// Tenant catalog to rebuild.
        #[arg(long)]
        tenant: Option<String>,
        /// Output format for the rebuilt catalog.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
}

#[derive(Debug, Subcommand)]
enum CourseCommands {
    /// Show one training course by ID.
    Show {
        course_id: String,
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Recommend courses for a task description.
    Recommend {
        #[arg(long)]
        task: String,
        #[arg(long)]
        audience: Option<String>,
        #[arg(long, default_value = "markdown")]
        format: String,
    },
}

#[derive(Debug, Subcommand)]
enum OrgCommands {
    /// Build a deterministic rollout plan for installing the index workflow.
    PlanIndexRollout {
        /// GitHub organization to inspect, for example `greenticai`.
        #[arg(long)]
        org: String,
        /// Repository source: `github-org`, `greenticai/.github`, or `repo-list-file`.
        #[arg(long, default_value = "github-org")]
        repo_source: String,
        /// JSON file containing repo metadata for deterministic/offline planning.
        #[arg(long)]
        repo_list_file: Option<PathBuf>,
        /// Include the org `.github` repository instead of skipping it.
        #[arg(long)]
        include_dot_github: bool,
        /// Output format for the rollout plan.
        #[arg(long, default_value = "json")]
        format: String,
    },
    /// Apply a rollout plan by creating branches and optionally opening PRs.
    ApplyIndexRollout {
        /// Rollout plan JSON generated by `org plan-index-rollout`.
        #[arg(long)]
        plan: PathBuf,
        /// Open pull requests after writing workflow branches.
        #[arg(long)]
        open_prs: bool,
        /// Print intended actions without creating branches, commits, or PRs.
        #[arg(long)]
        dry_run: bool,
        /// Output format for the apply report.
        #[arg(long, default_value = "json")]
        format: String,
    },
}

#[derive(Debug, Subcommand)]
enum AgentCommands {
    /// Build task context from the global merged index and local repo overlay.
    Context {
        /// Task description used to select relevant repos, owners, updates, courses, and validations.
        #[arg(long)]
        task: String,
        /// Branch/channel to read from local sync state.
        #[arg(long)]
        channel: Option<String>,
        /// Tenant name for tenant-aware context.
        #[arg(long)]
        tenant: Option<String>,
        /// Output format for the context response.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Return preflight context and warnings for a specific task/repo pair.
    Preflight {
        /// Task description used to select relevant context.
        #[arg(long)]
        task: String,
        /// Repo ID the agent expects to edit, for example `greenticai/greentic-pack`.
        #[arg(long)]
        repo: Option<String>,
        /// Branch/channel to read from local sync state.
        #[arg(long)]
        channel: Option<String>,
        /// Tenant name for tenant-aware context.
        #[arg(long)]
        tenant: Option<String>,
        /// Output format for the preflight response.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Find likely owner repos for a concept.
    Owner {
        /// Concept ID or search text to resolve.
        #[arg(long)]
        concept: String,
        /// Branch/channel to read from local sync state.
        #[arg(long)]
        channel: Option<String>,
        /// Tenant name for tenant-aware context.
        #[arg(long)]
        tenant: Option<String>,
        /// Output format for the owner response.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
}

#[derive(Debug, Subcommand)]
enum UpdatesCommands {
    /// Show one knowledge update by ID.
    Show {
        update_id: String,
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Mark one or more knowledge updates as seen in the local user state.
    MarkSeen {
        update_id: Option<String>,
        #[arg(long)]
        all: bool,
        /// Update scope: `repo` for current checkout updates or `org` for synced notification feed.
        #[arg(long)]
        scope: Option<String>,
        #[arg(long, default_value = "markdown")]
        format: String,
    },
}

fn main() {
    let cli = Cli::parse();

    let exit_code = match cli.command {
        Some(Commands::Analyze {
            print,
            show_bootstrap,
            format,
        }) => run_analyze(print, show_bootstrap, &format),
        Some(Commands::BootstrapInstructions { format }) => run_bootstrap_instructions(&format),
        Some(Commands::CommandList { format }) => run_commands(&format),
        Some(Commands::Courses { format }) => run_courses(&format),
        Some(Commands::Course { command }) => run_course_command(command),
        Some(Commands::Train {
            task,
            audience,
            format,
        }) => run_train(&task, Some(&audience), &format),
        Some(Commands::Updates {
            command: Some(UpdatesCommands::Show { update_id, format }),
            ..
        }) => run_update_show(&update_id, &format),
        Some(Commands::Updates {
            command:
                Some(UpdatesCommands::MarkSeen {
                    update_id,
                    all,
                    scope,
                    format,
                }),
            scope: parent_scope,
            ..
        }) => run_updates_mark_seen(
            update_id.as_deref(),
            all,
            scope.as_deref().unwrap_or(&parent_scope),
            &format,
        ),
        Some(Commands::Updates {
            command: None,
            scope,
            task,
            concept,
            severity,
            new_only,
            format,
        }) => run_updates(
            &scope,
            task.as_deref(),
            concept.as_deref(),
            severity.as_deref(),
            new_only,
            &format,
        ),
        Some(Commands::Catalog { command }) => run_catalog_command(command),
        Some(Commands::Org { command }) => run_org_command(command),
        Some(Commands::Agent { command }) => run_agent_command(command),
        Some(Commands::Concepts { format }) => run_concepts(&format),
        Some(Commands::Search {
            mode,
            engine,
            scope,
            repo,
            tenant,
            query,
            format,
        }) => run_search(
            SearchOptions {
                mode: &mode,
                engine: &engine,
                scope: &scope,
                repo: repo.as_deref(),
                tenant: tenant.as_deref(),
                query: &query,
            },
            &format,
        ),
        Some(Commands::LocateOwner { concept, format }) => run_locate_owner(&concept, &format),
        Some(Commands::RequiredValidations { task, format }) => {
            run_required_validations(&task, &format)
        }
        Some(Commands::PackageIndex { tag, format }) => run_package_index(&tag, &format),
        Some(Commands::PublishIndex {
            tag,
            ghcr,
            backend,
            token,
            token_env,
            format,
        }) => run_publish_index(
            &tag,
            if ghcr { "ghcr" } else { &backend },
            token.as_deref(),
            token_env.as_deref(),
            &format,
        ),
        Some(Commands::ListRemoteRepos { format }) => run_list_remote_repos(&format),
        Some(Commands::ShowCatalog { format }) => run_show_catalog(&format),
        Some(Commands::CheckRefresh { format }) => run_check_refresh(&format),
        Some(Commands::WatchIndexes {
            channel,
            tenant,
            token,
            token_env,
            sync_interval_seconds,
            strict_sync,
            prune_disabled,
            once,
            format,
        }) => run_watch_indexes(
            WatchOptions {
                channel: channel.as_deref(),
                tenant: tenant.as_deref(),
                token: token.as_deref(),
                token_env: token_env.as_deref(),
                sync_interval_seconds,
                strict_sync,
                prune_disabled,
                once,
            },
            &format,
        ),
        Some(Commands::Watch {
            channel,
            poll,
            tenant,
            token,
            token_env,
            strict_sync,
            prune_disabled,
            once,
            format,
        }) => run_watch_alias(
            channel.as_deref(),
            &poll,
            tenant.as_deref(),
            token.as_deref(),
            token_env.as_deref(),
            strict_sync,
            prune_disabled,
            once,
            &format,
        ),
        Some(Commands::Daemon {
            channel,
            poll,
            tenant,
            token,
            token_env,
            strict_sync,
            prune_disabled,
            once,
            format,
        }) => run_watch_alias(
            channel.as_deref(),
            &poll,
            tenant.as_deref(),
            token.as_deref(),
            token_env.as_deref(),
            strict_sync,
            prune_disabled,
            once,
            &format,
        ),
        Some(Commands::Impact { symbol, format }) => run_impact(&symbol, &format),
        Some(Commands::DetectChanges { format }) => run_detect_changes(&format),
        Some(Commands::ValidatePlan { plan_path, format }) => {
            run_validate_plan(&plan_path, &format)
        }
        Some(Commands::GenerateAgentFiles { write_root, format }) => {
            run_generate_agent_files(write_root, &format)
        }
        Some(Commands::InstallGithubWorkflow {
            publish_ghcr,
            catalog,
            tenant,
            format,
        }) => run_install_github_workflow(
            WorkflowInstallOptions {
                publish_ghcr,
                catalog: catalog.as_deref(),
                tenant: tenant.as_deref(),
            },
            &format,
        ),
        Some(Commands::Sync {
            repo,
            tag,
            channel,
            catalog,
            tenant,
            tenant_catalog,
            token,
            token_env,
            backend,
            strict,
            public_only,
            private_only,
            include_private,
            prune_disabled,
            format,
        }) => run_sync(
            SyncOptions {
                repo: repo.as_deref(),
                tag: tag.as_deref(),
                channel: channel.as_deref(),
                catalog: catalog.as_deref(),
                tenant: tenant.as_deref(),
                tenant_catalog: tenant_catalog.as_deref(),
                token: token.as_deref(),
                token_env: token_env.as_deref(),
                backend: &backend,
                strict,
                public_only,
                private_only,
                include_private,
                prune_disabled,
            },
            &format,
        ),
        Some(Commands::Init { channel, format }) => run_init(&channel, &format),
        Some(Commands::Status { channel, format }) => run_status(channel.as_deref(), &format),
        Some(Commands::RebuildMergedIndex { tenant, format }) => {
            run_rebuild_merged_index(tenant.as_deref(), &format)
        }
        Some(Commands::Serve {
            mcp,
            http,
            stdio,
            host,
            port,
            watch,
            sync_interval_seconds,
            strict_sync,
            prune_disabled,
            tenant,
            token,
            token_env,
            catalog_ref,
            tenant_catalog_ref,
            request_file,
            format,
        }) => run_serve(
            ServerConfigInput {
                mcp,
                http,
                stdio,
                host: &host,
                port,
                watch,
                sync_interval_seconds,
                strict_sync,
                prune_disabled,
                tenant: tenant.as_deref(),
                token: token.as_deref(),
                token_env: token_env.as_deref(),
                catalog_ref: catalog_ref.as_deref(),
                tenant_catalog_ref: tenant_catalog_ref.as_deref(),
                request_file: request_file.as_deref(),
            },
            &format,
        ),
        Some(Commands::Workflows { format }) => run_workflows(&format),
        Some(Commands::Describe { here: true, format }) => run_describe_here(&format),
        Some(Commands::Describe { here: false, .. }) => {
            eprintln!("describe currently supports only --here");
            2
        }
        Some(other) => {
            println!("{}", placeholder_message(&other));
            0
        }
        None => {
            println!("Run `greentic-coding-agent --help` to see available commands.");
            0
        }
    };

    if exit_code != 0 {
        std::process::exit(exit_code);
    }
}

fn placeholder_message(command: &Commands) -> &'static str {
    match command {
        Commands::Analyze { .. } => "analyze is scaffolded but not implemented yet",
        Commands::BootstrapInstructions { .. } => {
            "bootstrap-instructions is scaffolded but not implemented yet"
        }
        Commands::CommandList { .. } => "commands is scaffolded but not implemented yet",
        Commands::Courses { .. } => "courses is scaffolded but not implemented yet",
        Commands::Course { .. } => "course is scaffolded but not implemented yet",
        Commands::Train { .. } => "train is scaffolded but not implemented yet",
        Commands::Updates { .. } => "updates is scaffolded but not implemented yet",
        Commands::Catalog { .. } => "catalog is scaffolded but not implemented yet",
        Commands::Org { .. } => "org is scaffolded but not implemented yet",
        Commands::Agent { .. } => "agent is scaffolded but not implemented yet",
        Commands::Concepts { .. } => "concepts is scaffolded but not implemented yet",
        Commands::Workflows { .. } => "workflows is scaffolded but not implemented yet",
        Commands::Search { .. } => "search is scaffolded but not implemented yet",
        Commands::LocateOwner { .. } => "locate-owner is scaffolded but not implemented yet",
        Commands::RequiredValidations { .. } => {
            "required-validations is scaffolded but not implemented yet"
        }
        Commands::PackageIndex { .. } => "package-index is scaffolded but not implemented yet",
        Commands::PublishIndex { .. } => "publish-index is scaffolded but not implemented yet",
        Commands::ListRemoteRepos { .. } => {
            "list-remote-repos is scaffolded but not implemented yet"
        }
        Commands::ShowCatalog { .. } => "show-catalog is scaffolded but not implemented yet",
        Commands::CheckRefresh { .. } => "check-refresh is scaffolded but not implemented yet",
        Commands::WatchIndexes { .. } => "watch-indexes is scaffolded but not implemented yet",
        Commands::Watch { .. } => "watch is scaffolded but not implemented yet",
        Commands::Daemon { .. } => "daemon is scaffolded but not implemented yet",
        Commands::Impact { .. } => "impact is scaffolded but not implemented yet",
        Commands::DetectChanges { .. } => "detect-changes is scaffolded but not implemented yet",
        Commands::ValidatePlan { .. } => "validate-plan is scaffolded but not implemented yet",
        Commands::Plan => "plan is scaffolded but not implemented yet",
        Commands::Serve { .. } => "serve is scaffolded but not implemented yet",
        Commands::GenerateAgentFiles { .. } => {
            "generate-agent-files is scaffolded but not implemented yet"
        }
        Commands::InstallGithubWorkflow { .. } => {
            "install-github-workflow is scaffolded but not implemented yet"
        }
        Commands::Sync { .. } => "sync is scaffolded but not implemented yet",
        Commands::Init { .. } => "init is scaffolded but not implemented yet",
        Commands::Status { .. } => "status is scaffolded but not implemented yet",
        Commands::RebuildMergedIndex { .. } => {
            "rebuild-merged-index is scaffolded but not implemented yet"
        }
        Commands::Describe { .. } => "describe is scaffolded but not implemented yet",
    }
}

fn engine_service() -> Result<gca_engine::CodingAgentService, String> {
    let cwd =
        current_dir().map_err(|error| format!("failed to determine current directory: {error}"))?;
    Ok(gca_engine::CodingAgentService::new(cwd, home_dir()))
}

fn run_analyze(print: bool, show_bootstrap: bool, format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };

    let start_dir = match current_dir() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("failed to determine current directory: {error}");
            return 1;
        }
    };

    let home_dir = home_dir();
    let repo_root = match find_repo_root(&start_dir) {
        Some(repo_root) => repo_root,
        None => {
            eprintln!(
                "failed to detect repository root from {}",
                start_dir.display()
            );
            return 1;
        }
    };
    let first_run = !repo_root.join(LOCAL_INDEX_DIR).exists();
    match analyze_repo(&start_dir, &default_registry_path(&home_dir)) {
        Ok(outputs) => {
            if print {
                print_analyze_summary(&outputs, format);
            }
            if first_run || show_bootstrap {
                if print {
                    println!();
                }
                print_bootstrap_guidance(
                    &bootstrap_guidance_for(&outputs.manifest.repo_id),
                    format,
                );
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run_bootstrap_instructions(format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let repo_id = repo_root_from_cwd()
        .ok()
        .map(|repo_root| {
            let repo_name = repo_root
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("unknown-repo");
            detect_repo_id(&repo_root, repo_name)
        })
        .unwrap_or_else(default_repo_id);
    print_bootstrap_guidance(&bootstrap_guidance_for(&repo_id), format);
    0
}

fn run_concepts(format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };

    match engine_service().and_then(|service| {
        service
            .concepts(gca_engine::ConceptsOptions)
            .map_err(|error| error.to_string())
    }) {
        Ok(response) => {
            match format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&response.concepts)
                            .expect("concept graph should serialize")
                    );
                }
                OutputFormat::Markdown => {
                    println!("# Concepts");
                    println!();
                    for concept in response.concepts {
                        println!(
                            "- `{}`: {}",
                            concept.id,
                            if concept.summary.is_empty() {
                                concept.title
                            } else {
                                concept.summary
                            }
                        );
                    }
                }
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run_commands(format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };

    match engine_service().and_then(|service| {
        service
            .commands(gca_engine::CommandsOptions)
            .map_err(|error| error.to_string())
    }) {
        Ok(response) => {
            match format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&response.commands)
                            .expect("command catalog should serialize")
                    );
                }
                OutputFormat::Markdown => {
                    println!("# Commands");
                    println!();
                    for entry in response.commands {
                        println!("- `{}`: {}", entry.command, entry.purpose);
                    }
                }
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run_courses(format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };

    match engine_service().and_then(|service| {
        service
            .courses(gca_engine::CoursesOptions)
            .map_err(|error| error.to_string())
    }) {
        Ok(response) => {
            match format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&response.courses)
                            .expect("training courses should serialize")
                    );
                }
                OutputFormat::Markdown => {
                    println!("# Training Courses");
                    println!();
                    if response.courses.is_empty() {
                        println!("No training courses found.");
                    }
                    for course in response.courses {
                        println!("- `{}`: {}", course.id, course.summary);
                    }
                }
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run_course_command(command: CourseCommands) -> i32 {
    match command {
        CourseCommands::Show { course_id, format } => run_course_show(&course_id, &format),
        CourseCommands::Recommend {
            task,
            audience,
            format,
        } => run_course_recommend(&task, audience.as_deref(), &format),
    }
}

fn run_course_show(course_id: &str, format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    match engine_service().and_then(|service| {
        service
            .show_course(gca_engine::ShowCourseOptions {
                course_id: course_id.to_string(),
            })
            .map_err(|error| error.to_string())
    }) {
        Ok(response) => {
            match format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&response.course)
                            .expect("training course should serialize")
                    );
                }
                OutputFormat::Markdown => print_training_course(&response.course),
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run_course_recommend(task: &str, audience: Option<&str>, format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    match engine_service().and_then(|service| {
        service
            .recommend_training_courses(gca_engine::RecommendTrainingCoursesOptions {
                task: task.to_string(),
                audience: audience.map(str::to_string),
            })
            .map_err(|error| error.to_string())
    }) {
        Ok(response) => {
            match format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "updates": response.updates,
                            "courses": response.courses,
                        }))
                        .expect("training recommendations should serialize")
                    );
                }
                OutputFormat::Markdown => {
                    println!("# Recommended Training Courses");
                    println!();
                    print_task_updates(&response.updates);
                    for course in response.courses {
                        println!("- `{}`: {}", course.id, course.title);
                    }
                }
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run_train(task: &str, audience: Option<&str>, format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    match engine_service().and_then(|service| {
        service
            .train(gca_engine::TrainOptions {
                task: task.to_string(),
                audience: audience.map(str::to_string),
            })
            .map_err(|error| error.to_string())
    }) {
        Ok(response) => {
            match format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "task": response.task,
                            "updates": response.updates,
                            "courses": response.courses,
                        }))
                        .expect("training instructions should serialize")
                    );
                }
                OutputFormat::Markdown => {
                    println!("# Training Plan");
                    println!();
                    println!("Task: {}", response.task);
                    println!();
                    print_task_updates(&response.updates);
                    if response.courses.is_empty() {
                        println!("No matching training courses found.");
                    }
                    for course in response.courses {
                        print_training_course(&course);
                    }
                }
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn print_task_updates(updates: &[EngineKnowledgeUpdateDescriptor]) {
    if updates.is_empty() {
        return;
    }
    println!("# Knowledge updates affecting this task");
    println!();
    for update in updates {
        println!("- `{}`", update.id);
        println!("  - Severity: {}", update.severity.as_str());
        println!("  - Agent instruction: {}", update.agent_instruction);
        for command in &update.deprecated_commands {
            println!("  - Deprecated: `{}`", command.command);
            if let Some(replacement) = &command.replacement {
                println!("  - Replacement: `{replacement}`");
            }
        }
    }
    println!();
}

fn print_training_course(course: &EngineTrainingCourseDescriptor) {
    println!("## {}", course.title);
    println!();
    println!("{}", course.summary);
    println!();
    if !course.teaches_concepts.is_empty() {
        println!(
            "Concepts: {}",
            course
                .teaches_concepts
                .iter()
                .map(|concept| format!("`{concept}`"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!();
    }
    if !course.canonical_commands.is_empty() {
        println!("Canonical commands:");
        for command in &course.canonical_commands {
            println!("- `{command}`");
        }
        println!();
    }
    if !course.deprecated_commands.is_empty() {
        println!("Deprecated commands:");
        for command in &course.deprecated_commands {
            if let Some(replacement) = &command.replacement {
                println!(
                    "- Avoid `{}`: {} Use `{}`.",
                    command.command, command.reason, replacement
                );
            } else {
                println!("- Avoid `{}`: {}", command.command, command.reason);
            }
        }
        println!();
    }
    for module in &course.modules {
        println!("### {}", module.title);
        println!();
        println!("{}", module.objective);
        println!();
        for step in &module.steps {
            println!("{}. {}", step.order, step.instruction);
            if let Some(command) = &step.command {
                println!("   Command: `{command}`");
            }
            if let Some(expected) = &step.expected_output {
                println!("   Expected: {expected}");
            }
            if let Some(validation) = &step.validation {
                println!("   Validate: `{validation}`");
            }
        }
        println!();
    }
    if !course.required_validations.is_empty() {
        println!("Required validations:");
        for validation in &course.required_validations {
            println!("- `{validation}`");
        }
        println!();
    }
}

fn run_updates(
    scope: &str,
    task: Option<&str>,
    concept: Option<&str>,
    severity: Option<&str>,
    new_only: bool,
    format: &str,
) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    if scope == "org" {
        return run_org_updates(new_only, format);
    }
    if scope != "repo" {
        eprintln!("unsupported update scope: {scope}");
        return 2;
    }
    match engine_service().and_then(|service| {
        service
            .updates(gca_engine::UpdatesOptions {
                task: task.map(str::to_string),
                concept: concept.map(str::to_string),
                severity: severity.map(str::to_string),
                new_only,
            })
            .map_err(|error| error.to_string())
    }) {
        Ok(response) => {
            match format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&response.updates)
                            .expect("knowledge updates should serialize")
                    );
                }
                OutputFormat::Markdown => {
                    println!("# Knowledge Updates");
                    println!();
                    if response.updates.is_empty() {
                        println!("No knowledge updates found.");
                    }
                    for update in response.updates {
                        println!(
                            "- `{}` [{}]: {}",
                            update.id,
                            update.severity.as_str(),
                            update.title
                        );
                    }
                }
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run_updates_mark_seen(update_id: Option<&str>, all: bool, scope: &str, format: &str) -> i32 {
    if !all && update_id.is_none() {
        eprintln!("updates mark-seen requires an update ID or --all");
        return 2;
    }
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    if scope == "org" {
        return run_org_updates_mark_seen(update_id, all, format);
    }
    if scope != "repo" {
        eprintln!("unsupported update scope: {scope}");
        return 2;
    }
    match engine_service().and_then(|service| {
        service
            .mark_knowledge_update_seen(gca_engine::MarkKnowledgeUpdateSeenOptions {
                update_id: update_id.map(str::to_string),
                all,
            })
            .map_err(|error| error.to_string())
    }) {
        Ok(response) => {
            match format {
                OutputFormat::Json => println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "state_path": response.state_path,
                        "marked_updates": response.marked_updates,
                        "state": response.state,
                    }))
                    .expect("seen-state response should serialize")
                ),
                OutputFormat::Markdown => {
                    println!("# Mark Knowledge Updates Seen");
                    println!();
                    println!("- State: `{}`", response.state_path.display());
                    if response.marked_updates.is_empty() {
                        println!("- No updates marked.");
                    } else {
                        for update in response.marked_updates {
                            println!("- Marked `{update}`");
                        }
                    }
                }
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run_update_show(update_id: &str, format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    match engine_service().and_then(|service| {
        service
            .show_update(gca_engine::ShowUpdateOptions {
                update_id: update_id.to_string(),
            })
            .map_err(|error| error.to_string())
    }) {
        Ok(response) => {
            match format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&response.update)
                            .expect("knowledge update should serialize")
                    );
                }
                OutputFormat::Markdown => print_knowledge_update(&response.update),
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run_org_updates(new_only: bool, format: OutputFormat) -> i32 {
    let feed = load_notification_feed(&home_dir());
    let seen = load_notification_seen_state(&home_dir());
    let mut items = feed.items;
    if new_only {
        items.retain(|item| !seen.seen.contains_key(&item.id));
    }
    items.sort_by(|left, right| {
        right
            .created_at
            .cmp(&left.created_at)
            .then(left.repo_id.cmp(&right.repo_id))
            .then(left.id.cmp(&right.id))
    });
    match format {
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&items).expect("notification feed should serialize")
        ),
        OutputFormat::Markdown => {
            println!("# Org Knowledge Updates");
            println!();
            if items.is_empty() {
                println!("No org knowledge updates found.");
            }
            for item in &items {
                println!("- `{}` [{}]: {}", item.id, item.branch, item.agent_impact);
            }
        }
    }
    0
}

fn run_org_updates_mark_seen(update_id: Option<&str>, all: bool, format: OutputFormat) -> i32 {
    let home = home_dir();
    let feed = load_notification_feed(&home);
    let mut seen = load_notification_seen_state(&home);
    let seen_at = timestamp_string();
    let mut marked = Vec::new();
    if all {
        for item in &feed.items {
            seen.seen.insert(item.id.clone(), seen_at.clone());
            marked.push(item.id.clone());
        }
    } else if let Some(update_id) = update_id
        && feed.items.iter().any(|item| item.id == update_id)
    {
        seen.seen.insert(update_id.to_string(), seen_at);
        marked.push(update_id.to_string());
    }
    if let Err(error) = write_notification_seen_state(&home, &seen) {
        eprintln!("{error}");
        return 1;
    }
    match format {
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "state_path": notification_seen_path(&home),
                "marked_updates": marked,
                "state": seen,
            }))
            .expect("notification seen-state should serialize")
        ),
        OutputFormat::Markdown => {
            println!("# Mark Org Updates Seen");
            println!();
            println!("- State: `{}`", notification_seen_path(&home).display());
            if marked.is_empty() {
                println!("- No updates marked.");
            } else {
                for update in marked {
                    println!("- Marked `{update}`");
                }
            }
        }
    }
    0
}

fn print_knowledge_update(update: &EngineKnowledgeUpdateDescriptor) {
    println!("## {}", update.title);
    println!();
    println!("Severity: `{}`", update.severity.as_str());
    println!("Type: `{}`", update.update_type.as_str());
    println!("Published: {}", update.published_at);
    println!();
    println!("{}", update.summary);
    println!();
    println!("Agent instruction: {}", update.agent_instruction);
    println!();
    if !update.deprecated_commands.is_empty() {
        println!("Deprecated commands:");
        for command in &update.deprecated_commands {
            if let Some(replacement) = &command.replacement {
                println!(
                    "- Avoid `{}`: {} Use `{}`.",
                    command.command, command.reason, replacement
                );
            } else {
                println!("- Avoid `{}`: {}", command.command, command.reason);
            }
        }
        println!();
    }
    if !update.migration_steps.is_empty() {
        println!("Migration steps:");
        for step in &update.migration_steps {
            println!("{}. {}", step.order, step.instruction);
            if let Some(command) = &step.command {
                println!("   Command: `{command}`");
            }
            if let Some(validation) = &step.validation {
                println!("   Validate: `{validation}`");
            }
        }
        println!();
    }
    if !update.required_validations.is_empty() {
        println!("Required validations:");
        for validation in &update.required_validations {
            println!("- `{validation}`");
        }
        println!();
    }
}

fn run_search(options: SearchOptions<'_>, format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let mode = match SearchMode::parse(options.mode) {
        Ok(mode) => mode,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let engine = match SearchEngineChoice::parse(options.engine) {
        Ok(engine) => engine,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let scope = match SearchScope::parse(options.scope) {
        Ok(scope) => scope,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };

    let mut combined = SearchResponse {
        mode,
        query: options.query.trim().to_string(),
        results: Vec::new(),
    };

    if matches!(scope, SearchScope::Local | SearchScope::All) {
        match load_or_analyze_repo_index() {
            Ok(repo_index) => {
                let tantivy_dir = repo_root_from_cwd()
                    .ok()
                    .map(|root| root.join(LOCAL_INDEX_DIR).join("tantivy").join("local"));
                match search_repo_index_with_engine(
                    &repo_index,
                    tantivy_dir.as_deref(),
                    mode,
                    options.query,
                    engine,
                ) {
                    Ok(response) => combined.results.extend(response.results),
                    Err(error) => {
                        eprintln!("{error}");
                        return 1;
                    }
                }
            }
            Err(error) if scope == SearchScope::Local => {
                eprintln!("{error}");
                return 1;
            }
            Err(_) => {}
        }
    }

    if matches!(scope, SearchScope::Merged | SearchScope::All) {
        let merged_dir = merged_tantivy_path(&home_dir());
        if merged_dir.exists() {
            match search_tantivy_index_filtered(
                &merged_dir,
                mode,
                options.query,
                options.repo,
                options.tenant,
            ) {
                Ok(response) => combined.results.extend(response.results),
                Err(error) if scope == SearchScope::Merged => {
                    eprintln!("{error}");
                    return 1;
                }
                Err(_) => {}
            }
        } else if scope == SearchScope::Merged {
            eprintln!("merged Tantivy index not found at {}", merged_dir.display());
            return 1;
        }
    }

    combined.results.sort_by(|left, right| {
        left.repo_id
            .cmp(&right.repo_id)
            .then(left.id.cmp(&right.id))
    });
    combined.results.truncate(20);

    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&combined).expect("search response should serialize")
            );
        }
        OutputFormat::Markdown => print_search_response(&combined),
    }
    0
}

fn run_locate_owner(concept: &str, format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };

    match engine_service().and_then(|service| {
        service
            .locate_owner(gca_engine::LocateOwnerOptions {
                concept_id: concept.to_string(),
            })
            .map_err(|error| error.to_string())
    }) {
        Ok(owner) => {
            match format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&Some(&owner))
                            .expect("owner lookup should serialize")
                    );
                }
                OutputFormat::Markdown => {
                    println!("# Owner Lookup");
                    println!();
                    println!("- Concept: `{}`", owner.concept_id);
                    println!("- Owner repo: `{}`", owner.owner_repo);
                    println!("- Rationale: {}", owner.rationale);
                }
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run_agent_command(command: AgentCommands) -> i32 {
    match command {
        AgentCommands::Context {
            task,
            channel,
            tenant,
            format,
        } => run_agent_context(&task, channel.as_deref(), tenant.as_deref(), &format),
        AgentCommands::Preflight {
            task,
            repo,
            channel,
            tenant,
            format,
        } => run_agent_preflight(
            &task,
            repo.as_deref(),
            channel.as_deref(),
            tenant.as_deref(),
            &format,
        ),
        AgentCommands::Owner {
            concept,
            channel,
            tenant,
            format,
        } => run_agent_owner(&concept, channel.as_deref(), tenant.as_deref(), &format),
    }
}

fn run_agent_context(task: &str, channel: Option<&str>, tenant: Option<&str>, format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let response = build_agent_context(task, channel, tenant, None);
    print_agent_context(&response, format);
    0
}

fn run_agent_preflight(
    task: &str,
    repo: Option<&str>,
    channel: Option<&str>,
    tenant: Option<&str>,
    format: &str,
) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let response = build_agent_context(task, channel, tenant, repo);
    print_agent_context(&response, format);
    0
}

fn run_agent_owner(
    concept: &str,
    channel: Option<&str>,
    tenant: Option<&str>,
    format: &str,
) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let response = build_agent_context(concept, channel, tenant, None);
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "concept": concept,
                    "channel": response.channel,
                    "owner_candidates": response.owner_candidates,
                    "warnings": response.warnings,
                }))
                .expect("agent owner response should serialize")
            );
        }
        OutputFormat::Markdown => {
            println!("# Agent Owner");
            println!();
            println!("- Concept: `{concept}`");
            println!("- Channel: `{}`", response.channel);
            if response.owner_candidates.is_empty() {
                println!("- Owner candidates: none");
            } else {
                println!();
                println!("Owner candidates:");
                for owner in &response.owner_candidates {
                    println!("- `{}` owns `{}`", owner.owner_repo, owner.concept_id);
                }
            }
            print_agent_warnings(&response.warnings);
        }
    }
    0
}

fn build_agent_context(
    task: &str,
    channel: Option<&str>,
    tenant: Option<&str>,
    preflight_repo: Option<&str>,
) -> AgentContextResponse {
    let home = home_dir();
    let selected_channel = channel
        .map(ToString::to_string)
        .or_else(|| default_channel_from_config(&home))
        .unwrap_or_else(|| "develop".to_string());
    let mut warnings = Vec::new();
    let mut relevant_repos = BTreeMap::<String, AgentRepoCandidate>::new();
    let mut owner_candidates = Vec::<OwnerLookup>::new();
    let mut required_validations_by_id = BTreeMap::<String, ValidationDescriptor>::new();
    let mut recent_updates_by_id = BTreeMap::<String, KnowledgeUpdateDescriptor>::new();
    let mut tutorials_by_id = BTreeMap::<String, TrainingCourseDescriptor>::new();

    let cached = match load_cached_repo_indexes(&home, tenant) {
        Ok(cached) => cached,
        Err(error) => {
            warnings.push(error);
            Vec::new()
        }
    };
    if cached.is_empty() {
        warnings.push(
            "no global synced indexes found; run `greentic-coding-agent sync --channel <channel>`"
                .to_string(),
        );
    }

    for cached_index in cached
        .iter()
        .filter(|entry| synced_entry_matches_channel(&entry.state, &selected_channel))
    {
        collect_agent_context_from_repo(
            &cached_index.repo_index,
            cached_index
                .repo_index
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.branch.clone()),
            "global",
            task,
            &mut relevant_repos,
            &mut owner_candidates,
            &mut required_validations_by_id,
            &mut recent_updates_by_id,
            &mut tutorials_by_id,
        );
    }

    if let Ok(local_index) = load_or_analyze_repo_index() {
        collect_agent_context_from_repo(
            &local_index,
            local_index
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.branch.clone()),
            "local_overlay",
            task,
            &mut relevant_repos,
            &mut owner_candidates,
            &mut required_validations_by_id,
            &mut recent_updates_by_id,
            &mut tutorials_by_id,
        );
    }

    if let Some(repo) = preflight_repo
        && !relevant_repos.contains_key(repo)
    {
        warnings.push(format!(
            "requested repo `{repo}` did not match task context in channel `{selected_channel}`"
        ));
    }

    owner_candidates.sort_by(|left, right| {
        left.owner_repo
            .cmp(&right.owner_repo)
            .then(left.concept_id.cmp(&right.concept_id))
    });
    owner_candidates.dedup_by(|left, right| {
        left.owner_repo == right.owner_repo && left.concept_id == right.concept_id
    });

    AgentContextResponse {
        task: task.to_string(),
        channel: selected_channel,
        relevant_repos: relevant_repos.into_values().collect(),
        owner_candidates,
        required_validations: required_validations_by_id.into_values().collect(),
        recent_updates: recent_updates_by_id.into_values().collect(),
        tutorials: tutorials_by_id.into_values().collect(),
        warnings,
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_agent_context_from_repo(
    repo_index: &RepoIndex,
    branch: Option<String>,
    source: &str,
    task: &str,
    relevant_repos: &mut BTreeMap<String, AgentRepoCandidate>,
    owner_candidates: &mut Vec<OwnerLookup>,
    required_validations_by_id: &mut BTreeMap<String, ValidationDescriptor>,
    recent_updates_by_id: &mut BTreeMap<String, KnowledgeUpdateDescriptor>,
    tutorials_by_id: &mut BTreeMap<String, TrainingCourseDescriptor>,
) {
    let mut matched = Vec::new();
    if text_matches_task(&repo_index.repo_id, task)
        || text_matches_task(&repo_index.repo_name, task)
    {
        matched.push("repo".to_string());
    }
    for concept in &repo_index.concept_graph {
        if text_matches_task(&concept.id, task)
            || text_matches_task(&concept.title, task)
            || text_matches_task(&concept.summary, task)
        {
            matched.push(format!("concept:{}", concept.id));
        }
    }
    for instruction in &repo_index.instruction_graph {
        if text_matches_task(&instruction.title, task)
            || text_matches_task(&instruction.path, task)
            || instruction
                .commands
                .iter()
                .any(|command| text_matches_task(command, task))
        {
            matched.push(format!("instruction:{}", instruction.id));
        }
    }
    for reuse in &repo_index.reuse {
        if text_matches_task(&reuse.concept_id, task) || text_matches_task(&reuse.rationale, task) {
            matched.push(format!("owner:{}", reuse.concept_id));
            owner_candidates.push(OwnerLookup {
                concept_id: reuse.concept_id.clone(),
                owner_repo: reuse.owner_repo.clone(),
                rationale: reuse.rationale.clone(),
                forbidden_locations: reuse.forbidden_locations.clone(),
                required_validations: reuse.required_validations.clone(),
            });
        }
    }
    for validation in required_validations(repo_index, task).validations {
        matched.push(format!("validation:{}", validation.id));
        required_validations_by_id.insert(validation.id.clone(), validation);
    }
    for update in &repo_index.knowledge_updates {
        if text_matches_task(&update.title, task)
            || text_matches_task(&update.summary, task)
            || text_matches_task(&update.agent_instruction, task)
            || update
                .affected_concepts
                .iter()
                .any(|concept| text_matches_task(concept, task))
        {
            matched.push(format!("update:{}", update.id));
            recent_updates_by_id.insert(update.id.clone(), update.clone());
        }
    }
    for course in &repo_index.training_courses {
        if text_matches_task(&course.title, task)
            || text_matches_task(&course.summary, task)
            || course
                .tasks
                .iter()
                .any(|course_task| text_matches_task(course_task, task))
            || course
                .teaches_concepts
                .iter()
                .any(|concept| text_matches_task(concept, task))
        {
            matched.push(format!("course:{}", course.id));
            tutorials_by_id.insert(course.id.clone(), course.clone());
        }
    }
    matched.sort();
    matched.dedup();
    if !matched.is_empty() {
        relevant_repos.insert(
            repo_index.repo_id.clone(),
            AgentRepoCandidate {
                repo_id: repo_index.repo_id.clone(),
                branch,
                source: source.to_string(),
                matched,
            },
        );
    }
}

fn text_matches_task(text: &str, task: &str) -> bool {
    let text = text.to_ascii_lowercase().replace(['_', '-'], " ");
    task.to_ascii_lowercase()
        .replace(['_', '-'], " ")
        .split_whitespace()
        .filter(|token| token.len() > 2)
        .any(|token| text.contains(token))
}

fn synced_entry_matches_channel(entry: &SyncedRepoState, channel: &str) -> bool {
    let suffix = format!(":{channel}");
    entry.package_ref.ends_with(&suffix)
        || entry
            .local_index_path
            .components()
            .any(|component| component.as_os_str().to_string_lossy() == channel)
}

fn default_channel_from_config(home: &Path) -> Option<String> {
    let raw = fs::read_to_string(home.join(".greentic-agent").join("config.toml")).ok()?;
    raw.lines().find_map(|line| {
        let line = line.trim();
        line.strip_prefix("default_channel")
            .and_then(|value| value.split_once('='))
            .map(|(_, value)| value.trim().trim_matches('"').to_string())
            .filter(|value| !value.is_empty())
    })
}

fn print_agent_context(response: &AgentContextResponse, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(response)
                    .expect("agent context response should serialize")
            );
        }
        OutputFormat::Markdown => {
            println!("# Agent Context");
            println!();
            println!("- Task: {}", response.task);
            println!("- Channel: `{}`", response.channel);
            println!("- Relevant repos: {}", response.relevant_repos.len());
            println!("- Owner candidates: {}", response.owner_candidates.len());
            println!(
                "- Required validations: {}",
                response.required_validations.len()
            );
            println!("- Recent updates: {}", response.recent_updates.len());
            println!("- Tutorials: {}", response.tutorials.len());
            print_agent_warnings(&response.warnings);
        }
    }
}

fn print_agent_warnings(warnings: &[String]) {
    if !warnings.is_empty() {
        println!();
        println!("Warnings:");
        for warning in warnings {
            println!("- {warning}");
        }
    }
}

fn run_required_validations(task: &str, format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };

    match engine_service().and_then(|service| {
        service
            .required_validations(gca_engine::RequiredValidationsOptions {
                task: task.to_string(),
            })
            .map_err(|error| error.to_string())
    }) {
        Ok(response) => {
            match format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&response)
                            .expect("required validations should serialize")
                    );
                }
                OutputFormat::Markdown => {
                    println!("# Required Validations");
                    println!();
                    println!("- Task: `{}`", response.task);
                    if response.validations.is_empty() {
                        println!("- No matching validations found.");
                    } else {
                        for validation in &response.validations {
                            println!("- `{}`: {}", validation.id, validation.summary);
                        }
                    }
                }
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run_generate_agent_files(write_root: bool, format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };

    match engine_service().and_then(|service| {
        service
            .generate_agent_files(gca_engine::GenerateAgentFilesOptions { write_root })
            .map_err(|error| error.to_string())
    }) {
        Ok(response) => {
            match format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&response.written_paths)
                            .expect("generated paths should serialize")
                    );
                }
                OutputFormat::Markdown => {
                    println!("# Generate Agent Files");
                    println!();
                    println!("- Files written: `{}`", response.written_paths.len());
                    for path in &response.written_paths {
                        println!("- `{}`", path.display());
                    }
                }
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run_package_index(tags: &[String], format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };

    match engine_service().and_then(|service| {
        service
            .package_index(gca_engine::PackageIndexOptions {
                tags: tags.to_vec(),
            })
            .map_err(|error| error.to_string())
    }) {
        Ok(response) => {
            match format {
                OutputFormat::Json => println!(
                    "{}",
                    serde_json::to_string_pretty(&response.packages)
                        .expect("package result should serialize")
                ),
                OutputFormat::Markdown => {
                    println!("# Package Index");
                    println!();
                    for package in response.packages {
                        println!("- Reference: `{}`", package.reference);
                        println!("- Package dir: `{}`", package.package_dir.display());
                    }
                }
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run_publish_index(
    tags: &[String],
    backend: &str,
    token: Option<&str>,
    token_env: Option<&str>,
    format: &str,
) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    if backend == "local" {
        return match engine_service().and_then(|service| {
            service
                .publish_index(gca_engine::PublishIndexOptions {
                    tags: tags.to_vec(),
                    remote_root: None,
                })
                .map_err(|error| error.to_string())
        }) {
            Ok(response) => {
                match format {
                    OutputFormat::Json => println!(
                        "{}",
                        serde_json::to_string_pretty(&response.published_paths)
                            .expect("publish target should serialize")
                    ),
                    OutputFormat::Markdown => {
                        println!("# Publish Index");
                        println!();
                        for path in response.published_paths {
                            println!("- Remote store: `{}`", path.display());
                        }
                    }
                }
                0
            }
            Err(error) => {
                eprintln!("{error}");
                1
            }
        };
    }
    let repo_root = match repo_root_from_cwd() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            return 1;
        }
    };
    let remote_root = default_remote_store_path(&home_dir());
    let remote_config = match resolve_remote_config(SyncOptions {
        repo: None,
        tag: tags.first().map(String::as_str),
        channel: None,
        catalog: None,
        tenant: None,
        tenant_catalog: None,
        token,
        token_env,
        backend,
        strict: false,
        public_only: false,
        private_only: false,
        include_private: false,
        prune_disabled: false,
    }) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };

    match load_or_analyze_repo_index() {
        Ok(repo_index) => {
            let mut results = Vec::new();
            let mut targets = Vec::new();
            for tag in normalized_cli_tags(tags) {
                match package_index_layout(&repo_root, &repo_index, &tag) {
                    Ok(result) => {
                        let target = match remote_config.backend {
                            RemoteBackendKind::LocalFixture => {
                                let target = remote_root
                                    .join(repo_id_path(&repo_index.repo_id))
                                    .join(&tag);
                                if let Err(error) = copy_dir_all(&result.package_dir, &target) {
                                    eprintln!("failed to publish package: {error}");
                                    return 1;
                                }
                                target
                            }
                            RemoteBackendKind::GhcrOras => {
                                if let Err(error) = oras_push(
                                    &result.reference,
                                    &result.package_dir,
                                    remote_config.auth.as_ref(),
                                ) {
                                    eprintln!("{error}");
                                    return 1;
                                }
                                PathBuf::from(&result.reference)
                            }
                        };
                        results.push(result);
                        targets.push(target);
                    }
                    Err(error) => {
                        eprintln!("failed to package index: {error}");
                        return 1;
                    }
                }
            }
            match format {
                OutputFormat::Json => println!(
                    "{}",
                    serde_json::to_string_pretty(&targets)
                        .expect("publish targets should serialize")
                ),
                OutputFormat::Markdown => {
                    println!("# Publish Index");
                    println!();
                    for (result, target) in results.iter().zip(targets.iter()) {
                        println!("- Reference: `{}`", result.reference);
                        println!("- Remote store: `{}`", target.display());
                    }
                }
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn normalized_cli_tags(tags: &[String]) -> Vec<String> {
    let mut normalized = if tags.is_empty() {
        vec!["latest".to_string()]
    } else {
        tags.iter()
            .map(|tag| tag.trim().to_string())
            .filter(|tag| !tag.is_empty())
            .collect::<Vec<_>>()
    };
    normalized.sort();
    normalized.dedup();
    if normalized.is_empty() {
        normalized.push("latest".to_string());
    }
    normalized
}

fn run_sync(options: SyncOptions<'_>, format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let remote_config = resolve_remote_config(options).ok();
    let report = match execute_sync(options) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("{error}");
            return 1;
        }
    };
    print_sync_report(&report, format);
    if report.failed.is_empty() || remote_config.is_some_and(|config| !config.strict) {
        0
    } else {
        1
    }
}

fn execute_sync(options: SyncOptions<'_>) -> Result<SyncReport, String> {
    let home = home_dir();
    let remote_root = default_remote_store_path(&home);
    let cache_root = default_sync_cache_path(&home);
    let indexes_root = default_indexes_path(&home);
    let remote_config = resolve_remote_config(options)?;
    let mut report = SyncReport {
        channel: options.channel.map(ToString::to_string),
        public_catalog: Some(remote_config.public_catalog_ref.clone()),
        tenant_catalog: remote_config.tenant_catalog_ref.clone(),
        downloaded: Vec::new(),
        skipped: Vec::new(),
        failed: Vec::new(),
        merged_index_path: merged_tantivy_path(&home),
    };

    if let Some(repo) = options.repo {
        let tag = options.tag.or(options.channel).unwrap_or("latest");
        let source = remote_root.join(repo_id_path(repo)).join(tag);
        let target = cache_root.join(repo_id_path(repo)).join(tag);
        match remote_config.backend {
            RemoteBackendKind::LocalFixture => {
                let sync_report = gca_oci::sync_repo_with_state(
                    &remote_root,
                    &cache_root,
                    &indexes_root,
                    &home,
                    repo,
                    tag,
                    None,
                )?;
                report.downloaded = sync_report
                    .downloaded
                    .into_iter()
                    .map(|path| path.display().to_string())
                    .collect();
                report.channel = sync_report.channel;
                report.skipped = sync_report.skipped;
                report.failed = sync_report
                    .failed
                    .into_iter()
                    .map(|failure| SyncFailure {
                        repo_id: failure.repo_id,
                        error: failure.error,
                    })
                    .collect();
                let _ = source;
                let _ = target;
            }
            RemoteBackendKind::GhcrOras => {
                let reference = if repo.starts_with("ghcr.io/") {
                    repo.to_string()
                } else {
                    format!("ghcr.io/greenticai/indexes/{repo}:{tag}")
                };
                oras_pull(&reference, &target, remote_config.auth.as_ref())?;
                report.downloaded.push(target.display().to_string());
            }
        }
    } else {
        match remote_config.backend {
            RemoteBackendKind::LocalFixture => match gca_oci::sync_catalog_with_state(
                &remote_root,
                &cache_root,
                &indexes_root,
                &home,
                &gca_oci::SyncCatalogOptions {
                    channel: options.channel.map(ToString::to_string),
                    tenant: options.tenant.map(ToString::to_string),
                    public_only: options.public_only,
                    private_only: options.private_only,
                    include_private: options.include_private,
                    prune_disabled: options.prune_disabled,
                },
            ) {
                Ok(sync_report) => {
                    report.public_catalog = sync_report.public_catalog;
                    report.tenant_catalog = sync_report.tenant_catalog;
                    report.downloaded = sync_report
                        .downloaded
                        .into_iter()
                        .map(|path| path.display().to_string())
                        .collect();
                    report.skipped = sync_report.skipped;
                    report.failed = sync_report
                        .failed
                        .into_iter()
                        .map(|failure| SyncFailure {
                            repo_id: failure.repo_id,
                            error: failure.error,
                        })
                        .collect();
                    report.merged_index_path = sync_report.merged_index_path;
                }
                Err(error) => {
                    return Err(format!("failed to sync catalog: {error}"));
                }
            },
            RemoteBackendKind::GhcrOras => {
                let public_target = cache_root.join("catalogs").join("public");
                oras_pull(
                    &remote_config.public_catalog_ref,
                    &public_target,
                    remote_config.auth.as_ref(),
                )?;
                let mut synced = vec![public_target];
                if let (Some(_tenant), Some(tenant_catalog_ref)) =
                    (&remote_config.tenant, &remote_config.tenant_catalog_ref)
                {
                    let tenant_target = cache_root.join("catalogs").join("tenant");
                    if let Err(error) = oras_pull(
                        tenant_catalog_ref,
                        &tenant_target,
                        remote_config.auth.as_ref(),
                    ) {
                        if remote_config.strict {
                            return Err(error);
                        }
                    } else {
                        synced.push(tenant_target);
                    }
                }
                report.downloaded = synced
                    .into_iter()
                    .map(|path| path.display().to_string())
                    .collect();
            }
        }
    }

    if remote_config.backend == RemoteBackendKind::LocalFixture {
        match gca_oci::rebuild_merged_tantivy_index(&home, remote_config.tenant.as_deref()) {
            Ok(merged) => report.merged_index_path = merged.merged_index_path,
            Err(error) => {
                report.failed.push(SyncFailure {
                    repo_id: "merged-index".to_string(),
                    error,
                });
            }
        }
    }
    Ok(report)
}

fn print_sync_report(report: &SyncReport, format: OutputFormat) {
    match format {
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("sync report should serialize")
        ),
        OutputFormat::Markdown => {
            println!("# Sync");
            println!();
            if let Some(channel) = &report.channel {
                println!("- Channel: `{channel}`");
            }
            println!("- Merged index: `{}`", report.merged_index_path.display());
            if report.downloaded.is_empty() {
                println!("- No repo packages were synced.");
            } else {
                for repo in &report.downloaded {
                    println!("- Downloaded: `{repo}`");
                }
            }
            for repo in &report.skipped {
                println!("- Skipped: `{repo}`");
            }
            for failure in &report.failed {
                println!("- Failed `{}`: {}", failure.repo_id, failure.error);
            }
        }
    }
}

fn run_init(channel: &str, format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let home = home_dir();
    let paths = vec![
        default_remote_store_path(&home),
        default_sync_cache_path(&home),
        default_indexes_path(&home),
        merged_tantivy_path(&home),
        home.join(".greentic-agent").join("catalogs"),
        home.join(".greentic-agent").join("notifications"),
    ];
    for path in &paths {
        if let Err(error) = fs::create_dir_all(path) {
            eprintln!("failed to create {}: {error}", path.display());
            return 1;
        }
    }
    let config_path = home.join(".greentic-agent").join("config.toml");
    let public_catalog = format!("ghcr.io/greenticai/indexes/catalog:{channel}");
    if !config_path.exists() {
        let raw = format!(
            "default_channel = \"{}\"\npublic_catalog = \"{}\"\n",
            channel, public_catalog
        );
        if let Err(error) = fs::write(&config_path, raw) {
            eprintln!("failed to write {}: {error}", config_path.display());
            return 1;
        }
    }
    let response = serde_json::json!({
        "home": home.join(".greentic-agent"),
        "config": config_path,
        "default_channel": channel,
        "public_catalog": public_catalog,
        "paths": paths,
        "compatibility": "Using ~/.greentic-agent for this compatibility period; ~/.greentic/coding-agent migration is deferred."
    });
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&response).expect("init response should serialize")
            );
        }
        OutputFormat::Markdown => {
            println!("# Init");
            println!();
            println!("- Home: `{}`", home.join(".greentic-agent").display());
            println!("- Config: `{}`", config_path.display());
            println!("- Default channel: `{channel}`");
        }
    }
    0
}

fn run_status(channel: Option<&str>, format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let home = home_dir();
    let state = gca_oci::load_sync_state(&home).unwrap_or_else(gca_oci::empty_sync_state);
    let repos = state
        .repos
        .iter()
        .filter(|entry| {
            channel.is_none_or(|channel| {
                entry.channel.as_deref() == Some(channel)
                    || entry.branch.as_deref() == Some(channel)
            })
        })
        .map(|entry| {
            serde_json::json!({
                "repo_id": entry.repo_id,
                "channel": entry.channel,
                "branch": entry.branch,
                "commit_sha": entry.source_commit,
                "indexed_at": entry.indexed_at,
                "fresh": entry.local_index_path.join("repo-index.json").exists(),
                "package_ref": entry.package_ref,
                "local_index_path": entry.local_index_path,
            })
        })
        .collect::<Vec<_>>();
    let repo_count = repos.len();
    let selected_channel = channel.unwrap_or("all");
    let catalog = channel
        .map(|channel| format!("ghcr.io/greenticai/indexes/catalog:{channel}"))
        .unwrap_or_else(|| DEFAULT_PUBLIC_CATALOG_REF.to_string());
    let response = serde_json::json!({
        "channel": selected_channel,
        "catalog": catalog,
        "sync_state_path": sync_state_path(&home),
        "merged_index_path": merged_tantivy_path(&home),
        "repos": repos,
    });
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&response).expect("status response should serialize")
            );
        }
        OutputFormat::Markdown => {
            println!("# Status");
            println!();
            println!("- Channel: `{selected_channel}`");
            println!("- Catalog: `{catalog}`");
            println!("- Repos: `{repo_count}`");
        }
    }
    0
}

fn run_list_remote_repos(format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    match engine_service().and_then(|service| {
        service
            .list_remote_repos(gca_engine::ListRemoteReposOptions::default())
            .map_err(|error| error.to_string())
    }) {
        Ok(response) => {
            match format {
                OutputFormat::Json => println!(
                    "{}",
                    serde_json::to_string_pretty(&response.repos)
                        .expect("remote repos should serialize")
                ),
                OutputFormat::Markdown => {
                    println!("# Remote Repos");
                    println!();
                    if response.repos.is_empty() {
                        println!("- No remote repo packages found.");
                    } else {
                        for repo in &response.repos {
                            println!("- `{}`: {}", repo.repo_id, repo.tags.join(", "));
                        }
                    }
                }
            }
            0
        }
        Err(error) => {
            eprintln!("failed to list remote repos: {error}");
            1
        }
    }
}

fn run_show_catalog(format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    match engine_service().and_then(|service| {
        service
            .show_catalog(gca_engine::ShowCatalogOptions::default())
            .map_err(|error| error.to_string())
    }) {
        Ok(response) => {
            match format {
                OutputFormat::Json => println!(
                    "{}",
                    serde_json::to_string_pretty(&response.catalog)
                        .expect("catalog should serialize")
                ),
                OutputFormat::Markdown => {
                    println!("# Catalog");
                    println!();
                    println!("- Version: `{}`", response.catalog.version);
                    println!("- Generated at: `{}`", response.catalog.generated_at);
                    if response.catalog.repos.is_empty() {
                        println!("- No published repo indexes discovered.");
                    } else {
                        for repo in &response.catalog.repos {
                            println!(
                                "- `{}` (`{}`) latest `{}`",
                                repo.repo_id,
                                serde_json::to_value(repo.repo_role)
                                    .ok()
                                    .and_then(|value| value.as_str().map(ToString::to_string))
                                    .unwrap_or_else(|| format!("{:?}", repo.repo_role)),
                                repo.latest_tag
                            );
                        }
                    }
                }
            }
            0
        }
        Err(error) => {
            eprintln!("failed to build catalog: {error}");
            1
        }
    }
}

fn run_catalog_command(command: CatalogCommands) -> i32 {
    match command {
        CatalogCommands::Show { tenant, format } => run_catalog_show(tenant.as_deref(), &format),
        CatalogCommands::AddRepo {
            repo,
            index_uri,
            tenant,
            reason,
            publish,
            backend,
            token,
            token_env,
            format,
        } => run_catalog_add_repo(CatalogAddOptions {
            repo: &repo,
            index_uri: &index_uri,
            tenant: tenant.as_deref(),
            reason: reason.as_deref(),
            publish,
            backend: &backend,
            token: token.as_deref(),
            token_env: token_env.as_deref(),
            format: &format,
        }),
        CatalogCommands::RemoveRepo {
            repo,
            tenant,
            reason,
            format,
        } => run_catalog_mutate_repo(
            CatalogAction::RemoveRepo,
            &repo,
            tenant.as_deref(),
            reason.as_deref(),
            &format,
        ),
        CatalogCommands::EnableRepo {
            repo,
            tenant,
            reason,
            format,
        } => run_catalog_mutate_repo(
            CatalogAction::EnableRepo,
            &repo,
            tenant.as_deref(),
            reason.as_deref(),
            &format,
        ),
        CatalogCommands::DisableRepo {
            repo,
            tenant,
            reason,
            format,
        } => run_catalog_mutate_repo(
            CatalogAction::DisableRepo,
            &repo,
            tenant.as_deref(),
            reason.as_deref(),
            &format,
        ),
        CatalogCommands::Validate { tenant, format } => {
            run_catalog_validate(tenant.as_deref(), &format)
        }
        CatalogCommands::Publish {
            tenant,
            channel,
            expected_digest,
            backend,
            token,
            token_env,
            format,
        } => run_catalog_publish(
            tenant.as_deref(),
            channel.as_deref(),
            expected_digest.as_deref(),
            &backend,
            token.as_deref(),
            token_env.as_deref(),
            &format,
        ),
        CatalogCommands::RebuildFromGhcr {
            org,
            channel,
            tenant,
            format,
        } => run_catalog_rebuild_from_ghcr(&org, &channel, tenant.as_deref(), &format),
    }
}

fn run_org_command(command: OrgCommands) -> i32 {
    match command {
        OrgCommands::PlanIndexRollout {
            org,
            repo_source,
            repo_list_file,
            include_dot_github,
            format,
        } => run_org_plan_index_rollout(
            &org,
            &repo_source,
            repo_list_file.as_deref(),
            include_dot_github,
            &format,
        ),
        OrgCommands::ApplyIndexRollout {
            plan,
            open_prs,
            dry_run,
            format,
        } => run_org_apply_index_rollout(&plan, open_prs, dry_run, &format),
    }
}

fn run_org_plan_index_rollout(
    org: &str,
    repo_source: &str,
    repo_list_file: Option<&Path>,
    include_dot_github: bool,
    format: &str,
) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let repo_list = match repo_list_file {
        Some(path) => match load_rollout_repo_list(path) {
            Ok(repos) => Some(repos),
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        },
        None if repo_source == "repo-list-file" => {
            eprintln!("--repo-list-file is required when --repo-source repo-list-file is used");
            return 2;
        }
        None => None,
    };
    let include_dot_github = include_dot_github
        || repo_source == "greenticai/.github"
        || repo_source.ends_with("/.github");
    let plan = if let Some(repo_list) = repo_list {
        let client = RepoListRolloutClient {
            files: repo_list.files,
        };
        gca_engine::plan_index_rollout_for_repos(
            &client,
            org,
            repo_list.repos,
            include_dot_github,
            None,
        )
    } else {
        let client = GhCliRepoClient;
        gca_engine::plan_index_rollout(&client, org, None, include_dot_github, None)
    };
    match plan {
        Ok(plan) => {
            match format {
                OutputFormat::Json => println!(
                    "{}",
                    serde_json::to_string_pretty(&plan).expect("rollout plan should serialize")
                ),
                OutputFormat::Markdown => print_rollout_plan_markdown(&plan),
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run_org_apply_index_rollout(
    plan_path: &Path,
    open_prs: bool,
    dry_run: bool,
    format: &str,
) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let plan = match fs::read_to_string(plan_path)
        .map_err(|error| format!("failed to read {}: {error}", plan_path.display()))
        .and_then(|raw| {
            serde_json::from_str::<gca_engine::IndexRolloutPlan>(&raw)
                .map_err(|error| format!("failed to parse {}: {error}", plan_path.display()))
        }) {
        Ok(plan) => plan,
        Err(error) => {
            eprintln!("{error}");
            return 1;
        }
    };
    if let Err(error) = gca_engine::validate_rollout_plan(&plan) {
        eprintln!("{error}");
        return 1;
    }
    let client = GhCliRepoClient;
    match gca_engine::apply_index_rollout(&client, &plan, dry_run, open_prs) {
        Ok(report) => {
            match format {
                OutputFormat::Json => println!(
                    "{}",
                    serde_json::to_string_pretty(&report)
                        .expect("rollout apply report should serialize")
                ),
                OutputFormat::Markdown => print_rollout_apply_markdown(&report),
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

struct RolloutRepoList {
    repos: Vec<gca_engine::RepoMetadata>,
    files: BTreeMap<String, Option<String>>,
}

#[derive(Deserialize)]
struct RolloutRepoListEntry {
    repo_id: String,
    default_branch: String,
    #[serde(default)]
    archived: bool,
    #[serde(default, alias = "current_workflow")]
    workflow_content: Option<String>,
}

fn load_rollout_repo_list(path: &Path) -> Result<RolloutRepoList, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    if let Ok(repos) = serde_json::from_str::<Vec<RolloutRepoListEntry>>(&raw) {
        return Ok(rollout_repo_list_from_entries(repos));
    }
    #[derive(Deserialize)]
    struct RepoList {
        repos: Vec<RolloutRepoListEntry>,
    }
    serde_json::from_str::<RepoList>(&raw)
        .map(|list| rollout_repo_list_from_entries(list.repos))
        .map_err(|error| format!("failed to parse {} as repo list: {error}", path.display()))
}

fn rollout_repo_list_from_entries(entries: Vec<RolloutRepoListEntry>) -> RolloutRepoList {
    let mut files = BTreeMap::new();
    let mut repos = Vec::with_capacity(entries.len());
    for entry in entries {
        files.insert(entry.repo_id.clone(), entry.workflow_content.clone());
        repos.push(gca_engine::RepoMetadata {
            repo_id: entry.repo_id,
            default_branch: entry.default_branch,
            archived: entry.archived,
        });
    }
    RolloutRepoList { repos, files }
}

fn print_rollout_plan_markdown(plan: &gca_engine::IndexRolloutPlan) {
    println!("# Index Rollout Plan");
    println!();
    println!("- Org: `{}`", plan.org);
    println!("- Generated: {}", plan.generated_at);
    println!("- Repositories: {}", plan.repos.len());
    for repo in &plan.repos {
        println!("- `{}`: {:?} - {}", repo.repo_id, repo.action, repo.reason);
    }
}

fn print_rollout_apply_markdown(report: &gca_engine::ApplyIndexRolloutReport) {
    println!("# Index Rollout Apply");
    println!();
    println!("- Dry run: {}", report.dry_run);
    println!("- Open PRs: {}", report.open_prs);
    for result in &report.results {
        println!(
            "- `{}`: {:?} - {}",
            result.repo_id, result.status, result.message
        );
        if let Some(url) = &result.pr_url {
            println!("  - PR: {url}");
        }
    }
}

struct GhCliRepoClient;

struct RepoListRolloutClient {
    files: BTreeMap<String, Option<String>>,
}

impl gca_engine::GitHubRepoClient for RepoListRolloutClient {
    fn list_repos(&self, _org: &str) -> Result<Vec<gca_engine::RepoMetadata>, String> {
        Ok(Vec::new())
    }

    fn fetch_file(
        &self,
        repo_id: &str,
        _path: &str,
        _ref_name: &str,
    ) -> Result<Option<String>, String> {
        Ok(self.files.get(repo_id).cloned().flatten())
    }

    fn create_branch(&self, _repo_id: &str, _branch: &str, _base: &str) -> Result<(), String> {
        Err("repo-list-file client cannot apply rollout changes".to_string())
    }

    fn upsert_file(
        &self,
        _repo_id: &str,
        _branch: &str,
        _path: &str,
        _content: &str,
        _message: &str,
    ) -> Result<(), String> {
        Err("repo-list-file client cannot apply rollout changes".to_string())
    }

    fn open_pr(
        &self,
        _repo_id: &str,
        _branch: &str,
        _base: &str,
        _title: &str,
        _body: &str,
    ) -> Result<String, String> {
        Err("repo-list-file client cannot apply rollout changes".to_string())
    }
}

impl gca_engine::GitHubRepoClient for GhCliRepoClient {
    fn list_repos(&self, org: &str) -> Result<Vec<gca_engine::RepoMetadata>, String> {
        #[derive(Deserialize)]
        struct GhRepo {
            full_name: String,
            default_branch: String,
            archived: bool,
        }

        let pages: Vec<Vec<GhRepo>> =
            gh_api_json(&["api", &format!("orgs/{org}/repos"), "--paginate", "--slurp"])?;
        let repos = pages.into_iter().flatten();
        Ok(repos
            .map(|repo| gca_engine::RepoMetadata {
                repo_id: repo.full_name,
                default_branch: repo.default_branch,
                archived: repo.archived,
            })
            .collect())
    }

    fn fetch_file(
        &self,
        repo_id: &str,
        path: &str,
        ref_name: &str,
    ) -> Result<Option<String>, String> {
        let endpoint = format!("repos/{repo_id}/contents/{path}?ref={ref_name}");
        let output = ProcessCommand::new("gh")
            .args(["api", &endpoint, "-H", "Accept: application/vnd.github.raw"])
            .output()
            .map_err(|error| format!("failed to run gh api: {error}"))?;
        if output.status.success() {
            String::from_utf8(output.stdout)
                .map(Some)
                .map_err(|error| format!("gh returned non-utf8 file content: {error}"))
        } else if String::from_utf8_lossy(&output.stderr).contains("404") {
            Ok(None)
        } else {
            Err(format!(
                "gh api failed for {repo_id}/{path}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ))
        }
    }

    fn create_branch(&self, repo_id: &str, branch: &str, base: &str) -> Result<(), String> {
        #[derive(Deserialize)]
        struct GitRef {
            object: GitObject,
        }
        #[derive(Deserialize)]
        struct GitObject {
            sha: String,
        }

        let base_ref: GitRef =
            gh_api_json(&["api", &format!("repos/{repo_id}/git/ref/heads/{base}")])?;
        let output = ProcessCommand::new("gh")
            .args([
                "api",
                &format!("repos/{repo_id}/git/refs"),
                "--method",
                "POST",
                "-f",
                &format!("ref=refs/heads/{branch}"),
                "-f",
                &format!("sha={}", base_ref.object.sha),
            ])
            .output()
            .map_err(|error| format!("failed to run gh api: {error}"))?;
        if output.status.success()
            || String::from_utf8_lossy(&output.stderr).contains("Reference already exists")
        {
            Ok(())
        } else {
            Err(format!(
                "failed to create branch `{branch}` in `{repo_id}`: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ))
        }
    }

    fn upsert_file(
        &self,
        repo_id: &str,
        branch: &str,
        path: &str,
        content: &str,
        message: &str,
    ) -> Result<(), String> {
        let sha = fetch_github_file_sha(repo_id, path, branch)?;
        let encoded = base64_encode(content.as_bytes());
        let mut args = vec![
            "api".to_string(),
            format!("repos/{repo_id}/contents/{path}"),
            "--method".to_string(),
            "PUT".to_string(),
            "-f".to_string(),
            format!("message={message}"),
            "-f".to_string(),
            format!("branch={branch}"),
            "-f".to_string(),
            format!("content={encoded}"),
        ];
        if let Some(sha) = sha {
            args.push("-f".to_string());
            args.push(format!("sha={sha}"));
        }
        let output = ProcessCommand::new("gh")
            .args(args.iter().map(String::as_str))
            .output()
            .map_err(|error| format!("failed to run gh api: {error}"))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(format!(
                "failed to upsert `{path}` in `{repo_id}`: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ))
        }
    }

    fn open_pr(
        &self,
        repo_id: &str,
        branch: &str,
        base: &str,
        title: &str,
        body: &str,
    ) -> Result<String, String> {
        let output = ProcessCommand::new("gh")
            .args([
                "pr", "create", "--repo", repo_id, "--head", branch, "--base", base, "--title",
                title, "--body", body,
            ])
            .output()
            .map_err(|error| format!("failed to run gh pr create: {error}"))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(format!(
                "failed to open PR for `{repo_id}`: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ))
        }
    }
}

fn gh_api_json<T: for<'de> Deserialize<'de>>(args: &[&str]) -> Result<T, String> {
    let output = ProcessCommand::new("gh")
        .args(args)
        .output()
        .map_err(|error| format!("failed to run gh api: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "gh api failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("failed to parse gh api JSON output: {error}"))
}

fn fetch_github_file_sha(
    repo_id: &str,
    path: &str,
    ref_name: &str,
) -> Result<Option<String>, String> {
    #[derive(Deserialize)]
    struct GhContent {
        sha: String,
    }

    let endpoint = format!("repos/{repo_id}/contents/{path}?ref={ref_name}");
    let output = ProcessCommand::new("gh")
        .args(["api", &endpoint])
        .output()
        .map_err(|error| format!("failed to run gh api: {error}"))?;
    if output.status.success() {
        let content: GhContent = serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("failed to parse gh file metadata: {error}"))?;
        Ok(Some(content.sha))
    } else if String::from_utf8_lossy(&output.stderr).contains("404") {
        Ok(None)
    } else {
        Err(format!(
            "failed to read `{path}` metadata in `{repo_id}`: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = *chunk.get(1).unwrap_or(&0);
        let third = *chunk.get(2).unwrap_or(&0);
        let triple = ((first as u32) << 16) | ((second as u32) << 8) | third as u32;
        output.push(ALPHABET[((triple >> 18) & 0x3f) as usize] as char);
        output.push(ALPHABET[((triple >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            output.push(ALPHABET[((triple >> 6) & 0x3f) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(ALPHABET[(triple & 0x3f) as usize] as char);
        } else {
            output.push('=');
        }
    }
    output
}

fn run_rebuild_merged_index(tenant: Option<&str>, format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    match engine_service().and_then(|service| {
        service
            .rebuild_merged_index(gca_engine::RebuildMergedIndexOptions {
                tenant: tenant.map(ToString::to_string),
            })
            .map_err(|error| error.to_string())
    }) {
        Ok(report) => {
            match format {
                OutputFormat::Json => println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "index_path": report.index_path,
                        "merged_index_path": report.index_path,
                        "repos_indexed": report.repos_indexed,
                        "documents_indexed": report.documents_indexed,
                    }))
                    .expect("merged index report should serialize")
                ),
                OutputFormat::Markdown => {
                    println!("# Rebuild Merged Index");
                    println!();
                    println!("- Repos indexed: `{}`", report.repos_indexed);
                    println!("- Documents indexed: `{}`", report.documents_indexed);
                    println!("- Path: `{}`", report.index_path.display());
                }
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run_watch_indexes(options: WatchOptions<'_>, format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    loop {
        match run_watch_tick(options, None) {
            Ok(event) => print_watch_event(&event, format),
            Err(error) => {
                let event = WatchEvent {
                    watch_enabled: true,
                    last_sync_at: Some(timestamp_string()),
                    last_sync_status: "failed".to_string(),
                    changed: false,
                    notifications: Vec::new(),
                    report: None,
                    error: Some(error),
                };
                print_watch_event(&event, format);
                if options.strict_sync || options.once {
                    return 1;
                }
            }
        }
        if options.once {
            return 0;
        }
        thread::sleep(Duration::from_secs(options.sync_interval_seconds.max(1)));
    }
}

#[allow(clippy::too_many_arguments)]
fn run_watch_alias(
    channel: Option<&str>,
    poll: &str,
    tenant: Option<&str>,
    token: Option<&str>,
    token_env: Option<&str>,
    strict_sync: bool,
    prune_disabled: bool,
    once: bool,
    format: &str,
) -> i32 {
    let sync_interval_seconds = match parse_poll_interval_seconds(poll) {
        Ok(seconds) => seconds,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    run_watch_indexes(
        WatchOptions {
            channel,
            tenant,
            token,
            token_env,
            sync_interval_seconds,
            strict_sync,
            prune_disabled,
            once,
        },
        format,
    )
}

fn start_watch_thread(
    config: ServerConfig,
    _home: PathBuf,
    watch_status: Arc<RwLock<WatchStatus>>,
) {
    thread::spawn(move || {
        let options = WatchOptions {
            channel: None,
            tenant: config.tenant.as_deref(),
            token: config.token.as_deref(),
            token_env: None,
            sync_interval_seconds: config.sync_interval_seconds,
            strict_sync: config.strict_sync,
            prune_disabled: config.prune_disabled,
            once: false,
        };
        loop {
            if let Err(error) = run_watch_tick(options, Some(&watch_status))
                && config.strict_sync
            {
                eprintln!("watch sync failed: {error}");
            }
            thread::sleep(Duration::from_secs(config.sync_interval_seconds.max(1)));
        }
    });
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct WatchEvent {
    watch_enabled: bool,
    last_sync_at: Option<String>,
    last_sync_status: String,
    changed: bool,
    notifications: Vec<NotificationItem>,
    report: Option<SyncReport>,
    error: Option<String>,
}

fn run_watch_tick(
    options: WatchOptions<'_>,
    status: Option<&Arc<RwLock<WatchStatus>>>,
) -> Result<WatchEvent, String> {
    let home = home_dir();
    let before = catalog_fingerprint(&home);
    let before_state = gca_oci::load_sync_state(&home);
    let sync_options = SyncOptions {
        repo: None,
        tag: None,
        channel: options.channel,
        catalog: None,
        tenant: options.tenant,
        tenant_catalog: None,
        token: options.token,
        token_env: options.token_env,
        backend: "local",
        strict: options.strict_sync,
        public_only: false,
        private_only: false,
        include_private: options.tenant.is_some(),
        prune_disabled: options.prune_disabled,
    };
    let report = execute_sync(sync_options)?;
    let after = catalog_fingerprint(&home);
    let changed = before != after || !report.downloaded.is_empty();
    let after_state = gca_oci::load_sync_state(&home).unwrap_or_else(gca_oci::empty_sync_state);
    let notifications = append_notification_feed_items(
        &home,
        before_state.as_ref(),
        &after_state,
        options.channel,
    )?;
    let event = WatchEvent {
        watch_enabled: true,
        last_sync_at: Some(timestamp_string()),
        last_sync_status: if report.failed.is_empty() {
            "ok".to_string()
        } else if options.strict_sync {
            "failed".to_string()
        } else {
            "warning".to_string()
        },
        changed,
        notifications,
        report: Some(report),
        error: None,
    };
    if let Some(status) = status {
        update_watch_status(status, &event);
    }
    Ok(event)
}

fn update_watch_status(status: &Arc<RwLock<WatchStatus>>, event: &WatchEvent) {
    if let Ok(mut status) = status.write() {
        status.watch_enabled = event.watch_enabled;
        status.last_sync_at = event.last_sync_at.clone();
        status.last_sync_status = event.last_sync_status.clone();
        status.last_error = event.error.clone();
        status.indexed_repos = event
            .report
            .as_ref()
            .map(|report| report.downloaded.len() + report.skipped.len())
            .unwrap_or(status.indexed_repos);
    }
}

fn print_watch_event(event: &WatchEvent, format: OutputFormat) {
    match format {
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(event).expect("watch event should serialize")
        ),
        OutputFormat::Markdown => {
            println!("# Watch Indexes");
            println!();
            println!("- Status: `{}`", event.last_sync_status);
            println!("- Changed: `{}`", event.changed);
            if let Some(last_sync_at) = &event.last_sync_at {
                println!("- Last sync: `{last_sync_at}`");
            }
            if !event.notifications.is_empty() {
                println!("- Notifications: `{}`", event.notifications.len());
            }
            if let Some(error) = &event.error {
                println!("- Error: {error}");
            }
        }
    }
}

fn parse_poll_interval_seconds(value: &str) -> Result<u64, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("poll interval cannot be empty".to_string());
    }
    let (number, multiplier) = match trimmed.chars().last().unwrap_or_default() {
        's' | 'S' => (&trimmed[..trimmed.len() - 1], 1),
        'm' | 'M' => (&trimmed[..trimmed.len() - 1], 60),
        'h' | 'H' => (&trimmed[..trimmed.len() - 1], 60 * 60),
        _ => (trimmed, 1),
    };
    let amount = number
        .parse::<u64>()
        .map_err(|_| format!("unsupported poll interval: {value}"))?;
    Ok(amount.saturating_mul(multiplier).max(1))
}

fn notification_dir(home: &Path) -> PathBuf {
    home.join(".greentic-agent").join("notifications")
}

fn notification_feed_path(home: &Path) -> PathBuf {
    notification_dir(home).join("feed.json")
}

fn notification_seen_path(home: &Path) -> PathBuf {
    notification_dir(home).join("seen.json")
}

fn empty_notification_feed() -> NotificationFeed {
    NotificationFeed {
        schema_version: "gca.notifications.v1".to_string(),
        items: Vec::new(),
    }
}

fn empty_notification_seen_state() -> NotificationSeenState {
    NotificationSeenState {
        schema_version: "gca.notifications.v1".to_string(),
        seen: BTreeMap::new(),
    }
}

fn load_notification_feed(home: &Path) -> NotificationFeed {
    let path = notification_feed_path(home);
    let Ok(raw) = fs::read_to_string(path) else {
        return empty_notification_feed();
    };
    serde_json::from_str(&raw).unwrap_or_else(|_| empty_notification_feed())
}

fn write_notification_feed(home: &Path, feed: &NotificationFeed) -> Result<(), String> {
    let path = notification_feed_path(home);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let raw =
        serde_json::to_string_pretty(feed).expect("notification feed should serialize as json");
    fs::write(&path, format!("{raw}\n"))
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn load_notification_seen_state(home: &Path) -> NotificationSeenState {
    let path = notification_seen_path(home);
    let Ok(raw) = fs::read_to_string(path) else {
        return empty_notification_seen_state();
    };
    serde_json::from_str(&raw).unwrap_or_else(|_| empty_notification_seen_state())
}

fn write_notification_seen_state(home: &Path, state: &NotificationSeenState) -> Result<(), String> {
    let path = notification_seen_path(home);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let raw =
        serde_json::to_string_pretty(state).expect("notification seen state should serialize");
    fs::write(&path, format!("{raw}\n"))
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn append_notification_feed_items(
    home: &Path,
    before: Option<&gca_oci::SyncState>,
    after: &gca_oci::SyncState,
    channel: Option<&str>,
) -> Result<Vec<NotificationItem>, String> {
    let mut feed = load_notification_feed(home);
    let mut existing_ids = feed
        .items
        .iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    let before_by_key = before
        .map(|state| {
            state
                .repos
                .iter()
                .map(|entry| (notification_state_key(entry), entry))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let created_at = timestamp_string();
    let mut added = Vec::new();
    for entry in &after.repos {
        if let Some(channel) = channel
            && !notification_entry_matches_channel(entry, channel)
        {
            continue;
        }
        let old = before_by_key.get(&notification_state_key(entry)).copied();
        let old_marker = old.and_then(notification_commit_marker);
        let new_marker = notification_commit_marker(entry);
        if old.is_some() && old_marker == new_marker {
            continue;
        }
        let branch = notification_entry_branch(entry);
        let id_marker = new_marker.clone().unwrap_or_else(|| {
            entry
                .digest
                .clone()
                .unwrap_or_else(|| entry.downloaded_at.clone())
        });
        let id = format!("{}/{}/{}", entry.repo_id, branch, id_marker);
        if existing_ids.iter().any(|existing| existing == &id) {
            continue;
        }
        let repo_name = entry.repo_id.rsplit('/').next().unwrap_or(&entry.repo_id);
        let item = NotificationItem {
            id: id.clone(),
            repo_id: entry.repo_id.clone(),
            branch: branch.clone(),
            item_type: "index_updated".to_string(),
            title: format!("{repo_name} {branch} index updated"),
            old_commit: old_marker,
            new_commit: new_marker,
            created_at: created_at.clone(),
            agent_impact: format!(
                "Review updated guidance for `{}` on `{}` before editing.",
                entry.repo_id, branch
            ),
        };
        existing_ids.push(id);
        feed.items.push(item.clone());
        added.push(item);
    }
    feed.items.sort_by(|left, right| {
        right
            .created_at
            .cmp(&left.created_at)
            .then(left.repo_id.cmp(&right.repo_id))
            .then(left.id.cmp(&right.id))
    });
    write_notification_feed(home, &feed)?;
    Ok(added)
}

fn notification_state_key(entry: &gca_oci::SyncedRepoState) -> String {
    format!(
        "{}|{}|{}",
        entry.repo_id,
        entry.tenant.as_deref().unwrap_or("public"),
        notification_entry_branch(entry)
    )
}

fn notification_entry_branch(entry: &gca_oci::SyncedRepoState) -> String {
    entry
        .branch
        .clone()
        .or_else(|| entry.channel.clone())
        .or_else(|| {
            entry
                .package_ref
                .rsplit_once(':')
                .map(|(_, tag)| tag.to_string())
        })
        .unwrap_or_else(|| "latest".to_string())
}

fn notification_entry_matches_channel(entry: &gca_oci::SyncedRepoState, channel: &str) -> bool {
    entry.channel.as_deref() == Some(channel)
        || entry.branch.as_deref() == Some(channel)
        || entry.package_ref.ends_with(&format!(":{channel}"))
}

fn notification_commit_marker(entry: &gca_oci::SyncedRepoState) -> Option<String> {
    entry
        .source_commit
        .clone()
        .or_else(|| entry.digest.clone())
        .or_else(|| entry.indexed_at.clone())
}

#[derive(Clone)]
struct QueryService {
    config: ServerConfig,
    home: PathBuf,
    repo_index: Option<RepoIndex>,
    refresh: Option<RefreshCheck>,
    watch_status: Arc<RwLock<WatchStatus>>,
}

impl QueryService {
    fn load(config: ServerConfig) -> Self {
        let home = home_dir();
        let repo_root = repo_root_from_cwd().ok();
        let refresh = repo_root.as_ref().and_then(|root| check_refresh(root).ok());
        let repo_index = load_or_analyze_repo_index().ok();
        let watch_status = Arc::new(RwLock::new(WatchStatus::disabled(config.tenant.clone())));
        if config.watch {
            start_watch_thread(config.clone(), home.clone(), Arc::clone(&watch_status));
        }
        Self {
            config,
            home,
            repo_index,
            refresh,
            watch_status,
        }
    }

    fn snapshot(&self) -> McpServerSnapshot {
        mcp_server_snapshot(self.refresh.as_ref())
    }

    fn status(&self) -> serde_json::Value {
        let state = load_sync_state(&self.home).unwrap_or_else(empty_sync_state);
        let watch_status = self
            .watch_status
            .read()
            .map(|status| status.clone())
            .unwrap_or_else(|_| WatchStatus::disabled(self.config.tenant.clone()));
        serde_json::json!({
            "ok": true,
            "config": self.config.status_json(),
            "watch_enabled": watch_status.watch_enabled,
            "last_sync_at": watch_status.last_sync_at,
            "last_sync_status": watch_status.last_sync_status,
            "last_error": watch_status.last_error,
            "has_local_repo": self.repo_index.is_some(),
            "freshness_warning": freshness_warning(self.refresh.as_ref()),
            "indexed_repos": state.repos.len(),
            "tenant": self.config.tenant,
            "merged_index_path": merged_tantivy_path(&self.home),
            "merged_index_ready": merged_tantivy_path(&self.home).exists(),
        })
    }

    fn search(
        &self,
        mode: SearchMode,
        query: &str,
        scope: SearchScope,
        repo: Option<&str>,
        tenant: Option<&str>,
    ) -> Result<SearchResponse, String> {
        let mut combined = SearchResponse {
            mode,
            query: query.trim().to_string(),
            results: Vec::new(),
        };
        if matches!(scope, SearchScope::Local | SearchScope::All)
            && let Some(repo_index) = &self.repo_index
        {
            combined
                .results
                .extend(search_repo_index(repo_index, mode, query).results);
        }
        if matches!(scope, SearchScope::Merged | SearchScope::All) {
            let merged_dir = merged_tantivy_path(&self.home);
            if merged_dir.exists() {
                combined.results.extend(
                    search_tantivy_index_filtered(&merged_dir, mode, query, repo, tenant)?.results,
                );
            } else if scope == SearchScope::Merged {
                return Err(format!(
                    "merged Tantivy index not found at {}",
                    merged_dir.display()
                ));
            }
        }
        combined.results.sort_by(|left, right| {
            left.repo_id
                .cmp(&right.repo_id)
                .then(left.id.cmp(&right.id))
        });
        combined.results.truncate(20);
        Ok(combined)
    }

    fn dispatch_mcp_request(&self, request: McpRequest) -> McpResponse {
        dispatch_mcp_request(self, request)
    }

    fn indexed_repos(&self) -> serde_json::Value {
        let state = load_sync_state(&self.home).unwrap_or_else(empty_sync_state);
        serde_json::to_value(state.repos).expect("sync state repos should serialize")
    }

    fn catalog(&self) -> serde_json::Value {
        serde_json::to_value(
            load_editable_catalog(self.config.tenant.as_deref()).unwrap_or(Catalog {
                version: SCHEMA_VERSION_V1.to_string(),
                generated_at: timestamp_string(),
                catalog_id: None,
                default_channel: None,
                repos: Vec::new(),
                change_log: Vec::new(),
            }),
        )
        .expect("catalog should serialize")
    }

    fn sync_indexes(&self) -> Result<serde_json::Value, String> {
        let event = run_watch_tick(
            WatchOptions {
                channel: None,
                tenant: self.config.tenant.as_deref(),
                token: self.config.token.as_deref(),
                token_env: None,
                sync_interval_seconds: self.config.sync_interval_seconds,
                strict_sync: self.config.strict_sync,
                prune_disabled: self.config.prune_disabled,
                once: true,
            },
            Some(&self.watch_status),
        )?;
        serde_json::to_value(event).map_err(|error| error.to_string())
    }
}

fn run_mcp_stdio(service: QueryService) -> i32 {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            break;
        };
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<McpRequest>(&line) {
            Ok(request) => service.dispatch_mcp_request(request),
            Err(error) => mcp_error(None, &format!("failed to parse MCP request: {error}")),
        };
        let raw = serde_json::to_string(&response).expect("mcp response should serialize");
        if writeln!(stdout, "{raw}").is_err() {
            break;
        }
        let _ = stdout.flush();
    }
    0
}

fn run_http_server(service: QueryService) -> i32 {
    let bind_addr = format!("{}:{}", service.config.host, service.config.port);
    let listener = match TcpListener::bind(&bind_addr) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("failed to bind HTTP server at {bind_addr}: {error}");
            return 1;
        }
    };
    eprintln!("greentic-coding-agent HTTP server listening on {bind_addr}");
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => handle_http_stream(stream, &service),
            Err(error) => eprintln!("failed to accept HTTP connection: {error}"),
        }
    }
    0
}

fn handle_http_stream(mut stream: TcpStream, service: &QueryService) {
    let mut buffer = Vec::new();
    let mut temp = [0_u8; 4096];
    loop {
        match stream.read(&mut temp) {
            Ok(0) => break,
            Ok(n) => {
                buffer.extend_from_slice(&temp[..n]);
                if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
                    let content_length = http_content_length(&buffer);
                    if let Some(header_end) = http_header_end(&buffer)
                        && buffer.len() >= header_end + content_length
                    {
                        break;
                    }
                }
            }
            Err(_) => return,
        }
    }
    let (status, body) = handle_http_request(&buffer, service);
    let status_text = if status == 200 { "OK" } else { "Bad Request" };
    let response = format!(
        "HTTP/1.1 {status} {status_text}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
}

fn handle_http_request(raw: &[u8], service: &QueryService) -> (u16, String) {
    let request = String::from_utf8_lossy(raw);
    let mut lines = request.lines();
    let Some(request_line) = lines.next() else {
        return json_http_error("missing request line");
    };
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or_default();
    let body = http_body(raw);
    let result: Result<serde_json::Value, String> =
        (|| match (method, path) {
            ("GET", "/healthz") => Ok(serde_json::json!({ "ok": true })),
            ("GET", "/readyz") => Ok(serde_json::json!({
                "ok": merged_tantivy_path(&service.home).exists() || service.repo_index.is_some()
            })),
            ("GET", "/status") => Ok(service.status()),
            ("GET", "/catalog") => Ok(service.catalog()),
            ("GET", "/repos") => Ok(service.indexed_repos()),
            ("POST", "/search") => http_search(service, body),
            ("POST", "/describe") => Ok(serde_json::to_value(&service.repo_index)
                .expect("repo description should serialize")),
            ("POST", "/sync") => service.sync_indexes(),
            ("POST", "/impact") => {
                let args = parse_http_json(body)?;
                let repo_index = service
                    .repo_index
                    .as_ref()
                    .ok_or_else(|| "current repo index is not loaded".to_string())?;
                let symbol = args
                    .get("symbol")
                    .or_else(|| args.get("query"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                serde_json::to_value(impact_analysis(
                    repo_index,
                    symbol,
                    service.refresh.as_ref(),
                ))
                .map_err(|error| error.to_string())
            }
            ("POST", "/required-validations") => {
                let args = parse_http_json(body)?;
                let repo_index = service
                    .repo_index
                    .as_ref()
                    .ok_or_else(|| "current repo index is not loaded".to_string())?;
                let task = args
                    .get("task")
                    .or_else(|| args.get("query"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                serde_json::to_value(required_validations(repo_index, task))
                    .map_err(|error| error.to_string())
            }
            _ => Err(format!("unsupported route: {method} {path}")),
        })();
    match result {
        Ok(value) => (
            200,
            serde_json::to_string_pretty(&value).expect("http response should serialize"),
        ),
        Err(error) => json_http_error(&error),
    }
}

fn http_search(service: &QueryService, body: &[u8]) -> Result<serde_json::Value, String> {
    let args = parse_http_json(body)?;
    let query = args
        .get("query")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "missing `query`".to_string())?;
    let mode = args
        .get("mode")
        .and_then(serde_json::Value::as_str)
        .map(SearchMode::parse)
        .transpose()?
        .unwrap_or(SearchMode::Instruction);
    let scope = args
        .get("scope")
        .and_then(serde_json::Value::as_str)
        .map(SearchScope::parse)
        .transpose()?
        .unwrap_or(SearchScope::All);
    let repo = args.get("repo").and_then(serde_json::Value::as_str);
    let tenant = args
        .get("tenant")
        .and_then(serde_json::Value::as_str)
        .or(service.config.tenant.as_deref());
    serde_json::to_value(service.search(mode, query, scope, repo, tenant)?)
        .map_err(|error| error.to_string())
}

fn parse_http_json(body: &[u8]) -> Result<serde_json::Value, String> {
    if body.is_empty() {
        return Ok(serde_json::json!({}));
    }
    serde_json::from_slice(body).map_err(|error| format!("failed to parse JSON body: {error}"))
}

fn json_http_error(message: &str) -> (u16, String) {
    (
        400,
        serde_json::to_string_pretty(&serde_json::json!({
            "ok": false,
            "error": message,
        }))
        .expect("error response should serialize"),
    )
}

fn http_header_end(raw: &[u8]) -> Option<usize> {
    raw.windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
}

fn http_body(raw: &[u8]) -> &[u8] {
    http_header_end(raw)
        .map(|index| &raw[index..])
        .unwrap_or(&[])
}

fn http_content_length(raw: &[u8]) -> usize {
    let header_end = http_header_end(raw).unwrap_or(raw.len());
    let headers = String::from_utf8_lossy(&raw[..header_end]);
    headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0)
}

fn run_catalog_show(tenant: Option<&str>, format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    match load_editable_catalog(tenant) {
        Ok(catalog) => {
            print_catalog(&catalog, format);
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run_catalog_add_repo(options: CatalogAddOptions<'_>) -> i32 {
    let format = match OutputFormat::parse(options.format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let repo_id = normalize_repo_id(options.repo);
    if parse_repo_id(&repo_id).is_none() {
        eprintln!("repo must use org/repo form");
        return 2;
    }

    let mut catalog = match load_editable_catalog(options.tenant) {
        Ok(catalog) => catalog,
        Err(error) => {
            eprintln!("{error}");
            return 1;
        }
    };
    let repo_name = repo_id.rsplit('/').next().unwrap_or(&repo_id).to_string();
    let entry = CatalogRepo {
        repo_id: repo_id.clone(),
        repo_name,
        repo_role: repo_role_for_repo_id(&repo_id),
        latest_tag: tag_from_index_uri(options.index_uri),
        package_ref: options.index_uri.to_string(),
        updated_at: timestamp_string(),
        default_branch: None,
        preferred_branch: None,
        branches: BTreeMap::new(),
        visibility: if options.tenant.is_some() {
            IndexVisibility::Tenant
        } else {
            IndexVisibility::Public
        },
        tenant: options.tenant.map(ToString::to_string),
        required_auth: options.tenant.map(|_| AuthKind::GhcrToken),
        digest: None,
        source_commit: None,
        enabled: true,
    };
    upsert_catalog_repo(&mut catalog, entry, CatalogAction::AddRepo, options.reason);
    if let Err(error) = write_editable_catalog(options.tenant, &catalog) {
        eprintln!("{error}");
        return 1;
    }
    if options.publish {
        let status = run_catalog_publish(
            options.tenant,
            None,
            None,
            options.backend,
            options.token,
            options.token_env,
            options.format,
        );
        if status != 0 {
            return status;
        }
    } else {
        print_catalog(&catalog, format);
    }
    0
}

fn run_catalog_mutate_repo(
    action: CatalogAction,
    repo: &str,
    tenant: Option<&str>,
    reason: Option<&str>,
    format: &str,
) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let repo_id = normalize_repo_id(repo);
    let mut catalog = match load_editable_catalog(tenant) {
        Ok(catalog) => catalog,
        Err(error) => {
            eprintln!("{error}");
            return 1;
        }
    };
    let Some(index) = catalog
        .repos
        .iter()
        .position(|entry| entry.repo_id == repo_id)
    else {
        eprintln!("repo not found in catalog: {repo_id}");
        return 1;
    };

    match action {
        CatalogAction::RemoveRepo => {
            catalog.repos.remove(index);
        }
        CatalogAction::EnableRepo => {
            catalog.repos[index].enabled = true;
        }
        CatalogAction::DisableRepo => {
            catalog.repos[index].enabled = false;
        }
        CatalogAction::AddRepo | CatalogAction::Publish => {}
    }
    catalog_change(&mut catalog, action, &repo_id, tenant, reason);
    normalize_catalog(&mut catalog);
    if let Err(error) = write_editable_catalog(tenant, &catalog) {
        eprintln!("{error}");
        return 1;
    }
    print_catalog(&catalog, format);
    0
}

fn run_catalog_validate(tenant: Option<&str>, format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let catalog = match load_editable_catalog_for_validation(tenant) {
        Ok(catalog) => catalog,
        Err(error) => {
            eprintln!("{error}");
            return 1;
        }
    };
    let warnings = validate_catalog_entries(&catalog);
    match format {
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": warnings.is_empty(),
                "warnings": warnings,
            }))
            .expect("validation result should serialize")
        ),
        OutputFormat::Markdown => {
            println!("# Catalog Validate");
            println!();
            println!("- OK: `{}`", warnings.is_empty());
            for warning in warnings {
                println!("- Warning: {warning}");
            }
        }
    }
    0
}

fn run_catalog_rebuild_from_ghcr(
    org: &str,
    channel: &str,
    tenant: Option<&str>,
    format: &str,
) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let remote_root = default_remote_store_path(&home_dir());
    let mut catalog = match rebuild_catalog_from_remote(&remote_root, org, channel, tenant) {
        Ok(catalog) => catalog,
        Err(error) => {
            eprintln!("{error}");
            return 1;
        }
    };
    catalog_change(
        &mut catalog,
        CatalogAction::AddRepo,
        "catalog",
        tenant,
        Some("rebuilt from published index packages"),
    );
    if let Err(error) = write_editable_catalog(tenant, &catalog) {
        eprintln!("{error}");
        return 1;
    }
    print_catalog(&catalog, format);
    0
}

fn run_catalog_publish(
    tenant: Option<&str>,
    channel: Option<&str>,
    expected_digest: Option<&str>,
    backend: &str,
    token: Option<&str>,
    token_env: Option<&str>,
    format: &str,
) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let mut catalog = match load_editable_catalog(tenant) {
        Ok(catalog) => catalog,
        Err(error) => {
            eprintln!("{error}");
            return 1;
        }
    };
    if let Some(channel) = channel {
        catalog.default_channel = Some(channel.to_string());
    }
    catalog_change(
        &mut catalog,
        CatalogAction::Publish,
        "catalog",
        tenant,
        None,
    );
    normalize_catalog(&mut catalog);
    if let Err(error) = write_editable_catalog(tenant, &catalog) {
        eprintln!("{error}");
        return 1;
    }

    let remote_config = match resolve_remote_config(SyncOptions {
        repo: None,
        tag: None,
        channel,
        catalog: None,
        tenant,
        tenant_catalog: None,
        token,
        token_env,
        backend,
        strict: false,
        public_only: false,
        private_only: false,
        include_private: tenant.is_some(),
        prune_disabled: false,
    }) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };

    let catalog_path = editable_catalog_path(tenant);
    let target = match remote_config.backend {
        RemoteBackendKind::LocalFixture => {
            let target = published_catalog_path(tenant, channel);
            if let Some(expected_digest) = expected_digest
                && target.exists()
            {
                match file_digest_hex(&target) {
                    Ok(actual) if actual == expected_digest => {}
                    Ok(actual) => {
                        eprintln!(
                            "catalog publish conflict: expected digest {expected_digest}, found {actual}"
                        );
                        return 1;
                    }
                    Err(error) => {
                        eprintln!("{error}");
                        return 1;
                    }
                }
            }
            if let Some(parent) = target.parent()
                && let Err(error) = fs::create_dir_all(parent)
            {
                eprintln!("failed to create catalog publish directory: {error}");
                return 1;
            }
            if let Err(error) = fs::copy(&catalog_path, &target) {
                eprintln!("failed to publish catalog: {error}");
                return 1;
            }
            if channel.is_some() {
                let compatibility_target = published_catalog_path(tenant, None);
                if let Some(parent) = compatibility_target.parent()
                    && let Err(error) = fs::create_dir_all(parent)
                {
                    eprintln!("failed to create catalog publish directory: {error}");
                    return 1;
                }
                if let Err(error) = fs::copy(&catalog_path, &compatibility_target) {
                    eprintln!("failed to publish compatibility catalog: {error}");
                    return 1;
                }
            }
            target
        }
        RemoteBackendKind::GhcrOras => {
            let reference = if let Some(tenant) = tenant {
                default_tenant_catalog_ref(tenant)
            } else if let Some(channel) = channel {
                format!("ghcr.io/greenticai/indexes/catalog:{channel}")
            } else {
                DEFAULT_PUBLIC_CATALOG_REF.to_string()
            };
            let parent = catalog_path.parent().unwrap_or_else(|| Path::new("."));
            if let Err(error) = oras_push(&reference, parent, remote_config.auth.as_ref()) {
                eprintln!("{error}");
                return 1;
            }
            PathBuf::from(reference)
        }
    };

    match format {
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "published": target,
                "tenant": tenant,
            }))
            .expect("publish result should serialize")
        ),
        OutputFormat::Markdown => {
            println!("# Catalog Publish");
            println!();
            println!("- Published: `{}`", target.display());
        }
    }
    0
}

fn load_editable_catalog(tenant: Option<&str>) -> Result<Catalog, String> {
    let path = editable_catalog_path(tenant);
    if !path.exists() {
        return Ok(Catalog {
            version: SCHEMA_VERSION_V1.to_string(),
            generated_at: timestamp_string(),
            catalog_id: None,
            default_channel: None,
            repos: Vec::new(),
            change_log: Vec::new(),
        });
    }

    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read catalog {}: {error}", path.display()))?;
    let mut catalog: Catalog = serde_json::from_str(&raw)
        .map_err(|error| format!("failed to parse catalog {}: {error}", path.display()))?;
    normalize_catalog(&mut catalog);
    Ok(catalog)
}

fn load_editable_catalog_for_validation(tenant: Option<&str>) -> Result<Catalog, String> {
    let path = editable_catalog_path(tenant);
    if !path.exists() {
        return load_editable_catalog(tenant);
    }

    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read catalog {}: {error}", path.display()))?;
    serde_json::from_str(&raw)
        .map_err(|error| format!("failed to parse catalog {}: {error}", path.display()))
}

fn write_editable_catalog(tenant: Option<&str>, catalog: &Catalog) -> Result<(), String> {
    let path = editable_catalog_path(tenant);
    let mut catalog = catalog.clone();
    normalize_catalog(&mut catalog);
    catalog.generated_at = timestamp_string();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create catalog directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let raw =
        serde_json::to_string_pretty(&catalog).expect("editable catalog should serialize as json");
    fs::write(&path, format!("{raw}\n"))
        .map_err(|error| format!("failed to write catalog {}: {error}", path.display()))
}

fn print_catalog(catalog: &Catalog, format: OutputFormat) {
    match format {
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(catalog).expect("catalog should serialize")
        ),
        OutputFormat::Markdown => {
            println!("# Catalog");
            println!();
            println!("- Version: `{}`", catalog.version);
            println!("- Generated at: `{}`", catalog.generated_at);
            println!("- Entries: `{}`", catalog.repos.len());
            if catalog.repos.is_empty() {
                println!("- No repo entries configured.");
            } else {
                for repo in &catalog.repos {
                    println!(
                        "- `{}` (`{}`) tag `{}` enabled `{}`",
                        repo.repo_id,
                        repo.repo_role.as_str(),
                        repo.latest_tag,
                        repo.enabled
                    );
                }
            }
        }
    }
}

fn normalize_repo_id(value: &str) -> String {
    let value = value.trim().trim_end_matches(".git");
    for prefix in [
        "git@github.com:",
        "https://github.com/",
        "ssh://git@github.com/",
    ] {
        if let Some(rest) = value.strip_prefix(prefix) {
            return rest.trim_matches('/').to_string();
        }
    }
    value.trim_matches('/').to_string()
}

fn upsert_catalog_repo(
    catalog: &mut Catalog,
    entry: CatalogRepo,
    action: CatalogAction,
    reason: Option<&str>,
) {
    catalog.repos.retain(|repo| repo.repo_id != entry.repo_id);
    let repo_id = entry.repo_id.clone();
    let tenant = entry.tenant.clone();
    catalog.repos.push(entry);
    catalog_change(catalog, action, &repo_id, tenant.as_deref(), reason);
    normalize_catalog(catalog);
}

fn catalog_change(
    catalog: &mut Catalog,
    action: CatalogAction,
    repo_id: &str,
    tenant: Option<&str>,
    reason: Option<&str>,
) {
    catalog.change_log.push(CatalogChange {
        action,
        repo_id: repo_id.to_string(),
        tenant: tenant.map(ToString::to_string),
        at: timestamp_string(),
        by: env::var("USER").ok(),
        reason: reason.map(ToString::to_string),
    });
}

fn normalize_catalog(catalog: &mut Catalog) {
    if catalog.version.is_empty() {
        catalog.version = SCHEMA_VERSION_V1.to_string();
    }
    for repo in &mut catalog.repos {
        apply_legacy_repo_name_fallback(&mut repo.repo_id, &repo.repo_name);
    }
    catalog.repos.sort_by(|left, right| {
        left.repo_id
            .cmp(&right.repo_id)
            .then_with(|| left.latest_tag.cmp(&right.latest_tag))
    });
}

fn validate_catalog_entries(catalog: &Catalog) -> Vec<String> {
    let mut warnings = Vec::new();
    if catalog.version.trim().is_empty() {
        warnings.push("catalog version is empty".to_string());
    }
    let mut seen = Vec::new();
    for repo in &catalog.repos {
        if legacy_repo_name_only(&repo.repo_id, &repo.repo_name) {
            warnings.push(LEGACY_REPO_NAME_ONLY_WARNING.to_string());
        }
        if parse_repo_id(&repo.repo_id).is_none() {
            warnings.push(format!("repo_id must use org/repo form: {}", repo.repo_id));
        }
        if repo.repo_name.trim().is_empty() {
            warnings.push(format!("repo_name is empty for {}", repo.repo_id));
        }
        if repo.latest_tag.trim().is_empty() {
            warnings.push(format!("latest_tag is empty for {}", repo.repo_id));
        }
        if repo.package_ref.trim().is_empty() {
            warnings.push(format!("package_ref is empty for {}", repo.repo_id));
        }
        if repo.visibility == IndexVisibility::Tenant && repo.tenant.is_none() {
            warnings.push(format!("tenant entry is missing tenant: {}", repo.repo_id));
        }
        if seen.iter().any(|value| value == &repo.repo_id) {
            warnings.push(format!("duplicate repo_id in catalog: {}", repo.repo_id));
        } else {
            seen.push(repo.repo_id.clone());
        }
    }
    warnings
}

fn canonicalize_repo_index_identity(repo_index: &mut RepoIndex) {
    apply_legacy_repo_name_fallback(&mut repo_index.repo_id, &repo_index.repo_name);
    apply_legacy_repo_name_fallback(
        &mut repo_index.manifest.repo_id,
        &repo_index.manifest.repo_name,
    );
    if repo_index.manifest.repo_id != repo_index.repo_id {
        repo_index.manifest.repo_id = repo_index.repo_id.clone();
    }
}

fn apply_legacy_repo_name_fallback(repo_id: &mut String, repo_name: &str) {
    if legacy_repo_name_only(repo_id, repo_name) {
        *repo_id = format!("unknown/{}", repo_name.trim());
    }
}

fn legacy_repo_name_only(repo_id: &str, repo_name: &str) -> bool {
    repo_id == DEFAULT_REPO_ID && !repo_name.trim().is_empty()
}

fn editable_catalog_path(tenant: Option<&str>) -> PathBuf {
    let root = home_dir().join(".greentic-agent").join("catalogs");
    match tenant {
        Some(tenant) => root.join("tenants").join(tenant).join("catalog.json"),
        None => root.join("public").join("catalog.json"),
    }
}

fn published_catalog_path(tenant: Option<&str>, channel: Option<&str>) -> PathBuf {
    let root = default_remote_store_path(&home_dir()).join("catalogs");
    match (tenant, channel) {
        (Some(tenant), Some(channel)) => root
            .join("tenants")
            .join(tenant)
            .join(channel)
            .join("catalog.json"),
        (Some(tenant), None) => root.join("tenants").join(tenant).join("catalog.json"),
        (None, Some(channel)) => root.join("public").join(channel).join("catalog.json"),
        (None, None) => root.join("public").join("catalog.json"),
    }
}

fn file_digest_hex(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("failed to read digest input {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hex_encode(hasher.finalize()))
}

fn hex_encode(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn repo_role_for_repo_id(repo_id: &str) -> RepoRole {
    let name = repo_id
        .rsplit('/')
        .next()
        .unwrap_or(repo_id)
        .to_ascii_lowercase();
    if name.contains("type") || name.contains("contract") {
        RepoRole::CoreContracts
    } else if name.contains("cli") || name.contains("coding-agent") {
        RepoRole::CliLauncher
    } else if name.contains("component") {
        RepoRole::ComponentAuthoring
    } else if name.contains("flow") {
        RepoRole::FlowAuthoring
    } else if name.contains("pack") {
        RepoRole::PackAuthoring
    } else if name.contains("bundle") {
        RepoRole::BundleAssembly
    } else if name.contains("sorla") {
        RepoRole::SorlaLayer
    } else if name.contains("provider") {
        RepoRole::ProviderFamily
    } else if name.contains("demo") || name.contains("example") {
        RepoRole::DemoApp
    } else {
        RepoRole::SolutionLayer
    }
}

fn tag_from_index_uri(index_uri: &str) -> String {
    let last_segment = index_uri.rsplit('/').next().unwrap_or(index_uri);
    last_segment
        .rsplit_once(':')
        .map(|(_, tag)| tag.to_string())
        .filter(|tag| !tag.trim().is_empty())
        .unwrap_or_else(|| "latest".to_string())
}

fn run_check_refresh(format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    match engine_service().and_then(|service| {
        service
            .check_refresh(gca_engine::CheckRefreshOptions)
            .map_err(|error| error.to_string())
    }) {
        Ok(refresh) => {
            match format {
                OutputFormat::Json => println!(
                    "{}",
                    serde_json::to_string_pretty(&refresh).expect("refresh should serialize")
                ),
                OutputFormat::Markdown => {
                    println!("# Refresh Check");
                    println!();
                    println!("- Needs refresh: `{}`", refresh.needs_refresh);
                    println!("- Current head: `{}`", refresh.current_head_sha);
                    if let Some(indexed_head_sha) = &refresh.indexed_head_sha {
                        println!("- Indexed head: `{}`", indexed_head_sha);
                    }
                    if refresh.reasons.is_empty() {
                        println!("- No refresh reasons detected.");
                    } else {
                        println!("- Reasons:");
                        for reason in &refresh.reasons {
                            println!("  - {}", reason);
                        }
                    }
                }
            }
            0
        }
        Err(error) => {
            eprintln!("failed to check refresh: {error}");
            1
        }
    }
}

fn run_impact(symbol: &str, format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let repo_root = match repo_root_from_cwd() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            return 1;
        }
    };

    match load_or_analyze_repo_index() {
        Ok(repo_index) => match check_refresh(&repo_root) {
            Ok(refresh) => {
                let impact = impact_analysis(&repo_index, symbol, Some(&refresh));
                match format {
                    OutputFormat::Json => println!(
                        "{}",
                        serde_json::to_string_pretty(&impact).expect("impact should serialize")
                    ),
                    OutputFormat::Markdown => {
                        println!("# Impact");
                        println!();
                        println!("- Symbol: `{}`", impact.symbol);
                        println!("- Confidence: `{}`", impact.confidence);
                        if let Some(warning) = &impact.freshness_warning {
                            println!("- Freshness warning: {}", warning);
                        }
                        if !impact.concepts.is_empty() {
                            println!("- Concepts: {}", impact.concepts.join(", "));
                        }
                        if !impact.workflows.is_empty() {
                            println!("- Workflows: {}", impact.workflows.join(", "));
                        }
                        if !impact.validations.is_empty() {
                            println!("- Validations: {}", impact.validations.join(", "));
                        }
                        if !impact.owner_repos.is_empty() {
                            println!("- Owner repos: {}", impact.owner_repos.join(", "));
                        }
                    }
                }
                0
            }
            Err(error) => {
                eprintln!("failed to check refresh: {error}");
                1
            }
        },
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run_detect_changes(format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let repo_root = match repo_root_from_cwd() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            return 1;
        }
    };

    match load_or_analyze_repo_index() {
        Ok(repo_index) => match detect_changes(&repo_root, &repo_index) {
            Ok(changes) => {
                match format {
                    OutputFormat::Json => println!(
                        "{}",
                        serde_json::to_string_pretty(&changes).expect("changes should serialize")
                    ),
                    OutputFormat::Markdown => {
                        println!("# Detect Changes");
                        println!();
                        if let Some(warning) = &changes.freshness_warning {
                            println!("- Freshness warning: {}", warning);
                        }
                        if changes.changed_files.is_empty() {
                            println!(
                                "- No tracked-file changes detected relative to the indexed snapshot."
                            );
                        } else {
                            println!("- Changed files: {}", changes.changed_files.join(", "));
                        }
                        if !changes.likely_concepts.is_empty() {
                            println!("- Likely concepts: {}", changes.likely_concepts.join(", "));
                        }
                        if !changes.likely_workflows.is_empty() {
                            println!(
                                "- Likely workflows: {}",
                                changes.likely_workflows.join(", ")
                            );
                        }
                    }
                }
                0
            }
            Err(error) => {
                eprintln!("failed to detect changes: {error}");
                1
            }
        },
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run_validate_plan(plan_path: &str, format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    match engine_service().and_then(|service| {
        service
            .validate_plan(gca_engine::ValidatePlanOptions {
                plan_path: PathBuf::from(plan_path),
            })
            .map_err(|error| error.to_string())
    }) {
        Ok(validation) => {
            match format {
                OutputFormat::Json => println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "plan_path": validation.plan_path,
                        "task_summary": validation.task_summary,
                        "owner_hints": validation.owner_hints,
                        "validations": validation.validations,
                        "knowledge_updates": validation.knowledge_updates,
                        "acknowledged_updates": validation.acknowledged_updates,
                        "freshness_warning": validation.freshness_warning,
                        "issues": validation.issues,
                    }))
                    .expect("plan validation should serialize")
                ),
                OutputFormat::Markdown => {
                    println!("# Validate Plan");
                    println!();
                    println!("- Plan: `{}`", validation.plan_path.display());
                    if let Some(warning) = &validation.freshness_warning {
                        println!("- Freshness warning: {}", warning);
                    }
                    if !validation.issues.is_empty() {
                        println!("- Issues: {}", validation.issues.join(", "));
                    }
                    if !validation.knowledge_updates.is_empty() {
                        println!(
                            "- Knowledge updates: {}",
                            validation
                                .knowledge_updates
                                .iter()
                                .map(|update| format!(
                                    "{}({})",
                                    update.id,
                                    update.severity.as_str()
                                ))
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                    }
                    if !validation.owner_hints.is_empty() {
                        println!(
                            "- Owner hints: {}",
                            validation
                                .owner_hints
                                .iter()
                                .map(|hint| format!("{}->{}", hint.concept_id, hint.owner_repo))
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                    }
                }
            }
            if validation
                .issues
                .iter()
                .any(|issue| issue.contains("must be acknowledged"))
            {
                1
            } else {
                0
            }
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run_serve(input: ServerConfigInput<'_>, format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let config = ServerConfig::from_input(input);
    let service = QueryService::load(config.clone());

    if let Some(request_file) = input.request_file {
        let request = match fs::read_to_string(request_file) {
            Ok(raw) => match serde_json::from_str::<McpRequest>(&raw) {
                Ok(request) => request,
                Err(error) => {
                    eprintln!("failed to parse request file {}: {error}", request_file);
                    return 1;
                }
            },
            Err(error) => {
                eprintln!("failed to read request file {}: {error}", request_file);
                return 1;
            }
        };
        let response = service.dispatch_mcp_request(request);
        match format {
            OutputFormat::Json => println!(
                "{}",
                serde_json::to_string_pretty(&response).expect("mcp response should serialize")
            ),
            OutputFormat::Markdown => {
                println!("# Serve Response");
                println!();
                println!("- Ok: `{}`", response.ok);
                if let Some(error) = &response.error {
                    println!("- Error: {}", error);
                }
                if let Some(result) = &response.result {
                    println!("- Result: `{}`", result);
                }
            }
        }
        return if response.ok { 0 } else { 1 };
    }

    if config.mode == ServerMode::Http {
        return run_http_server(service);
    }

    if input.stdio
        || input.mcp
        || (!input.http && input.request_file.is_none() && matches!(format, OutputFormat::Markdown))
    {
        return run_mcp_stdio(service);
    }

    let snapshot = service.snapshot();

    match format {
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&snapshot).expect("mcp snapshot should serialize")
        ),
        OutputFormat::Markdown => {
            println!("# Serve");
            println!();
            println!("- Protocol: `{}`", snapshot.protocol);
            if let Some(warning) = &snapshot.freshness_warning {
                println!("- Freshness warning: {}", warning);
            }
            println!(
                "- Tools: {}",
                snapshot
                    .tools
                    .iter()
                    .map(|tool| tool.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            println!("- Resources: {}", snapshot.resources.join(", "));
            println!("- Request mode: use `--request-file <json>` to dispatch a tool call.");
        }
    }
    0
}

fn run_install_github_workflow(options: WorkflowInstallOptions<'_>, format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let repo_root = match repo_root_from_cwd() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            return 1;
        }
    };
    let catalog_kind = match options.catalog {
        Some("public") => Some("public"),
        Some("tenant") => Some("tenant"),
        Some(other) => {
            eprintln!("unsupported catalog workflow kind: {other}");
            return 2;
        }
        None => None,
    };
    if catalog_kind == Some("tenant") && options.tenant.is_none() {
        eprintln!("--catalog tenant requires --tenant");
        return 2;
    }
    let workflow_name = if catalog_kind.is_some() {
        "greentic-agent-catalog.yml"
    } else {
        "greentic-agent-index.yml"
    };
    let workflow_path = repo_root
        .join(".github")
        .join("workflows")
        .join(workflow_name);
    if let Some(parent) = workflow_path.parent()
        && let Err(error) = fs::create_dir_all(parent)
    {
        eprintln!("failed to prepare workflow directory: {error}");
        return 1;
    }
    let workflow = if let Some(kind) = catalog_kind {
        render_installed_catalog_workflow(kind, options.tenant)
    } else {
        render_installed_github_workflow(options)
    };
    if let Err(error) = fs::write(&workflow_path, workflow) {
        eprintln!("failed to write workflow: {error}");
        return 1;
    }

    match format {
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&workflow_path).expect("workflow path should serialize")
        ),
        OutputFormat::Markdown => {
            println!("# Install GitHub Workflow");
            println!();
            println!("- Workflow: `{}`", workflow_path.display());
            println!("- Permissions: `contents: read`, `packages: write`");
        }
    }
    0
}

fn run_workflows(format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };

    match engine_service().and_then(|service| {
        service
            .workflows(gca_engine::WorkflowsOptions)
            .map_err(|error| error.to_string())
    }) {
        Ok(response) => {
            match format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&response.workflows)
                            .expect("workflow graph should serialize")
                    );
                }
                OutputFormat::Markdown => {
                    println!("# Workflows");
                    println!();
                    for workflow in response.workflows {
                        println!("- `{}`: {}", workflow.id, workflow.summary);
                        if !workflow.commands.is_empty() {
                            println!("  commands: {}", workflow.commands.join(", "));
                        }
                    }
                }
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run_describe_here(format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };

    match engine_service().and_then(|service| {
        service
            .describe_here(gca_engine::DescribeOptions)
            .map_err(|error| error.to_string())
    }) {
        Ok(response) => {
            match format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "repo_root": response.repo_root,
                            "repo_id": response.repo_index.repo_id,
                            "repo_name": response.repo_index.repo_name,
                            "repo_role": response.repo_index.repo_role,
                            "primary_language": response.repo_index.manifest.primary_language,
                            "version": env!("CARGO_PKG_VERSION"),
                            "has_git_dir": response.repo_root.join(".git").exists(),
                        }))
                        .expect("describe payload should serialize")
                    );
                }
                OutputFormat::Markdown => {
                    println!("# Describe Here");
                    println!();
                    println!("- Repo ID: `{}`", response.repo_index.repo_id);
                    println!("- Repo name: `{}`", response.repo_index.repo_name);
                    let role = serde_json::to_value(response.repo_index.repo_role)
                        .ok()
                        .and_then(|value| value.as_str().map(ToString::to_string))
                        .unwrap_or_else(|| format!("{:?}", response.repo_index.repo_role));
                    println!("- Role: `{role}`");
                    println!(
                        "- Primary language: `{}`",
                        response.repo_index.manifest.primary_language
                    );
                    println!("- Root: `{}`", response.repo_root.display());
                }
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn print_summary(summary: &DescribeHere, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(summary).expect("describe payload should serialize")
            );
        }
        OutputFormat::Markdown => {
            println!("# Describe Here");
            println!();
            println!("- Version: `{}`", summary.version);
            println!("- Repo root: `{}`", summary.repo_root.display());
            println!("- Repo ID: `{}`", summary.repo_id);
            println!("- Repo name: `{}`", summary.repo_name);
            println!("- Manifest: `{}`", summary.manifest_path.display());
            println!("- Git detected: `{}`", summary.has_git_dir);
            if let Some(local_index) = &summary.local_index_path {
                println!("- Local index: `{}`", local_index.display());
            }
            if let Some(repo_role) = summary.repo_role {
                println!("- Repo role: `{}`", repo_role.as_str());
            }
            if let Some(concept_count) = summary.concept_count {
                println!("- Concepts indexed: `{concept_count}`");
            }
            if let Some(workflow_count) = summary.workflow_count {
                println!("- Workflows indexed: `{workflow_count}`");
            }
            if let Some(instruction_count) = summary.instruction_count {
                println!("- Instruction docs indexed: `{instruction_count}`");
            }
        }
    }
}

fn print_analyze_summary(outputs: &AnalyzeOutputs, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(outputs).expect("analyze payload should serialize")
            );
        }
        OutputFormat::Markdown => {
            println!("# Analyze");
            println!();
            println!("- Repo: `{}`", outputs.manifest.repo_name);
            println!("- Repo ID: `{}`", outputs.manifest.repo_id);
            println!("- Repo root: `{}`", outputs.manifest.repo_root);
            println!("- Head SHA: `{}`", outputs.fingerprints.head_sha);
            println!("- Registry: `{}`", outputs.registry_path.display());
            println!("- Manifest output: `{}`", outputs.manifest_path.display());
            println!(
                "- Repo index output: `{}`",
                outputs.repo_index_path.display()
            );
            println!(
                "- Fingerprints output: `{}`",
                outputs.fingerprints_path.display()
            );
            println!(
                "- Concepts inferred: `{}`",
                outputs.repo_index.concept_graph.len()
            );
            println!(
                "- Workflows inferred: `{}`",
                outputs.repo_index.workflow_graph.len()
            );
            println!(
                "- Instruction docs indexed: `{}`",
                outputs.repo_index.instruction_graph.len()
            );
            if let Some(report) = &outputs.tantivy_report {
                println!(
                    "- Tantivy documents indexed: `{}`",
                    report.documents_indexed
                );
                println!("- Tantivy index: `{}`", report.index_path.display());
            }
        }
    }
}

fn bootstrap_guidance_for(repo_id: &str) -> BootstrapGuidance {
    BootstrapGuidance {
        repo_id: repo_id.to_string(),
        describe_command: "greentic-coding-agent describe --here".to_string(),
        analyze_command: "greentic-coding-agent analyze".to_string(),
        sync_public_command: "greentic-coding-agent sync".to_string(),
        search_command: "greentic-coding-agent search --scope all --mode instruction \"the task\""
            .to_string(),
        mcp_server_command: "greentic-coding-agent serve --mcp --watch".to_string(),
        http_server_command: "greentic-coding-agent serve --http --host 127.0.0.1 --port 7757 --watch"
            .to_string(),
        tenant_sync_command:
            "greentic-coding-agent sync --tenant <tenant> --token-env TENANT_GHCR_TOKEN"
                .to_string(),
        tenant_server_command:
            "greentic-coding-agent serve --mcp --tenant <tenant> --token-env TENANT_GHCR_TOKEN --watch"
                .to_string(),
        install_workflow_command: "greentic-coding-agent install-github-workflow --publish-ghcr"
            .to_string(),
        rules: vec![
            "Always call `describe --here` first.".to_string(),
            "Search before creating new abstractions.".to_string(),
            "Run `impact` before editing shared contracts.".to_string(),
            "Run `required-validations` before finishing.".to_string(),
            "Do not duplicate concepts that have a known owner repo.".to_string(),
        ],
    }
}

fn print_bootstrap_guidance(guidance: &BootstrapGuidance, format: OutputFormat) {
    match format {
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(guidance).expect("bootstrap guidance should serialize")
        ),
        OutputFormat::Markdown => println!("{}", render_bootstrap_guidance(guidance)),
    }
}

fn render_bootstrap_guidance(guidance: &BootstrapGuidance) -> String {
    let mut out = BOOTSTRAP_TEMPLATE
        .replace("{{repo_id}}", &guidance.repo_id)
        .replace("{{describe_command}}", &guidance.describe_command)
        .replace("{{analyze_command}}", &guidance.analyze_command)
        .replace("{{sync_public_command}}", &guidance.sync_public_command)
        .replace("{{search_command}}", &guidance.search_command)
        .replace("{{mcp_server_command}}", &guidance.mcp_server_command)
        .replace("{{http_server_command}}", &guidance.http_server_command)
        .replace("{{tenant_sync_command}}", &guidance.tenant_sync_command)
        .replace("{{tenant_server_command}}", &guidance.tenant_server_command)
        .replace(
            "{{install_workflow_command}}",
            &guidance.install_workflow_command,
        );
    out.push('\n');
    out
}

fn print_search_response(response: &SearchResponse) {
    println!("# Search");
    println!();
    println!("- Mode: `{}`", response.mode.as_str());
    println!("- Query: `{}`", response.query);
    println!("- Results: `{}`", response.results.len());
    if response.results.is_empty() {
        println!("- No results found.");
        return;
    }
    for result in &response.results {
        println!(
            "- `{}` [{}] {}",
            result.id,
            result.result_type.as_str(),
            result.title
        );
        println!("  locator: {}", result.locator);
        println!("  provenance: {}", result.provenance);
        println!("  freshness: {}", result.freshness.as_str());
        println!("  snippet: {}", result.snippet);
    }
}

fn print_owner_lookup(concept: &str, owner: Option<&OwnerLookup>) {
    println!("# Locate Owner");
    println!();
    println!("- Concept: `{concept}`");
    match owner {
        Some(owner) => {
            println!("- Owner repo: `{}`", owner.owner_repo);
            println!("- Rationale: {}", owner.rationale);
            if !owner.forbidden_locations.is_empty() {
                println!(
                    "- Forbidden locations: {}",
                    owner.forbidden_locations.join(", ")
                );
            }
            if !owner.required_validations.is_empty() {
                println!(
                    "- Required validations: {}",
                    owner.required_validations.join(", ")
                );
            }
        }
        None => println!("- No owner policy found."),
    }
}

fn print_required_validations(response: &RequiredValidationsResponse) {
    println!("# Required Validations");
    println!();
    println!("- Task: `{}`", response.task);
    println!("- Matches: `{}`", response.validations.len());
    if response.validations.is_empty() {
        println!("- No validation guidance found.");
        return;
    }
    for validation in &response.validations {
        println!("- `{}`: {}", validation.id, validation.summary);
        if !validation.command_groups.is_empty() {
            println!("  commands: {}", validation.command_groups.join(", "));
        }
    }
}

fn render_agents(repo_index: &RepoIndex) -> String {
    let mut out = String::new();
    out.push_str("# AGENTS.md\n\n");
    out.push_str(&generated_provenance());
    out.push_str(&format!(
        "This repository is `{}` with repo role `{}`.\n\n",
        repo_index.repo_name,
        repo_index.repo_role.as_str()
    ));
    out.push_str("## Repo Summary\n");
    out.push_str(&format!(
        "- Freshness: `{}`\n- Concepts indexed: `{}`\n- Workflows indexed: `{}`\n\n",
        repo_index.freshness.as_str(),
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
        repo_index.freshness.as_str()
    ));
    out.push_str("## Editing Policy\n");
    out.push_str("- Check impact before editing shared concepts.\n");
    out.push_str("- Prefer seeded owner lookup before changing cross-repo contracts.\n\n");
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
    out.push_str("\nUseful commands:\n");
    for entry in command_catalog().into_iter().take(8) {
        out.push_str(&format!("- {}\n", entry.command));
    }
    out
}

fn generated_provenance() -> String {
    format!(
        "<!-- generated by greentic-coding-agent {} -->\n\n",
        env!("CARGO_PKG_VERSION")
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Json,
    Markdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SearchMode {
    Code,
    Instruction,
    Concept,
    Reuse,
    Course,
    Update,
}

impl SearchMode {
    fn parse(value: &str) -> Result<Self, String> {
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

    fn as_str(self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Instruction => "instruction",
            Self::Concept => "concept",
            Self::Reuse => "reuse",
            Self::Course => "course",
            Self::Update => "update",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SearchEngineChoice {
    Auto,
    Tantivy,
    Fallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SearchScope {
    Local,
    Merged,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ServerMode {
    McpStdio,
    Http,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
struct ServerConfig {
    mode: ServerMode,
    host: String,
    port: u16,
    watch: bool,
    sync_interval_seconds: u64,
    strict_sync: bool,
    prune_disabled: bool,
    tenant: Option<String>,
    #[serde(skip_serializing)]
    token: Option<String>,
    catalog_ref: Option<String>,
    tenant_catalog_ref: Option<String>,
}

impl fmt::Debug for ServerConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServerConfig")
            .field("mode", &self.mode)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("watch", &self.watch)
            .field("sync_interval_seconds", &self.sync_interval_seconds)
            .field("strict_sync", &self.strict_sync)
            .field("prune_disabled", &self.prune_disabled)
            .field("tenant", &self.tenant)
            .field("token", &self.token.as_ref().map(|_| "[redacted]"))
            .field("catalog_ref", &self.catalog_ref)
            .field("tenant_catalog_ref", &self.tenant_catalog_ref)
            .finish()
    }
}

impl ServerConfig {
    fn from_input(input: ServerConfigInput<'_>) -> Self {
        let token = if let Some(token) = input.token {
            Some(token.to_string())
        } else if let Some(token_env) = input.token_env {
            env::var(token_env).ok()
        } else {
            env::var("GREENTIC_AGENT_TOKEN").ok()
        };
        let mode = if input.http {
            ServerMode::Http
        } else {
            ServerMode::McpStdio
        };
        Self {
            mode,
            host: input.host.to_string(),
            port: input.port,
            watch: input.watch,
            sync_interval_seconds: input.sync_interval_seconds,
            strict_sync: input.strict_sync,
            prune_disabled: input.prune_disabled,
            tenant: input.tenant.map(ToString::to_string),
            token,
            catalog_ref: input.catalog_ref.map(ToString::to_string),
            tenant_catalog_ref: input.tenant_catalog_ref.map(ToString::to_string),
        }
    }

    fn status_json(&self) -> serde_json::Value {
        serde_json::json!({
            "mode": self.mode,
            "host": self.host,
            "port": self.port,
            "watch": self.watch,
            "watch_enabled": self.watch,
            "sync_interval_seconds": self.sync_interval_seconds,
            "strict_sync": self.strict_sync,
            "prune_disabled": self.prune_disabled,
            "tenant": self.tenant,
            "token": self.token.as_ref().map(|_| "[redacted]"),
            "catalog_ref": self.catalog_ref,
            "tenant_catalog_ref": self.tenant_catalog_ref,
        })
    }
}

impl SearchScope {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "local" => Ok(Self::Local),
            "merged" => Ok(Self::Merged),
            "all" => Ok(Self::All),
            other => Err(format!("unsupported search scope: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RemoteBackendKind {
    LocalFixture,
    GhcrOras,
}

impl RemoteBackendKind {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "local" | "local_fixture" => Ok(Self::LocalFixture),
            "ghcr" | "ghcr_oras" => Ok(Self::GhcrOras),
            other => Err(format!("unsupported remote backend: {other}")),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RegistryAuth {
    registry: String,
    username: Option<String>,
    token: String,
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
struct RemoteConfig {
    backend: RemoteBackendKind,
    public_catalog_ref: String,
    tenant: Option<String>,
    tenant_catalog_ref: Option<String>,
    auth: Option<RegistryAuth>,
    strict: bool,
    public_only: bool,
    private_only: bool,
    include_private: bool,
}

#[derive(Debug, Clone, Copy)]
struct SyncOptions<'a> {
    repo: Option<&'a str>,
    tag: Option<&'a str>,
    channel: Option<&'a str>,
    catalog: Option<&'a str>,
    tenant: Option<&'a str>,
    tenant_catalog: Option<&'a str>,
    token: Option<&'a str>,
    token_env: Option<&'a str>,
    backend: &'a str,
    strict: bool,
    public_only: bool,
    private_only: bool,
    include_private: bool,
    prune_disabled: bool,
}

#[derive(Debug, Clone, Copy)]
struct SearchOptions<'a> {
    mode: &'a str,
    engine: &'a str,
    scope: &'a str,
    repo: Option<&'a str>,
    tenant: Option<&'a str>,
    query: &'a str,
}

#[derive(Debug, Clone, Copy)]
struct ServerConfigInput<'a> {
    mcp: bool,
    http: bool,
    stdio: bool,
    host: &'a str,
    port: u16,
    watch: bool,
    sync_interval_seconds: u64,
    strict_sync: bool,
    prune_disabled: bool,
    tenant: Option<&'a str>,
    token: Option<&'a str>,
    token_env: Option<&'a str>,
    catalog_ref: Option<&'a str>,
    tenant_catalog_ref: Option<&'a str>,
    request_file: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
struct WatchOptions<'a> {
    channel: Option<&'a str>,
    tenant: Option<&'a str>,
    token: Option<&'a str>,
    token_env: Option<&'a str>,
    sync_interval_seconds: u64,
    strict_sync: bool,
    prune_disabled: bool,
    once: bool,
}

#[derive(Debug, Clone, Copy)]
struct WorkflowInstallOptions<'a> {
    publish_ghcr: bool,
    catalog: Option<&'a str>,
    tenant: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
struct CatalogAddOptions<'a> {
    repo: &'a str,
    index_uri: &'a str,
    tenant: Option<&'a str>,
    reason: Option<&'a str>,
    publish: bool,
    backend: &'a str,
    token: Option<&'a str>,
    token_env: Option<&'a str>,
    format: &'a str,
}

impl SearchEngineChoice {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "auto" => Ok(Self::Auto),
            "tantivy" => Ok(Self::Tantivy),
            "fallback" => Ok(Self::Fallback),
            other => Err(format!("unsupported search engine: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SearchResultType {
    Code,
    Instruction,
    Concept,
    Reuse,
    Course,
    Update,
}

impl SearchResultType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Instruction => "instruction",
            Self::Concept => "concept",
            Self::Reuse => "reuse",
            Self::Course => "course",
            Self::Update => "update",
        }
    }
}

impl OutputFormat {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "json" => Ok(Self::Json),
            "markdown" => Ok(Self::Markdown),
            other => Err(format!("unsupported output format: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RepoRole {
    CoreContracts,
    CliLauncher,
    ComponentAuthoring,
    FlowAuthoring,
    PackAuthoring,
    BundleAssembly,
    SolutionLayer,
    SorlaLayer,
    ProviderFamily,
    DemoApp,
    CustomerSolution,
    ExamplesOnly,
}

impl RepoRole {
    fn as_str(self) -> &'static str {
        match self {
            Self::CoreContracts => "core_contracts",
            Self::CliLauncher => "cli_launcher",
            Self::ComponentAuthoring => "component_authoring",
            Self::FlowAuthoring => "flow_authoring",
            Self::PackAuthoring => "pack_authoring",
            Self::BundleAssembly => "bundle_assembly",
            Self::SolutionLayer => "solution_layer",
            Self::SorlaLayer => "sorla_layer",
            Self::ProviderFamily => "provider_family",
            Self::DemoApp => "demo_app",
            Self::CustomerSolution => "customer_solution",
            Self::ExamplesOnly => "examples_only",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FreshnessStatus {
    Fresh,
}

impl FreshnessStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum KnowledgeScope {
    LocalRepo,
    CrossRepo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LifecyclePhase {
    Design,
    Build,
    Setup,
    Start,
    Runtime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RepoAgentManifest {
    version: String,
    #[serde(default = "default_repo_id")]
    repo_id: String,
    repo_name: String,
    #[serde(default)]
    org: Option<String>,
    repo_root: String,
    repo_role: RepoRole,
    primary_language: String,
    generated_at: String,
    candidate_docs: Vec<String>,
    cargo_manifests: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RepoIndex {
    version: String,
    #[serde(default = "default_repo_id")]
    repo_id: String,
    repo_name: String,
    repo_role: RepoRole,
    generated_at: String,
    #[serde(default)]
    metadata: Option<RepoIndexMetadata>,
    freshness: FreshnessStatus,
    manifest: RepoAgentManifest,
    concept_graph: Vec<ConceptDescriptor>,
    workflow_graph: Vec<WorkflowDescriptor>,
    validations: Vec<ValidationDescriptor>,
    reuse: Vec<ReuseDescriptor>,
    #[serde(default)]
    training_courses: Vec<TrainingCourseDescriptor>,
    #[serde(default)]
    knowledge_updates: Vec<KnowledgeUpdateDescriptor>,
    instruction_graph: Vec<InstructionDescriptor>,
    instruction_paths: Vec<String>,
    source_stats: SourceStats,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RepoIndexMetadata {
    repo_id: String,
    #[serde(default)]
    branch: Option<String>,
    #[serde(default)]
    commit_sha: Option<String>,
    #[serde(default)]
    commit_time: Option<String>,
    indexed_at: String,
    index_schema_version: String,
    tool_version: String,
    #[serde(default)]
    source_tree_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ConceptDescriptor {
    id: String,
    title: String,
    summary: String,
    scope: KnowledgeScope,
    lifecycle_phase: LifecyclePhase,
    owners: Vec<String>,
    related_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct WorkflowDescriptor {
    id: String,
    title: String,
    summary: String,
    phase: LifecyclePhase,
    commands: Vec<String>,
    docs: Vec<String>,
    concept_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct InstructionDescriptor {
    id: String,
    path: String,
    title: String,
    kind: String,
    headings: Vec<String>,
    commands: Vec<String>,
    concept_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
struct SourceStats {
    workspace_members: Vec<String>,
    crate_names: Vec<String>,
    modules: Vec<String>,
    public_items: Vec<String>,
    test_targets: Vec<String>,
    feature_names: Vec<String>,
    dependencies: Vec<String>,
    markdown_docs: Vec<String>,
    workflow_files: Vec<String>,
    example_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SearchResult {
    repo_id: String,
    id: String,
    title: String,
    result_type: SearchResultType,
    locator: String,
    snippet: String,
    provenance: String,
    freshness: FreshnessStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SearchResponse {
    mode: SearchMode,
    query: String,
    results: Vec<SearchResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct AgentContextResponse {
    task: String,
    channel: String,
    relevant_repos: Vec<AgentRepoCandidate>,
    owner_candidates: Vec<OwnerLookup>,
    required_validations: Vec<ValidationDescriptor>,
    recent_updates: Vec<KnowledgeUpdateDescriptor>,
    tutorials: Vec<TrainingCourseDescriptor>,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct AgentRepoCandidate {
    repo_id: String,
    branch: Option<String>,
    source: String,
    matched: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct NotificationFeed {
    schema_version: String,
    items: Vec<NotificationItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct NotificationItem {
    id: String,
    repo_id: String,
    branch: String,
    #[serde(rename = "type")]
    item_type: String,
    title: String,
    old_commit: Option<String>,
    new_commit: Option<String>,
    created_at: String,
    agent_impact: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct NotificationSeenState {
    schema_version: String,
    seen: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CommandCatalogEntry {
    command: String,
    purpose: String,
    phase: LifecyclePhase,
    inputs: Vec<String>,
    outputs: Vec<String>,
    when_to_use: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ValidationDescriptor {
    id: String,
    title: String,
    summary: String,
    phase: LifecyclePhase,
    command_groups: Vec<String>,
    triggered_by: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ReuseDescriptor {
    id: String,
    concept_id: String,
    owner_repo: String,
    rationale: String,
    forbidden_locations: Vec<String>,
    required_validations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct TrainingCourseDescriptor {
    version: String,
    id: String,
    title: String,
    summary: String,
    owner_repo: String,
    teaches_concepts: Vec<String>,
    tasks: Vec<String>,
    audience: Vec<TrainingAudience>,
    lifecycle_phase: LifecyclePhase,
    modules: Vec<TrainingModuleDescriptor>,
    canonical_commands: Vec<String>,
    deprecated_commands: Vec<DeprecatedCommandDescriptor>,
    required_validations: Vec<String>,
    examples: Vec<String>,
    source_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct TrainingModuleDescriptor {
    id: String,
    title: String,
    objective: String,
    steps: Vec<TrainingStepDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct TrainingStepDescriptor {
    order: u32,
    instruction: String,
    command: Option<String>,
    expected_output: Option<String>,
    validation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct DeprecatedCommandDescriptor {
    command: String,
    reason: String,
    replacement: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TrainingAudience {
    CodingAgent,
    HumanDeveloper,
    CiAutomation,
    RepoMaintainer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct KnowledgeUpdateDescriptor {
    version: String,
    id: String,
    title: String,
    summary: String,
    owner_repo: String,
    update_type: KnowledgeUpdateType,
    published_at: String,
    effective_from: Option<String>,
    expires_at: Option<String>,
    affected_concepts: Vec<String>,
    affected_workflows: Vec<String>,
    affected_courses: Vec<String>,
    affected_repos: Vec<String>,
    agent_instruction: String,
    human_summary: Option<String>,
    new_capabilities: Vec<CapabilityAnnouncement>,
    deprecated_commands: Vec<DeprecatedCommandDescriptor>,
    replaced_guidance: Vec<ReplacedGuidanceDescriptor>,
    migration_steps: Vec<MigrationStepDescriptor>,
    required_validations: Vec<String>,
    source_paths: Vec<String>,
    severity: KnowledgeUpdateSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum KnowledgeUpdateType {
    NewCapability,
    BehaviourChange,
    DeprecatedWorkflow,
    DeprecatedCommand,
    MigrationRequired,
    ValidationChanged,
    OwnershipChanged,
    CourseUpdated,
    SecurityNotice,
    BreakingChange,
    DocumentationCorrection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum KnowledgeUpdateSeverity {
    Info,
    Recommended,
    Important,
    Breaking,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CapabilityAnnouncement {
    id: String,
    title: String,
    summary: String,
    use_when: Vec<String>,
    owner_repo: String,
    related_course: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ReplacedGuidanceDescriptor {
    old_guidance: String,
    replacement_guidance: String,
    reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct MigrationStepDescriptor {
    order: u32,
    instruction: String,
    command: Option<String>,
    validation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct OwnerLookup {
    concept_id: String,
    owner_repo: String,
    rationale: String,
    forbidden_locations: Vec<String>,
    required_validations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RequiredValidationsResponse {
    task: String,
    validations: Vec<ValidationDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct GeneratedFile {
    file_name: String,
    content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PackageMetadata {
    repo_id: String,
    repo_name: String,
    tag: String,
    reference: String,
    generated_at: String,
    compatibility: String,
    files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PackageResult {
    package_dir: PathBuf,
    reference: String,
    metadata: PackageMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct TantivyBuildReport {
    index_path: PathBuf,
    documents_indexed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RemoteRepo {
    repo_id: String,
    repo_name: String,
    tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CatalogRepo {
    #[serde(default = "default_repo_id")]
    repo_id: String,
    #[serde(default)]
    repo_name: String,
    #[serde(alias = "role", default = "default_repo_role")]
    repo_role: RepoRole,
    #[serde(default = "default_latest_tag")]
    latest_tag: String,
    #[serde(default)]
    package_ref: String,
    #[serde(default)]
    updated_at: String,
    #[serde(default)]
    default_branch: Option<String>,
    #[serde(default)]
    preferred_branch: Option<String>,
    #[serde(default)]
    branches: BTreeMap<String, CatalogBranchEntry>,
    #[serde(default)]
    visibility: IndexVisibility,
    #[serde(default)]
    tenant: Option<String>,
    #[serde(default)]
    required_auth: Option<AuthKind>,
    #[serde(default)]
    digest: Option<String>,
    #[serde(default)]
    source_commit: Option<String>,
    #[serde(default = "default_enabled")]
    enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CatalogBranchEntry {
    index_uri: String,
    #[serde(default)]
    commit_sha: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default)]
    digest: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
enum IndexVisibility {
    #[default]
    Public,
    Tenant,
    Private,
}

impl IndexVisibility {
    fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Tenant => "tenant",
            Self::Private => "private",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AuthKind {
    GhcrToken,
    BearerToken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Catalog {
    #[serde(alias = "schema_version")]
    version: String,
    generated_at: String,
    #[serde(default)]
    catalog_id: Option<String>,
    #[serde(default)]
    default_channel: Option<String>,
    repos: Vec<CatalogRepo>,
    #[serde(default)]
    change_log: Vec<CatalogChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CatalogChange {
    action: CatalogAction,
    repo_id: String,
    tenant: Option<String>,
    at: String,
    by: Option<String>,
    reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CatalogAction {
    AddRepo,
    RemoveRepo,
    EnableRepo,
    DisableRepo,
    Publish,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SyncState {
    version: String,
    updated_at: String,
    repos: Vec<SyncedRepoState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SyncedRepoState {
    repo_id: String,
    tenant: Option<String>,
    visibility: IndexVisibility,
    package_ref: String,
    digest: Option<String>,
    source_commit: Option<String>,
    downloaded_at: String,
    local_index_path: PathBuf,
    local_tantivy_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct SyncReport {
    channel: Option<String>,
    public_catalog: Option<String>,
    tenant_catalog: Option<String>,
    downloaded: Vec<String>,
    skipped: Vec<String>,
    failed: Vec<SyncFailure>,
    merged_index_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct SyncFailure {
    repo_id: String,
    error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct MergedIndexReport {
    merged_index_path: PathBuf,
    repos_indexed: usize,
    documents_indexed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct WatchStatus {
    watch_enabled: bool,
    last_sync_at: Option<String>,
    last_sync_status: String,
    last_error: Option<String>,
    indexed_repos: usize,
    tenant: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct BootstrapGuidance {
    repo_id: String,
    describe_command: String,
    analyze_command: String,
    sync_public_command: String,
    search_command: String,
    mcp_server_command: String,
    http_server_command: String,
    tenant_sync_command: String,
    tenant_server_command: String,
    install_workflow_command: String,
    rules: Vec<String>,
}

impl WatchStatus {
    fn disabled(tenant: Option<String>) -> Self {
        Self {
            watch_enabled: false,
            last_sync_at: None,
            last_sync_status: "idle".to_string(),
            last_error: None,
            indexed_repos: 0,
            tenant,
        }
    }
}

#[derive(Debug, Clone)]
struct CachedRepoIndex {
    repo_index: RepoIndex,
    state: SyncedRepoState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RefreshCheck {
    needs_refresh: bool,
    reasons: Vec<String>,
    current_head_sha: String,
    indexed_head_sha: Option<String>,
    current_tracked_files: Vec<String>,
    indexed_tracked_files: Vec<String>,
    current_generator_version: String,
    indexed_generator_version: Option<String>,
    current_schema_version: String,
    indexed_schema_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ImpactAnalysis {
    symbol: String,
    confidence: String,
    provenance: Vec<String>,
    concepts: Vec<String>,
    workflows: Vec<String>,
    validations: Vec<String>,
    owner_repos: Vec<String>,
    freshness_warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ChangeDetection {
    changed_files: Vec<String>,
    likely_concepts: Vec<String>,
    likely_workflows: Vec<String>,
    suggested_validations: Vec<ValidationDescriptor>,
    freshness_warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PlanValidation {
    plan_path: String,
    task_summary: String,
    owner_hints: Vec<OwnerLookup>,
    required_validations: Vec<ValidationDescriptor>,
    freshness_warning: Option<String>,
    issues: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct McpTool {
    name: String,
    description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct McpServerSnapshot {
    protocol: String,
    tools: Vec<McpTool>,
    resources: Vec<String>,
    freshness_warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct McpRequest {
    id: Option<String>,
    tool: String,
    #[serde(default)]
    arguments: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct McpResponse {
    id: Option<String>,
    ok: bool,
    result: Option<serde_json::Value>,
    error: Option<String>,
}

struct ConceptRule<'a> {
    id: &'a str,
    title: &'a str,
    summary: &'a str,
    needles: &'a [&'a str],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Fingerprints {
    version: String,
    head_sha: String,
    default_branch: Option<String>,
    tracked_files: Vec<String>,
    #[serde(default)]
    generator_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RegistryEntry {
    #[serde(default = "default_repo_id")]
    repo_id: String,
    repo_name: String,
    #[serde(default)]
    org: Option<String>,
    repo_path: String,
    repo_role: RepoRole,
    last_analyzed_commit: String,
    manifest_path: String,
    repo_index_path: String,
    updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Registry {
    version: String,
    repos: Vec<RegistryEntry>,
}

impl Registry {
    fn empty() -> Self {
        Self {
            version: SCHEMA_VERSION_V1.to_string(),
            repos: Vec::new(),
        }
    }

    fn upsert(&mut self, entry: RegistryEntry) {
        if let Some(existing) = self.repos.iter_mut().find(|existing| {
            existing.repo_id == entry.repo_id || existing.repo_path == entry.repo_path
        }) {
            *existing = entry;
            return;
        }

        self.repos.push(entry);
        self.repos
            .sort_by(|left, right| left.repo_id.cmp(&right.repo_id));
    }
}

fn default_repo_id() -> String {
    DEFAULT_REPO_ID.to_string()
}

fn default_enabled() -> bool {
    true
}

fn default_repo_role() -> RepoRole {
    RepoRole::DemoApp
}

fn default_latest_tag() -> String {
    "latest".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct AnalyzeOutputs {
    manifest: RepoAgentManifest,
    repo_index: RepoIndex,
    fingerprints: Fingerprints,
    manifest_path: PathBuf,
    repo_index_path: PathBuf,
    fingerprints_path: PathBuf,
    registry_path: PathBuf,
    tantivy_report: Option<TantivyBuildReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct DescribeHere {
    version: String,
    repo_root: PathBuf,
    repo_id: String,
    repo_name: String,
    manifest_path: PathBuf,
    has_git_dir: bool,
    local_index_path: Option<PathBuf>,
    repo_role: Option<RepoRole>,
    concept_count: Option<usize>,
    workflow_count: Option<usize>,
    instruction_count: Option<usize>,
}

fn builtin_concepts() -> Vec<ConceptDescriptor> {
    BUILTIN_CONCEPT_IDS
        .iter()
        .map(|id| ConceptDescriptor {
            id: (*id).to_string(),
            title: id.replace('_', " "),
            summary: format!("Built-in Greentic concept `{id}`."),
            scope: KnowledgeScope::CrossRepo,
            lifecycle_phase: LifecyclePhase::Build,
            owners: vec!["greentic-coding-agent".to_string()],
            related_paths: Vec::new(),
        })
        .collect()
}

fn command_catalog() -> Vec<CommandCatalogEntry> {
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
            command: "greentic-coding-agent package-index --tag <branch> --tag sha-<commit>"
                .to_string(),
            purpose:
                "Build a local OCI-style package for the current repo index and generated agent docs."
                    .to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["One or more tags".to_string()],
            outputs: vec![".greentic-agent/oci/<repo>/<tag>".to_string()],
            when_to_use:
                "Before publishing or inspecting a distributable repo index artifact.".to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent publish-index --tag <branch> --tag sha-<commit>"
                .to_string(),
            purpose: "Publish the local OCI-style package into the configured remote store."
                .to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["One or more tags".to_string()],
            outputs: vec!["~/.greentic-agent/remote-oci/<repo>/<tag>".to_string()],
            when_to_use:
                "When sharing a packaged repo index for later sync or inspection.".to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent sync --repo <repo> --tag <tag>".to_string(),
            purpose: "Copy a published OCI-style package into the local cache.".to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Repo name".to_string(), "Tag".to_string()],
            outputs: vec!["~/.greentic-agent/cache-oci/<repo>/<tag>".to_string()],
            when_to_use: "When pulling a packaged repo index into the local machine cache."
                .to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent watch --channel <branch> --poll 10m".to_string(),
            purpose: "Keep the global cache current and append org notification feed items when synced indexes change."
                .to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Channel".to_string(), "Poll interval".to_string()],
            outputs: vec![
                "~/.greentic-agent/sync-state.json".to_string(),
                "~/.greentic-agent/notifications/feed.json".to_string(),
            ],
            when_to_use: "When a developer or agent host should keep merged Greentic knowledge fresh."
                .to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent list-remote-repos".to_string(),
            purpose: "List repos and tags currently available in the configured remote store."
                .to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Remote store".to_string()],
            outputs: vec!["Remote repo list".to_string()],
            when_to_use: "When discovering which packaged repo indexes are available to sync."
                .to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent show-catalog".to_string(),
            purpose: "Build a discovery catalog from the currently published remote repo indexes."
                .to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Remote store".to_string()],
            outputs: vec!["Catalog".to_string()],
            when_to_use: "When discovering multiple published repos and their latest tags."
                .to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent check-refresh".to_string(),
            purpose: "Explain whether the local repo index should be regenerated and republished."
                .to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Current checkout".to_string(), "Local index outputs".to_string()],
            outputs: vec!["Refresh decision".to_string()],
            when_to_use: "Before publishing or in CI when deciding whether refresh is needed."
                .to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent install-github-workflow".to_string(),
            purpose: "Generate the GitHub workflow that analyzes, checks refresh, packages, and publishes repo indexes."
                .to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Current repository".to_string()],
            outputs: vec![".github/workflows/greentic-agent-index.yml".to_string()],
            when_to_use: "When enabling per-repo self-refresh automation.".to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent org plan-index-rollout --org <org>".to_string(),
            purpose: "Create a deterministic org-wide plan for enabling the standard Greentic coding-agent index workflow."
                .to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec![
                "GitHub org".to_string(),
                "Repo source or repo-list file".to_string(),
            ],
            outputs: vec!["Index rollout plan JSON".to_string()],
            when_to_use: "Before applying indexing automation across multiple Greentic repositories."
                .to_string(),
        },
        CommandCatalogEntry {
            command:
                "greentic-coding-agent org apply-index-rollout --plan <plan.json> --open-prs"
                    .to_string(),
            purpose: "Apply an org-wide index rollout plan by writing workflow branches and opening pull requests."
                .to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Index rollout plan JSON".to_string()],
            outputs: vec!["Rollout apply report".to_string()],
            when_to_use: "After reviewing a rollout plan and deciding to create repository PRs."
                .to_string(),
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

fn built_in_policy_bundle() -> (Vec<ValidationDescriptor>, Vec<ReuseDescriptor>) {
    (
        vec![
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
                triggered_by: vec![
                    "docs".to_string(),
                    "readme".to_string(),
                    "architecture".to_string(),
                ],
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
        vec![
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
        ],
    )
}

fn render_generated_files(repo_index: &RepoIndex) -> Vec<GeneratedFile> {
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

fn write_generated_files(
    repo_root: &Path,
    files: &[GeneratedFile],
    write_root: bool,
) -> Result<Vec<PathBuf>, std::io::Error> {
    let generated_dir = repo_root.join(LOCAL_INDEX_DIR).join("generated");
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

fn package_index_layout(
    repo_root: &Path,
    repo_index: &RepoIndex,
    tag: &str,
) -> Result<PackageResult, std::io::Error> {
    let package_dir = repo_root
        .join(LOCAL_INDEX_DIR)
        .join("oci")
        .join(repo_id_path(&repo_index.repo_id))
        .join(tag);
    let artifacts_dir = package_dir.join("artifacts");
    let agents_dir = artifacts_dir.join("agents");
    let blobs_dir = package_dir.join("blobs").join("sha256");
    fs::create_dir_all(&agents_dir)?;
    fs::create_dir_all(&blobs_dir)?;

    let manifest_bytes = fs::read(repo_root.join(LOCAL_INDEX_DIR).join("manifest.json"))?;
    let repo_index_bytes = fs::read(repo_root.join(LOCAL_INDEX_DIR).join("repo-index.json"))?;
    fs::write(artifacts_dir.join("repo-manifest.json"), &manifest_bytes)?;
    fs::write(artifacts_dir.join("repo-index.json"), &repo_index_bytes)?;

    let generated = render_generated_files(repo_index);
    let mut files = vec![
        "repo-manifest.json".to_string(),
        "repo-index.json".to_string(),
    ];
    for file in &generated {
        fs::write(agents_dir.join(&file.file_name), &file.content)?;
        files.push(format!("agents/{}", file.file_name));
    }

    let reference = format!("ghcr.io/greenticai/indexes/{}:{}", repo_index.repo_id, tag);
    let metadata = PackageMetadata {
        repo_id: repo_index.repo_id.clone(),
        repo_name: repo_index.repo_name.clone(),
        tag: tag.to_string(),
        reference: reference.clone(),
        generated_at: repo_index.generated_at.clone(),
        compatibility: repo_index.version.clone(),
        files: files.clone(),
    };
    fs::write(
        artifacts_dir.join("package-metadata.json"),
        serde_json::to_string_pretty(&metadata).expect("package metadata should serialize"),
    )?;
    files.push("package-metadata.json".to_string());

    let mut manifests = Vec::new();
    for relative in &files {
        let bytes = fs::read(artifacts_dir.join(relative))?;
        let digest = digest_hex(&bytes);
        fs::write(blobs_dir.join(&digest), &bytes)?;
        manifests.push(serde_json::json!({
            "mediaType": media_type_for(relative),
            "digest": format!("sha256:{digest}"),
            "size": bytes.len(),
            "annotations": { "org.opencontainers.image.title": relative }
        }));
    }

    fs::write(
        package_dir.join("oci-layout"),
        serde_json::to_string_pretty(&serde_json::json!({
            "imageLayoutVersion": "1.0.0"
        }))
        .expect("oci layout should serialize"),
    )?;
    fs::write(
        package_dir.join("index.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "schemaVersion": 2,
            "manifests": manifests,
            "annotations": { "org.opencontainers.image.ref.name": reference }
        }))
        .expect("oci index should serialize"),
    )?;

    Ok(PackageResult {
        package_dir,
        reference,
        metadata,
    })
}

fn repo_root_from_cwd() -> Result<PathBuf, String> {
    let current =
        current_dir().map_err(|error| format!("failed to determine current directory: {error}"))?;
    find_repo_root(&current).ok_or_else(|| {
        format!(
            "failed to detect repository root from {}",
            current.display()
        )
    })
}

fn default_remote_store_path(home: &Path) -> PathBuf {
    home.join(".greentic-agent").join("remote-oci")
}

fn default_sync_cache_path(home: &Path) -> PathBuf {
    home.join(".greentic-agent").join("cache-oci")
}

fn default_indexes_path(home: &Path) -> PathBuf {
    home.join(".greentic-agent").join("indexes")
}

fn sync_state_path(home: &Path) -> PathBuf {
    home.join(".greentic-agent").join("sync-state.json")
}

fn merged_tantivy_path(home: &Path) -> PathBuf {
    home.join(".greentic-agent").join("tantivy").join("merged")
}

fn resolve_remote_config(options: SyncOptions<'_>) -> Result<RemoteConfig, String> {
    let backend = RemoteBackendKind::parse(options.backend)?;
    let tenant = options
        .tenant
        .map(ToString::to_string)
        .or_else(|| env::var("GREENTIC_AGENT_TENANT").ok());
    let public_catalog_ref = options
        .catalog
        .map(ToString::to_string)
        .or_else(|| env::var("GREENTIC_AGENT_CATALOG").ok())
        .unwrap_or_else(|| {
            options
                .channel
                .map(|channel| format!("ghcr.io/greenticai/indexes/catalog:{channel}"))
                .unwrap_or_else(|| DEFAULT_PUBLIC_CATALOG_REF.to_string())
        });
    let tenant_catalog_ref = options
        .tenant_catalog
        .map(ToString::to_string)
        .or_else(|| env::var("GREENTIC_AGENT_TENANT_CATALOG").ok())
        .or_else(|| {
            tenant
                .as_ref()
                .map(|tenant| default_tenant_catalog_ref(tenant))
        });
    let token = if let Some(token) = options.token {
        Some(token.to_string())
    } else if let Some(token_env) = options.token_env {
        env::var(token_env).ok()
    } else {
        env::var("GREENTIC_AGENT_TOKEN")
            .ok()
            .or_else(|| env::var("GHCR_TOKEN").ok())
    };

    Ok(RemoteConfig {
        backend,
        public_catalog_ref,
        tenant,
        tenant_catalog_ref,
        auth: token.map(|token| RegistryAuth {
            registry: "ghcr.io".to_string(),
            username: Some("greentic-agent".to_string()),
            token,
        }),
        strict: options.strict,
        public_only: options.public_only,
        private_only: options.private_only,
        include_private: options.include_private,
    })
}

fn default_tenant_catalog_ref(tenant: &str) -> String {
    format!("ghcr.io/greenticai/indexes/tenants/{tenant}/catalog:latest")
}

fn list_remote_repos(remote_root: &Path) -> Result<Vec<RemoteRepo>, std::io::Error> {
    if fs::read_dir(remote_root).is_err() {
        return Ok(Vec::new());
    };
    let mut repos = Vec::new();
    collect_remote_repos(remote_root, remote_root, &mut repos)?;
    repos.sort_by(|left, right| left.repo_id.cmp(&right.repo_id));
    Ok(repos)
}

fn build_catalog(remote_root: &Path) -> Result<Catalog, std::io::Error> {
    let repos = list_remote_repos(remote_root)?;
    let mut catalog_repos = Vec::new();
    for repo in repos {
        let Some(latest_tag) = repo.tags.last().cloned() else {
            continue;
        };
        let repo_index = load_repo_index_from_path(
            &remote_root
                .join(&repo.repo_id)
                .join(&latest_tag)
                .join("artifacts")
                .join("repo-index.json"),
        )?;
        catalog_repos.push(CatalogRepo {
            repo_id: repo.repo_id.clone(),
            repo_name: repo.repo_name,
            repo_role: repo_index.repo_role,
            latest_tag: latest_tag.clone(),
            package_ref: format!(
                "ghcr.io/greenticai/indexes/{}:{}",
                repo_index.repo_id, latest_tag
            ),
            updated_at: repo_index.generated_at.clone(),
            default_branch: repo_index
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.branch.clone()),
            preferred_branch: None,
            branches: catalog_branches_for_repo_index(&repo_index, &repo.repo_id, &latest_tag),
            visibility: IndexVisibility::Public,
            tenant: None,
            required_auth: None,
            digest: None,
            source_commit: None,
            enabled: true,
        });
    }
    catalog_repos.sort_by(|left, right| left.repo_id.cmp(&right.repo_id));
    Ok(Catalog {
        version: SCHEMA_VERSION_V1.to_string(),
        generated_at: timestamp_string(),
        catalog_id: None,
        default_channel: None,
        repos: catalog_repos,
        change_log: Vec::new(),
    })
}

fn rebuild_catalog_from_remote(
    remote_root: &Path,
    org: &str,
    channel: &str,
    tenant: Option<&str>,
) -> Result<Catalog, String> {
    let repos = list_remote_repos(remote_root).map_err(|error| error.to_string())?;
    let mut catalog_repos = Vec::new();
    for repo in repos
        .into_iter()
        .filter(|repo| repo.repo_id.starts_with(&format!("{org}/")))
    {
        let mut branch_entries = BTreeMap::new();
        let mut loaded = BTreeMap::<String, (String, RepoIndex, Option<String>)>::new();
        for tag in &repo.tags {
            let package_index_path = remote_root
                .join(repo_id_path(&repo.repo_id))
                .join(tag)
                .join("artifacts")
                .join("repo-index.json");
            if !package_index_path.exists() {
                continue;
            }
            let repo_index = load_repo_index_from_path(&package_index_path)
                .map_err(|error| error.to_string())?;
            let branch = repo_index
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.branch.clone())
                .unwrap_or_else(|| tag.clone());
            let digest = file_digest_hex(&package_index_path).ok();
            branch_entries.insert(
                branch.clone(),
                CatalogBranchEntry {
                    index_uri: format!("ghcr.io/greenticai/indexes/{}:{tag}", repo.repo_id),
                    commit_sha: repo_index
                        .metadata
                        .as_ref()
                        .and_then(|metadata| metadata.commit_sha.clone()),
                    updated_at: Some(repo_index.generated_at.clone()),
                    digest: digest.clone(),
                },
            );
            loaded.insert(branch, (tag.clone(), repo_index, digest));
        }
        if loaded.is_empty() {
            continue;
        }
        let selected_branch = if loaded.contains_key(channel) {
            channel.to_string()
        } else if loaded.contains_key("develop") {
            "develop".to_string()
        } else if loaded.contains_key("main") {
            "main".to_string()
        } else {
            loaded
                .keys()
                .next()
                .cloned()
                .unwrap_or_else(|| channel.to_string())
        };
        let Some((selected_tag, selected_index, selected_digest)) = loaded.get(&selected_branch)
        else {
            continue;
        };
        catalog_repos.push(CatalogRepo {
            repo_id: selected_index.repo_id.clone(),
            repo_name: selected_index.repo_name.clone(),
            repo_role: selected_index.repo_role,
            latest_tag: selected_tag.clone(),
            package_ref: format!(
                "ghcr.io/greenticai/indexes/{}:{}",
                selected_index.repo_id, selected_tag
            ),
            updated_at: selected_index.generated_at.clone(),
            default_branch: Some("main".to_string()).filter(|branch| loaded.contains_key(branch)),
            preferred_branch: Some(selected_branch),
            branches: branch_entries,
            visibility: if tenant.is_some() {
                IndexVisibility::Tenant
            } else {
                IndexVisibility::Public
            },
            tenant: tenant.map(ToString::to_string),
            required_auth: tenant.map(|_| AuthKind::GhcrToken),
            digest: selected_digest.clone(),
            source_commit: selected_index
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.commit_sha.clone()),
            enabled: true,
        });
    }
    catalog_repos.sort_by(|left, right| left.repo_id.cmp(&right.repo_id));
    Ok(Catalog {
        version: "gca.catalog.v2".to_string(),
        generated_at: timestamp_string(),
        catalog_id: Some(
            tenant
                .map(|tenant| format!("{org}/{tenant}"))
                .unwrap_or_else(|| format!("{org}/public")),
        ),
        default_channel: Some(channel.to_string()),
        repos: catalog_repos,
        change_log: Vec::new(),
    })
}

fn load_published_catalog(remote_root: &Path) -> Result<Option<Catalog>, std::io::Error> {
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
    normalize_catalog(&mut catalog);
    Ok(Some(catalog))
}

fn sync_catalog_with_state(
    remote_root: &Path,
    cache_root: &Path,
    indexes_root: &Path,
    home: &Path,
    options: SyncOptions<'_>,
) -> Result<SyncReport, String> {
    let catalog = match load_published_catalog(remote_root).map_err(|error| error.to_string())? {
        Some(catalog) => catalog,
        None => build_catalog(remote_root).map_err(|error| error.to_string())?,
    };
    let mut state = load_sync_state(home).unwrap_or_else(empty_sync_state);
    let mut report = SyncReport {
        channel: options.channel.map(ToString::to_string),
        public_catalog: Some(DEFAULT_PUBLIC_CATALOG_REF.to_string()),
        tenant_catalog: options.tenant.map(default_tenant_catalog_ref),
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

        let source = remote_root
            .join(repo_id_path(&repo.repo_id))
            .join(&repo.latest_tag);
        let legacy_target = cache_root
            .join(repo_id_path(&repo.repo_id))
            .join(&repo.latest_tag);
        let digest = repo
            .digest
            .clone()
            .or_else(|| file_digest_hex(&source.join("artifacts").join("repo-index.json")).ok());
        let unchanged = state.repos.iter().any(|entry| {
            entry.repo_id == repo.repo_id
                && entry.tenant == repo.tenant
                && entry.digest == digest
                && entry.source_commit == repo.source_commit
                && entry.local_index_path.join("repo-index.json").exists()
        });
        if unchanged {
            report.skipped.push(repo.repo_id.clone());
            continue;
        }

        if let Err(error) = copy_dir_all(&source, &legacy_target) {
            report.failed.push(SyncFailure {
                repo_id: repo.repo_id.clone(),
                error: error.to_string(),
            });
            continue;
        }
        match sync_cached_index_from_package(repo, &source, indexes_root, digest) {
            Ok(entry) => {
                upsert_synced_repo(&mut state, entry);
                report.downloaded.push(legacy_target.display().to_string());
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

fn sync_options_include_repo(options: SyncOptions<'_>, repo: &CatalogRepo) -> bool {
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
    if let Some(tenant) = options.tenant
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
    let repo_index =
        load_repo_index_from_path(&package_dir.join("artifacts").join("repo-index.json"))
            .map_err(|error| error.to_string())?;
    Ok(CatalogRepo {
        repo_id: repo_index.repo_id.clone(),
        repo_name: repo_index.repo_name.clone(),
        repo_role: repo_index.repo_role,
        latest_tag: tag.to_string(),
        package_ref: format!("ghcr.io/greenticai/indexes/{requested_repo}:{tag}"),
        updated_at: repo_index.generated_at.clone(),
        default_branch: repo_index
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.branch.clone()),
        preferred_branch: None,
        branches: catalog_branches_for_repo_index(&repo_index, requested_repo, tag),
        visibility: if tenant.is_some() {
            IndexVisibility::Tenant
        } else {
            IndexVisibility::Public
        },
        tenant: tenant.map(ToString::to_string),
        required_auth: tenant.map(|_| AuthKind::GhcrToken),
        digest: file_digest_hex(&package_dir.join("artifacts").join("repo-index.json")).ok(),
        source_commit: None,
        enabled: true,
    })
}

fn catalog_branches_for_repo_index(
    repo_index: &RepoIndex,
    requested_repo: &str,
    tag: &str,
) -> BTreeMap<String, CatalogBranchEntry> {
    let branch = repo_index
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.branch.clone())
        .unwrap_or_else(|| tag.to_string());
    let mut branches = BTreeMap::new();
    branches.insert(
        branch,
        CatalogBranchEntry {
            index_uri: format!("ghcr.io/greenticai/indexes/{requested_repo}:{tag}"),
            commit_sha: repo_index
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.commit_sha.clone()),
            updated_at: Some(repo_index.generated_at.clone()),
            digest: None,
        },
    );
    branches
}

fn sync_cached_index_from_package(
    repo: &CatalogRepo,
    package_dir: &Path,
    indexes_root: &Path,
    digest: Option<String>,
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

    let repo_index = load_repo_index_from_path(&target.join("repo-index.json"))
        .map_err(|error| error.to_string())?;
    let tantivy_path = target.join("tantivy");
    build_tantivy_index_for_repo(
        &repo_index,
        &tantivy_path,
        repo.tenant.as_deref().unwrap_or_default(),
        repo.visibility,
        &repo.package_ref,
    )?;
    Ok(SyncedRepoState {
        repo_id: repo.repo_id.clone(),
        tenant: repo.tenant.clone(),
        visibility: repo.visibility,
        package_ref: repo.package_ref.clone(),
        digest,
        source_commit: repo.source_commit.clone(),
        downloaded_at: timestamp_string(),
        local_index_path: target,
        local_tantivy_path: Some(tantivy_path),
    })
}

fn local_index_path_for(indexes_root: &Path, repo: &CatalogRepo) -> PathBuf {
    match repo.visibility {
        IndexVisibility::Tenant | IndexVisibility::Private => indexes_root
            .join("tenants")
            .join(repo.tenant.as_deref().unwrap_or("default"))
            .join(repo_id_path(&repo.repo_id)),
        IndexVisibility::Public => indexes_root
            .join("public")
            .join(repo_id_path(&repo.repo_id)),
    }
}

fn empty_sync_state() -> SyncState {
    SyncState {
        version: SCHEMA_VERSION_V1.to_string(),
        updated_at: timestamp_string(),
        repos: Vec::new(),
    }
}

fn load_sync_state(home: &Path) -> Option<SyncState> {
    let raw = fs::read_to_string(sync_state_path(home)).ok()?;
    serde_json::from_str(&raw).ok()
}

fn write_sync_state(home: &Path, state: &SyncState) -> Result<(), String> {
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
    });
    let raw = serde_json::to_string_pretty(&state).expect("sync state should serialize as json");
    fs::write(&path, format!("{raw}\n"))
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn catalog_fingerprint(home: &Path) -> String {
    let mut hasher = Sha256::new();
    for path in [
        editable_catalog_path(None),
        home.join(".greentic-agent")
            .join("remote-oci")
            .join("catalogs")
            .join("public")
            .join("catalog.json"),
    ] {
        if let Ok(bytes) = fs::read(path) {
            hasher.update(bytes);
        }
    }
    hex_encode(hasher.finalize())
}

fn upsert_synced_repo(state: &mut SyncState, entry: SyncedRepoState) {
    state
        .repos
        .retain(|repo| repo.repo_id != entry.repo_id || repo.tenant != entry.tenant);
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

fn rebuild_merged_index(home: &Path, tenant: Option<&str>) -> Result<MergedIndexReport, String> {
    let cached = load_cached_repo_indexes(home, tenant)?;
    let merged_path = merged_tantivy_path(home);
    let next_path = home
        .join(".greentic-agent")
        .join("tantivy")
        .join("merged.next");
    let previous_path = home
        .join(".greentic-agent")
        .join("tantivy")
        .join("merged.previous");
    let build = build_merged_tantivy_index(&cached, &next_path)?;
    open_tantivy_index(&next_path)?;
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

fn open_tantivy_index(index_dir: &Path) -> Result<(), String> {
    tantivy::Index::open_in_dir(index_dir)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn load_cached_repo_indexes(
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
        let repo_index = load_repo_index_from_path(&path).map_err(|error| error.to_string())?;
        cached.push(CachedRepoIndex {
            repo_index,
            state: entry,
        });
    }
    cached.sort_by(|left, right| left.state.repo_id.cmp(&right.state.repo_id));
    Ok(cached)
}

fn recover_sync_state_from_cache(home: &Path) -> SyncState {
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

fn recover_cached_indexes_under(
    root: &Path,
    tenant: Option<String>,
    visibility: IndexVisibility,
    state: &mut SyncState,
) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for org_entry in entries.flatten() {
        let org_path = org_entry.path();
        if !org_path.is_dir() {
            continue;
        }
        let Ok(repo_entries) = fs::read_dir(&org_path) else {
            continue;
        };
        for repo_entry in repo_entries.flatten() {
            let repo_path = repo_entry.path();
            let repo_index_path = repo_path.join("repo-index.json");
            if !repo_index_path.exists() {
                continue;
            }
            let Ok(repo_index) = load_repo_index_from_path(&repo_index_path) else {
                continue;
            };
            let digest = file_digest_hex(&repo_index_path).ok();
            state.repos.push(SyncedRepoState {
                repo_id: repo_index.repo_id.clone(),
                tenant: tenant.clone(),
                visibility,
                package_ref: format!("ghcr.io/greenticai/indexes/{}:latest", repo_index.repo_id),
                digest,
                source_commit: None,
                downloaded_at: timestamp_string(),
                local_index_path: repo_path.clone(),
                local_tantivy_path: Some(repo_path.join("tantivy")),
            });
        }
    }
}

fn check_refresh(repo_root: &Path) -> Result<RefreshCheck, std::io::Error> {
    let current_head_sha = read_head_sha(repo_root).unwrap_or_else(|| "unknown".to_string());
    let current_tracked_files = find_tracked_files(repo_root);
    let fingerprints_path = repo_root.join(LOCAL_INDEX_DIR).join("fingerprints.json");
    let repo_index_path = repo_root.join(LOCAL_INDEX_DIR).join("repo-index.json");
    let indexed_fingerprints = load_optional_fingerprints(&fingerprints_path)?;
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
    if !repo_index_path.exists() {
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
    if indexed_generator_version.as_deref() != Some(env!("CARGO_PKG_VERSION")) {
        reasons.push(format!(
            "generator version changed: indexed={}, current={}",
            indexed_generator_version
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            env!("CARGO_PKG_VERSION")
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
        current_generator_version: env!("CARGO_PKG_VERSION").to_string(),
        indexed_generator_version,
        current_schema_version: SCHEMA_VERSION_V1.to_string(),
        indexed_schema_version,
    })
}

fn render_installed_github_workflow(options: WorkflowInstallOptions<'_>) -> String {
    let _publish_ghcr = options.publish_ghcr;
    let tenant_env = options
        .tenant
        .map(|tenant| {
            format!(
                r#"      GREENTIC_AGENT_TENANT: {tenant}
"#
            )
        })
        .unwrap_or_default();
    let token_expr = if options.tenant.is_some() {
        "${{ secrets.TENANT_GHCR_TOKEN || secrets.GITHUB_TOKEN }}"
    } else {
        "${{ secrets.GITHUB_TOKEN }}"
    };
    format!(
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
{tenant_env}      GHCR_TOKEN: {token_expr}
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
        run: ./target/release/greentic-coding-agent package-index --tag "${{{{ github.ref_name }}}}" --tag "sha-${{{{ github.sha }}}}" --format json | tee .greentic-agent-package.json

      - name: Publish index to GHCR when refresh is needed
        shell: bash
        run: |
          if ./target/release/greentic-coding-agent check-refresh --format json | grep -q '"needs_refresh": true'; then
            ./target/release/greentic-coding-agent publish-index --tag "${{{{ github.ref_name }}}}" --tag "sha-${{{{ github.sha }}}}" --backend ghcr --token-env GHCR_TOKEN --format json | tee .greentic-agent-publish.json
          else
            echo '{{"published": false, "reason": "refresh not required"}}' | tee .greentic-agent-publish.json
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
    )
}

fn render_installed_catalog_workflow(kind: &str, tenant: Option<&str>) -> String {
    let tenant_flag = tenant
        .map(|tenant| format!(" --tenant {tenant}"))
        .unwrap_or_default();
    let tenant_env = tenant
        .map(|tenant| {
            format!(
                r#"      GREENTIC_AGENT_TENANT: {tenant}
      GHCR_TOKEN: ${{{{ secrets.TENANT_GHCR_TOKEN || secrets.GITHUB_TOKEN }}}}
"#
            )
        })
        .unwrap_or_else(|| {
            r#"      GHCR_TOKEN: ${{ secrets.GITHUB_TOKEN }}
"#
            .to_string()
        });
    format!(
        r#"name: Greentic Agent Catalog

on:
  push:
    branches: [main, develop]
    paths:
      - ".greentic-agent/catalogs/**"
      - ".github/workflows/greentic-agent-catalog.yml"
  schedule:
    - cron: "31 2 * * *"
  workflow_dispatch:

permissions:
  contents: read
  packages: write

jobs:
  catalog:
    runs-on: ubuntu-latest
    env:
{tenant_env}    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Install ORAS
        uses: oras-project/setup-oras@v1

      - name: Build greentic-coding-agent
        run: cargo build --release --package greentic-coding-agent

      - name: Validate {kind} catalog
        run: ./target/release/greentic-coding-agent catalog validate{tenant_flag} --format json | tee .greentic-agent-catalog-validate.json

      - name: Publish {kind} catalog to GHCR
        run: ./target/release/greentic-coding-agent catalog publish{tenant_flag} --channel "${{{{ github.ref_name }}}}" --backend ghcr --token-env GHCR_TOKEN --format json | tee .greentic-agent-catalog-publish.json

      - name: Upload catalog summaries
        uses: actions/upload-artifact@v4
        with:
          name: greentic-agent-catalog-summary
          path: |
            .greentic-agent-catalog-validate.json
            .greentic-agent-catalog-publish.json
"#
    )
}

fn copy_dir_all(source: &Path, target: &Path) -> Result<(), std::io::Error> {
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

fn oras_pull(reference: &str, out_dir: &Path, auth: Option<&RegistryAuth>) -> Result<(), String> {
    if let Some(auth) = auth {
        oras_login(
            &auth.registry,
            auth.username.as_deref().unwrap_or("greentic-agent"),
            &auth.token,
        )?;
    }
    fs::create_dir_all(out_dir)
        .map_err(|error| format!("failed to create {}: {error}", out_dir.display()))?;
    run_oras(
        ProcessCommand::new("oras")
            .arg("pull")
            .arg(reference)
            .arg("-o")
            .arg(out_dir),
        "",
    )
}

fn oras_push(reference: &str, dir: &Path, auth: Option<&RegistryAuth>) -> Result<(), String> {
    if let Some(auth) = auth {
        oras_login(
            &auth.registry,
            auth.username.as_deref().unwrap_or("greentic-agent"),
            &auth.token,
        )?;
    }
    run_oras(
        ProcessCommand::new("oras")
            .arg("push")
            .arg(reference)
            .arg(dir),
        "",
    )
}

fn oras_login(registry: &str, username: &str, token: &str) -> Result<(), String> {
    let mut child = ProcessCommand::new("oras")
        .arg("login")
        .arg(registry)
        .arg("-u")
        .arg(username)
        .arg("--password-stdin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(map_oras_spawn_error)?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin
            .write_all(token.as_bytes())
            .map_err(|error| error.to_string())?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "oras command failed: {}",
            redacted_output(&output.stderr, token)
        ))
    }
}

fn run_oras(command: &mut ProcessCommand, token: &str) -> Result<(), String> {
    let output = command.output().map_err(map_oras_spawn_error)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "oras command failed: {}",
            redacted_output(&output.stderr, token)
        ))
    }
}

fn map_oras_spawn_error(error: std::io::Error) -> String {
    if error.kind() == std::io::ErrorKind::NotFound {
        "oras is required for GHCR sync. Install it with: brew install oras".to_string()
    } else {
        format!("failed to run oras: {error}")
    }
}

fn redacted_output(raw: &[u8], token: &str) -> String {
    let text = String::from_utf8_lossy(raw).to_string();
    if token.is_empty() {
        text
    } else {
        text.replace(token, "[redacted]")
    }
}

fn repo_id_path(repo_id: &str) -> PathBuf {
    repo_id.split('/').collect()
}

fn collect_remote_repos(
    remote_root: &Path,
    current: &Path,
    repos: &mut Vec<RemoteRepo>,
) -> Result<(), std::io::Error> {
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

#[derive(Debug, Clone)]
struct TantivyIndexDocument {
    path: String,
    kind: String,
    title: String,
    body: String,
    concept_ids: String,
    phase: String,
    provenance: String,
}

fn build_local_tantivy_index(
    repo_index: &RepoIndex,
    index_dir: &Path,
) -> Result<TantivyBuildReport, String> {
    build_tantivy_index_for_repo(repo_index, index_dir, "", IndexVisibility::Public, "")
}

fn build_merged_tantivy_index(
    cached: &[CachedRepoIndex],
    index_dir: &Path,
) -> Result<TantivyBuildReport, String> {
    build_tantivy_index_from_sources(
        index_dir,
        cached.iter().map(|entry| {
            (
                &entry.repo_index,
                entry.state.tenant.as_deref().unwrap_or_default(),
                entry.state.visibility,
                entry.state.package_ref.as_str(),
            )
        }),
    )
}

fn build_tantivy_index_for_repo(
    repo_index: &RepoIndex,
    index_dir: &Path,
    tenant: &str,
    visibility: IndexVisibility,
    source_package_ref: &str,
) -> Result<TantivyBuildReport, String> {
    build_tantivy_index_from_sources(
        index_dir,
        std::iter::once((repo_index, tenant, visibility, source_package_ref)),
    )
}

fn build_tantivy_index_from_sources<'a>(
    index_dir: &Path,
    sources: impl IntoIterator<Item = (&'a RepoIndex, &'a str, IndexVisibility, &'a str)>,
) -> Result<TantivyBuildReport, String> {
    use tantivy::doc;
    use tantivy::schema::{STORED, STRING, Schema, TEXT};

    if index_dir.exists() {
        fs::remove_dir_all(index_dir)
            .map_err(|error| format!("failed to remove {}: {error}", index_dir.display()))?;
    }
    fs::create_dir_all(index_dir)
        .map_err(|error| format!("failed to create {}: {error}", index_dir.display()))?;

    let mut builder = Schema::builder();
    let repo_id = builder.add_text_field("repo_id", STRING | STORED);
    let tenant = builder.add_text_field("tenant", STRING | STORED);
    let visibility = builder.add_text_field("visibility", STRING | STORED);
    let source_package_ref = builder.add_text_field("source_package_ref", STRING | STORED);
    let path = builder.add_text_field("path", STRING | STORED);
    let kind = builder.add_text_field("kind", STRING | STORED);
    let title = builder.add_text_field("title", TEXT | STORED);
    let body = builder.add_text_field("body", TEXT | STORED);
    let concept_ids = builder.add_text_field("concept_ids", TEXT | STORED);
    let phase = builder.add_text_field("phase", STRING | STORED);
    let provenance = builder.add_text_field("provenance", STRING | STORED);
    let schema = builder.build();

    let index =
        tantivy::Index::create_in_dir(index_dir, schema).map_err(|error| error.to_string())?;
    let mut writer = index
        .writer(50_000_000)
        .map_err(|error| error.to_string())?;
    let mut documents_indexed = 0;
    for (repo_index, tenant_value, visibility_value, source_package_ref_value) in sources {
        let documents = collect_tantivy_documents(repo_index);
        for document in &documents {
            writer
                .add_document(doc!(
                    repo_id => repo_index.repo_id.clone(),
                    tenant => tenant_value.to_string(),
                    visibility => visibility_value.as_str(),
                    source_package_ref => source_package_ref_value.to_string(),
                    path => document.path.clone(),
                    kind => document.kind.clone(),
                    title => document.title.clone(),
                    body => document.body.clone(),
                    concept_ids => document.concept_ids.clone(),
                    phase => document.phase.clone(),
                    provenance => document.provenance.clone(),
                ))
                .map_err(|error| error.to_string())?;
        }
        documents_indexed += documents.len();
    }
    writer.commit().map_err(|error| error.to_string())?;

    Ok(TantivyBuildReport {
        index_path: index_dir.to_path_buf(),
        documents_indexed,
    })
}

fn collect_tantivy_documents(repo_index: &RepoIndex) -> Vec<TantivyIndexDocument> {
    let mut documents = Vec::new();
    for concept in &repo_index.concept_graph {
        documents.push(TantivyIndexDocument {
            path: concept.related_paths.first().cloned().unwrap_or_default(),
            kind: "concept".to_string(),
            title: concept.title.clone(),
            body: concept.summary.clone(),
            concept_ids: concept.id.clone(),
            phase: phase_label(&concept.lifecycle_phase).to_string(),
            provenance: format!("concept_graph:{}", concept.id),
        });
    }
    for workflow in &repo_index.workflow_graph {
        documents.push(TantivyIndexDocument {
            path: workflow.docs.first().cloned().unwrap_or_default(),
            kind: "workflow".to_string(),
            title: workflow.title.clone(),
            body: format!("{} {}", workflow.summary, workflow.commands.join(" ")),
            concept_ids: workflow.concept_ids.join(" "),
            phase: phase_label(&workflow.phase).to_string(),
            provenance: format!("workflow_graph:{}", workflow.id),
        });
    }
    for instruction in &repo_index.instruction_graph {
        let document_kind = match instruction.kind.as_str() {
            "training_course" => "course",
            "knowledge_update" => "update",
            _ => "instruction",
        };
        documents.push(TantivyIndexDocument {
            path: instruction.path.clone(),
            kind: document_kind.to_string(),
            title: instruction.title.clone(),
            body: format!(
                "{} {} {}",
                instruction.kind,
                instruction.headings.join(" "),
                instruction.commands.join(" ")
            ),
            concept_ids: instruction.concept_ids.join(" "),
            phase: String::new(),
            provenance: format!("instruction_graph:{}", instruction.kind),
        });
    }
    for validation in &repo_index.validations {
        documents.push(TantivyIndexDocument {
            path: String::new(),
            kind: "validation".to_string(),
            title: validation.title.clone(),
            body: format!(
                "{} {} {}",
                validation.summary,
                validation.command_groups.join(" "),
                validation.triggered_by.join(" ")
            ),
            concept_ids: String::new(),
            phase: phase_label(&validation.phase).to_string(),
            provenance: format!("validations:{}", validation.id),
        });
    }
    for reuse in &repo_index.reuse {
        documents.push(TantivyIndexDocument {
            path: reuse.concept_id.clone(),
            kind: "reuse".to_string(),
            title: format!("{} owned by {}", reuse.concept_id, reuse.owner_repo),
            body: format!(
                "{} {} {}",
                reuse.rationale,
                reuse.forbidden_locations.join(" "),
                reuse.required_validations.join(" ")
            ),
            concept_ids: reuse.concept_id.clone(),
            phase: String::new(),
            provenance: format!("reuse_policy:{}", reuse.id),
        });
    }
    for update in &repo_index.knowledge_updates {
        documents.push(TantivyIndexDocument {
            path: update.source_paths.first().cloned().unwrap_or_default(),
            kind: "instruction".to_string(),
            title: update.title.clone(),
            body: format!(
                "{} {} {} {} {} {} {}",
                update.summary,
                update.agent_instruction,
                local_update_type_label(update.update_type),
                local_update_severity_label(update.severity),
                update.affected_workflows.join(" "),
                update.affected_courses.join(" "),
                update
                    .migration_steps
                    .iter()
                    .filter_map(|step| step.command.clone())
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            concept_ids: update.affected_concepts.join(" "),
            phase: String::new(),
            provenance: format!("knowledge_updates:{}", update.id),
        });
    }
    for module in &repo_index.source_stats.modules {
        documents.push(simple_tantivy_document(
            "module",
            module,
            "source_stats.modules",
        ));
    }
    for item in &repo_index.source_stats.public_items {
        documents.push(simple_tantivy_document(
            "code_symbol",
            item,
            "source_stats.public_items",
        ));
    }
    for dependency in &repo_index.source_stats.dependencies {
        documents.push(simple_tantivy_document(
            "dependency",
            dependency,
            "source_stats.dependencies",
        ));
    }
    for path in &repo_index.source_stats.markdown_docs {
        documents.push(simple_tantivy_document(
            "instruction",
            path,
            "source_stats.markdown_docs",
        ));
    }
    for path in &repo_index.source_stats.workflow_files {
        documents.push(simple_tantivy_document(
            "workflow",
            path,
            "source_stats.workflow_files",
        ));
    }
    for path in &repo_index.source_stats.example_paths {
        documents.push(simple_tantivy_document(
            "summary",
            path,
            "source_stats.example_paths",
        ));
    }
    documents
}

fn simple_tantivy_document(kind: &str, value: &str, provenance: &str) -> TantivyIndexDocument {
    TantivyIndexDocument {
        path: value.to_string(),
        kind: kind.to_string(),
        title: value.to_string(),
        body: value.to_string(),
        concept_ids: String::new(),
        phase: String::new(),
        provenance: provenance.to_string(),
    }
}

fn phase_label(phase: &LifecyclePhase) -> &'static str {
    match phase {
        LifecyclePhase::Design => "design",
        LifecyclePhase::Build => "build",
        LifecyclePhase::Setup => "setup",
        LifecyclePhase::Start => "start",
        LifecyclePhase::Runtime => "runtime",
    }
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

fn load_policy_bundle(repo_root: &Path) -> (Vec<ValidationDescriptor>, Vec<ReuseDescriptor>) {
    let (mut validations, mut reuse) = built_in_policy_bundle();
    reuse.extend(load_json_descriptors::<ReuseDescriptor>(
        &repo_root.join(".codex").join("policy").join("reuse"),
    ));
    reuse.extend(load_json_descriptors::<ReuseDescriptor>(
        &repo_root
            .join(".greentic-agent")
            .join("policy")
            .join("reuse"),
    ));
    validations.extend(load_json_descriptors::<ValidationDescriptor>(
        &repo_root.join(".codex").join("policy").join("validations"),
    ));
    validations.extend(load_json_descriptors::<ValidationDescriptor>(
        &repo_root
            .join(".greentic-agent")
            .join("policy")
            .join("validations"),
    ));
    validations.sort_by(|left, right| left.id.cmp(&right.id));
    validations.dedup_by(|left, right| left.id == right.id);
    reuse.sort_by(|left, right| left.id.cmp(&right.id));
    reuse.dedup_by(|left, right| left.id == right.id);
    (validations, reuse)
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

fn search_repo_index(repo_index: &RepoIndex, mode: SearchMode, query: &str) -> SearchResponse {
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

fn search_repo_index_with_engine(
    repo_index: &RepoIndex,
    tantivy_index_dir: Option<&Path>,
    mode: SearchMode,
    query: &str,
    engine: SearchEngineChoice,
) -> Result<SearchResponse, String> {
    match engine {
        SearchEngineChoice::Fallback => Ok(search_repo_index(repo_index, mode, query)),
        SearchEngineChoice::Tantivy => {
            let Some(index_dir) = tantivy_index_dir else {
                return Err("tantivy index path was not provided".to_string());
            };
            search_tantivy_index(index_dir, mode, query)
        }
        SearchEngineChoice::Auto => {
            if let Some(index_dir) = tantivy_index_dir
                && index_dir.exists()
                && let Ok(response) = search_tantivy_index(index_dir, mode, query)
            {
                return Ok(response);
            }
            Ok(search_repo_index(repo_index, mode, query))
        }
    }
}

fn search_tantivy_index(
    index_dir: &Path,
    mode: SearchMode,
    query: &str,
) -> Result<SearchResponse, String> {
    search_tantivy_index_filtered(index_dir, mode, query, None, None)
}

fn search_tantivy_index_filtered(
    index_dir: &Path,
    mode: SearchMode,
    query: &str,
    repo_filter: Option<&str>,
    tenant_filter: Option<&str>,
) -> Result<SearchResponse, String> {
    use tantivy::collector::TopDocs;
    use tantivy::query::QueryParser;
    use tantivy::schema::Value;

    let index = tantivy::Index::open_in_dir(index_dir).map_err(|error| error.to_string())?;
    let schema = index.schema();
    let title = schema
        .get_field("title")
        .map_err(|error| error.to_string())?;
    let body = schema
        .get_field("body")
        .map_err(|error| error.to_string())?;
    let concept_ids = schema
        .get_field("concept_ids")
        .map_err(|error| error.to_string())?;
    let path = schema
        .get_field("path")
        .map_err(|error| error.to_string())?;
    let kind = schema
        .get_field("kind")
        .map_err(|error| error.to_string())?;
    let repo_id = schema
        .get_field("repo_id")
        .map_err(|error| error.to_string())?;
    let tenant = schema.get_field("tenant").ok();
    let provenance = schema
        .get_field("provenance")
        .map_err(|error| error.to_string())?;

    let reader = index.reader().map_err(|error| error.to_string())?;
    let searcher = reader.searcher();
    let parser = QueryParser::for_index(&index, vec![title, body, concept_ids, path]);
    let parsed = parser
        .parse_query(query)
        .map_err(|error| error.to_string())?;
    let top_docs = searcher
        .search(&parsed, &TopDocs::with_limit(50).order_by_score())
        .map_err(|error| error.to_string())?;

    let mut results = Vec::new();
    for (_score, address) in top_docs {
        let document: tantivy::TantivyDocument =
            searcher.doc(address).map_err(|error| error.to_string())?;
        let document_kind = document
            .get_first(kind)
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        if !tantivy_kind_matches(mode, document_kind) {
            continue;
        }
        let document_repo_id = tantivy_text(&document, repo_id).unwrap_or_default();
        if let Some(repo_filter) = repo_filter
            && document_repo_id != repo_filter
        {
            continue;
        }
        if let (Some(tenant_filter), Some(tenant_field)) = (tenant_filter, tenant)
            && tantivy_text(&document, tenant_field).unwrap_or_default() != tenant_filter
        {
            continue;
        }
        let title_value = tantivy_text(&document, title).unwrap_or_default();
        let locator = tantivy_text(&document, path).unwrap_or_default();
        let id = format!(
            "{}:{}",
            document_kind,
            if locator.is_empty() {
                title_value.clone()
            } else {
                locator.clone()
            }
        );
        results.push(SearchResult {
            repo_id: document_repo_id,
            id,
            title: title_value.clone(),
            result_type: result_type_for_tantivy_kind(document_kind),
            locator,
            snippet: tantivy_text(&document, body).unwrap_or(title_value),
            provenance: tantivy_text(&document, provenance).unwrap_or_default(),
            freshness: FreshnessStatus::Fresh,
        });
    }

    results.sort_by(|left, right| left.id.cmp(&right.id));
    results.truncate(20);
    Ok(SearchResponse {
        mode,
        query: query.trim().to_string(),
        results,
    })
}

fn tantivy_text(
    document: &tantivy::TantivyDocument,
    field: tantivy::schema::Field,
) -> Option<String> {
    use tantivy::schema::Value;

    document
        .get_first(field)
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
}

fn tantivy_kind_matches(mode: SearchMode, kind: &str) -> bool {
    match mode {
        SearchMode::Code => matches!(kind, "code_symbol" | "module" | "dependency"),
        SearchMode::Instruction => kind == "instruction",
        SearchMode::Concept => kind == "concept",
        SearchMode::Reuse => kind == "reuse",
        SearchMode::Course => kind == "course",
        SearchMode::Update => kind == "update",
    }
}

fn result_type_for_tantivy_kind(kind: &str) -> SearchResultType {
    match kind {
        "instruction" => SearchResultType::Instruction,
        "concept" => SearchResultType::Concept,
        "reuse" => SearchResultType::Reuse,
        "course" => SearchResultType::Course,
        "update" => SearchResultType::Update,
        _ => SearchResultType::Code,
    }
}

fn locate_owner(reuse: &[ReuseDescriptor], concept_id: &str) -> Option<OwnerLookup> {
    reuse
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

fn required_validations(repo_index: &RepoIndex, task: &str) -> RequiredValidationsResponse {
    let lower = task.to_ascii_lowercase();
    let mut validations = repo_index
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

    for reuse in &repo_index.reuse {
        if lower.contains(&reuse.concept_id.to_ascii_lowercase())
            || lower.contains(&reuse.concept_id.replace('_', " "))
        {
            for validation_id in &reuse.required_validations {
                if let Some(validation) = repo_index
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

fn impact_analysis(
    repo_index: &RepoIndex,
    symbol: &str,
    refresh: Option<&RefreshCheck>,
) -> ImpactAnalysis {
    let query = symbol.trim();
    let lower = query.to_ascii_lowercase();
    let mut concepts = Vec::new();
    let mut workflows = Vec::new();
    let mut validations = Vec::new();
    let mut owner_repos = Vec::new();
    let mut provenance = Vec::new();

    for concept in &repo_index.concept_graph {
        let exact =
            concept.id.eq_ignore_ascii_case(query) || concept.title.eq_ignore_ascii_case(query);
        let fuzzy = concept.summary.to_ascii_lowercase().contains(&lower)
            || concept.id.to_ascii_lowercase().contains(&lower)
            || concept.title.to_ascii_lowercase().contains(&lower);
        if exact || fuzzy {
            concepts.push(concept.id.clone());
            provenance.push(format!("concept:{}", concept.id));
            if let Some(owner) = locate_owner(&repo_index.reuse, &concept.id) {
                owner_repos.push(owner.owner_repo);
            }
        }
    }

    for workflow in &repo_index.workflow_graph {
        if workflow.id.to_ascii_lowercase().contains(&lower)
            || workflow.title.to_ascii_lowercase().contains(&lower)
            || workflow.summary.to_ascii_lowercase().contains(&lower)
            || workflow
                .commands
                .iter()
                .any(|command| command.to_ascii_lowercase().contains(&lower))
        {
            workflows.push(workflow.id.clone());
            provenance.push(format!("workflow:{}", workflow.id));
        }
    }

    for validation in &repo_index.validations {
        if validation.id.to_ascii_lowercase().contains(&lower)
            || validation.title.to_ascii_lowercase().contains(&lower)
            || validation.summary.to_ascii_lowercase().contains(&lower)
        {
            validations.push(validation.id.clone());
            provenance.push(format!("validation:{}", validation.id));
        }
    }

    for item in &repo_index.source_stats.public_items {
        if item.to_ascii_lowercase().contains(&lower) {
            provenance.push(format!("public_item:{item}"));
        }
    }
    for item in &repo_index.source_stats.modules {
        if item.to_ascii_lowercase().contains(&lower) {
            provenance.push(format!("module:{item}"));
        }
    }

    concepts.sort();
    concepts.dedup();
    workflows.sort();
    workflows.dedup();
    validations.sort();
    validations.dedup();
    owner_repos.sort();
    owner_repos.dedup();
    provenance.sort();
    provenance.dedup();

    let confidence = if concepts
        .iter()
        .any(|entry| entry.eq_ignore_ascii_case(query))
        || workflows
            .iter()
            .any(|entry| entry.eq_ignore_ascii_case(query))
    {
        "high"
    } else if !provenance.is_empty() {
        "medium"
    } else {
        "low"
    };

    ImpactAnalysis {
        symbol: query.to_string(),
        confidence: confidence.to_string(),
        provenance,
        concepts,
        workflows,
        validations,
        owner_repos,
        freshness_warning: freshness_warning(refresh),
    }
}

fn detect_changes(
    repo_root: &Path,
    repo_index: &RepoIndex,
) -> Result<ChangeDetection, std::io::Error> {
    let refresh = check_refresh(repo_root)?;
    let current_files = find_tracked_files(repo_root);
    let indexed_files =
        load_optional_fingerprints(&repo_root.join(LOCAL_INDEX_DIR).join("fingerprints.json"))?
            .map(|fingerprints| fingerprints.tracked_files)
            .unwrap_or_default();
    let mut changed_files = current_files
        .iter()
        .filter(|path| !indexed_files.contains(*path))
        .cloned()
        .collect::<Vec<_>>();
    changed_files.extend(
        indexed_files
            .iter()
            .filter(|path| !current_files.contains(*path))
            .cloned(),
    );
    changed_files.sort();
    changed_files.dedup();

    Ok(ChangeDetection {
        changed_files: changed_files.clone(),
        likely_concepts: infer_changed_concepts(repo_index, &changed_files),
        likely_workflows: infer_changed_workflows(repo_index, &changed_files),
        suggested_validations: infer_changed_validations(repo_index, &changed_files),
        freshness_warning: freshness_warning(Some(&refresh)),
    })
}

fn validate_plan_file(
    repo_root: &Path,
    repo_index: &RepoIndex,
    plan_path: &Path,
) -> Result<PlanValidation, std::io::Error> {
    let raw = fs::read_to_string(plan_path)?;
    let task_summary = extract_plan_summary(&raw);
    let refresh = check_refresh(repo_root)?;
    let mut owner_hints = Vec::new();
    for concept in &repo_index.concept_graph {
        let concept_phrase = concept.id.replace('_', " ");
        let lower = task_summary.to_ascii_lowercase();
        if (lower.contains(&concept.id.to_ascii_lowercase()) || lower.contains(&concept_phrase))
            && let Some(owner) = locate_owner(&repo_index.reuse, &concept.id)
        {
            owner_hints.push(owner);
        }
    }
    owner_hints.sort_by(|left, right| left.concept_id.cmp(&right.concept_id));
    owner_hints.dedup_by(|left, right| left.concept_id == right.concept_id);

    let validations = required_validations(repo_index, &task_summary).validations;
    let mut issues = Vec::new();
    if task_summary.trim().is_empty() {
        issues.push("plan file did not contain any extractable task text".to_string());
    }
    if refresh.needs_refresh {
        issues.push("local index appears stale relative to the current checkout".to_string());
    }

    Ok(PlanValidation {
        plan_path: plan_path.display().to_string(),
        task_summary,
        owner_hints,
        required_validations: validations,
        freshness_warning: freshness_warning(Some(&refresh)),
        issues,
    })
}

fn mcp_server_snapshot(refresh: Option<&RefreshCheck>) -> McpServerSnapshot {
    McpServerSnapshot {
        protocol: "mcp-lite-v1".to_string(),
        tools: vec![
            mcp_tool("describe_repo", "Summarize the indexed repository state."),
            mcp_tool(
                "list_workflows",
                "List inferred workflows for the repository.",
            ),
            mcp_tool(
                "explain_concept",
                "Show details and ownership for a concept.",
            ),
            mcp_tool("search_code", "Search indexed code metadata."),
            mcp_tool(
                "search_instructions",
                "Search indexed docs and instructions.",
            ),
            mcp_tool("search_reuse", "Search seeded reuse and ownership policy."),
            mcp_tool(
                "search_all",
                "Search local and merged indexes across all modes.",
            ),
            mcp_tool("locate_owner", "Find the owner repo for a concept."),
            mcp_tool(
                "locate_extension_point",
                "Find likely extension points for a concept or task.",
            ),
            mcp_tool(
                "plan_change",
                "Validate a proposed plan against repo policy.",
            ),
            mcp_tool(
                "required_validations",
                "List validations implied by a task.",
            ),
            mcp_tool(
                "impact_analysis",
                "Estimate blast radius for a symbol or concept.",
            ),
            mcp_tool(
                "detect_changes",
                "Summarize changed files and likely affected areas.",
            ),
            mcp_tool("show_freshness", "Report whether the local index is stale."),
            mcp_tool(
                "list_indexed_repos",
                "List repos currently available in the local index cache.",
            ),
            mcp_tool(
                "sync_indexes",
                "Refresh merged index state from cached indexes.",
            ),
            mcp_tool(
                "show_catalog",
                "Show the configured public or tenant catalog.",
            ),
            mcp_tool("gca.search", "Search global merged and local indexes."),
            mcp_tool("gca.agent_context", "Build task context for an agent."),
            mcp_tool("gca.find_owner", "Find owner candidates for a concept."),
            mcp_tool(
                "gca.required_validations",
                "List validations implied by a task.",
            ),
            mcp_tool("gca.recent_updates", "Find recent updates for a task."),
            mcp_tool("gca.branch_status", "Report synced branch/channel status."),
        ],
        resources: vec![
            "greentic://repo/current/manifest".to_string(),
            "greentic://repo/current/index".to_string(),
            "greentic://catalog/public".to_string(),
            "greentic://catalog/tenant/<tenant>".to_string(),
            "greentic://indexes/merged/status".to_string(),
        ],
        freshness_warning: freshness_warning(refresh),
    }
}

fn dispatch_mcp_request(service: &QueryService, request: McpRequest) -> McpResponse {
    let McpRequest {
        id,
        tool,
        arguments,
    } = request;
    let repo_index = service.repo_index.as_ref();

    let result = match tool.as_str() {
        "describe_repo" => serde_json::to_value(repo_index).map_err(|error| error.to_string()),
        "list_workflows" => {
            let Some(repo_index) = repo_index else {
                return mcp_error(id, "current repo index is not loaded");
            };
            serde_json::to_value(&repo_index.workflow_graph).map_err(|error| error.to_string())
        }
        "explain_concept" => {
            let Some(repo_index) = repo_index else {
                return mcp_error(id, "current repo index is not loaded");
            };
            let Some(concept_id) = arguments
                .get("concept_id")
                .and_then(serde_json::Value::as_str)
            else {
                return mcp_error(id, "missing `concept_id` argument");
            };
            let concept = repo_index
                .concept_graph
                .iter()
                .find(|entry| entry.id == concept_id)
                .cloned();
            let owner = locate_owner(&repo_index.reuse, concept_id);
            serde_json::to_value(serde_json::json!({
                "concept": concept,
                "owner": owner
            }))
            .map_err(|error| error.to_string())
        }
        "search_code" => dispatch_service_search(service, SearchMode::Code, &arguments),
        "gca.search" => {
            let Some(query) = arguments.get("query").and_then(serde_json::Value::as_str) else {
                return mcp_error(id, "missing `query` argument");
            };
            let mode = match arguments
                .get("mode")
                .and_then(serde_json::Value::as_str)
                .map(SearchMode::parse)
                .transpose()
            {
                Ok(mode) => mode.unwrap_or(SearchMode::Instruction),
                Err(error) => return mcp_error(id, &error),
            };
            let scope = match arguments
                .get("scope")
                .and_then(serde_json::Value::as_str)
                .map(SearchScope::parse)
                .transpose()
            {
                Ok(scope) => scope.unwrap_or(SearchScope::All),
                Err(error) => return mcp_error(id, &error),
            };
            service
                .search(
                    mode,
                    query,
                    scope,
                    arguments.get("repo").and_then(serde_json::Value::as_str),
                    arguments
                        .get("tenant")
                        .and_then(serde_json::Value::as_str)
                        .or(service.config.tenant.as_deref()),
                )
                .and_then(|response| {
                    serde_json::to_value(response).map_err(|error| error.to_string())
                })
        }
        "search_instructions" => {
            dispatch_service_search(service, SearchMode::Instruction, &arguments)
        }
        "search_reuse" => dispatch_service_search(service, SearchMode::Reuse, &arguments),
        "search_all" => dispatch_search_all(service, &arguments),
        "locate_owner" => {
            let Some(repo_index) = repo_index else {
                return mcp_error(id, "current repo index is not loaded");
            };
            let Some(concept_id) = arguments
                .get("concept_id")
                .and_then(serde_json::Value::as_str)
            else {
                return mcp_error(id, "missing `concept_id` argument");
            };
            serde_json::to_value(locate_owner(&repo_index.reuse, concept_id))
                .map_err(|error| error.to_string())
        }
        "gca.agent_context" => {
            let Some(task) = arguments.get("task").and_then(serde_json::Value::as_str) else {
                return mcp_error(id, "missing `task` argument");
            };
            serde_json::to_value(build_agent_context(
                task,
                arguments.get("channel").and_then(serde_json::Value::as_str),
                arguments
                    .get("tenant")
                    .and_then(serde_json::Value::as_str)
                    .or(service.config.tenant.as_deref()),
                arguments.get("repo").and_then(serde_json::Value::as_str),
            ))
            .map_err(|error| error.to_string())
        }
        "gca.find_owner" => {
            let Some(concept) = arguments
                .get("concept")
                .or_else(|| arguments.get("concept_id"))
                .and_then(serde_json::Value::as_str)
            else {
                return mcp_error(id, "missing `concept` argument");
            };
            let response = build_agent_context(
                concept,
                arguments.get("channel").and_then(serde_json::Value::as_str),
                arguments
                    .get("tenant")
                    .and_then(serde_json::Value::as_str)
                    .or(service.config.tenant.as_deref()),
                None,
            );
            serde_json::to_value(serde_json::json!({
                "concept": concept,
                "channel": response.channel,
                "owner_candidates": response.owner_candidates,
                "warnings": response.warnings,
            }))
            .map_err(|error| error.to_string())
        }
        "locate_extension_point" => {
            let query = arguments
                .get("query")
                .or_else(|| arguments.get("concept_id"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            service
                .search(
                    SearchMode::Instruction,
                    query,
                    SearchScope::All,
                    None,
                    service.config.tenant.as_deref(),
                )
                .and_then(|response| {
                    serde_json::to_value(response).map_err(|error| error.to_string())
                })
        }
        "plan_change" | "required_validations" => {
            let Some(repo_index) = repo_index else {
                return mcp_error(id, "current repo index is not loaded");
            };
            let Some(task) = arguments.get("task").and_then(serde_json::Value::as_str) else {
                return mcp_error(id, "missing `task` argument");
            };
            serde_json::to_value(required_validations(repo_index, task))
                .map_err(|error| error.to_string())
        }
        "gca.required_validations" => {
            let Some(task) = arguments.get("task").and_then(serde_json::Value::as_str) else {
                return mcp_error(id, "missing `task` argument");
            };
            let response = build_agent_context(
                task,
                arguments.get("channel").and_then(serde_json::Value::as_str),
                arguments
                    .get("tenant")
                    .and_then(serde_json::Value::as_str)
                    .or(service.config.tenant.as_deref()),
                None,
            );
            serde_json::to_value(serde_json::json!({
                "task": task,
                "channel": response.channel,
                "validations": response.required_validations,
                "warnings": response.warnings,
            }))
            .map_err(|error| error.to_string())
        }
        "gca.recent_updates" => {
            let Some(task) = arguments.get("task").and_then(serde_json::Value::as_str) else {
                return mcp_error(id, "missing `task` argument");
            };
            let response = build_agent_context(
                task,
                arguments.get("channel").and_then(serde_json::Value::as_str),
                arguments
                    .get("tenant")
                    .and_then(serde_json::Value::as_str)
                    .or(service.config.tenant.as_deref()),
                None,
            );
            serde_json::to_value(serde_json::json!({
                "task": task,
                "channel": response.channel,
                "updates": response.recent_updates,
                "warnings": response.warnings,
            }))
            .map_err(|error| error.to_string())
        }
        "impact_analysis" => {
            let Some(repo_index) = repo_index else {
                return mcp_error(id, "current repo index is not loaded");
            };
            let Some(symbol) = arguments.get("symbol").and_then(serde_json::Value::as_str) else {
                return mcp_error(id, "missing `symbol` argument");
            };
            serde_json::to_value(impact_analysis(
                repo_index,
                symbol,
                service.refresh.as_ref(),
            ))
            .map_err(|error| error.to_string())
        }
        "detect_changes" => {
            let Some(repo_index) = repo_index else {
                return mcp_error(id, "current repo index is not loaded");
            };
            let changed_files = arguments
                .get("changed_files")
                .and_then(serde_json::Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if changed_files.is_empty() {
                return mcp_error(
                    id,
                    "missing `changed_files` argument; expected a non-empty array of repo-relative paths",
                );
            }
            serde_json::to_value(serde_json::json!({
                "changed_files": changed_files,
                "likely_concepts": infer_changed_concepts(repo_index, &changed_files),
                "likely_workflows": infer_changed_workflows(repo_index, &changed_files),
                "suggested_validations": infer_changed_validations(repo_index, &changed_files),
                "freshness_warning": freshness_warning(service.refresh.as_ref()),
            }))
            .map_err(|error| error.to_string())
        }
        "show_freshness" => serde_json::to_value(serde_json::json!({
            "freshness_warning": freshness_warning(service.refresh.as_ref())
        }))
        .map_err(|error| error.to_string()),
        "list_indexed_repos" | "list_remote_repos" => Ok(service.indexed_repos()),
        "sync_indexes" => service.sync_indexes(),
        "show_catalog" => Ok(service.catalog()),
        "gca.branch_status" => Ok(service.status()),
        other => return mcp_error(id, &format!("unknown tool: {other}")),
    };

    match result {
        Ok(result) => McpResponse {
            id,
            ok: true,
            result: Some(result),
            error: None,
        },
        Err(error) => mcp_error(id, &error),
    }
}

fn infer_changed_concepts(repo_index: &RepoIndex, changed_files: &[String]) -> Vec<String> {
    let mut likely_concepts = Vec::new();
    for path in changed_files {
        let lower = path.to_ascii_lowercase();
        for concept in &repo_index.concept_graph {
            if concept.related_paths.iter().any(|related| related == path)
                || lower.contains(&concept.id.to_ascii_lowercase())
            {
                likely_concepts.push(concept.id.clone());
            }
        }
        for instruction in &repo_index.instruction_graph {
            if instruction.path == *path {
                likely_concepts.extend(instruction.concept_ids.clone());
            }
        }
    }
    likely_concepts.sort();
    likely_concepts.dedup();
    likely_concepts
}

fn infer_changed_workflows(repo_index: &RepoIndex, changed_files: &[String]) -> Vec<String> {
    let mut likely_workflows = Vec::new();
    for path in changed_files {
        let lower = path.to_ascii_lowercase();
        for workflow in &repo_index.workflow_graph {
            if workflow.docs.iter().any(|doc| doc == path)
                || lower.contains(&workflow.id.to_ascii_lowercase())
            {
                likely_workflows.push(workflow.id.clone());
            }
        }
        if lower.contains(".github/workflows/") {
            likely_workflows.extend(
                repo_index
                    .workflow_graph
                    .iter()
                    .map(|workflow| workflow.id.clone()),
            );
        }
    }
    likely_workflows.sort();
    likely_workflows.dedup();
    likely_workflows
}

fn infer_changed_validations(
    repo_index: &RepoIndex,
    changed_files: &[String],
) -> Vec<ValidationDescriptor> {
    required_validations(repo_index, &changed_files.join(" ")).validations
}

fn dispatch_service_search(
    service: &QueryService,
    mode: SearchMode,
    arguments: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let Some(query) = arguments.get("query").and_then(serde_json::Value::as_str) else {
        return Err("missing `query` argument".to_string());
    };
    let scope = arguments
        .get("scope")
        .and_then(serde_json::Value::as_str)
        .map(SearchScope::parse)
        .transpose()?
        .unwrap_or(SearchScope::All);
    let repo = arguments.get("repo").and_then(serde_json::Value::as_str);
    let tenant = arguments
        .get("tenant")
        .and_then(serde_json::Value::as_str)
        .or(service.config.tenant.as_deref());
    serde_json::to_value(service.search(mode, query, scope, repo, tenant)?)
        .map_err(|error| error.to_string())
}

fn dispatch_search_all(
    service: &QueryService,
    arguments: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let Some(query) = arguments.get("query").and_then(serde_json::Value::as_str) else {
        return Err("missing `query` argument".to_string());
    };
    let scope = arguments
        .get("scope")
        .and_then(serde_json::Value::as_str)
        .map(SearchScope::parse)
        .transpose()?
        .unwrap_or(SearchScope::All);
    let repo = arguments.get("repo").and_then(serde_json::Value::as_str);
    let tenant = arguments
        .get("tenant")
        .and_then(serde_json::Value::as_str)
        .or(service.config.tenant.as_deref());
    let responses = [
        service.search(SearchMode::Code, query, scope, repo, tenant)?,
        service.search(SearchMode::Instruction, query, scope, repo, tenant)?,
        service.search(SearchMode::Concept, query, scope, repo, tenant)?,
        service.search(SearchMode::Reuse, query, scope, repo, tenant)?,
        service.search(SearchMode::Course, query, scope, repo, tenant)?,
        service.search(SearchMode::Update, query, scope, repo, tenant)?,
    ];
    serde_json::to_value(responses).map_err(|error| error.to_string())
}

fn mcp_error(id: Option<String>, message: &str) -> McpResponse {
    McpResponse {
        id,
        ok: false,
        result: None,
        error: Some(message.to_string()),
    }
}

fn mcp_tool(name: &str, description: &str) -> McpTool {
    McpTool {
        name: name.to_string(),
        description: description.to_string(),
    }
}

fn extract_plan_summary(raw: &str) -> String {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) {
        let mut parts = Vec::new();
        collect_plan_strings(&value, &mut parts);
        return parts.join(" ");
    }
    raw.to_string()
}

fn collect_plan_strings(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(text) => out.push(text.clone()),
        serde_json::Value::Array(values) => {
            for value in values {
                collect_plan_strings(value, out);
            }
        }
        serde_json::Value::Object(map) => {
            for value in map.values() {
                collect_plan_strings(value, out);
            }
        }
        _ => {}
    }
}

fn freshness_warning(refresh: Option<&RefreshCheck>) -> Option<String> {
    refresh.and_then(|refresh| {
        if refresh.needs_refresh {
            Some(format!(
                "index may be stale: {}",
                refresh.reasons.join("; ")
            ))
        } else {
            None
        }
    })
}

fn search_code(repo_index: &RepoIndex, query: &str) -> Vec<SearchResult> {
    let query = query.to_ascii_lowercase();
    let mut results = Vec::new();

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
    repo_index
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
        .collect()
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
        .filter(|course| {
            course.id.to_ascii_lowercase().contains(&query)
                || course.title.to_ascii_lowercase().contains(&query)
                || course.summary.to_ascii_lowercase().contains(&query)
                || course
                    .tasks
                    .iter()
                    .any(|task| task.to_ascii_lowercase().contains(&query))
                || course
                    .canonical_commands
                    .iter()
                    .any(|command| command.to_ascii_lowercase().contains(&query))
        })
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
        .filter(|update| {
            update.id.to_ascii_lowercase().contains(&query)
                || update.title.to_ascii_lowercase().contains(&query)
                || update.summary.to_ascii_lowercase().contains(&query)
                || update
                    .agent_instruction
                    .to_ascii_lowercase()
                    .contains(&query)
                || update
                    .affected_concepts
                    .iter()
                    .any(|concept| concept.to_ascii_lowercase().contains(&query))
                || update
                    .deprecated_commands
                    .iter()
                    .any(|command| command.command.to_ascii_lowercase().contains(&query))
        })
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

fn analyze_repo(start_dir: &Path, registry_path: &Path) -> Result<AnalyzeOutputs, String> {
    let repo_root = find_repo_root(start_dir).ok_or_else(|| {
        format!(
            "failed to detect repository root from {}",
            start_dir.display()
        )
    })?;
    let repo_name = repo_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown-repo")
        .to_string();
    let repo_id = detect_repo_id(&repo_root, &repo_name);
    let org = parse_repo_id(&repo_id).map(|(org, _)| org);
    let generated_at = timestamp_string();
    let head_sha = read_head_sha(&repo_root).unwrap_or_else(|| "unknown".to_string());
    let default_branch = read_default_branch(&repo_root);
    let candidate_docs = find_candidate_docs(&repo_root);
    let cargo_manifests = find_cargo_manifests(&repo_root);
    let tracked_files = find_tracked_files(&repo_root);
    let source_stats = build_source_stats(&repo_root, &cargo_manifests);
    let training_courses = load_training_courses(&repo_root);
    let knowledge_updates = load_knowledge_updates(&repo_root);
    let instruction_graph = build_instruction_graph(
        &repo_root,
        &source_stats,
        &training_courses,
        &knowledge_updates,
    );
    let instruction_paths = instruction_graph
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<Vec<_>>();
    let commands = instruction_graph
        .iter()
        .flat_map(|entry| entry.commands.iter().cloned())
        .collect::<Vec<_>>();
    let repo_role = infer_repo_role(&repo_name, &source_stats);
    let manifest = RepoAgentManifest {
        version: SCHEMA_VERSION_V1.to_string(),
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
    for concept in infer_concepts(&repo_name, &source_stats, &commands) {
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
    let workflow_graph = infer_workflows(&source_stats, &commands, &concept_ids);
    let (validations, reuse) = load_policy_bundle(&repo_root);

    let repo_index = RepoIndex {
        version: SCHEMA_VERSION_V1.to_string(),
        repo_id: repo_id.clone(),
        repo_name: repo_name.clone(),
        repo_role,
        generated_at: generated_at.clone(),
        metadata: Some(RepoIndexMetadata {
            repo_id: repo_id.clone(),
            branch: default_branch.clone(),
            commit_sha: Some(head_sha.clone()),
            commit_time: None,
            indexed_at: generated_at.clone(),
            index_schema_version: "gca.repo_index.v1".to_string(),
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            source_tree_hash: Some(source_tree_hash(&tracked_files)),
        }),
        freshness: FreshnessStatus::Fresh,
        manifest: manifest.clone(),
        concept_graph,
        workflow_graph,
        validations,
        reuse,
        training_courses,
        knowledge_updates,
        instruction_graph,
        instruction_paths,
        source_stats,
    };
    let fingerprints = Fingerprints {
        version: SCHEMA_VERSION_V1.to_string(),
        head_sha: head_sha.clone(),
        default_branch,
        tracked_files,
        generator_version: Some(env!("CARGO_PKG_VERSION").to_string()),
    };

    let local_dir = repo_root.join(LOCAL_INDEX_DIR);
    fs::create_dir_all(&local_dir)
        .map_err(|error| format!("failed to create {}: {error}", local_dir.display()))?;
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

fn load_or_analyze_repo_index() -> Result<RepoIndex, String> {
    let current =
        current_dir().map_err(|error| format!("failed to determine current directory: {error}"))?;
    let repo_root = find_repo_root(&current).ok_or_else(|| {
        format!(
            "failed to detect repository root from {}",
            current.display()
        )
    })?;
    let local_index_path = repo_root.join(LOCAL_INDEX_DIR).join("repo-index.json");

    if local_index_path.exists() {
        let raw = fs::read_to_string(&local_index_path)
            .map_err(|error| format!("failed to read {}: {error}", local_index_path.display()))?;
        let mut repo_index: RepoIndex = serde_json::from_str(&raw)
            .map_err(|error| format!("failed to parse {}: {error}", local_index_path.display()))?;
        canonicalize_repo_index_identity(&mut repo_index);
        return Ok(repo_index);
    }

    let outputs = analyze_repo(&repo_root, &default_registry_path(&home_dir()))?;
    Ok(outputs.repo_index)
}

fn load_repo_index_from_path(path: &Path) -> Result<RepoIndex, std::io::Error> {
    let raw = fs::read_to_string(path)?;
    let mut repo_index: RepoIndex = serde_json::from_str(&raw)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    canonicalize_repo_index_identity(&mut repo_index);
    Ok(repo_index)
}

fn load_optional_fingerprints(path: &Path) -> Result<Option<Fingerprints>, std::io::Error> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)?;
    let parsed = serde_json::from_str(&raw)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    Ok(Some(parsed))
}

fn describe_here() -> Result<DescribeHere, String> {
    let current_dir =
        current_dir().map_err(|error| format!("failed to determine current directory: {error}"))?;
    let repo_root = find_repo_root(&current_dir).ok_or_else(|| {
        format!(
            "failed to detect repository root from {}",
            current_dir.display()
        )
    })?;

    let repo_name = repo_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown-repo")
        .to_string();
    let repo_id = detect_repo_id(&repo_root, &repo_name);
    let local_index_path = repo_root.join(LOCAL_INDEX_DIR).join("repo-index.json");
    let local_index = if local_index_path.exists() {
        let raw = fs::read_to_string(&local_index_path)
            .map_err(|error| format!("failed to read {}: {error}", local_index_path.display()))?;
        let mut parsed = serde_json::from_str::<RepoIndex>(&raw)
            .map_err(|error| format!("failed to parse {}: {error}", local_index_path.display()))?;
        canonicalize_repo_index_identity(&mut parsed);
        Some(parsed)
    } else {
        None
    };

    Ok(DescribeHere {
        version: env!("CARGO_PKG_VERSION").to_string(),
        manifest_path: repo_root.join("Cargo.toml"),
        has_git_dir: repo_root.join(".git").exists(),
        repo_id,
        repo_name,
        repo_root,
        local_index_path: local_index.as_ref().map(|_| local_index_path),
        repo_role: local_index.as_ref().map(|index| index.repo_role),
        concept_count: local_index.as_ref().map(|index| index.concept_graph.len()),
        workflow_count: local_index.as_ref().map(|index| index.workflow_graph.len()),
        instruction_count: local_index
            .as_ref()
            .map(|index| index.instruction_graph.len()),
    })
}

fn load_registry(path: &Path) -> Result<Registry, String> {
    match fs::read_to_string(path) {
        Ok(raw) => serde_json::from_str(&raw)
            .map_err(|error| format!("failed to parse registry {}: {error}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Registry::empty()),
        Err(error) => Err(format!(
            "failed to read registry {}: {error}",
            path.display()
        )),
    }
}

fn write_registry(path: &Path, registry: &Registry) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }

    write_json(path, registry)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let raw = serde_json::to_string_pretty(value)
        .map_err(|error| format!("failed to serialize {}: {error}", path.display()))?;
    fs::write(path, raw).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn current_dir() -> Result<PathBuf, std::io::Error> {
    env::current_dir()
}

fn find_repo_root(start: &Path) -> Option<PathBuf> {
    for candidate in start.ancestors() {
        if candidate.join(".git").exists() && candidate.join("Cargo.toml").exists() {
            return Some(candidate.to_path_buf());
        }
    }

    None
}

fn default_registry_path(home_dir: &Path) -> PathBuf {
    home_dir.join(".greentic-agent").join("registry.json")
}

fn detect_repo_id(repo_root: &Path, repo_name: &str) -> String {
    read_origin_url(repo_root)
        .and_then(|url| parse_github_remote_url(&url))
        .or_else(|| {
            env::var("GITHUB_REPOSITORY")
                .ok()
                .filter(|repository| parse_repo_id(repository).is_some())
        })
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

fn parse_github_remote_url(value: &str) -> Option<String> {
    let value = value.trim().trim_end_matches(".git");
    for prefix in [
        "git@github.com:",
        "https://github.com/",
        "ssh://git@github.com/",
    ] {
        if let Some(path) = value.strip_prefix(prefix)
            && let Some((org, name)) = parse_repo_id(path)
        {
            return Some(format!("{org}/{name}"));
        }
    }
    None
}

fn parse_repo_id(value: &str) -> Option<(String, String)> {
    let value = value.trim().trim_end_matches(".git");
    let mut parts = value.split('/');
    let org = parts.next()?;
    let name = parts.next()?;
    if org.is_empty() || name.is_empty() || parts.next().is_some() {
        return None;
    }
    Some((org.to_string(), name.to_string()))
}

fn home_dir() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn read_head_sha(repo_root: &Path) -> Option<String> {
    let head = fs::read_to_string(repo_root.join(".git").join("HEAD")).ok()?;
    let head = head.trim();

    if let Some(reference) = head.strip_prefix("ref: ") {
        return fs::read_to_string(repo_root.join(".git").join(reference))
            .ok()
            .map(|value| value.trim().to_string());
    }

    if !head.is_empty() {
        return Some(head.to_string());
    }

    env::var("GITHUB_SHA")
        .ok()
        .filter(|sha| !sha.trim().is_empty())
}

fn read_default_branch(repo_root: &Path) -> Option<String> {
    let head = fs::read_to_string(repo_root.join(".git").join("HEAD")).ok()?;
    head.trim()
        .strip_prefix("ref: refs/heads/")
        .map(|branch| branch.to_string())
        .or_else(|| {
            env::var("GITHUB_REF_NAME")
                .ok()
                .filter(|branch| !branch.trim().is_empty())
        })
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
    dedup_sorted(&mut manifests);
    manifests
}

fn find_tracked_files(repo_root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    gather_regular_files(repo_root, repo_root, &mut files);
    dedup_sorted(&mut files);
    files
}

fn source_tree_hash(tracked_files: &[String]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for file in tracked_files {
        for byte in file.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= u64::from(b'\n');
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv64:{hash:016x}")
}

fn build_source_stats(repo_root: &Path, cargo_manifests: &[String]) -> SourceStats {
    let mut workspace_members = Vec::new();
    let mut crate_names = Vec::new();
    let mut feature_names = Vec::new();
    let mut dependencies = Vec::new();

    for manifest_path in cargo_manifests {
        let raw = fs::read_to_string(repo_root.join(manifest_path)).unwrap_or_default();
        if manifest_path == "Cargo.toml" {
            workspace_members.extend(parse_workspace_members(&raw));
        }
        if let Some(crate_name) = parse_manifest_string(&raw, "name") {
            crate_names.push(crate_name);
        }
        feature_names.extend(parse_table_keys(&raw, "[features]"));
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

fn build_instruction_graph(
    repo_root: &Path,
    source_stats: &SourceStats,
    training_courses: &[TrainingCourseDescriptor],
    knowledge_updates: &[KnowledgeUpdateDescriptor],
) -> Vec<InstructionDescriptor> {
    let mut paths = source_stats.markdown_docs.clone();
    paths.extend(source_stats.workflow_files.clone());
    dedup_sorted(&mut paths);

    let mut descriptors = Vec::new();
    for path in paths {
        let raw = fs::read_to_string(repo_root.join(&path)).unwrap_or_default();
        let headings = collect_headings(&raw);
        let title = headings
            .first()
            .cloned()
            .unwrap_or_else(|| fallback_title(&path));
        let kind = if path.ends_with(".yml") || path.ends_with(".yaml") {
            "workflow"
        } else if path.starts_with(".codex/") {
            "codex"
        } else {
            "doc"
        };
        descriptors.push(InstructionDescriptor {
            id: sanitize_id(&path),
            path: path.clone(),
            title,
            kind: kind.to_string(),
            headings,
            commands: known_command_matches(&raw),
            concept_ids: infer_instruction_concepts(&path, &raw),
        });
    }

    for course in training_courses {
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
        let mut commands = course.canonical_commands.clone();
        commands.extend(
            course
                .modules
                .iter()
                .flat_map(|module| module.steps.iter())
                .filter_map(|step| step.command.clone()),
        );
        dedup_sorted(&mut commands);
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
            commands,
            concept_ids,
        });
    }

    for update in knowledge_updates {
        let mut headings = vec![
            update.summary.clone(),
            update.agent_instruction.clone(),
            local_update_type_label(update.update_type).to_string(),
            local_update_severity_label(update.severity).to_string(),
        ];
        if let Some(summary) = &update.human_summary {
            headings.push(summary.clone());
        }
        headings.extend(update.affected_workflows.clone());
        headings.extend(update.affected_courses.clone());
        headings.extend(update.new_capabilities.iter().flat_map(|capability| {
            [
                capability.title.clone(),
                capability.summary.clone(),
                capability.use_when.join(" "),
            ]
        }));
        headings.extend(update.replaced_guidance.iter().flat_map(|guidance| {
            [
                guidance.old_guidance.clone(),
                guidance.replacement_guidance.clone(),
                guidance.reason.clone(),
            ]
        }));
        headings.extend(
            update
                .migration_steps
                .iter()
                .map(|step| step.instruction.clone()),
        );
        let mut commands = update
            .migration_steps
            .iter()
            .filter_map(|step| step.command.clone())
            .chain(
                update
                    .deprecated_commands
                    .iter()
                    .map(|command| command.command.clone()),
            )
            .collect::<Vec<_>>();
        dedup_sorted(&mut commands);
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
            commands,
            concept_ids,
        });
    }

    descriptors.sort_by(|left, right| left.path.cmp(&right.path));
    descriptors
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
        if course.version.is_empty()
            || course.id.is_empty()
            || course.title.is_empty()
            || course.owner_repo.is_empty()
            || course.modules.is_empty()
        {
            continue;
        }
        let relative = format!("{TRAINING_DIR}/{file_name}");
        if !course.source_paths.iter().any(|path| path == &relative) {
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
        if update.version.is_empty()
            || update.id.is_empty()
            || update.title.is_empty()
            || update.summary.is_empty()
            || update.owner_repo.is_empty()
            || update.published_at.is_empty()
            || update.agent_instruction.is_empty()
        {
            continue;
        }
        let relative = format!("{UPDATES_DIR}/{file_name}");
        if !update.source_paths.iter().any(|path| path == &relative) {
            update.source_paths.push(relative);
        }
        update.source_paths.sort();
        updates.push(update);
    }
    updates.sort_by(|left, right| left.id.cmp(&right.id));
    updates
}

fn local_update_type_label(update_type: KnowledgeUpdateType) -> &'static str {
    match update_type {
        KnowledgeUpdateType::NewCapability => "new_capability",
        KnowledgeUpdateType::BehaviourChange => "behaviour_change",
        KnowledgeUpdateType::DeprecatedWorkflow => "deprecated_workflow",
        KnowledgeUpdateType::DeprecatedCommand => "deprecated_command",
        KnowledgeUpdateType::MigrationRequired => "migration_required",
        KnowledgeUpdateType::ValidationChanged => "validation_changed",
        KnowledgeUpdateType::OwnershipChanged => "ownership_changed",
        KnowledgeUpdateType::CourseUpdated => "course_updated",
        KnowledgeUpdateType::SecurityNotice => "security_notice",
        KnowledgeUpdateType::BreakingChange => "breaking_change",
        KnowledgeUpdateType::DocumentationCorrection => "documentation_correction",
    }
}

fn local_update_severity_label(severity: KnowledgeUpdateSeverity) -> &'static str {
    match severity {
        KnowledgeUpdateSeverity::Info => "info",
        KnowledgeUpdateSeverity::Recommended => "recommended",
        KnowledgeUpdateSeverity::Important => "important",
        KnowledgeUpdateSeverity::Breaking => "breaking",
        KnowledgeUpdateSeverity::Critical => "critical",
    }
}

fn infer_repo_role(repo_name: &str, source_stats: &SourceStats) -> RepoRole {
    let repo_name = repo_name.to_ascii_lowercase();

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
    if !source_stats.example_paths.is_empty() && source_stats.markdown_docs.len() <= 2 {
        return RepoRole::ExamplesOnly;
    }

    RepoRole::CliLauncher
}

fn infer_concepts(
    repo_name: &str,
    source_stats: &SourceStats,
    commands: &[String],
) -> Vec<ConceptDescriptor> {
    let mut concepts = Vec::new();
    add_concept_if_detected(
        &mut concepts,
        repo_name,
        source_stats,
        commands,
        ConceptRule {
            id: "digital_worker",
            title: "Digital worker",
            summary: "Greentic digital worker orchestration appears in repo docs or commands.",
            needles: &["digital worker", "worker"],
        },
    );
    add_concept_if_detected(
        &mut concepts,
        repo_name,
        source_stats,
        commands,
        ConceptRule {
            id: "application_pack",
            title: "Application pack",
            summary: "Pack authoring or application packaging terminology appears in the repo.",
            needles: &["application pack", "pack"],
        },
    );
    add_concept_if_detected(
        &mut concepts,
        repo_name,
        source_stats,
        commands,
        ConceptRule {
            id: "extension_pack",
            title: "Extension pack",
            summary: "Extension-pack terminology appears in docs or workflows.",
            needles: &["extension pack"],
        },
    );
    add_concept_if_detected(
        &mut concepts,
        repo_name,
        source_stats,
        commands,
        ConceptRule {
            id: "greentic_x",
            title: "Greentic X",
            summary: "Greentic-X terminology appears in repo knowledge sources.",
            needles: &["greentic-x", "greentic x"],
        },
    );
    add_concept_if_detected(
        &mut concepts,
        repo_name,
        source_stats,
        commands,
        ConceptRule {
            id: "greentic_sorla",
            title: "Greentic sorla",
            summary: "Greentic-sorla terminology appears in repo knowledge sources.",
            needles: &["greentic-sorla", "greentic sorla"],
        },
    );
    add_concept_if_detected(
        &mut concepts,
        repo_name,
        source_stats,
        commands,
        ConceptRule {
            id: "wizard",
            title: "Wizard",
            summary: "Wizard-driven setup flows are referenced in repo docs or workflows.",
            needles: &["wizard"],
        },
    );
    add_concept_if_detected(
        &mut concepts,
        repo_name,
        source_stats,
        commands,
        ConceptRule {
            id: "setup",
            title: "Setup",
            summary: "Setup commands or setup guidance appear in the repo.",
            needles: &["setup"],
        },
    );
    add_concept_if_detected(
        &mut concepts,
        repo_name,
        source_stats,
        commands,
        ConceptRule {
            id: "start",
            title: "Start",
            summary: "Start commands appear in repo docs or workflows.",
            needles: &["start"],
        },
    );

    concepts.sort_by(|left, right| left.id.cmp(&right.id));
    concepts
}

fn infer_workflows(
    source_stats: &SourceStats,
    commands: &[String],
    concept_ids: &[String],
) -> Vec<WorkflowDescriptor> {
    let mut workflows = Vec::new();
    let docs = source_stats.markdown_docs.clone();

    if contains_command(commands, "gtc dev coding-agent analyze")
        || !source_stats.markdown_docs.is_empty()
    {
        workflows.push(WorkflowDescriptor {
            id: "analyze_repo".to_string(),
            title: "Analyze repo".to_string(),
            summary: "Generate repo-local Greentic coding-agent metadata for the current checkout."
                .to_string(),
            phase: LifecyclePhase::Build,
            commands: collect_matching_commands(commands, &["gtc dev coding-agent analyze"]),
            docs: docs.clone(),
            concept_ids: relevant_concepts(concept_ids, &["digital_worker", "setup"]),
        });
    }

    if contains_command(commands, "gtc wizard --schema")
        || contains_command(commands, "gtc wizard --answers")
    {
        workflows.push(WorkflowDescriptor {
            id: "wizard_bootstrap".to_string(),
            title: "Wizard bootstrap".to_string(),
            summary: "Wizard-driven bootstrapping commands are referenced in repo materials."
                .to_string(),
            phase: LifecyclePhase::Setup,
            commands: collect_matching_commands(
                commands,
                &["gtc wizard --schema", "gtc wizard --answers"],
            ),
            docs: docs.clone(),
            concept_ids: relevant_concepts(concept_ids, &["wizard", "setup"]),
        });
    }

    if contains_command(commands, "gtc setup") {
        workflows.push(WorkflowDescriptor {
            id: "setup_bundle".to_string(),
            title: "Setup bundle".to_string(),
            summary: "A `gtc setup` flow is described in repo docs or workflows.".to_string(),
            phase: LifecyclePhase::Setup,
            commands: collect_matching_commands(commands, &["gtc setup --schema", "gtc setup"]),
            docs: docs.clone(),
            concept_ids: relevant_concepts(concept_ids, &["setup", "application_pack"]),
        });
    }

    if contains_command(commands, "gtc start") {
        workflows.push(WorkflowDescriptor {
            id: "start_bundle".to_string(),
            title: "Start bundle".to_string(),
            summary: "A `gtc start` flow is described in repo docs or workflows.".to_string(),
            phase: LifecyclePhase::Start,
            commands: collect_matching_commands(commands, &["gtc start"]),
            docs,
            concept_ids: relevant_concepts(concept_ids, &["start", "digital_worker"]),
        });
    }

    workflows.sort_by(|left, right| left.id.cmp(&right.id));
    workflows
}

fn known_command_matches(raw: &str) -> Vec<String> {
    let lower = raw.to_ascii_lowercase();
    let mut matches = KNOWN_COMMANDS
        .iter()
        .filter(|command| lower.contains(&command.to_ascii_lowercase()))
        .map(|command| (*command).to_string())
        .collect::<Vec<_>>();
    dedup_sorted(&mut matches);
    matches
}

fn add_concept_if_detected(
    concepts: &mut Vec<ConceptDescriptor>,
    repo_name: &str,
    source_stats: &SourceStats,
    commands: &[String],
    rule: ConceptRule<'_>,
) {
    if !contains_any(repo_name, source_stats, commands, rule.needles) {
        return;
    }

    let mut related_paths = source_stats.markdown_docs.clone();
    related_paths.extend(source_stats.workflow_files.clone());
    dedup_sorted(&mut related_paths);

    concepts.push(ConceptDescriptor {
        id: rule.id.to_string(),
        title: rule.title.to_string(),
        summary: rule.summary.to_string(),
        scope: KnowledgeScope::LocalRepo,
        lifecycle_phase: LifecyclePhase::Build,
        owners: vec!["greentic-coding-agent".to_string()],
        related_paths,
    });
}

fn contains_any(
    repo_name: &str,
    source_stats: &SourceStats,
    commands: &[String],
    needles: &[&str],
) -> bool {
    let mut corpus = Vec::new();
    corpus.push(repo_name.to_ascii_lowercase());
    corpus.extend(
        source_stats
            .markdown_docs
            .iter()
            .map(|value| value.to_ascii_lowercase()),
    );
    corpus.extend(
        source_stats
            .workflow_files
            .iter()
            .map(|value| value.to_ascii_lowercase()),
    );
    corpus.extend(
        source_stats
            .example_paths
            .iter()
            .map(|value| value.to_ascii_lowercase()),
    );
    corpus.extend(
        source_stats
            .public_items
            .iter()
            .map(|value| value.to_ascii_lowercase()),
    );
    corpus.extend(commands.iter().map(|value| value.to_ascii_lowercase()));

    needles.iter().any(|needle| {
        let needle = needle.to_ascii_lowercase();
        corpus.iter().any(|value| value.contains(&needle))
    })
}

fn contains_command(commands: &[String], needle: &str) -> bool {
    let needle = needle.to_ascii_lowercase();
    commands
        .iter()
        .any(|command| command.to_ascii_lowercase().contains(&needle))
}

fn collect_matching_commands(commands: &[String], patterns: &[&str]) -> Vec<String> {
    let mut matches = commands
        .iter()
        .filter(|command| {
            let lower = command.to_ascii_lowercase();
            patterns
                .iter()
                .any(|pattern| lower.contains(&pattern.to_ascii_lowercase()))
        })
        .cloned()
        .collect::<Vec<_>>();
    dedup_sorted(&mut matches);
    matches
}

fn relevant_concepts(concept_ids: &[String], desired: &[&str]) -> Vec<String> {
    concept_ids
        .iter()
        .filter(|id| desired.iter().any(|desired| id == desired))
        .cloned()
        .collect()
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
            keys.push(key.trim().to_string());
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

        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
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
