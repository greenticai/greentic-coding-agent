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

#[cfg(test)]
mod tests {
    use super::*;
    use tantivy::doc;
    use tantivy::schema::{STORED, STRING, Schema, TEXT};

    #[test]
    fn parses_search_engine_choice() {
        assert_eq!(
            SearchEngineChoice::parse("auto"),
            Ok(SearchEngineChoice::Auto)
        );
        assert_eq!(
            SearchEngineChoice::parse("tantivy"),
            Ok(SearchEngineChoice::Tantivy)
        );
        assert_eq!(
            SearchEngineChoice::parse("fallback"),
            Ok(SearchEngineChoice::Fallback)
        );
        assert_eq!(
            SearchEngineChoice::parse("sqlite").expect_err("unsupported engine"),
            "unsupported search engine: sqlite"
        );
    }

    #[test]
    fn searches_index_filters_by_mode_and_maps_results() {
        let temp = tempfile::tempdir().expect("tempdir");
        let schema = test_schema();
        let repo_id = schema.get_field("repo_id").expect("repo_id");
        let path = schema.get_field("path").expect("path");
        let kind = schema.get_field("kind").expect("kind");
        let title = schema.get_field("title").expect("title");
        let body = schema.get_field("body").expect("body");
        let concept_ids = schema.get_field("concept_ids").expect("concept_ids");
        let provenance = schema.get_field("provenance").expect("provenance");
        let index = Index::create_in_dir(temp.path(), schema).expect("create index");
        let mut writer = index.writer(50_000_000).expect("writer");

        writer
            .add_document(doc!(
                repo_id => "repo-a",
                path => "src/lib.rs",
                kind => "module",
                title => "Search module",
                body => "needle handles code lookup",
                concept_ids => "code_search",
                provenance => "source_stats.modules",
            ))
            .expect("add module");
        writer
            .add_document(doc!(
                repo_id => "repo-a",
                path => "docs/policy.md",
                kind => "instruction",
                title => "Instruction policy",
                body => "needle explains the workflow",
                concept_ids => "agent_policy",
                provenance => "instruction_graph:policy",
            ))
            .expect("add instruction");
        writer
            .add_document(doc!(
                repo_id => "repo-a",
                path => "",
                kind => "concept",
                title => "Needle Concept",
                body => "concept summary",
                concept_ids => "needle_concept",
                provenance => "concept_graph:needle",
            ))
            .expect("add concept");
        writer.commit().expect("commit");

        let code = search_tantivy_index(temp.path(), SearchMode::Code, " needle ")
            .expect("code search succeeds");
        assert_eq!(code.mode, SearchMode::Code);
        assert_eq!(code.query, "needle");
        assert_eq!(code.results.len(), 1);
        assert_eq!(code.results[0].id, "module:src/lib.rs");
        assert_eq!(code.results[0].result_type, SearchResultType::Code);
        assert_eq!(code.results[0].snippet, "needle handles code lookup");
        assert_eq!(code.results[0].freshness, FreshnessStatus::Fresh);

        let instruction = search_tantivy_index(temp.path(), SearchMode::Instruction, "needle")
            .expect("instruction search succeeds");
        assert_eq!(instruction.results.len(), 1);
        assert_eq!(instruction.results[0].id, "instruction:docs/policy.md");
        assert_eq!(
            instruction.results[0].result_type,
            SearchResultType::Instruction
        );
        assert_eq!(
            instruction.results[0].provenance,
            "instruction_graph:policy"
        );

        let concept = search_tantivy_index(temp.path(), SearchMode::Concept, "needle_concept")
            .expect("concept search succeeds");
        assert_eq!(concept.results.len(), 1);
        assert_eq!(concept.results[0].id, "concept:Needle Concept");
        assert_eq!(concept.results[0].locator, "");
        assert_eq!(concept.results[0].result_type, SearchResultType::Concept);
    }

    #[test]
    fn reports_open_and_parse_errors() {
        let missing = tempfile::tempdir().expect("tempdir").path().join("missing");
        assert!(search_tantivy_index(&missing, SearchMode::Code, "needle").is_err());

        let temp = tempfile::tempdir().expect("tempdir");
        let index = Index::create_in_dir(temp.path(), test_schema()).expect("create index");
        index
            .writer::<TantivyDocument>(50_000_000)
            .expect("writer")
            .commit()
            .expect("commit");
        let error = search_tantivy_index(temp.path(), SearchMode::Code, "\"unterminated")
            .expect_err("query parse fails");
        assert!(!error.is_empty());
    }

    fn test_schema() -> Schema {
        let mut builder = Schema::builder();
        builder.add_text_field("repo_id", STRING | STORED);
        builder.add_text_field("path", STRING | STORED);
        builder.add_text_field("kind", STRING | STORED);
        builder.add_text_field("title", TEXT | STORED);
        builder.add_text_field("body", TEXT | STORED);
        builder.add_text_field("concept_ids", TEXT | STORED);
        builder.add_text_field("provenance", STRING | STORED);
        builder.build()
    }
}
