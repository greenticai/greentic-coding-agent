# Server

`serve` exposes the same `QueryService` through MCP-style stdio and a small local HTTP API. Both transports use local repo indexes, cached catalog indexes, and the merged Tantivy index.

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
