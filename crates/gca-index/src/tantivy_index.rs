use gca_core::{RepoIndex, ReuseDescriptor};
use std::path::{Path, PathBuf};
use tantivy::schema::{STORED, STRING, Schema, TEXT};
use tantivy::{Index, doc};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TantivyBuildReport {
    pub index_path: PathBuf,
    pub documents_indexed: usize,
}

#[derive(Debug, Error)]
pub enum TantivyIndexError {
    #[error("failed to create tantivy index directory at {path}: {source}")]
    CreateDir {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to remove previous tantivy index at {path}: {source}")]
    RemoveDir {
        path: String,
        source: std::io::Error,
    },
    #[error("tantivy error: {0}")]
    Tantivy(#[from] tantivy::TantivyError),
}

#[derive(Debug, Clone)]
struct IndexDocument {
    path: String,
    kind: String,
    title: String,
    body: String,
    concept_ids: String,
    phase: String,
    provenance: String,
}

pub fn build_local_tantivy_index(
    repo_index: &RepoIndex,
    index_dir: &Path,
) -> Result<TantivyBuildReport, TantivyIndexError> {
    if index_dir.exists() {
        std::fs::remove_dir_all(index_dir).map_err(|source| TantivyIndexError::RemoveDir {
            path: index_dir.display().to_string(),
            source,
        })?;
    }
    std::fs::create_dir_all(index_dir).map_err(|source| TantivyIndexError::CreateDir {
        path: index_dir.display().to_string(),
        source,
    })?;

    let schema = local_schema();
    let repo_id = schema.get_field("repo_id")?;
    let path = schema.get_field("path")?;
    let kind = schema.get_field("kind")?;
    let title = schema.get_field("title")?;
    let body = schema.get_field("body")?;
    let concept_ids = schema.get_field("concept_ids")?;
    let phase = schema.get_field("phase")?;
    let provenance = schema.get_field("provenance")?;

    let index = Index::create_in_dir(index_dir, schema)?;
    let mut writer = index.writer(50_000_000)?;
    let documents = collect_documents(repo_index);
    for document in &documents {
        writer.add_document(doc!(
            repo_id => repo_index.repo_id.clone(),
            path => document.path.clone(),
            kind => document.kind.clone(),
            title => document.title.clone(),
            body => document.body.clone(),
            concept_ids => document.concept_ids.clone(),
            phase => document.phase.clone(),
            provenance => document.provenance.clone(),
        ))?;
    }
    writer.commit()?;

    Ok(TantivyBuildReport {
        index_path: index_dir.to_path_buf(),
        documents_indexed: documents.len(),
    })
}

pub fn local_schema() -> Schema {
    let mut builder = Schema::builder();
    builder.add_text_field("repo_id", STRING | STORED);
    builder.add_text_field("path", STRING | STORED);
    builder.add_text_field("kind", STRING | STORED);
    builder.add_text_field("title", TEXT | STORED);
    builder.add_text_field("body", TEXT | STORED);
    builder.add_text_field("concept_ids", TEXT | STORED);
    builder.add_text_field("phase", STRING | STORED);
    builder.add_text_field("provenance", STRING | STORED);
    builder.build()
}

fn collect_documents(repo_index: &RepoIndex) -> Vec<IndexDocument> {
    let mut documents = Vec::new();

    for concept in &repo_index.concept_graph {
        documents.push(IndexDocument {
            path: concept.related_paths.first().cloned().unwrap_or_default(),
            kind: "concept".to_string(),
            title: concept.title.clone(),
            body: concept.summary.clone(),
            concept_ids: concept.id.clone(),
            phase: concept.lifecycle_phase.as_str().to_string(),
            provenance: format!("concept_graph:{}", concept.id),
        });
    }

    for workflow in &repo_index.workflow_graph {
        documents.push(IndexDocument {
            path: workflow.docs.first().cloned().unwrap_or_default(),
            kind: "workflow".to_string(),
            title: workflow.title.clone(),
            body: format!("{} {}", workflow.summary, workflow.commands.join(" ")),
            concept_ids: workflow.concept_ids.join(" "),
            phase: workflow.phase.as_str().to_string(),
            provenance: format!("workflow_graph:{}", workflow.id),
        });
    }

    for instruction in &repo_index.instruction_graph {
        documents.push(IndexDocument {
            path: instruction.path.clone(),
            kind: "instruction".to_string(),
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
        documents.push(IndexDocument {
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
            phase: validation.phase.as_str().to_string(),
            provenance: format!("validations:{}", validation.id),
        });
    }

    for reuse in &repo_index.reuse {
        documents.push(reuse_document(reuse));
    }

    for module in &repo_index.source_stats.modules {
        documents.push(simple_source_document(
            "module",
            module,
            "source_stats.modules",
        ));
    }
    for item in &repo_index.source_stats.public_items {
        documents.push(simple_source_document(
            "code_symbol",
            item,
            "source_stats.public_items",
        ));
    }
    for dependency in &repo_index.source_stats.dependencies {
        documents.push(simple_source_document(
            "dependency",
            dependency,
            "source_stats.dependencies",
        ));
    }
    for path in &repo_index.source_stats.markdown_docs {
        documents.push(simple_source_document(
            "instruction",
            path,
            "source_stats.markdown_docs",
        ));
    }
    for path in &repo_index.source_stats.workflow_files {
        documents.push(simple_source_document(
            "workflow",
            path,
            "source_stats.workflow_files",
        ));
    }
    for path in &repo_index.source_stats.example_paths {
        documents.push(simple_source_document(
            "summary",
            path,
            "source_stats.example_paths",
        ));
    }

    documents
}

fn reuse_document(reuse: &ReuseDescriptor) -> IndexDocument {
    IndexDocument {
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
    }
}

fn simple_source_document(kind: &str, value: &str, provenance: &str) -> IndexDocument {
    IndexDocument {
        path: value.to_string(),
        kind: kind.to_string(),
        title: value.to_string(),
        body: value.to_string(),
        concept_ids: String::new(),
        phase: String::new(),
        provenance: provenance.to_string(),
    }
}

trait PhaseLabel {
    fn as_str(&self) -> &'static str;
}

impl PhaseLabel for gca_core::LifecyclePhase {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Design => "design",
            Self::Build => "build",
            Self::Setup => "setup",
            Self::Start => "start",
            Self::Runtime => "runtime",
            Self::Update => "update",
            Self::Remove => "remove",
        }
    }
}
