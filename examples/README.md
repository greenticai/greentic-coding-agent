# Examples

This directory contains committed example artifacts for the current implemented feature set.

- `repo-manifest.v1.json`
  Example local manifest output written by `analyze`.
- `repo-index.v1.json`
  Example enriched repo index with concepts, workflows, validations, reuse, and source stats.
- `catalog.v1.json`
  Example discovery catalog payload.
- `catalog.public.v1.json`, `catalog.tenant.meeza.v1.json`
  Example public and tenant catalog payloads used by sync.
- `concept.v1.json`, `workflow.v1.json`, `validation.v1.json`, `reuse.v1.json`
  Example descriptor payloads used by fixture tests.
- `greentic-agent-index.workflow.yml`, `greentic-agent-catalog.workflow.yml`
  Example installed GitHub workflow outputs from `install-github-workflow`.
- `mcp-request.describe-repo.json`, `mcp-request.search-all.json`
  Example MCP-style request payloads for stdio/request-file dispatch.
- `server-search-request.json`
  Example HTTP `/search` request body.
- `plan.v1.json`
  Example plan file that can be checked with `validate-plan`.

Runnable examples:

```bash
cargo run -p greentic-coding-agent -- describe --here --format markdown
cargo run -p greentic-coding-agent -- check-refresh --format json
cargo run -p greentic-coding-agent -- serve --format json
cargo run -p greentic-coding-agent -- serve --request-file examples/mcp-request.describe-repo.json --format json
cargo run -p greentic-coding-agent -- serve --request-file examples/mcp-request.search-all.json --format json
cargo run -p greentic-coding-agent -- validate-plan examples/plan.v1.json --format markdown
```
