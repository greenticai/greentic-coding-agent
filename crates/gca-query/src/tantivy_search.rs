use crate::{SearchMode, SearchResponse, SearchResult, SearchResultType};
use gca_core::FreshnessStatus;
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Value};
use tantivy::{Index, TantivyDocument};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchEngineChoice {
    Auto,
    Tantivy,
    Fallback,
}

impl SearchEngineChoice {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "auto" => Ok(Self::Auto),
            "tantivy" => Ok(Self::Tantivy),
            "fallback" => Ok(Self::Fallback),
            other => Err(format!("unsupported search engine: {other}")),
        }
    }
}

pub fn search_tantivy_index(
    index_dir: &Path,
    mode: SearchMode,
    query: &str,
) -> Result<SearchResponse, String> {
    let index = Index::open_in_dir(index_dir).map_err(|error| error.to_string())?;
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
        let document: TantivyDocument = searcher.doc(address).map_err(|error| error.to_string())?;
        let document_kind = field_text(&document, kind).unwrap_or_default();
        if !kind_matches(mode, &document_kind) {
            continue;
        }
        let title_value = field_text(&document, title).unwrap_or_default();
        let locator = field_text(&document, path).unwrap_or_default();
        let provenance_value = field_text(&document, provenance).unwrap_or_default();
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
            repo_id: field_text(&document, repo_id).unwrap_or_default(),
            id,
            title: title_value.clone(),
            result_type: result_type_for_kind(&document_kind),
            locator,
            snippet: field_text(&document, body).unwrap_or(title_value),
            provenance: provenance_value,
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

fn field_text(document: &TantivyDocument, field: Field) -> Option<String> {
    document
        .get_first(field)
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
}

fn kind_matches(mode: SearchMode, kind: &str) -> bool {
    match mode {
        SearchMode::Code => matches!(kind, "code_symbol" | "module" | "dependency"),
        SearchMode::Instruction => kind == "instruction",
        SearchMode::Concept => kind == "concept",
        SearchMode::Reuse => kind == "reuse",
        SearchMode::Course => kind == "course",
        SearchMode::Update => kind == "update",
    }
}

fn result_type_for_kind(kind: &str) -> SearchResultType {
    match kind {
        "instruction" => SearchResultType::Instruction,
        "concept" => SearchResultType::Concept,
        "reuse" => SearchResultType::Reuse,
        "course" => SearchResultType::Course,
        "update" => SearchResultType::Update,
        _ => SearchResultType::Code,
    }
}
