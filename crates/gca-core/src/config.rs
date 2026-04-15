use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    Json,
    Markdown,
}

impl OutputFormat {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "json" => Ok(Self::Json),
            "markdown" => Ok(Self::Markdown),
            other => Err(format!("unsupported output format: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecyclePhase {
    Design,
    Build,
    Setup,
    Start,
    Runtime,
    Update,
    Remove,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeScope {
    LocalRepo,
    Workspace,
    CrossRepo,
    PublishedCatalog,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepoRole {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FreshnessStatus {
    Fresh,
    Stale,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentConfig {
    pub format: OutputFormat,
    pub registry_path: PathBuf,
    pub local_index_dir_name: String,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            format: OutputFormat::Markdown,
            registry_path: PathBuf::from(".greentic-agent/registry.json"),
            local_index_dir_name: ".greentic-agent".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AgentConfig, FreshnessStatus, KnowledgeScope, LifecyclePhase, OutputFormat, RepoRole,
    };
    use std::path::PathBuf;

    #[test]
    fn default_agent_config_matches_expected_scaffold() {
        let config = AgentConfig::default();

        assert_eq!(config.format, OutputFormat::Markdown);
        assert_eq!(
            config.registry_path,
            PathBuf::from(".greentic-agent/registry.json")
        );
        assert_eq!(config.local_index_dir_name, ".greentic-agent");
    }

    #[test]
    fn output_format_parser_supports_json_and_markdown() {
        assert_eq!(OutputFormat::parse("json"), Ok(OutputFormat::Json));
        assert_eq!(OutputFormat::parse("markdown"), Ok(OutputFormat::Markdown));
        assert!(OutputFormat::parse("yaml").is_err());
    }

    #[test]
    fn enum_string_stability_matches_expected_contract() {
        assert_eq!(
            serde_json::to_string(&LifecyclePhase::Runtime).unwrap(),
            "\"runtime\""
        );
        assert_eq!(
            serde_json::to_string(&KnowledgeScope::CrossRepo).unwrap(),
            "\"cross_repo\""
        );
        assert_eq!(
            serde_json::to_string(&RepoRole::CliLauncher).unwrap(),
            "\"cli_launcher\""
        );
        assert_eq!(
            serde_json::to_string(&FreshnessStatus::Stale).unwrap(),
            "\"stale\""
        );
        assert_eq!(
            serde_json::to_string(&OutputFormat::Markdown).unwrap(),
            "\"markdown\""
        );
    }
}
