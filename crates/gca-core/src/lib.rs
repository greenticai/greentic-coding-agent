mod config;
mod describe;
mod headings;
mod model;
mod registry;

pub use config::{
    AgentConfig, FreshnessStatus, KnowledgeScope, LifecyclePhase, OutputFormat, RepoRole,
};
pub use describe::{DescribeHere, DescribeHereError, describe_here};
pub use headings::{
    Heading, build_heading_index, extract_markdown_headings, repeated_heading_index_workload,
};
pub use model::{
    AgentKnowledgeState, AuthKind, BUILTIN_CONCEPT_IDS, CapabilityAnnouncement, Catalog,
    CatalogAction, CatalogChange, CatalogRepo, ConceptDescriptor, DeprecatedCommandDescriptor,
    IndexVisibility, InstructionDescriptor, KnowledgeUpdateDescriptor, KnowledgeUpdateSeverity,
    KnowledgeUpdateType, MigrationStepDescriptor, ReplacedGuidanceDescriptor, RepoAgentManifest,
    RepoId, RepoIndex, ReuseDescriptor, RustSymbolDescriptor, RustSymbolKind, SCHEMA_VERSION_V1,
    SeenKnowledgeUpdate, SourceStats, TrainingAudience, TrainingCourseDescriptor,
    TrainingModuleDescriptor, TrainingStepDescriptor, ValidationDescriptor, WorkflowDescriptor,
    builtin_concepts,
};
pub use registry::{Registry, RegistryEntry, RegistryError, load_registry, write_registry};
