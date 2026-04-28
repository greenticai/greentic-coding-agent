# Server

`serve` exposes the same `QueryService` through MCP-style stdio and a small local HTTP API. Both transports use the merged global index first, then the current repo index as a local working-tree overlay when the server is started inside a repository.

For full agent setup examples and the global daily workflow, see [agent-global-usage.md](agent-global-usage.md).

MCP stdio:

```bash
greentic-coding-agent serve --mcp
greentic-coding-agent serve --stdio
greentic-coding-agent serve --mcp --watch
```

HTTP:

```bash
greentic-coding-agent serve --http --host 127.0.0.1 --port 7757 --watch
```

Agent helpers:

```bash
greentic-coding-agent agent context --task "add static route support" --format json
greentic-coding-agent agent preflight --task "add static route support" --repo greenticai/greentic-pack --format json
greentic-coding-agent agent owner --concept greentic.static-routes.v1 --format json
```

Stable MCP tool names:

```text
gca.search
gca.agent_context
gca.find_owner
gca.required_validations
gca.recent_updates
gca.branch_status
```

Example request-file dispatch:

```bash
greentic-coding-agent serve --request-file examples/mcp-request.gca-agent-context.json --format json
```

Codex bootstrap:

```toml
[mcp_servers.greentic]
command = "greentic-coding-agent"
args = ["serve", "--stdio"]
```

Claude Code bootstrap:

```json
{
  "mcpServers": {
    "greentic": {
      "command": "greentic-coding-agent",
      "args": ["serve", "--stdio"]
    }
  }
}
```

Routes:

```text
GET  /healthz
GET  /readyz
GET  /status
GET  /catalog
GET  /repos
POST /search
POST /describe
POST /sync
POST /impact
POST /required-validations
```

Tokens are accepted from `--token`, `--token-env`, `GREENTIC_AGENT_TOKEN`, or `GHCR_TOKEN`, but status/debug output redacts them as `[redacted]`.
