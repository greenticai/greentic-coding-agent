# Greentic Coding Agent

Greentic Coding Agent gives developers, Codex, Claude Code, and other coding agents an always-current local knowledge base for the Greentic engineering ecosystem.

Each Greentic repository can publish branch-specific indexes from GitHub Actions. Developers install one binary, sync the Greentic catalog, and agents can search across all repos, tutorials, courses, ownership rules, validation rules, workflows, and recent updates.

It answers questions such as:

- What is this repository for?
- Which concepts, workflows, courses, and knowledge updates apply here?
- What commands should I run for this task?
- Which repo owns a concept?
- What validation is required before changing something?
- What recent guidance has changed?

It can also generate agent-facing files such as `AGENTS.md`, `CODEX.md`, `CLAUDE.md`, and `llms.txt`.

For repository administrators, setup and rollout instructions are in [ADMIN.md](ADMIN.md).

## Current Stable Workflow

Install a built binary or GitHub release binary on `PATH`. The workspace still uses unpublished internal path crates, so `cargo install` and `cargo binstall` are not the documented install path for this release.

For daily developer and agent use, sync the organization knowledge once and query the merged local index:

```bash
greentic-coding-agent init --channel develop --format markdown
greentic-coding-agent sync --channel develop --format markdown
greentic-coding-agent status --channel develop --format markdown
greentic-coding-agent search --mode instruction --scope merged "component manifest" --format markdown
greentic-coding-agent serve --stdio
```

The `main` channel is for released/default branch knowledge. The `develop` channel is for active integration work. Producer workflows also publish immutable SHA tags so a catalog can point to branch heads while retaining exact commit provenance.

Existing repo-local commands remain supported:

```bash
greentic-coding-agent analyze --print --format json
greentic-coding-agent sync --format json
greentic-coding-agent search --mode instruction --scope merged "component manifest" --format json
greentic-coding-agent serve --stdio
```

## Who This Is For

### Non-Technical Users

Use this tool when you want an AI coding agent to work more safely across Greentic repos.

Instead of asking the agent to guess how the ecosystem works, ask it to use Greentic Coding Agent first. The tool gives the agent structured summaries, ownership information, workflows, and current instructions or warnings.

You usually do not need to understand the generated JSON files. The important outcome is that the agent has better context before it edits code.

### Developers

Use this tool locally when you want indexed Greentic knowledge, command guidance, search, generated agent files, or cross-repo context. Most daily use starts with `sync`, `status`, merged `search`, and `serve`.

### Coding Agents

Use this tool as your first orientation step for Greentic work. Prefer its structured outputs over guessing from filenames alone.

Recommended first calls:

```bash
greentic-coding-agent agent context --task "<task>" --format json
greentic-coding-agent updates --new --scope org --format json
greentic-coding-agent status --channel develop --format json
```

For task-specific work:

```bash
greentic-coding-agent search --mode instruction "<task or keyword>" --format json
greentic-coding-agent locate-owner --concept <concept_id> --format json
greentic-coding-agent required-validations --task "<task>" --format json
greentic-coding-agent validate-plan examples/plan.v1.json --format json
```

MCP-style hosts can use the stable tool names `gca.search`, `gca.agent_context`, `gca.find_owner`, `gca.required_validations`, `gca.recent_updates`, and `gca.branch_status`.

## How Indexes Are Produced

Repository workflows run `analyze`, package branch and SHA-tagged indexes, and publish them to GHCR:

```bash
greentic-coding-agent analyze --print --format json
greentic-coding-agent package-index --tag main --tag sha-<commit> --format json
greentic-coding-agent publish-index --tag main --tag sha-<commit> --backend ghcr --format json
```

The generated GitHub workflow publishes the current branch tag, such as `main` or `develop`, plus a `sha-<commit>` tag. Central catalogs can be rebuilt from published GHCR indexes and then published per channel:

```bash
greentic-coding-agent catalog rebuild-from-ghcr --org greenticai --channel develop --format json
greentic-coding-agent catalog publish --channel develop --backend ghcr --format json
```

See [ADMIN.md](ADMIN.md) and [docs/workflow-installation.md](docs/workflow-installation.md).

## Branch And Channel Model

Branch-aware catalogs map a repo to package entries for branches such as `main` and `develop`.

- `main`: stable/default branch knowledge.
- `develop`: active integration knowledge.
- `sha-<commit>`: immutable provenance for exact published index contents.

`sync --channel <channel>` downloads the catalog and matching repo indexes into the local cache. `status --channel <channel>` reports freshness and the selected branch/commit for each repo.

## Repo-Local Producer Mode

Repo-local `analyze` remains supported, but it is now mainly a producer, debugging, bootstrap, and working-tree overlay command.

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

## Basic Repo-Local Use

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

## Default Agent Workflow

Start by syncing the branch channel relevant to the task:

```bash
greentic-coding-agent sync --channel develop --format json
greentic-coding-agent agent context --task "<task>" --format json
greentic-coding-agent serve --stdio
```

When `serve` starts inside a checkout, it uses the merged global channel index first and adds the current repo as a local overlay. When it starts outside a checkout, it still serves synced organization knowledge.

## Common Questions

### “What should I run before editing?”

```bash
greentic-coding-agent agent context --task "<your task>" --format markdown
greentic-coding-agent updates --new --scope org --format markdown
greentic-coding-agent required-validations --task "<your task>" --format markdown
```

### “Where is this concept owned?”

```bash
greentic-coding-agent locate-owner --concept component --format markdown
```

### “What changed recently that agents need to know?”

```bash
greentic-coding-agent updates --new --scope org --format markdown
```

### “Can this tool serve an agent or MCP-style host?”

Yes. It serves the merged global index by default, with the current repo as a local overlay when started inside a checkout. It can serve over stdio or HTTP:

```bash
greentic-coding-agent serve --stdio
greentic-coding-agent serve --http --host 127.0.0.1 --port 7757
```

Agent-oriented direct commands are also available:

```bash
greentic-coding-agent agent context --task "add static route support" --format json
greentic-coding-agent agent preflight --task "add static route support" --repo greenticai/greentic-pack --format json
greentic-coding-agent agent owner --concept greentic.static-routes.v1 --format json
```

Stable MCP tool names include `gca.search`, `gca.agent_context`, `gca.find_owner`, `gca.required_validations`, `gca.recent_updates`, and `gca.branch_status`.

HTTP mode exposes local endpoints such as health, search, catalog, and sync. See [docs/server.md](docs/server.md).

## Cross-Repo Knowledge

Greentic Coding Agent can package and publish repo indexes, sync public or tenant catalogs, and build a merged local search index.

For local consumer use:

```bash
greentic-coding-agent sync --channel develop --format markdown
greentic-coding-agent status --channel develop --format markdown
greentic-coding-agent search --mode concept --scope merged wizard --format markdown
greentic-coding-agent watch --channel develop --poll 10m --format markdown
greentic-coding-agent updates --new --scope org --format markdown
greentic-coding-agent updates mark-seen --scope org --all --format markdown
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
- [docs/producer-vs-consumer.md](docs/producer-vs-consumer.md): CI producer and local consumer responsibilities
- [docs/local-cache-layout.md](docs/local-cache-layout.md): local cache and notification paths
- [docs/agent-global-usage.md](docs/agent-global-usage.md): Codex, Claude Code, and MCP usage
- [docs/tenant-indexes.md](docs/tenant-indexes.md): tenant index behavior
- [docs/server.md](docs/server.md): stdio and HTTP serving
- [docs/ghcr-format.md](docs/ghcr-format.md): GHCR/OCI package format
- [docs/workflow-installation.md](docs/workflow-installation.md): generated GitHub workflows
- [docs/training-update-seeds.md](docs/training-update-seeds.md): authored courses and updates
- [docs/migration-0.1.2.md](docs/migration-0.1.2.md): compatibility and migration guide
- [docs/release-notes-0.1.2.md](docs/release-notes-0.1.2.md): release notes
