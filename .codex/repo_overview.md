# Repository Overview

## 1. High-Level Purpose
`greentic-coding-agent` is a Rust workspace for Greentic coding-agent tooling. The product is a dedicated CLI and MCP/HTTP-oriented intelligence layer that helps agents understand repository structure, workflows, reuse rules, validation expectations, and remote catalog/package state across Greentic codebases.

The active rewritten PR-01 through PR-10 plan is implemented locally. The workspace includes canonical repo identity and catalog models, Tantivy local and merged search, GHCR/ORAS-backed package/catalog transport, catalog membership management, user-global sync/cache state, MCP stdio and localhost HTTP serving, watcher-based sync orchestration, nightly workflow generation, first-run bootstrap guidance, integration docs, migration compatibility, and fixture coverage. The CLI can analyze the current repository, write repo-local outputs under `.greentic-agent/`, update a user-global registry entry, surface inferred concepts and workflows from Rust sources, markdown docs, `.codex` plans, examples, and GitHub workflow files, run deterministic structured searches over local or merged Tantivy indexes, answer owner/validation questions for seeded Greentic concepts, generate `AGENTS.md`, `CLAUDE.md`, `CODEX.md`, and `llms.txt`, package artifacts into an OCI-style layout, publish/pull via local fixture or GHCR/ORAS backends, manage public and tenant catalogs, sync public/tenant indexes into `~/.greentic-agent`, rebuild merged indexes atomically, serve agent queries through MCP stdio or HTTP, and apply higher-confidence curated enrichment rules for core Greentic repos such as `greentic-types`, `greentic-pack`, `greentic-bundle`, `greentic-dev`, `greentic-x`, and `greentic-sorla`.

## 2. Main Components and Functionality
- **Path:** `Cargo.toml`
  - **Role:** Workspace root manifest.
  - **Key functionality:** Declares eight workspace members under `crates/` plus shared workspace metadata and common dependencies such as `clap`, `serde`, `serde_json`, and `thiserror`.
  - **Key dependencies / integration points:** Provides the shared version and dependency baseline for the whole workspace.

- **Path:** `crates/gca-cli`
  - **Role:** Publishable binary crate named `greentic-coding-agent`.
  - **Key functionality:** Exposes working `analyze`, `bootstrap-instructions`, `describe --here`, `concepts`, `workflows`, `commands`, `catalog`, `search --mode code|instruction|concept|reuse --engine auto|tantivy|fallback --scope local|merged|all`, `locate-owner --concept <id>`, `required-validations --task <task>`, `generate-agent-files [--write-root]`, `package-index --tag <tag>`, `publish-index --tag <tag> --backend local|ghcr`, `list-remote-repos`, `show-catalog`, `sync`, `rebuild-merged-index`, `watch-indexes`, `check-refresh`, `install-github-workflow`, `impact --symbol <id>`, `detect-changes`, `validate-plan <plan.json>`, and `serve`. `serve` supports MCP/stdin, HTTP routes, status/token redaction, optional watcher mode, request-file dispatch, and shared query behavior through `QueryService`. `analyze` detects the repo root, scans Rust/docs/workflow/example files, writes enriched `.greentic-agent/manifest.json`, `.greentic-agent/repo-index.json`, `.greentic-agent/fingerprints.json`, and `.greentic-agent/tantivy/local`, and updates a user-global registry file under `$HOME/.greentic-agent/registry.json`.
  - **Key dependencies / integration points:** This is the crate packaged by `ci/local_check.sh` and published by `.github/workflows/publish.yml`; it is the binary that future `greentic-dev` launcher integration is meant to invoke.

- **Path:** `crates/gca-agent-files/src/lib.rs`
  - **Role:** Generated agent-document rendering and writing layer.
  - **Key functionality:** Renders deterministic `AGENTS.md`, `CLAUDE.md`, `CODEX.md`, and `llms.txt` content from `RepoIndex`, includes provenance/version stamping, writes files into `.greentic-agent/generated/`, and optionally mirrors them to repo root.
  - **Key dependencies / integration points:** Uses `gca-core::RepoIndex` and `gca-query::command_catalog`; covered by golden-style content tests, missing-data fallback tests, and write-path tests.

- **Path:** `crates/gca-query/src/lib.rs`
  - **Role:** Query and command-catalog layer for indexed repo knowledge.
  - **Key functionality:** Defines structured search modes, result types, response payloads, a static command catalog, a seeded built-in policy bundle, repo-local policy loading from `.codex/policy` or `.greentic-agent/policy`, owner lookup, required-validation matching, and Tantivy-backed search over local or merged indexes. Provides deterministic fallback searches over code metadata, instruction graph entries, concept graph entries, and reuse policy.
  - **Key dependencies / integration points:** Uses `gca-core::RepoIndex` as input and is covered by tests for code, instruction, concept, reuse, policy lookup, repo-local policy loading, and CLI-level Tantivy behavior.

- **Path:** `crates/gca-core/src/config.rs`
  - **Role:** Shared config and enum contract layer.
  - **Key functionality:** Defines `AgentConfig` plus the canonical serialized enums `OutputFormat`, `LifecyclePhase`, `KnowledgeScope`, `RepoRole`, and `FreshnessStatus`, with parsing and string-stability tests.
  - **Key dependencies / integration points:** These enums are the stable machine-readable vocabulary for later indexing, policy, search, and catalog features.

- **Path:** `crates/gca-core/src/model.rs`
  - **Role:** Canonical domain-model layer.
  - **Key functionality:** Defines `RepoId`, `RepoAgentManifest`, `RepoIndex`, `ConceptDescriptor`, `WorkflowDescriptor`, `InstructionDescriptor`, `SourceStats`, `ValidationDescriptor`, `ReuseDescriptor`, `Catalog`, `CatalogRepo`, catalog visibility/auth/change-log types, validation helpers, schema version constant `v1`, built-in concept IDs, and built-in concept descriptor generation. New outputs use canonical `repo_id` while old `repo_name`-only inputs remain readable for one compatibility version.
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
  - **Key functionality:** Implements repo root detection, git HEAD/default-branch reading, candidate-doc discovery, Cargo manifest discovery, tracked-file fingerprint gathering, Rust/docs/workflow/example scanning, instruction-graph generation, source-stat extraction, Greentic concept/workflow enrichment, seeded/local policy loading, local output writing, local Tantivy index creation, and global registry updates. Returns structured `AnalyzeOutputs`, `Fingerprints`, and Tantivy build metadata.
  - **Key dependencies / integration points:** Uses `gca-core` models/registry helpers and `gca-greentic` heuristics; covered by temp-repo tests for local output creation, instruction graph population, workflow inference, and idempotent registry updates.

- **Path:** `schemas/*.cddl`
  - **Role:** Canonical schema files for top-level formats and descriptors.
  - **Key functionality:** Define CDDL schemas for repo manifests, enriched repo indexes, catalogs, workflows, concepts, validations, and reuse descriptors using the same vocabulary as `gca-core`.
  - **Key dependencies / integration points:** Guide later serialization/validation work and now include `instruction_graph` and `source_stats`.

- **Path:** `examples/*.json`
  - **Role:** Example fixtures for the machine-readable contract layer.
  - **Key functionality:** Provide versioned example JSON for repo manifests, enriched repo indexes, public and tenant catalogs, concepts, workflows, validations, reuse descriptors, MCP requests, HTTP search request bodies, workflow outputs, and a sample plan file for `validate-plan`.
  - **Key dependencies / integration points:** Loaded by `crates/gca-core/tests/examples.rs` and `crates/gca-oci` tests to verify that committed examples remain in sync with the Rust types and workflow renderer.

- **Path:** `crates/gca-oci/src/lib.rs`
  - **Role:** OCI-style package/export and local sync layer.
  - **Key functionality:** Packages repo-local outputs and generated agent files into a deterministic OCI-like layout with `oci-layout`, `index.json`, blob digests, and artifact payloads; supports local fixture backend behavior, ORAS-based GHCR push/pull wrappers, remote config/auth resolution, public/tenant catalog merge semantics, remote repo listing, discovery catalog generation, refresh checks based on local fingerprints and versions, and GitHub workflow rendering/installation helpers.
  - **Key dependencies / integration points:** Uses `gca-core::RepoIndex` plus `gca-agent-files` rendering output; network-free tests use the local fixture backend while GHCR behavior is isolated behind the ORAS abstraction.

- **Path:** `crates/gca-mcp`
  - **Role:** MCP-style tool-surface and heuristic assistant helpers.
  - **Key functionality:** Defines a machine-readable MCP-style tool list, server snapshot payload, request/response dispatch layer, heuristic impact analysis, and plan-change validation helpers built on top of the indexed repo and seeded policy data.
  - **Key dependencies / integration points:** Uses `gca-core` and `gca-query`; CLI `serve` provides the long-running stdio/HTTP transport on top of these helper concepts.

- **Path:** `templates/README.md`
  - **Role:** Future customization entrypoint for generated agent docs.
  - **Key functionality:** Documents reserved template filenames. First-run bootstrap guidance now uses `crates/gca-cli/templates/CODEX_BOOTSTRAP.md.hbs` so the CLI package verifies correctly.
  - **Key dependencies / integration points:** Runtime agent-file generation still uses built-in deterministic renderers.

- **Path:** `README.md`, `examples/README.md`
  - **Role:** Contributor-facing usage and executable documentation.
  - **Key functionality:** Document the implemented command surface, seeded adapter model, public/tenant catalog flow, GHCR format, server modes, release flow, and runnable example artifacts such as `examples/plan.v1.json`, `examples/greentic-agent-index.workflow.yml`, `examples/greentic-agent-catalog.workflow.yml`, and MCP/HTTP request fixtures.
  - **Key dependencies / integration points:** README examples now align with the real CLI, and the committed workflow example is verified against the workflow renderer by tests.

- **Path:** `ci/local_check.sh`, `ci/check_package_contents.sh`
  - **Role:** Standard local CI wrapper and package validation helper.
  - **Key functionality:** Runs workspace format, clippy, tests, build, docs, package verification, and crates.io dry-run publication checks for every publishable crate. It currently passes after the PR-01 through PR-10 implementation.
  - **Key dependencies / integration points:** Used directly by developers and by GitHub Actions workflows.

- **Path:** `.github/workflows/ci.yml`, `publish.yml`, `perf.yml`, `nightly-coverage.yml`
  - **Role:** CI, release, perf smoke, and nightly coverage workflows.
  - **Key functionality:** Validate workspace health, publish the CLI crate, build release archives, run perf smoke checks, and enforce the coverage policy.
  - **Key dependencies / integration points:** `ci.yml` and `publish.yml` are thin wrappers around standard reusable workflows from `greenticai/.github`; local custom CI/release logic lives in `ci/local_check.sh` for developer validation rather than duplicated GitHub Actions jobs.

- **Path:** `coverage-policy.json`
  - **Role:** Coverage enforcement policy.
  - **Key functionality:** Requires a 60% default threshold, excludes thin scaffold crates and the CLI entrypoint, and keeps a stricter threshold on `crates/gca-core/src/lib.rs`.
  - **Key dependencies / integration points:** `greentic-dev coverage` currently passes with 91.36% workspace line coverage after the PR-11 additions; the perf scaling guard now tolerates heavier coverage instrumentation overhead via runtime environment detection.

- **Path:** `docs/architecture.md`
  - **Role:** Long-term architecture document.
  - **Key functionality:** Describes the intended multi-crate indexing/query/policy/MCP system, remote OCI/GHCR model, and generated agent-file pipeline.
  - **Key dependencies / integration points:** Still functions more as roadmap than implementation reference for many areas.

- **Path:** `.codex/PR-01-*.md` through `.codex/PR-12-*.md`, `.codex/global_rules.md`, `.codex/repo_overview_task.md`
  - **Role:** Codex planning and repo-maintenance instructions.
  - **Key functionality:** Define the staged implementation plan and require ongoing maintenance of `.codex/repo_overview.md` plus `ci/local_check.sh` validation for future PR-style work.
  - **Key dependencies / integration points:** PR-01 through PR-12 are now materially implemented in-code; the PR docs now mostly serve as historical implementation slices and orientation material.

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
- **Location:** `crates/gca-cli/src/main.rs` versus `crates/gca-core/src/config.rs`, `crates/gca-core/src/model.rs`, `crates/gca-core/src/registry.rs`, `crates/gca-greentic/src/lib.rs`, and `crates/gca-index/src/lib.rs`
  - **Evidence:** `gca-cli` still defines its own local copies of output format, manifest/index/fingerprint/registry structures, enrichment helpers, policy data, query logic, and generated-file rendering logic even though shared workspace crates now contain canonical/shared versions.
  - **Likely cause / nature of issue:** This duplication remains in place to keep the publishable CLI crate independent from unpublished workspace path dependencies during crates.io dry-run validation. It should be reconciled once the shared-crate publish strategy is decided.

- **Location:** `crates/gca-oci/src/oras.rs`, `crates/gca-cli/src/main.rs`
  - **Evidence:** ORAS/GHCR support shells out to the `oras` CLI and is covered mostly by boundary/error tests to keep default tests network-free.
  - **Likely cause / nature of issue:** This is intentional for deterministic CI. Real GHCR smoke coverage should be added as an ignored/env-gated test if needed.

- **Location:** External `greentic-dev` integration
  - **Evidence:** PR-01 planning requires `gtc dev coding-agent ...` delegation, but this single-repo implementation could only prepare the binary and CLI surface locally.
  - **Likely cause / nature of issue:** The launcher change belongs in another repository, so this repo cannot complete that integration by itself.

- **Location:** `docs/architecture.md` compared with the implemented repo
  - **Evidence:** The architecture doc still includes some roadmap language around deeper semantic analysis and future cross-repo launcher integration.
  - **Likely cause / nature of issue:** The active PR-01 through PR-10 queue is implemented, but product roadmap notes remain for later work.

## 5. Notes for Future Work
- Decide how shared crates such as `gca-core`, `gca-greentic`, and `gca-index` should be versioned and published so the CLI can consume canonical types without duplicating them.
- Add ignored/env-gated live GHCR smoke tests if registry credentials and network access become available in CI.
- Expand MCP compatibility if external agent hosts require a stricter protocol implementation than the current stdio JSON-line tool dispatch.
- Route CLI behavior through shared workspace crates once the publish strategy allows it, reducing the current mirrored analyze/describe/search/policy/rendering code in `gca-cli`.
- Finish PR-01 cross-repo launcher wiring in `greentic-dev` so `gtc dev coding-agent` actually delegates to this binary.
