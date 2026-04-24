# Greentic-coding-agent Architecture

## Status

Draft architecture intended for code execution planning.

## Purpose

Greentic-coding-agent is a dedicated repository and product that makes coding agents such as Codex, Claude Code, Cursor-style agents, and other MCP-capable tools highly productive across Greentic repositories, bundles, and workspaces.

It is **not** just a documentation generator and **not** just a search indexer. It is a **code + workflow + concept + policy intelligence layer** for Greentic.

Operational entrypoint:

```bash
gtc dev coding-agent ...
```

Implementation repository:

```text
greentic-coding-agent
```

This follows the existing Greentic-dev launcher pattern, while keeping the coding-agent engine, schemas, OCI packaging, GHCR discovery, and MCP surface in a dedicated repository.

---

## Goals

1. Give coding agents enough information in **very few calls** to become productive.
2. Make Greentic concepts machine-readable, not prose-only.
3. Prevent agents from editing the wrong repository or duplicating existing abstractions.
4. Make local indexing and remote published indexes work together.
5. Allow automatic nightly refresh checks and OCI publication to GHCR.
6. Support generated `AGENTS.md`, `CLAUDE.md`, `CODEX.md`, and `llms.txt`, while keeping machine-readable manifests as the source of truth.
7. Support cross-repo discovery of Greentic core, Greentic-X, Greentic-sorla, packs, bundles, flows, components, and solution repos.

---

## Non-goals

1. Replacing existing Greentic build or bundle tooling.
2. Requiring a hosted SaaS service.
3. Making natural language docs the canonical source of truth.
4. Hard-coding repo-specific rules in the CLI binary.
5. Solving every programming language on day one.

---

## Core design principles

### 1. Canonical machine-readable outputs
Everything important should be queryable as structured data:
- repo identity
- workflows
- concepts
- ownership boundaries
- reuse rules
- validations
- impact relationships
- freshness metadata

### 2. Few calls, high signal
A coding agent should typically need only:
1. `describe --here`
2. `plan "<task>"`
3. `search --mode instruction|reuse "<query>"`

### 3. Local-first, registry-enhanced
- local indexes are authoritative for the current checkout
- published OCI indexes provide adjacent repo intelligence
- local and remote indexes merge into one query graph

### 4. Reuse-first policy
The system must actively steer agents away from unnecessary new code by answering:
- who owns this concept?
- where does an existing implementation live?
- what repo should change first?
- where must this concept *not* be implemented?

### 5. Lifecycle-aware intelligence
Greentic is not just code structure. The system must understand:
- design/build time
- setup time
- start/runtime
- update/remove
- bundle activation and runtime effects

### 6. Generated agent files are views, not truth
`AGENTS.md`, `CLAUDE.md`, `CODEX.md`, and `llms.txt` are generated from indexed knowledge and policies.

---

## Lessons incorporated from GitNexus

The architecture deliberately adopts and extends several patterns that have proved useful in GitNexus:

1. **One-shot bootstrap (`analyze`)**  
   Indexing should be a single entry command that sets up agent-facing context quickly.

2. **Local portable repo index + global registry**  
   Repo-local index data should live in a hidden gitignored directory, while a user-global registry tracks known workspaces.

3. **MCP + CLI dual surface**  
   Coding agents need both direct CLI commands and MCP-exposed tools.

4. **Generated agent context files**  
   Agent-facing docs should be produced automatically so models can consume them immediately.

5. **Stale index detection**  
   The system should detect when the source repo has moved ahead of the indexed graph.

6. **Impact / blast-radius analysis and change detection**  
   Agents should understand what breaks if they modify a symbol, concept, or workflow.

Greentic-coding-agent extends these ideas with:
- Greentic lifecycle phases
- cross-repo ownership / reuse policy
- GHCR-published common index format
- catalog-based remote discovery
- first-class support for Greentic-X and Greentic-sorla concepts
- application pack / extension pack / digital worker / bundle semantics

---

## Top-level architecture

```text
+---------------------------------------------------------------+
|                        Greentic-dev CLI                        |
|                    gtc dev coding-agent ...                   |
+-------------------------------+-------------------------------+
                                |
                                v
+---------------------------------------------------------------+
|                    greentic-coding-agent CLI                  |
| analyze / describe / plan / search / impact / sync / serve   |
+-------------------------------+-------------------------------+
                                |
        +-----------------------+-----------------------+
        |                                               |
        v                                               v
+-----------------------------+              +------------------------------+
|        Local Indexer        |              |      Remote Index Client     |
| scans repo/workspace        |              | pulls catalog + repo indexes |
| creates repo graph          |              | from GHCR OCI artifacts      |
+-----------------------------+              +------------------------------+
        |                                               |
        +-----------------------+-----------------------+
                                v
+---------------------------------------------------------------+
|                  Unified Knowledge / Policy Layer             |
| repo manifests | concept graph | workflow graph | reuse map  |
| validation map | impact graph  | freshness state| ownership  |
+-------------------------------+-------------------------------+
                                |
        +-----------------------+-----------------------+
        |                                               |
        v                                               v
+-----------------------------+              +------------------------------+
|    CLI Query Responses      |              |          MCP Server          |
| json/cbor/markdown outputs  |              | tools for code agents        |
+-----------------------------+              +------------------------------+
        |
        v
+---------------------------------------------------------------+
|             Generated agent-facing artifacts/views            |
| AGENTS.md | CLAUDE.md | CODEX.md | llms.txt | manifest JSON  |
+---------------------------------------------------------------+
```

---

## Repository layout

Recommended initial repo structure:

```text
greentic-coding-agent/
  Cargo.toml
  README.md
  .codex/
  crates/
    gca-cli/
    gca-core/
    gca-index/
    gca-rust/
    gca-docs/
    gca-greentic/
    gca-policy/
    gca-query/
    gca-mcp/
    gca-oci/
    gca-agent-files/
    gca-ghcr-catalog/
  schemas/
    greentic.agent.repo-manifest.v1.cddl
    greentic.agent.repo-index.v1.cddl
    greentic.agent.catalog.v1.cddl
    greentic.agent.workflow.v1.cddl
    greentic.agent.concept.v1.cddl
    greentic.agent.validation.v1.cddl
    greentic.agent.reuse.v1.cddl
  templates/
    AGENTS.md.hbs
    CLAUDE.md.hbs
    CODEX.md.hbs
    llms.txt.hbs
    github-workflow-index.yml.hbs
  examples/
    sample-index/
    sample-catalog/
  docs/
    architecture.md
    workflow-installation.md
    ghcr-format.md
```

---

## Crate responsibilities

### `gca-cli`
CLI entrypoint for:
- analyze
- describe
- commands
- concepts
- workflows
- search
- plan
- impact
- detect-changes
- required-validations
- check-refresh
- package-index
- publish-index
- sync
- generate-agent-files
- install-github-workflow
- serve

### `gca-core`
Shared domain types, traits, config, error types, serialization helpers, canonical IDs.

### `gca-index`
Index orchestration pipeline:
- repo scanning
- graph building
- content normalization
- fingerprints
- incremental refresh logic

### `gca-rust`
Rust-specific extraction:
- Cargo metadata
- modules
- symbols
- traits / structs / enums / impls
- tests
- features
- doc comments
- workspace relationships

### `gca-docs`
Documentation and instruction extraction:
- README
- ARCHITECTURE
- RUNBOOK
- TESTING
- CONTRIBUTING
- `.codex/*.md`
- examples
- CI workflows
- docs folder

### `gca-greentic`
Greentic-specific enrichment:
- pack / bundle / flow / component understanding
- wizard / setup / start semantics
- digital worker terminology
- Greentic-X terminology
- Greentic-sorla terminology
- capability / hook / observer / static route semantics
- application pack / extension pack semantics

### `gca-policy`
Reuse-first and ownership rules:
- concept owner repo
- consumer repos
- forbidden duplication zones
- required cross-repo follow-ups
- suggested validations by change type

### `gca-query`
High-level query engine for:
- describe
- search
- explain concept
- locate owner
- locate extension point
- plan
- impact
- detect changes
- required validations

### `gca-mcp`
MCP server that exposes tools/resources based on the unified graph.

### `gca-oci`
Packaging and publishing:
- repo manifest packaging
- repo index packaging
- OCI artifact layout
- digest metadata
- push/pull support

### `gca-agent-files`
Generation of:
- `AGENTS.md`
- `CLAUDE.md`
- `CODEX.md`
- `llms.txt`
- optional `GREENTIC_AGENT.json`

### `gca-ghcr-catalog`
Pull/push/update logic for:
- central catalog artifact
- catalog merge
- remote discovery
- compatibility filtering

---

## Knowledge model

### 1. `RepoAgentManifest`
Lightweight discovery record.

Fields:
- repo_id
- repo_name
- org
- default_branch
- current_commit
- generated_at
- repo_kind
- repo_role
- schema_version
- generator_version
- index_uri
- latest_digest
- supported_concepts
- compatibility

Use:
- cheap discovery
- catalog building
- compatibility checks
- repo listing

### 2. `RepoIndex`
Full heavy index.

Sections:
- identity
  - repo_id
  - repo_name
- concept_graph
- workflow_graph
- ownership_graph
- reuse_graph
- symbol_graph
- instruction_graph
- validation_graph
- freshness
- source_stats

### 3. `ConceptDescriptor`
Canonical Greentic concept definitions.

Examples:
- digital_worker
- application_pack
- extension_pack
- bundle
- component
- flow
- wizard
- setup
- start
- Greentic-X
- Greentic-sorla
- capability_pack
- provider
- hook
- observer
- static_route

Each concept should capture:
- definition
- related concepts
- owner repo if applicable
- lifecycle phases
- examples
- forbidden misconceptions
- adjacent workflows

### 4. `WorkflowDescriptor`
Concrete task chains.

Examples:
- create_component
- create_application_pack
- create_extension_pack
- run_wizard_schema
- run_wizard_answers
- run_setup_schema
- run_setup_answers
- start_bundle
- publish_pack
- validate_bundle

Each workflow should capture:
- purpose
- phase
- inputs
- outputs
- commands
- required files
- validations
- likely adjacent repos
- error-prone confusions

### 5. `ValidationDescriptor`
Maps changes to checks.

Examples:
- changed schema => run shared schema tests, downstream fixtures, docs update
- changed pack semantics => run pack doctor, inspect snapshots, example regen
- changed setup model => run setup schema fixtures and bundle integration tests

### 6. `ReuseDescriptor`
Answers:
- where should this concept live?
- where does similar code already exist?
- what not to duplicate?
- what repo to inspect first?

---

## Index storage model

### Repo-local
Store analyzed data in a hidden gitignored directory:

```text
.greentic-agent/
```

Suggested structure:

```text
.greentic-agent/
  manifest.json
  repo-index.cbor
  repo-index.json
  fingerprints.json
  generated/
    AGENTS.md
    CLAUDE.md
    CODEX.md
    llms.txt
  cache/
```

### User-global registry
Store known workspaces locally:

```text
~/.greentic-agent/registry.json
```

Registry entries:
- repo path
- repo name
- repo role
- last analyzed commit
- local index path
- last sync metadata
- preferred remote index URI

---

## Local + remote merge model

### If inside a git checkout
- local working tree and local index are authoritative
- remote indexes provide adjacent repo context

### If outside a git checkout
- remote published indexes are used if available
- local registered indexes can override matching repos when explicitly requested

### Merge rules
1. Prefer local when repo root matches current workspace.
2. Prefer newer index generation timestamp only if source commit is equal.
3. If source commit differs, prefer the local working tree index for the active repo.
4. Keep provenance on every query result:
   - local index
   - remote OCI index
   - merged inference

---

## GHCR-distributed index model

### Why OCI
Greentic already thinks in OCI artifacts and GHCR distribution, so the index format should reuse that distribution model.

### Artifact naming
Recommended per-repo artifact path:

```text
ghcr.io/greenticai/indexes/<repo-name>:latest
ghcr.io/greenticai/indexes/<repo-name>:sha-<shortsha>
ghcr.io/greenticai/indexes/<repo-name>:<yyyy-mm-dd>
```

### Artifact contents
Recommended OCI layout:

```text
/
  repo-manifest.json
  repo-index.cbor
  repo-index.json
  agents/AGENTS.md
  agents/CLAUDE.md
  agents/CODEX.md
  agents/llms.txt
```

### Artifact media types
Suggested:
- `application/vnd.greentic.agent.repo-manifest.v1+json`
- `application/vnd.greentic.agent.repo-index.v1+cbor`
- `application/vnd.greentic.agent.repo-index.v1+json`
- `text/markdown` for generated agent docs

---

## Central catalog model

### Catalog artifact
A central catalog should also be published:

```text
ghcr.io/greenticai/indexes/catalog:latest
```

### Purpose
- list all known repo index artifacts
- allow cheap discovery without enumerating the registry namespace
- hold compatibility metadata
- provide latest digest per repo

### Catalog format
`greentic.agent.catalog.v1`

Catalog entries:
- repo_id
- repo_name
- org
- repo_role
- index_uri
- digest
- generated_at
- current_commit
- schema_version
- generator_version
- supported_concepts
- compatibility

### Catalog update strategy
**Recommended:** use a separate builder workflow/repo to regenerate the catalog from published repo manifests.  
Do not require each repo to write to the catalog directly.

## Implemented Integration Surface

The current implementation ties repo-local analysis, local Tantivy indexing, GHCR/ORAS package publication, public and tenant catalogs, sync-state recovery, merged index rebuilds, MCP stdio, HTTP serving, and watcher-driven refresh into one flow.

Primary docs:

- `docs/catalogs.md`
- `docs/tenant-indexes.md`
- `docs/server.md`
- `docs/ghcr-format.md`
- `docs/workflow-installation.md`

Compatibility matrix:

| Input type | Old `repo_name` only | New `repo_id` |
| --- | --- | --- |
| repo manifest | read with warning | canonical |
| repo index | read with warning | canonical |
| catalog | read with warning | canonical |
| registry | migrate on write | canonical |
| package/cache path | read old path | write new org/repo path |
| search/MCP response | add repo_id | canonical |

Exact legacy catalog warning:

```text
legacy repo_name-only input: repo_id missing; using inferred repo_id unknown/<repo_name> for this version
```

---

## Nightly refresh and publishing design

### Why nightly
Nightly gives:
- stale detection even if push-triggered workflows were skipped
- generator drift detection
- periodic repair of missing artifacts
- low-touch freshness guarantees

### Trigger model
Each participating repo should have:
- `push` on main
- nightly `schedule`
- `workflow_dispatch`

### Refresh conditions
Refresh if any of the following changed:
- HEAD commit SHA
- fingerprint of indexed files
- index schema version
- generator version
- extraction rules version
- repo role / metadata version
- forced rebuild input

### Recommended workflow steps
1. checkout
2. install toolchain + greentic-coding-agent binary
3. run `gtc dev coding-agent analyze --format json --out .greentic-agent/out`
4. run `gtc dev coding-agent check-refresh --out .greentic-agent/out`
5. if refresh needed:
   - `package-index`
   - publish to GHCR
   - optionally upload generated files as workflow artifacts
6. emit summary with source commit, digest, freshness reason

### Generated workflow install command
```bash
gtc dev coding-agent install-github-workflow
```

This should create/update a standard workflow file from a template.

---

## CLI design

### Bootstrap and maintenance
```bash
gtc dev coding-agent analyze
gtc dev coding-agent check-refresh
gtc dev coding-agent package-index
gtc dev coding-agent publish-index
gtc dev coding-agent install-github-workflow
gtc dev coding-agent generate-agent-files
gtc dev coding-agent sync
```

### Discovery and understanding
```bash
gtc dev coding-agent describe --here
gtc dev coding-agent commands
gtc dev coding-agent concepts
gtc dev coding-agent workflows
gtc dev coding-agent list-remote-repos
gtc dev coding-agent show-catalog
```

### Search and planning
```bash
gtc dev coding-agent search --mode code "..."
gtc dev coding-agent search --mode instruction "..."
gtc dev coding-agent search --mode reuse "..."
gtc dev coding-agent plan "..."
gtc dev coding-agent locate-owner --concept extension_pack
gtc dev coding-agent locate-extension-point --task "new setup question"
```

### Safety and verification
```bash
gtc dev coding-agent impact --symbol WorkflowDescriptor
gtc dev coding-agent detect-changes
gtc dev coding-agent required-validations --task "modify setup schema"
gtc dev coding-agent validate-plan plan.json
gtc dev coding-agent validate-scope --task "..."
```

### MCP
```bash
gtc dev coding-agent serve
```

---

## MCP tool surface

Recommended first MCP tools:

- `describe_repo`
- `list_workflows`
- `explain_concept`
- `search_code`
- `search_instructions`
- `search_reuse`
- `locate_owner`
- `locate_extension_point`
- `plan_change`
- `impact_analysis`
- `detect_changes`
- `required_validations`
- `show_freshness`
- `list_remote_repos`

Each MCP response should include:
- provenance
- freshness warning if stale
- confidence / inference marker if derived
- machine-stable identifiers

---

## Greentic-specific semantics

### Phases
Every command, workflow, and question should be tagged with one or more phases:
- design
- build
- setup
- start
- update
- remove
- runtime
- deploy

### Scopes
- repo
- workspace
- component
- flow
- pack
- bundle
- tenant
- team
- runtime
- org

### Repo roles
Examples:
- core-contracts
- cli-launcher
- component-authoring
- flow-authoring
- pack-authoring
- bundle-assembly
- solution-layer
- sorla-layer
- provider-family
- demo-app
- customer-solution
- examples-only

### First-class concepts
- digital worker
- application pack
- extension pack
- bundle
- flow
- component
- Greentic-X
- Greentic-sorla
- capability
- provider
- hook
- observer
- static route
- answer document
- schema
- manifest
- lock

---

## Reuse-first policy engine

The policy engine should answer questions such as:

### Example 1: extension pack schema
```json
{
  "concept": "extension_pack_schema",
  "owner_repo": "greentic-types",
  "consumers": ["greentic-pack", "greentic-bundle", "gtc"],
  "forbidden_in": ["greentic-dev", "customer app repos"],
  "required_followups": [
    "update inspect snapshots",
    "update doctor checks",
    "add example fixture"
  ]
}
```

### Example 2: setup question change
```json
{
  "task": "add setup question for public base URL",
  "phase": "setup",
  "likely_owner": "greentic-setup or setup-owning repo",
  "cross_repo_checks": [
    "bundle runtime assumptions",
    "start compatibility",
    "docs/examples update"
  ]
}
```

This is the mechanism that reduces overcoding and wrong-repo edits.

---

## Agent-facing generated files

### `AGENTS.md`
General repo guidance:
- repo purpose
- top workflows
- reuse rules
- risky areas
- mandatory validations
- command cheat sheet

### `CLAUDE.md`
Claude-optimized operating guide:
- best first MCP calls
- impact-before-edit rules
- stale-index reminder
- validation expectations
- high-risk edit policy

### `CODEX.md`
Codex-optimized execution guide:
- first discovery commands
- reviewable work packet expectations
- exact validation commands
- repo ownership reminders
- guidance to finish as much as safely possible

### `llms.txt`
Minimal routing file:
- most important docs
- top commands
- concepts to inspect first
- where to go for workflow vs policy vs architecture

These are generated from the same canonical graph.

---

## Security and trust

### Source of truth
- canonical code and extracted graph in repo
- signed/published index artifacts from trusted CI

### Trust recommendations
- publish only from trusted workflows
- include source commit metadata in artifacts
- sign OCI artifacts when signing infrastructure is ready
- verify digest on pull
- record origin/provenance on merged results

### Privacy
- local mode should work with no remote calls
- remote pull should be optional
- enterprise/private registries can later reuse the same format

---

## Compatibility model

Each index should declare:
- `index_schema_version`
- `generator_version`
- `compatible_min_tool_version`
- `compatible_max_tool_version`

This prevents silent mismatch as formats evolve.

---

## Example query flows

### Flow A: agent enters repo and needs orientation
1. `describe --here`
2. `workflows`
3. `search --mode reuse "<task>"`

### Flow B: user asks for a new extension pack type
1. `plan "Add new extension pack type"`
2. `locate-owner --concept extension_pack`
3. `required-validations --task "new extension pack type"`

### Flow C: working across repos with remote indexes
1. `sync`
2. `list-remote-repos`
3. `search --mode instruction "Greentic-sorla provider workflow"`

---

## Suggested phased implementation

### Phase 1
- repo scaffolding
- core schemas and types
- local analyze
- basic repo-local storage
- describe / concepts / workflows / search
- generated agent files
- `gtc dev` launcher integration

### Phase 2
- Greentic-specific enrichers
- reuse/ownership policy
- plan / locate-owner / required-validations
- stale detection
- package-index
- publish-index

### Phase 3
- GHCR repo index publication
- central catalog format + client
- sync / list-remote-repos / show-catalog
- install-github-workflow
- push + nightly publishing template

### Phase 4
- impact analysis
- detect changes
- richer MCP server
- merged local/remote inference
- compatibility enforcement
- signed artifacts

### Phase 5
- org-wide dashboards / browser explorer
- enterprise/private registry support
- deeper multi-repo planning and review packets

---

## Initial success criteria

1. In a known Greentic repo, an agent can get useful orientation in one call.
2. The system can tell an agent where a concept should be owned.
3. The system can produce a reviewable plan for a change.
4. Each repo can publish a standard OCI index to GHCR.
5. A central catalog can discover all published Greentic repo indexes.
6. Nightly workflows can determine whether refresh is required.
7. Agents can use generated `AGENTS.md` / `CLAUDE.md` / `CODEX.md` immediately.
8. Greentic-X and Greentic-sorla are first-class concepts in the model.

---

## Recommended first integration targets

1. `greentic-coding-agent` repo itself
2. `greentic-dev` launcher integration
3. `greentic-types`
4. `greentic-pack`
5. `greentic-bundle`
6. `greentic-x`
7. `greentic-sorla`

---

## Decision summary

### Chosen
- separate repo: `greentic-coding-agent`
- launcher entrypoint: `gtc dev coding-agent`
- local + remote published index model
- OCI publication to GHCR
- central catalog artifact
- generated agent docs as derived views
- nightly refresh checks with publish on change
- reuse-first policy and ownership graph
- Greentic-X + Greentic-sorla as first-class concepts

### Rejected
- docs-only solution
- manual repo-specific `AGENTS.md`
- GHCR discovery by raw registry enumeration only
- per-repo custom index formats
- local-only indexing with no organization discovery

---

## Next step

Implement the PR sequence in the included `.codex` files in order.
