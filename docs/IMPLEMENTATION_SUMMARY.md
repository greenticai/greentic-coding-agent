# Implementation Summary

## Final target architecture

```text
Current checkout
  ├─ .greentic-agent/repo-index.json
  ├─ .greentic-agent/tantivy/local
  └─ optional generated agent files

Global cache
  ├─ public catalog
  ├─ tenant catalogs
  ├─ downloaded repo indexes
  └─ merged Tantivy index

Server
  ├─ MCP stdio
  ├─ HTTP JSON API
  ├─ update watcher
  └─ automatic sync + merge
```

## Identity rule

All repo references use:

```text
repo_id = org/repo
```

Examples:

```text
greenticai/greentic-pack
greentic-biz/greentic-demo
```

## Catalog rule

The overall index is controlled by catalogs.

Public:

```text
ghcr.io/greenticai/indexes/catalog:latest
```

Tenant:

```text
ghcr.io/greenticai/indexes/tenants/<tenant>/catalog:latest
```

## Server rule

Coding agents should be able to either:

```bash
greentic-coding-agent serve --mcp --watch
```

or:

```bash
greentic-coding-agent serve --http --host 127.0.0.1 --port 7757 --watch
```

The server keeps the merged index updated automatically.
