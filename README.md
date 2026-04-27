# Greentic Coding Agent

Greentic Coding Agent helps people and AI coding agents understand Greentic repositories quickly.

It reads a repository, builds a local knowledge index, and answers questions such as:

- What is this repository for?
- Which concepts, workflows, courses, and knowledge updates apply here?
- What commands should I run for this task?
- Which repo owns a concept?
- What validation is required before changing something?
- What recent guidance has changed?

It can also generate agent-facing files such as `AGENTS.md`, `CODEX.md`, `CLAUDE.md`, and `llms.txt`.

For repository administrators, setup and rollout instructions are in [ADMIN.md](ADMIN.md).

## Who This Is For

### Non-Technical Users

Use this tool when you want an AI coding agent to work more safely in a Greentic repo.

Instead of asking the agent to guess how the repo works, ask it to use Greentic Coding Agent first. The tool gives the agent a structured summary of the repo, the important workflows, and any current instructions or warnings.

You usually do not need to understand the generated JSON files. The important outcome is that the agent has better context before it edits code.

### Developers

Use this tool locally when you are working in a Greentic repository and want indexed repo knowledge, command guidance, search, generated agent files, or cross-repo context.

### Coding Agents

Use this tool as your first orientation step in a Greentic repo. Prefer its structured outputs over guessing from filenames alone.

Recommended first calls:

```bash
greentic-coding-agent describe --here --format json
greentic-coding-agent concepts --format json
greentic-coding-agent workflows --format json
greentic-coding-agent updates --new --format json
```

For task-specific work:

```bash
greentic-coding-agent search --mode instruction "<task or keyword>" --format json
greentic-coding-agent locate-owner --concept <concept_id> --format json
greentic-coding-agent required-validations --task "<task>" --format json
greentic-coding-agent validate-plan examples/plan.v1.json --format json
```

## What It Creates

Running `analyze` creates repo-local files under:

```text
.greentic-agent/
```

These files contain the repository manifest, enriched repo index, fingerprints, local search index, generated agent files, and packaging data. They are intended for tooling and agents.

Generated human/agent files are written under:

```text
.greentic-agent/generated/
```

You can also mirror generated files to the repository root:

```bash
greentic-coding-agent generate-agent-files --write-root
```

That can create or update:

```text
AGENTS.md
CODEX.md
CLAUDE.md
llms.txt
```

## Basic Local Use

From inside a Greentic repository:

```bash
greentic-coding-agent analyze --print --format markdown
```

Then inspect the repo:

```bash
greentic-coding-agent describe --here --format markdown
greentic-coding-agent concepts --format markdown
greentic-coding-agent workflows --format markdown
greentic-coding-agent commands --format markdown
```

Search the indexed knowledge:

```bash
greentic-coding-agent search --mode instruction wizard --format markdown
greentic-coding-agent search --mode code analyze_repo --format markdown
greentic-coding-agent search --mode reuse component --format markdown
```

List training courses and knowledge updates:

```bash
greentic-coding-agent courses --format markdown
greentic-coding-agent course recommend --task "create a component" --format markdown
greentic-coding-agent updates --format markdown
greentic-coding-agent updates --new --format markdown
```

Mark updates as seen after reading them:

```bash
greentic-coding-agent updates mark-seen <update_id>
greentic-coding-agent updates mark-seen --all
```

Generate agent files:

```bash
greentic-coding-agent generate-agent-files --write-root
```

## Common Questions

### “What should I run before editing?”

```bash
greentic-coding-agent describe --here --format markdown
greentic-coding-agent updates --new --format markdown
greentic-coding-agent required-validations --task "<your task>" --format markdown
```

### “Where is this concept owned?”

```bash
greentic-coding-agent locate-owner --concept component --format markdown
```

### “What changed recently that agents need to know?”

```bash
greentic-coding-agent updates --new --format markdown
```

### “Can this tool serve an agent or MCP-style host?”

Yes. It can serve over stdio or HTTP:

```bash
greentic-coding-agent serve --stdio
greentic-coding-agent serve --http --host 127.0.0.1 --port 7757
```

HTTP mode exposes local endpoints such as health, search, catalog, and sync. See [docs/server.md](docs/server.md).

## Cross-Repo Knowledge

Greentic Coding Agent can package and publish repo indexes, sync public or tenant catalogs, and build a merged local search index.

For local use:

```bash
greentic-coding-agent sync --format markdown
greentic-coding-agent search --mode concept --scope merged wizard --format markdown
```

For repository setup and GHCR/catalog publishing, see [ADMIN.md](ADMIN.md).

## What The Index Contains

The index includes:

- repo identity and role
- concepts and workflow graph
- source statistics and Rust symbols
- Cargo workspace/package/dependency metadata
- markdown docs and GitHub workflow references
- training courses from `.greentic/training/*.course.v1.json`
- knowledge updates from `.greentic/updates/*.update.v1.json`
- reuse and ownership policy
- required validations
- generated agent-file content

## Project Layout

```text
crates/gca-cli          CLI entrypoint
crates/gca-engine       shared service layer
crates/gca-core         core data models
crates/gca-index        repo analysis and index writing
crates/gca-query        search, catalog, owner, validation, update queries
crates/gca-agent-files  generated AGENTS/CODEX/CLAUDE/llms renderers
crates/gca-oci          package, publish, sync, catalog, GHCR helpers
crates/gca-mcp          MCP-style tool surface
crates/gca-greentic     Greentic-specific enrichment
schemas/                CDDL schemas
examples/               example manifests, indexes, catalogs, courses, updates
docs/                   deeper implementation docs
```

## Local Development

Build and test the workspace:

```bash
bash ci/local_check.sh
```

Run from source:

```bash
cargo run -p greentic-coding-agent -- --help
cargo run -p greentic-coding-agent -- analyze --print --format json
```

Package-only validation:

```bash
bash ci/local_check.sh --mode package
```

## More Documentation

- [ADMIN.md](ADMIN.md): setup and rollout for Greentic repos
- [docs/catalogs.md](docs/catalogs.md): public and tenant catalogs
- [docs/tenant-indexes.md](docs/tenant-indexes.md): tenant index behavior
- [docs/server.md](docs/server.md): stdio and HTTP serving
- [docs/ghcr-format.md](docs/ghcr-format.md): GHCR/OCI package format
- [docs/workflow-installation.md](docs/workflow-installation.md): generated GitHub workflows
- [docs/training-update-seeds.md](docs/training-update-seeds.md): authored courses and updates
