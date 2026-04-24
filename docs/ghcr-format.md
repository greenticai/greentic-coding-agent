# GHCR Format

Repo indexes are published as OCI-compatible artifacts. The canonical reference is:

```text
ghcr.io/greenticai/indexes/<org>/<repo>:<tag>
```

Tenant-private indexes use:

```text
ghcr.io/greenticai/indexes/tenants/<tenant>/<org>/<repo>:<tag>
```

Each package contains:

```text
oci-layout
index.json
artifacts/repo-manifest.json
artifacts/repo-index.json
artifacts/package-metadata.json
artifacts/agents/AGENTS.md
artifacts/agents/CLAUDE.md
artifacts/agents/CODEX.md
artifacts/agents/llms.txt
blobs/sha256/*
```

`repo_id` is the identity key. Local fixture tests use the same logical paths without network access; GHCR operations are delegated to ORAS.
