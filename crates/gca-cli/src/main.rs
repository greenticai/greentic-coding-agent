use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const LOCAL_INDEX_DIR: &str = ".greentic-agent";
const SCHEMA_VERSION_V1: &str = "v1";
const BUILTIN_CONCEPT_IDS: &[&str] = &[
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
        /// Output format for the printed analyze summary.
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
    Catalog {
        /// Output format for the command catalog.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Search indexed code, instructions, concepts, or reuse policy for a query string.
    Search {
        /// Search domain: `code`, `instruction`, `concept`, or `reuse`.
        #[arg(long)]
        mode: String,
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
        #[arg(long, default_value = "latest")]
        tag: String,
        /// Output format for the package result.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Copy the packaged local index into the simulated remote OCI store.
    PublishIndex {
        /// Tag to publish from the local OCI-style package output.
        #[arg(long, default_value = "latest")]
        tag: String,
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
        /// Output format for the sync result.
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

fn main() {
    let cli = Cli::parse();

    let exit_code = match cli.command {
        Some(Commands::Analyze { print, format }) => run_analyze(print, &format),
        Some(Commands::Catalog { format }) => run_commands(&format),
        Some(Commands::Concepts { format }) => run_concepts(&format),
        Some(Commands::Search {
            mode,
            query,
            format,
        }) => run_search(&mode, &query, &format),
        Some(Commands::LocateOwner { concept, format }) => run_locate_owner(&concept, &format),
        Some(Commands::RequiredValidations { task, format }) => {
            run_required_validations(&task, &format)
        }
        Some(Commands::PackageIndex { tag, format }) => run_package_index(&tag, &format),
        Some(Commands::PublishIndex { tag, format }) => run_publish_index(&tag, &format),
        Some(Commands::ListRemoteRepos { format }) => run_list_remote_repos(&format),
        Some(Commands::ShowCatalog { format }) => run_show_catalog(&format),
        Some(Commands::CheckRefresh { format }) => run_check_refresh(&format),
        Some(Commands::Impact { symbol, format }) => run_impact(&symbol, &format),
        Some(Commands::DetectChanges { format }) => run_detect_changes(&format),
        Some(Commands::ValidatePlan { plan_path, format }) => {
            run_validate_plan(&plan_path, &format)
        }
        Some(Commands::GenerateAgentFiles { write_root, format }) => {
            run_generate_agent_files(write_root, &format)
        }
        Some(Commands::InstallGithubWorkflow { format }) => run_install_github_workflow(&format),
        Some(Commands::Sync { repo, tag, format }) => {
            run_sync(repo.as_deref(), tag.as_deref(), &format)
        }
        Some(Commands::Serve {
            request_file,
            format,
        }) => run_serve(request_file.as_deref(), &format),
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
        Commands::Catalog { .. } => "commands is scaffolded but not implemented yet",
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
        Commands::Describe { .. } => "describe is scaffolded but not implemented yet",
    }
}

fn run_analyze(print: bool, format: &str) -> i32 {
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
    match analyze_repo(&start_dir, &default_registry_path(&home_dir)) {
        Ok(outputs) => {
            if print {
                print_analyze_summary(&outputs, format);
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run_concepts(format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };

    match load_or_analyze_repo_index() {
        Ok(repo_index) => {
            match format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&repo_index.concept_graph)
                            .expect("concept graph should serialize")
                    );
                }
                OutputFormat::Markdown => {
                    println!("# Concepts");
                    println!();
                    for concept in repo_index.concept_graph {
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

    let commands = command_catalog();
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&commands).expect("command catalog should serialize")
            );
        }
        OutputFormat::Markdown => {
            println!("# Commands");
            println!();
            for entry in commands {
                println!("- `{}`: {}", entry.command, entry.purpose);
            }
        }
    }
    0
}

fn run_search(mode: &str, query: &str, format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let mode = match SearchMode::parse(mode) {
        Ok(mode) => mode,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };

    match load_or_analyze_repo_index() {
        Ok(repo_index) => {
            let response = search_repo_index(&repo_index, mode, query);
            match format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&response)
                            .expect("search response should serialize")
                    );
                }
                OutputFormat::Markdown => print_search_response(&response),
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run_locate_owner(concept: &str, format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };

    match load_or_analyze_repo_index() {
        Ok(repo_index) => {
            let owner = locate_owner(&repo_index.reuse, concept);
            match format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&owner)
                            .expect("owner lookup should serialize")
                    );
                }
                OutputFormat::Markdown => print_owner_lookup(concept, owner.as_ref()),
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
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

    match load_or_analyze_repo_index() {
        Ok(repo_index) => {
            let response = required_validations(&repo_index, task);
            match format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&response)
                            .expect("required validations should serialize")
                    );
                }
                OutputFormat::Markdown => print_required_validations(&response),
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

    let current = match current_dir() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("failed to determine current directory: {error}");
            return 1;
        }
    };
    let repo_root = match find_repo_root(&current) {
        Some(path) => path,
        None => {
            eprintln!(
                "failed to detect repository root from {}",
                current.display()
            );
            return 1;
        }
    };

    match load_or_analyze_repo_index() {
        Ok(repo_index) => {
            let files = render_generated_files(&repo_index);
            match write_generated_files(&repo_root, &files, write_root) {
                Ok(paths) => {
                    match format {
                        OutputFormat::Json => {
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&paths)
                                    .expect("generated paths should serialize")
                            );
                        }
                        OutputFormat::Markdown => {
                            println!("# Generate Agent Files");
                            println!();
                            println!("- Files written: `{}`", paths.len());
                            for path in &paths {
                                println!("- `{}`", path.display());
                            }
                        }
                    }
                    0
                }
                Err(error) => {
                    eprintln!("failed to write generated files: {error}");
                    1
                }
            }
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run_package_index(tag: &str, format: &str) -> i32 {
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
        Ok(repo_index) => match package_index_layout(&repo_root, &repo_index, tag) {
            Ok(result) => {
                match format {
                    OutputFormat::Json => println!(
                        "{}",
                        serde_json::to_string_pretty(&result)
                            .expect("package result should serialize")
                    ),
                    OutputFormat::Markdown => {
                        println!("# Package Index");
                        println!();
                        println!("- Reference: `{}`", result.reference);
                        println!("- Package dir: `{}`", result.package_dir.display());
                    }
                }
                0
            }
            Err(error) => {
                eprintln!("failed to package index: {error}");
                1
            }
        },
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run_publish_index(tag: &str, format: &str) -> i32 {
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
    let remote_root = default_remote_store_path(&home_dir());

    match load_or_analyze_repo_index() {
        Ok(repo_index) => match package_index_layout(&repo_root, &repo_index, tag) {
            Ok(result) => {
                let target = remote_root.join(&repo_index.repo_name).join(tag);
                if let Err(error) = copy_dir_all(&result.package_dir, &target) {
                    eprintln!("failed to publish package: {error}");
                    return 1;
                }
                match format {
                    OutputFormat::Json => println!(
                        "{}",
                        serde_json::to_string_pretty(&target)
                            .expect("publish target should serialize")
                    ),
                    OutputFormat::Markdown => {
                        println!("# Publish Index");
                        println!();
                        println!("- Reference: `{}`", result.reference);
                        println!("- Remote store: `{}`", target.display());
                    }
                }
                0
            }
            Err(error) => {
                eprintln!("failed to package index: {error}");
                1
            }
        },
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run_sync(repo: Option<&str>, tag: Option<&str>, format: &str) -> i32 {
    let format = match OutputFormat::parse(format) {
        Ok(format) => format,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let home = home_dir();
    let remote_root = default_remote_store_path(&home);
    let cache_root = default_sync_cache_path(&home);
    let synced = if let Some(repo) = repo {
        let tag = tag.unwrap_or("latest");
        let source = remote_root.join(repo).join(tag);
        let target = cache_root.join(repo).join(tag);
        if let Err(error) = copy_dir_all(&source, &target) {
            eprintln!("failed to sync package: {error}");
            return 1;
        }
        vec![target]
    } else {
        match sync_catalog(&remote_root, &cache_root) {
            Ok(synced) => synced,
            Err(error) => {
                eprintln!("failed to sync catalog: {error}");
                return 1;
            }
        }
    };

    match format {
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&synced).expect("sync targets should serialize")
        ),
        OutputFormat::Markdown => {
            println!("# Sync");
            println!();
            if synced.is_empty() {
                println!("- No repo packages were synced.");
            } else {
                for path in &synced {
                    println!("- Synced: `{}`", path.display());
                }
            }
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
    let remote_root = default_remote_store_path(&home_dir());
    match list_remote_repos(&remote_root) {
        Ok(repos) => {
            match format {
                OutputFormat::Json => println!(
                    "{}",
                    serde_json::to_string_pretty(&repos).expect("remote repos should serialize")
                ),
                OutputFormat::Markdown => {
                    println!("# Remote Repos");
                    println!();
                    if repos.is_empty() {
                        println!("- No remote repo packages found.");
                    } else {
                        for repo in &repos {
                            println!("- `{}`: {}", repo.repo_name, repo.tags.join(", "));
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
    let remote_root = default_remote_store_path(&home_dir());
    match build_catalog(&remote_root) {
        Ok(catalog) => {
            match format {
                OutputFormat::Json => println!(
                    "{}",
                    serde_json::to_string_pretty(&catalog).expect("catalog should serialize")
                ),
                OutputFormat::Markdown => {
                    println!("# Catalog");
                    println!();
                    println!("- Version: `{}`", catalog.version);
                    println!("- Generated at: `{}`", catalog.generated_at);
                    if catalog.repos.is_empty() {
                        println!("- No published repo indexes discovered.");
                    } else {
                        for repo in &catalog.repos {
                            println!(
                                "- `{}` (`{}`) latest `{}`",
                                repo.repo_name,
                                repo.repo_role.as_str(),
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

fn run_check_refresh(format: &str) -> i32 {
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

    match check_refresh(&repo_root) {
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
    let repo_root = match repo_root_from_cwd() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            return 1;
        }
    };

    match load_or_analyze_repo_index() {
        Ok(repo_index) => match validate_plan_file(&repo_root, &repo_index, Path::new(plan_path)) {
            Ok(validation) => {
                match format {
                    OutputFormat::Json => println!(
                        "{}",
                        serde_json::to_string_pretty(&validation)
                            .expect("plan validation should serialize")
                    ),
                    OutputFormat::Markdown => {
                        println!("# Validate Plan");
                        println!();
                        println!("- Plan: `{}`", validation.plan_path);
                        if let Some(warning) = &validation.freshness_warning {
                            println!("- Freshness warning: {}", warning);
                        }
                        if !validation.issues.is_empty() {
                            println!("- Issues: {}", validation.issues.join(", "));
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
                0
            }
            Err(error) => {
                eprintln!("failed to validate plan: {error}");
                1
            }
        },
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run_serve(request_file: Option<&str>, format: &str) -> i32 {
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
    let refresh = check_refresh(&repo_root).ok();

    if let Some(request_file) = request_file {
        let repo_index = match load_or_analyze_repo_index() {
            Ok(repo_index) => repo_index,
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        };
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
        let response = dispatch_mcp_request(&repo_index, request, refresh.as_ref());
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

    let snapshot = mcp_server_snapshot(refresh.as_ref());

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
            println!("- Request mode: use `--request-file <json>` to dispatch a tool call.");
        }
    }
    0
}

fn run_install_github_workflow(format: &str) -> i32 {
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
    let workflow_path = repo_root
        .join(".github")
        .join("workflows")
        .join("greentic-agent-index.yml");
    if let Some(parent) = workflow_path.parent()
        && let Err(error) = fs::create_dir_all(parent)
    {
        eprintln!("failed to prepare workflow directory: {error}");
        return 1;
    }
    if let Err(error) = fs::write(&workflow_path, render_installed_github_workflow()) {
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

    match load_or_analyze_repo_index() {
        Ok(repo_index) => {
            match format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&repo_index.workflow_graph)
                            .expect("workflow graph should serialize")
                    );
                }
                OutputFormat::Markdown => {
                    println!("# Workflows");
                    println!();
                    for workflow in repo_index.workflow_graph {
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

    match describe_here() {
        Ok(summary) => {
            print_summary(&summary, format);
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
        }
    }
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
}

impl SearchMode {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "code" => Ok(Self::Code),
            "instruction" => Ok(Self::Instruction),
            "concept" => Ok(Self::Concept),
            "reuse" => Ok(Self::Reuse),
            other => Err(format!("unsupported search mode: {other}")),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Instruction => "instruction",
            Self::Concept => "concept",
            Self::Reuse => "reuse",
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
}

impl SearchResultType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Instruction => "instruction",
            Self::Concept => "concept",
            Self::Reuse => "reuse",
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
    Build,
    Setup,
    Start,
    Runtime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RepoAgentManifest {
    version: String,
    repo_name: String,
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
    repo_name: String,
    repo_role: RepoRole,
    generated_at: String,
    freshness: FreshnessStatus,
    manifest: RepoAgentManifest,
    concept_graph: Vec<ConceptDescriptor>,
    workflow_graph: Vec<WorkflowDescriptor>,
    validations: Vec<ValidationDescriptor>,
    reuse: Vec<ReuseDescriptor>,
    instruction_graph: Vec<InstructionDescriptor>,
    instruction_paths: Vec<String>,
    source_stats: SourceStats,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RemoteRepo {
    repo_name: String,
    tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CatalogRepo {
    repo_name: String,
    repo_role: RepoRole,
    latest_tag: String,
    package_ref: String,
    updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Catalog {
    version: String,
    generated_at: String,
    repos: Vec<CatalogRepo>,
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
    repo_name: String,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct AnalyzeOutputs {
    manifest: RepoAgentManifest,
    repo_index: RepoIndex,
    fingerprints: Fingerprints,
    manifest_path: PathBuf,
    repo_index_path: PathBuf,
    fingerprints_path: PathBuf,
    registry_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct DescribeHere {
    version: String,
    repo_root: PathBuf,
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
            command: "greentic-coding-agent search --mode <mode> <query>".to_string(),
            purpose:
                "Search code, instructions, concepts, or reuse policy using the local repo index."
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
            command: "greentic-coding-agent package-index --tag <tag>".to_string(),
            purpose:
                "Build a local OCI-style package for the current repo index and generated agent docs."
                    .to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Tag".to_string()],
            outputs: vec![".greentic-agent/oci/<repo>/<tag>".to_string()],
            when_to_use:
                "Before publishing or inspecting a distributable repo index artifact.".to_string(),
        },
        CommandCatalogEntry {
            command: "greentic-coding-agent publish-index --tag <tag>".to_string(),
            purpose: "Publish the local OCI-style package into the configured remote store."
                .to_string(),
            phase: LifecyclePhase::Build,
            inputs: vec!["Tag".to_string()],
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
        .join(&repo_index.repo_name)
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

fn list_remote_repos(remote_root: &Path) -> Result<Vec<RemoteRepo>, std::io::Error> {
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
            for tag in tag_entries.flatten() {
                if tag.path().is_dir() {
                    tags.push(tag.file_name().to_string_lossy().to_string());
                }
            }
        }
        tags.sort();
        repos.push(RemoteRepo { repo_name, tags });
    }
    repos.sort_by(|left, right| left.repo_name.cmp(&right.repo_name));
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

fn sync_catalog(remote_root: &Path, cache_root: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let catalog = build_catalog(remote_root)?;
    let mut synced = Vec::new();
    for repo in catalog.repos {
        let source = remote_root.join(&repo.repo_name).join(&repo.latest_tag);
        let target = cache_root.join(&repo.repo_name).join(&repo.latest_tag);
        copy_dir_all(&source, &target)?;
        synced.push(target);
    }
    Ok(synced)
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

fn render_installed_github_workflow() -> String {
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
    };
    results.sort_by(|left, right| left.id.cmp(&right.id));

    SearchResponse {
        mode,
        query,
        results,
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
            mcp_tool("locate_owner", "Find the owner repo for a concept."),
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
                "list_remote_repos",
                "List repos currently available in the remote store.",
            ),
        ],
        freshness_warning: freshness_warning(refresh),
    }
}

fn dispatch_mcp_request(
    repo_index: &RepoIndex,
    request: McpRequest,
    refresh: Option<&RefreshCheck>,
) -> McpResponse {
    let McpRequest {
        id,
        tool,
        arguments,
    } = request;

    let result = match tool.as_str() {
        "describe_repo" => serde_json::to_value(repo_index).map_err(|error| error.to_string()),
        "list_workflows" => {
            serde_json::to_value(&repo_index.workflow_graph).map_err(|error| error.to_string())
        }
        "explain_concept" => {
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
        "search_code" => dispatch_search(repo_index, SearchMode::Code, &arguments),
        "search_instructions" => dispatch_search(repo_index, SearchMode::Instruction, &arguments),
        "search_reuse" => dispatch_search(repo_index, SearchMode::Reuse, &arguments),
        "locate_owner" => {
            let Some(concept_id) = arguments
                .get("concept_id")
                .and_then(serde_json::Value::as_str)
            else {
                return mcp_error(id, "missing `concept_id` argument");
            };
            serde_json::to_value(locate_owner(&repo_index.reuse, concept_id))
                .map_err(|error| error.to_string())
        }
        "plan_change" | "required_validations" => {
            let Some(task) = arguments.get("task").and_then(serde_json::Value::as_str) else {
                return mcp_error(id, "missing `task` argument");
            };
            serde_json::to_value(required_validations(repo_index, task))
                .map_err(|error| error.to_string())
        }
        "impact_analysis" => {
            let Some(symbol) = arguments.get("symbol").and_then(serde_json::Value::as_str) else {
                return mcp_error(id, "missing `symbol` argument");
            };
            serde_json::to_value(impact_analysis(repo_index, symbol, refresh))
                .map_err(|error| error.to_string())
        }
        "detect_changes" => {
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
                "freshness_warning": freshness_warning(refresh),
            }))
            .map_err(|error| error.to_string())
        }
        "show_freshness" => serde_json::to_value(serde_json::json!({
            "freshness_warning": freshness_warning(refresh)
        }))
        .map_err(|error| error.to_string()),
        "list_remote_repos" => {
            let remote_root = default_remote_store_path(&home_dir());
            serde_json::to_value(list_remote_repos(&remote_root).unwrap_or_default())
                .map_err(|error| error.to_string())
        }
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

fn dispatch_search(
    repo_index: &RepoIndex,
    mode: SearchMode,
    arguments: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let Some(query) = arguments.get("query").and_then(serde_json::Value::as_str) else {
        return Err("missing `query` argument".to_string());
    };
    serde_json::to_value(search_repo_index(repo_index, mode, query))
        .map_err(|error| error.to_string())
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
    let generated_at = timestamp_string();
    let head_sha = read_head_sha(&repo_root).unwrap_or_else(|| "unknown".to_string());
    let default_branch = read_default_branch(&repo_root);
    let candidate_docs = find_candidate_docs(&repo_root);
    let cargo_manifests = find_cargo_manifests(&repo_root);
    let tracked_files = find_tracked_files(&repo_root);
    let source_stats = build_source_stats(&repo_root, &cargo_manifests);
    let instruction_graph = build_instruction_graph(&repo_root, &source_stats);
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
        repo_name: repo_name.clone(),
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
        repo_name: repo_name.clone(),
        repo_role,
        generated_at: generated_at.clone(),
        freshness: FreshnessStatus::Fresh,
        manifest: manifest.clone(),
        concept_graph,
        workflow_graph,
        validations,
        reuse,
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

    let mut registry = load_registry(registry_path)?;
    registry.upsert(RegistryEntry {
        repo_name,
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
        return serde_json::from_str(&raw)
            .map_err(|error| format!("failed to parse {}: {error}", local_index_path.display()));
    }

    let outputs = analyze_repo(&repo_root, &default_registry_path(&home_dir()))?;
    Ok(outputs.repo_index)
}

fn load_repo_index_from_path(path: &Path) -> Result<RepoIndex, std::io::Error> {
    let raw = fs::read_to_string(path)?;
    serde_json::from_str(&raw)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
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
    let local_index_path = repo_root.join(LOCAL_INDEX_DIR).join("repo-index.json");
    let local_index = if local_index_path.exists() {
        let raw = fs::read_to_string(&local_index_path)
            .map_err(|error| format!("failed to read {}: {error}", local_index_path.display()))?;
        Some(
            serde_json::from_str::<RepoIndex>(&raw).map_err(|error| {
                format!("failed to parse {}: {error}", local_index_path.display())
            })?,
        )
    } else {
        None
    };

    Ok(DescribeHere {
        version: env!("CARGO_PKG_VERSION").to_string(),
        manifest_path: repo_root.join("Cargo.toml"),
        has_git_dir: repo_root.join(".git").exists(),
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

    Some(head.to_string())
}

fn read_default_branch(repo_root: &Path) -> Option<String> {
    let head = fs::read_to_string(repo_root.join(".git").join("HEAD")).ok()?;
    head.trim()
        .strip_prefix("ref: refs/heads/")
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
    dedup_sorted(&mut manifests);
    manifests
}

fn find_tracked_files(repo_root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    gather_regular_files(repo_root, repo_root, &mut files);
    dedup_sorted(&mut files);
    files
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

    descriptors.sort_by(|left, right| left.path.cmp(&right.path));
    descriptors
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
