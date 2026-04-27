# PR-06 — Implement full MCP server and CLI query server

## Goal

The current repo has CLI access and placeholder MCP. Implement a real long-running server that coding agents can contact.

There must be two server modes:

1. MCP server mode for MCP-capable agents.
2. CLI/HTTP server mode for simple local agent clients.

Both server modes must query the same local and merged indexes.

## Depends on

- PR-01 repo identity migration.
- PR-02 query engine abstraction.
- PR-05 local/global cache and merged index.

## Implementation split

Keep this PR reviewable by splitting the work into two internal phases.

Phase A:

- shared `QueryService`
- MCP stdio server
- real MCP tool dispatch against local and merged indexes
- no HTTP server yet

Phase B:

- HTTP JSON API
- shared server runtime config
- status/health/readiness endpoints

If this gets too large in implementation, make Phase B a follow-up PR and keep PR-06 focused on `QueryService` plus MCP stdio.

## Commands

```bash
greentic-coding-agent serve --mcp
greentic-coding-agent serve --http --host 127.0.0.1 --port 7757
greentic-coding-agent serve --stdio
greentic-coding-agent serve --watch
greentic-coding-agent serve --tenant meeza --token $TOKEN --watch
```

Defaults:

```bash
greentic-coding-agent serve
```

should run MCP over stdio, because many coding agents expect stdio MCP.

## Server responsibilities

The server must:

- load current repo index if inside a repo
- load global merged index from `~/.greentic-agent/tantivy/merged`
- expose search, describe, concepts, workflows, ownership, impact, validations
- optionally watch for catalog/index updates
- automatically sync and rebuild merged index
- return provenance and freshness metadata

## MCP tools

Implement these MCP tools:

```text
describe_repo
list_workflows
explain_concept
search_code
search_instructions
search_reuse
search_all
locate_owner
locate_extension_point
plan_change
impact_analysis
detect_changes
required_validations
show_freshness
list_indexed_repos
sync_indexes
show_catalog
```

## MCP resources

Expose resources:

```text
greentic://repo/current/manifest
greentic://repo/current/index
greentic://catalog/public
greentic://catalog/tenant/<tenant>
greentic://indexes/merged/status
```

## HTTP server

Add simple JSON API:

```text
GET  /healthz
GET  /readyz
GET  /status
POST /search
POST /describe
POST /sync
POST /impact
POST /required-validations
GET  /catalog
GET  /repos
```

Example:

```bash
curl -X POST http://127.0.0.1:7757/search   -H 'content-type: application/json'   -d '{"query":"wizard setup","scope":"all"}'
```

## Crate structure

Use existing `gca-mcp`, and optionally add `gca-server`.

Recommended:

```text
crates/gca-server/
  src/lib.rs
  src/http.rs
  src/runtime.rs
  src/watch.rs

crates/gca-mcp/
  src/lib.rs
  src/tools.rs
  src/protocol.rs
```

Keep MCP logic separate from HTTP transport.

The shared runtime should live outside the transport crates:

```text
crates/gca-server/
  src/query_service.rs
  src/runtime.rs
  src/status.rs
  src/http.rs
  src/watch.rs
```

MCP and HTTP should both call the same `QueryService`; neither transport should implement search, sync, impact, or validation logic directly.

## Runtime config

```rust
pub struct ServerConfig {
    pub mode: ServerMode,
    pub host: String,
    pub port: u16,
    pub watch: bool,
    pub sync_interval_seconds: u64,
    pub tenant: Option<String>,
    pub token: Option<String>,
    pub catalog_ref: Option<String>,
    pub tenant_catalog_ref: Option<String>,
}
```

## Security

Default HTTP bind must be:

```text
127.0.0.1
```

Never bind to `0.0.0.0` unless user explicitly passes:

```bash
--host 0.0.0.0
```

Do not log tokens.

`ServerConfig` and status responses must redact tokens. Tests should assert that raw token values never appear in formatted debug output, JSON status output, or error messages.

## Tests

- MCP tool list includes required tools.
- HTTP `/healthz` returns ok.
- Search endpoint works against fixture merged index.
- Server config redacts token in debug/status output.
- Default host is localhost.
- CLI, MCP, and HTTP search use the same `QueryService` fixture and return compatible result data.
- `serve` without flags defaults to MCP over stdio.

## Acceptance criteria

- Coding agents can contact the server without spawning a new process per query.
- MCP stdio mode works.
- HTTP mode works locally.
- Same query engine powers CLI, MCP, and HTTP.
