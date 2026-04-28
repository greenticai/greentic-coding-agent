# Repository Overview

## 1. High-Level Purpose
`greentic-coding-agent` is a Rust workspace for Greentic coding-agent tooling. The product is a dedicated CLI and MCP/HTTP-oriented intelligence layer that helps agents understand repository structure, workflows, reuse rules, validation expectations, and remote catalog/package state across Greentic codebases.

The previous rewritten PR-01 through PR-12 plan is implemented locally and has been moved under `.codex/done/`. The current active `.codex` queue is PR-01 and PR-05 through PR-08, focused on shifting the product toward a branch-aware organisation-wide knowledge fabric. The active plan has been adapted to the current codebase: the new PR-02 branch/catalog metadata foundation, PR-03 producer workflow/multi-tag publishing, and PR-04 global consumer cache/status/channel workflow are implemented locally and moved to `.codex/done/`; PR-05 through PR-08 build on that channel-aware baseline. The completed historical PR-01 added an internal `gca-engine` service crate with coverage for analyze, describe, concept/workflow listing, command catalog listing, local search, owner/validation lookup, generated agent files, default GitHub workflow installation, package/local publish/sync helpers, remote repo/catalog reads, merged-index rebuild, refresh checks, MCP snapshot/dispatch, and MCP-backed impact/change/plan helpers. The completed historical PR-02 adds first-class `agent_training_course` support: authored `.greentic/training/*.course.v1.json` files are indexed into `RepoIndex.training_courses`, reflected into instruction/search metadata, exposed through query and engine APIs, surfaced in CLI `courses`, `course show`, `course recommend`, and `train`, and available through MCP training-course tools. The completed historical PR-03 adds first-class `knowledge_update` support: authored `.greentic/updates/*.update.v1.json` files are indexed into `RepoIndex.knowledge_updates`, reflected into instruction/Tantivy search metadata, exposed through query and engine APIs, surfaced in CLI `updates` and `updates show`, and available through MCP update tools. The completed historical PR-04 integrates training/update intelligence into decision flows: `course recommend` and `train` include important/breaking/critical update context before course output, `validate-plan` reports matching knowledge updates and blocks unacknowledged breaking/critical deprecated guidance, and search now supports `course` and `update` modes across fallback, Tantivy, CLI, query, and MCP dispatch paths. The completed historical PR-05 adds org-wide index rollout support with the built-in `repository_index_rollout` concept, testable `IndexRolloutPlan`/apply models, a `GitHubRepoClient` abstraction, `org plan-index-rollout`, `org apply-index-rollout`, offline repo-list planning, `gh`-backed live boundaries, and docs for deterministic rollout usage. The completed historical PR-06 seeds high-quality training-course and knowledge-update examples for core Greentic repos, adds recursive fixture validation for all `.course.v1.json` and `.update.v1.json` examples, and documents how authoritative repos should copy/adapt those seeds. The completed historical PR-07 adds user-local knowledge-update seen state in `~/.greentic-agent/agent-knowledge-state.json`, `updates --new`, `updates mark-seen <update_id>`, `updates mark-seen --all`, digest-change rediscovery, generated-agent-file recent update sections, and MCP tools for new-update listing and seen marking. The completed historical PR-08 improves repo extraction with `cargo metadata --format-version 1 --no-deps` for workspace/package/dependency/feature/target data, adds `syn`-based Rust symbol extraction with defaulted `SourceStats.rust_symbols`, and teaches code search/Tantivy indexing to use structured symbols while preserving old flat source-stat fields. The richer sync-state and normalized index cache behavior has been lifted into `gca-oci`, so engine and CLI sync use shared `SyncState`/`SyncReport`/`MergedIndexReport` models, CLI-compatible `remote-oci`/`cache-oci`/`indexes`/`sync-state.json` paths, branch/channel-aware normalized repo-index caching, digest-based skip detection, cached-index loading, and shared atomic merged Tantivy rebuilds with metadata. `gca-cli` now depends on `gca-core`/`gca-engine`/`gca-oci` for primary command behavior and is marked `publish = false` until a multi-crate release layout is chosen. The workspace already includes canonical repo identity and branch-aware catalog/repo-index metadata models, training-course and knowledge-update models, local seen-state tracking for update freshness, cargo-metadata-backed source extraction, structured Rust symbol indexing, core-repo course/update seed examples, Tantivy local and merged search, GHCR/ORAS-backed package/catalog transport, catalog membership management, org-wide index rollout planning/apply support, user-global sync/cache state, MCP stdio and localhost HTTP serving, watcher-based sync orchestration, nightly workflow generation, first-run bootstrap guidance, integration docs, migration compatibility, and fixture coverage. The CLI can analyze the current repository, write repo-local outputs under `.greentic-agent/`, update a user-global registry entry, initialize user-global cache directories, report sync status by channel, surface inferred concepts, workflows, authored training courses, authored knowledge updates, Cargo workspace/package metadata, and structured Rust symbols from Rust sources, markdown docs, `.codex` plans, `.greentic/training`, `.greentic/updates`, examples, and GitHub workflow files, run deterministic structured searches over local or merged Tantivy indexes, answer owner/validation questions for seeded Greentic concepts, render agent-ready training instructions with current update warnings, list task/concept/severity/new-filtered knowledge updates, mark update guidance as seen without auto-marking list operations, validate plans against deprecated update guidance with acknowledgements, plan/apply organization-wide workflow rollout PRs, generate `AGENTS.md`, `CLAUDE.md`, `CODEX.md`, and `llms.txt` with recent update context, package artifacts into an OCI-style layout with branch/commit metadata, package and publish repeated branch/SHA tags, publish/pull via local fixture or GHCR/ORAS backends, manage public and tenant catalogs, sync public/tenant indexes into branch-specific paths under `~/.greentic-agent`, rebuild merged indexes atomically, serve agent queries through MCP stdio or HTTP, and apply higher-confidence curated enrichment rules for core Greentic repos such as `greentic-types`, `greentic-pack`, `greentic-bundle`, `greentic-dev`, `greentic-x`, and `greentic-sorla`.

## 2. Main Components and Functionality
- **Path:** `Cargo.toml`
  - **Role:** Workspace root manifest.
  - **Key functionality:** Declares nine workspace members under `crates/` plus shared workspace metadata, currently version `0.1.2`, and common dependencies such as `clap`, `serde`, `serde_json`, `syn`, and `thiserror`.
  - **Key dependencies / integration points:** Provides the shared version and dependency baseline for the whole workspace.

- **Path:** `crates/gca-cli`
  - **Role:** Internal workspace binary crate named `greentic-coding-agent`.
  - **Key functionality:** Exposes working `analyze`, `bootstrap-instructions`, `describe --here`, `concepts`, `workflows`, `commands`, `courses`, `course show <id>`, `course recommend --task <task>`, `train --task <task> --audience coding_agent`, `updates [--new]`, `updates show <id>`, `updates mark-seen <id>|--all`, `catalog`, `org plan-index-rollout --org <org>`, `org apply-index-rollout --plan <plan.json>`, `search --mode code|instruction|concept|reuse|course|update --engine auto|tantivy|fallback --scope local|merged|all`, `locate-owner --concept <id>`, `required-validations --task <task>`, `generate-agent-files [--write-root]`, `package-index --tag <tag>`, `publish-index --tag <tag> --backend local|ghcr`, `list-remote-repos`, `show-catalog`, `init`, `status [--channel <branch>]`, `sync [--channel <branch>]`, `rebuild-merged-index`, `watch-indexes`, `check-refresh`, `install-github-workflow`, `impact --symbol <id>`, `detect-changes`, `validate-plan <plan.json>`, and `serve`. Primary commands such as describe, concepts, workflows, command catalog, training-course list/show/recommend/train, knowledge-update list/show/new filtering/seen marking, owner/validation lookup, generated files, local package/publish/sync, remote listing/catalog display, merged-index rebuild, check-refresh, and validate-plan now route through `gca-engine`/`gca-oci`; search, catalog mutation, org rollout CLI orchestration, watcher orchestration, serve transports, GHCR boundary behavior, analyze/bootstrap shaping, impact, and detect-changes still retain transitional CLI-local behavior where the engine surface is not yet equivalent.
  - **Key dependencies / integration points:** The crate now depends on internal workspace crates and is `publish = false`; a future release-layout PR must either publish/version the shared crates or provide a separate publishable wrapper before crates.io publishing resumes.

- **Path:** `crates/gca-engine`
  - **Role:** Internal shared service layer for CLI, MCP/HTTP/server mode, and future launcher integration.
  - **Key functionality:** Exposes `CodingAgentService` with typed option/response structs for analyze, describe, concepts, workflows, command catalog listing, training-course listing/show/recommendation/training with relevant update context, knowledge-update listing/show/new filtering/seen marking, user-local agent knowledge-state loading/writing, local search, owner lookup, required validations, generated agent-file writing, default GitHub workflow installation, package/local publish/sync helpers using the CLI-compatible `remote-oci`/`cache-oci`/`indexes`/`sync-state.json` layout, remote repo/catalog reads, merged-index rebuild through shared `gca-oci` sync-cache APIs, refresh checks, MCP snapshot/dispatch, impact, change detection, and structured plan validation with task summary, owner hints, validation hints, matched knowledge updates, acknowledged updates, freshness warning, and issues. It also owns the org rollout plan/apply models and `GitHubRepoClient` trait so rollout behavior is fake-client testable. It delegates to `gca-core`, `gca-index`, `gca-query`, `gca-agent-files`, `gca-oci`, and `gca-mcp` rather than duplicating those models.
  - **Key dependencies / integration points:** Covered by engine-level integration tests over synthetic repos. It is currently `publish = false`; the publishable CLI cannot depend on it until the shared-crate publish/release strategy is resolved.

- **Path:** `crates/gca-agent-files/src/lib.rs`
  - **Role:** Generated agent-document rendering and writing layer.
  - **Key functionality:** Renders deterministic `AGENTS.md`, `CLAUDE.md`, `CODEX.md`, and `llms.txt` content from `RepoIndex`, includes provenance/version stamping and concise recent important/breaking/critical knowledge updates that affect the repo, writes files into `.greentic-agent/generated/`, and optionally mirrors them to repo root.
  - **Key dependencies / integration points:** Uses `gca-core::RepoIndex` and `gca-query::command_catalog`; covered by golden-style content tests, missing-data fallback tests, and write-path tests.

- **Path:** `crates/gca-query/src/lib.rs`
  - **Role:** Query and command-catalog layer for indexed repo knowledge.
  - **Key functionality:** Defines structured search modes, result types, response payloads, a static command catalog, a seeded built-in policy bundle, repo-local policy loading from `.codex/policy` or `.greentic-agent/policy`, owner lookup, required-validation matching, training-course listing/show/recommendation, knowledge-update listing/show/task/concept recommendation, and Tantivy-backed search over local or merged indexes. Provides deterministic fallback searches over structured Rust symbols, code metadata, instruction graph entries, training course content, knowledge update content, concept graph entries, and reuse policy.
  - **Key dependencies / integration points:** Uses `gca-core::RepoIndex` as input and is covered by tests for code, structured Rust symbol search, instruction, training-course recommendation/search, knowledge-update filtering/search, concept, reuse, policy lookup, repo-local policy loading, and CLI-level Tantivy behavior.

- **Path:** `crates/gca-core/src/config.rs`
  - **Role:** Shared config and enum contract layer.
  - **Key functionality:** Defines `AgentConfig` plus the canonical serialized enums `OutputFormat`, `LifecyclePhase`, `KnowledgeScope`, `RepoRole`, and `FreshnessStatus`, with parsing and string-stability tests.
  - **Key dependencies / integration points:** These enums are the stable machine-readable vocabulary for later indexing, policy, search, and catalog features.

- **Path:** `crates/gca-core/src/model.rs`
  - **Role:** Canonical domain-model layer.
  - **Key functionality:** Defines `RepoId`, `RepoAgentManifest`, `RepoIndex`, `RepoIndexMetadata`, `ConceptDescriptor`, `WorkflowDescriptor`, `InstructionDescriptor`, `SourceStats`, `RustSymbolDescriptor`/`RustSymbolKind`, `ValidationDescriptor`, `ReuseDescriptor`, `TrainingCourseDescriptor` and related module/step/audience/deprecated-command types, `KnowledgeUpdateDescriptor` and related capability/replaced-guidance/migration/severity/type models, `AgentKnowledgeState`/`SeenKnowledgeUpdate`, `Catalog`, `CatalogRepo`, `CatalogBranchEntry`, catalog visibility/auth/change-log types, validation helpers, schema version constant `v1`, built-in concept IDs including `agent_training_course` and `knowledge_update`, and built-in concept descriptor generation. New outputs use canonical `repo_id` plus defaulted branch/commit/index metadata while old `repo_name`-only inputs and old flat catalog entries remain readable for one compatibility version, and old repo-index JSON without `metadata`, `training_courses`, `knowledge_updates`, or `rust_symbols` remains readable through serde defaults.
  - **Key dependencies / integration points:** These types are the intended shared contract for later PRs and are covered by JSON roundtrip tests and fixture-loading tests.

- **Path:** `crates/gca-core/src/registry.rs`
  - **Role:** User-global registry model and persistence helpers.
  - **Key functionality:** Defines `Registry` and `RegistryEntry`, supports empty/default registry creation, idempotent `upsert`, disk loading/writing, and regression tests for missing-file handling and update semantics.
  - **Key dependencies / integration points:** Used by `gca-index` for registry persistence; conceptually overlaps with CLI-local registry code that exists for publishability reasons.

- **Path:** `crates/gca-greentic/src/lib.rs`
  - **Role:** Greentic-specific enrichment heuristics.
  - **Key functionality:** Infers repo roles, concept descriptors, workflow descriptors, and known `gtc` command matches from scanned docs, workflow files, example paths, public Rust items, and repo naming. It also contains a seeded adapter registry for `greentic-types`, `greentic-pack`, `greentic-bundle`, `greentic-dev`, `greentic-x`, and `greentic-sorla`, adding curated docs of interest, concept ownership, and repo-specific workflow hints.
  - **Key dependencies / integration points:** Used by `gca-index` to enrich `RepoIndex`; covered by focused tests for generic inference plus adapter registration and seeded repo behavior.

- **Path:** `crates/gca-index/src/lib.rs`
  - **Role:** Analyze/bootstrap and enrichment engine.
  - **Key functionality:** Implements repo root detection, git HEAD/default-branch reading, candidate-doc discovery, Cargo manifest discovery, `cargo metadata --format-version 1 --no-deps` extraction with graceful string-parser fallback, tracked-file fingerprint gathering, `syn`-based Rust symbol parsing, docs/workflow/example scanning, `.greentic/training/*.course.v1.json` course loading, `.greentic/updates/*.update.v1.json` update loading, instruction-graph generation, source-stat extraction, Greentic concept/workflow enrichment, seeded/local policy loading, local output writing, local Tantivy index creation, and global registry updates. Returns structured `AnalyzeOutputs`, `Fingerprints`, and Tantivy build metadata.
  - **Key dependencies / integration points:** Uses `gca-core` models/registry helpers, `gca-greentic` heuristics, and `syn` for full Rust-file parsing; covered by temp-repo tests for local output creation, Cargo workspace metadata/inherited dependency extraction, structured Rust symbols, training-course indexing, knowledge-update indexing, instruction graph population, workflow inference, and idempotent registry updates.

- **Path:** `schemas/*.cddl`
  - **Role:** Canonical schema files for top-level formats and descriptors.
  - **Key functionality:** Define CDDL schemas for repo manifests, enriched repo indexes, catalogs, workflows, concepts, validations, reuse descriptors, training-course descriptors, knowledge-update descriptors, and structured Rust symbols using the same vocabulary as `gca-core`.
  - **Key dependencies / integration points:** Guide later serialization/validation work and now include `instruction_graph` and `source_stats`.

- **Path:** `examples/*.json`
  - **Role:** Example fixtures for the machine-readable contract layer.
  - **Key functionality:** Provide versioned example JSON for repo manifests, enriched repo indexes with structured Rust symbols, public and tenant catalogs, concepts, workflows, validations, reuse descriptors, MCP requests, HTTP search request bodies, workflow outputs, a sample plan file for `validate-plan`, minimal course/update examples, and repo-specific seed courses/updates under `examples/training/greentic-*` and `examples/updates/greentic-*` for greentic-component, greentic-pack, greentic-bundle, greentic-flow, greentic-dev, and greentic-types.
  - **Key dependencies / integration points:** Loaded recursively by `crates/gca-core/tests/examples.rs` and by `crates/gca-oci` tests to verify that committed examples remain in sync with the Rust types and workflow renderer.

- **Path:** `crates/gca-oci/src/lib.rs`
  - **Role:** OCI-style package/export and local sync layer.
  - **Key functionality:** Packages repo-local outputs and generated agent files into a deterministic OCI-like layout with `oci-layout`, `index.json`, blob digests, and artifact payloads; supports local fixture backend behavior, ORAS-based GHCR push/pull wrappers, remote config/auth resolution, public/tenant catalog merge semantics, remote repo listing, discovery catalog generation, refresh checks based on local fingerprints and versions, GitHub workflow rendering/installation helpers, and shared sync-state/cache primitives. The shared sync layer persists `~/.greentic-agent/sync-state.json`, writes normalized cached indexes under branch/channel-specific `~/.greentic-agent/indexes/.../<branch>/` paths, builds per-repo Tantivy caches, detects unchanged repos by digest/source/branch state, exposes cached repo indexes, and rebuilds merged Tantivy indexes atomically with metadata.
  - **Key dependencies / integration points:** Uses `gca-core::RepoIndex`, `gca-agent-files` rendering output, and `gca-index` Tantivy builders; network-free tests use the local fixture backend while GHCR behavior is isolated behind the ORAS abstraction.

- **Path:** `crates/gca-mcp`
  - **Role:** MCP-style tool-surface and heuristic assistant helpers.
  - **Key functionality:** Defines a machine-readable MCP-style tool list, server snapshot payload, request/response dispatch layer, training-course listing/show/recommendation tools, knowledge-update listing/show/recommendation/new/mark-seen tools, heuristic impact analysis, and plan-change validation helpers built on top of the indexed repo and seeded policy data.
  - **Key dependencies / integration points:** Uses `gca-core` and `gca-query`; CLI `serve` provides the long-running stdio/HTTP transport on top of these helper concepts.

- **Path:** `templates/README.md`
  - **Role:** Future customization entrypoint for generated agent docs.
  - **Key functionality:** Documents reserved template filenames. First-run bootstrap guidance now uses `crates/gca-cli/templates/CODEX_BOOTSTRAP.md.hbs` so the CLI package verifies correctly.
  - **Key dependencies / integration points:** Runtime agent-file generation still uses built-in deterministic renderers.

- **Path:** `README.md`, `ADMIN.md`, `examples/README.md`, `docs/training-update-seeds.md`
  - **Role:** User-facing, administrator-facing, contributor-facing, and executable documentation.
  - **Key functionality:** Document what Greentic Coding Agent does for non-technical users, developers, and coding agents; how admins set it up across Greentic repos for local indexes, generated agent files, GHCR publication, catalogs, and org rollout; plus seeded adapter model, server modes, training/update seed copy guidance, and runnable example artifacts such as `examples/plan.v1.json`, `examples/greentic-agent-index.workflow.yml`, `examples/greentic-agent-catalog.workflow.yml`, and MCP/HTTP request fixtures.
  - **Key dependencies / integration points:** README examples now align with the real CLI, and the committed workflow example is verified against the workflow renderer by tests.

- **Path:** `ci/local_check.sh`, `ci/check_package_contents.sh`
  - **Role:** Standard local CI wrapper and package validation helper.
  - **Key functionality:** Runs workspace format, clippy, tests, build, docs, package verification, and crates.io dry-run publication checks for every publishable crate. It should be rerun after each implementation slice or release metadata change.
  - **Key dependencies / integration points:** Used directly by developers and by GitHub Actions workflows.

- **Path:** `.github/workflows/ci.yml`, `publish.yml`, `perf.yml`, `nightly-coverage.yml`
  - **Role:** CI, release, perf smoke, and nightly coverage workflows.
  - **Key functionality:** Validate workspace health, publish the CLI crate, build release archives, run perf smoke checks, and enforce the coverage policy.
  - **Key dependencies / integration points:** `ci.yml` and `publish.yml` are thin wrappers around standard reusable workflows from `greenticai/.github`; release binaries/GitHub Release updates finish before crates.io publication, and local custom CI/release logic lives in `ci/local_check.sh` for developer validation rather than duplicated GitHub Actions jobs.

- **Path:** `coverage-policy.json`
  - **Role:** Coverage enforcement policy.
  - **Key functionality:** Requires a 60% default threshold, excludes thin scaffold crates and the CLI entrypoint, and keeps a stricter threshold on `crates/gca-core/src/lib.rs`.
  - **Key dependencies / integration points:** `greentic-dev coverage` currently passes with 91.36% workspace line coverage after the PR-11 additions; the perf scaling guard now tolerates heavier coverage instrumentation overhead via runtime environment detection.

- **Path:** `docs/architecture.md`, `docs/org-index-rollout.md`
  - **Role:** Long-term architecture document.
  - **Key functionality:** Describe the intended multi-crate indexing/query/policy/MCP system, remote OCI/GHCR model, generated agent-file pipeline, and org-wide index rollout usage.
  - **Key dependencies / integration points:** `docs/architecture.md` still functions more as roadmap than implementation reference for many areas; `docs/org-index-rollout.md` documents the implemented PR-05 rollout commands.

- **Path:** `.codex/PR-01-*.md`, `.codex/PR-05-*.md` through `.codex/PR-08-*.md`, `.codex/done/PR-*.md`, `.codex/global_rules.md`, `.codex/repo_overview_task.md`
  - **Role:** Codex planning and repo-maintenance instructions.
  - **Key functionality:** Define the active staged roadmap for global agent/MCP context, notifications, org rollout/catalog automation, and final compatibility/release hardening, while preserving the implemented historical roadmap and requiring ongoing maintenance of `.codex/repo_overview.md` plus `ci/local_check.sh` validation for future PR-style work.
  - **Key dependencies / integration points:** `.codex/done/` preserves the implemented historical PR slices. The active PR docs have been adapted to current implementation constraints: top-level `agent`, notification feeds, deeper catalog automation, and `cargo binstall` assumptions should not be treated as implemented until their owning PR lands.

## 3. Work In Progress, TODOs, and Stubs
- **Location:** `crates/gca-cli/src/main.rs`
  - **Status:** Partial / scaffold
  - **Short description:** The active PR command surface is implemented, but deeper semantic impact analysis and detect-changes integration with real staged/unstaged git state remain future work.

- **Location:** `crates/gca-oci/src/lib.rs:1`
  - **Status:** Partial
  - **Short description:** OCI-style packaging, cataloging, refresh checks, local fixture sync, and ORAS/GHCR command wrappers are implemented. Live registry smoke tests are intentionally not run by default and should stay env-gated/ignored.

- **Location:** `crates/gca-mcp/src/lib.rs:1`
  - **Status:** Partial
  - **Short description:** The crate exposes a real MCP-style tool catalog plus request/response dispatch helpers. The full long-running transports live in the CLI `serve` implementation rather than this library crate.

- **Location:** `templates/README.md:1`
  - **Status:** Placeholder
  - **Short description:** Built-in agent-file generation is implemented, but repo-customizable template overrides are still not wired into the runtime.

- **Location:** `docs/architecture.md:5`
  - **Status:** Draft
  - **Short description:** The architecture document contains both implemented integration notes and longer-term roadmap material. The focused docs under `docs/catalogs.md`, `docs/tenant-indexes.md`, `docs/server.md`, `docs/ghcr-format.md`, and `docs/workflow-installation.md` are more current for operator flows.

- **Location:** Whole repository search
  - **Status:** No inline TODO markers found
  - **Short description:** Repository-wide marker search still finds no `TODO`/`FIXME`/`XXX` comments; unfinished work is tracked mostly through scaffold crates and `.codex` planning docs.

## 4. Broken, Failing, or Conflicting Areas
- **Location:** `crates/gca-cli/src/main.rs` versus shared crates
  - **Evidence:** `gca-cli` now calls `gca-engine`/`gca-oci` for primary command groups, but still contains legacy local models/helpers for transitional command paths and output compatibility.
  - **Likely cause / nature of issue:** PR-01 chose a large but incremental CLI thinning. Search merged-scope handling, catalog mutation, watcher orchestration, serve transports, GHCR boundary behavior, analyze/bootstrap output shaping, impact, and detect-changes still need later engine/API parity before their duplicated CLI-local code can be deleted.

- **Location:** `crates/gca-oci/src/oras.rs`, `crates/gca-cli/src/main.rs`
  - **Evidence:** ORAS/GHCR support shells out to the `oras` CLI and is covered mostly by boundary/error tests to keep default tests network-free.
  - **Likely cause / nature of issue:** This is intentional for deterministic CI. Real GHCR smoke coverage should be added as an ignored/env-gated test if needed.

- **Location:** External `greentic-dev` integration
  - **Evidence:** PR-01 planning requires `gtc dev coding-agent ...` delegation, but this single-repo implementation could only prepare the binary and CLI surface locally.
  - **Likely cause / nature of issue:** The launcher change belongs in another repository, so this repo cannot complete that integration by itself.

- **Location:** `docs/architecture.md` compared with the implemented repo
  - **Evidence:** The architecture doc still includes some roadmap language around deeper semantic analysis and future cross-repo launcher integration.
  - **Likely cause / nature of issue:** Implemented behavior and product roadmap material coexist in the docs; the `.codex/done` queue tracks the completed implementation slices.

- **Location:** Active `.codex/PR-01-*.md` and `.codex/PR-05-*.md` through `.codex/PR-08-*.md` compared with implemented CLI/catalog behavior
  - **Evidence:** The active queue still describes top-level `agent`, notification feeds, deeper catalog automation, and `cargo binstall` release installation. The branch-aware model/catalog metadata foundation, repeated-tag producer workflow, and `sync --channel`/`status --channel` consumer workflow are now present, but the CLI crate is still `publish = false`.
  - **Likely cause / nature of issue:** These are planned next-slice behaviors. PR docs have been updated so later agent/daemon/catalog automation PRs rely on the implemented channel-aware baseline.

## 5. Notes for Future Work
- Decide how shared crates such as `gca-core`, `gca-greentic`, and `gca-index` should be versioned and published so the CLI can consume canonical types without duplicating them.
- Add ignored/env-gated live GHCR smoke tests if registry credentials and network access become available in CI.
- Expand MCP compatibility if external agent hosts require a stricter protocol implementation than the current stdio JSON-line tool dispatch.
- Add a release-layout PR that either publishes shared crates in dependency order and re-enables `greentic-coding-agent` crates.io publishing, or introduces a separate publishable wrapper around the internal workspace binary.
- Complete cross-repo launcher wiring in `greentic-dev` so `gtc dev coding-agent` actually delegates to this binary.
