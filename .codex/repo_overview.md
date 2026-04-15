# Repository Overview

## 1. High-Level Purpose
`greentic-coding-agent` is a Rust workspace for Greentic coding-agent tooling. The intended product is a dedicated CLI and MCP-oriented intelligence layer that helps agents understand repository structure, workflows, reuse rules, validation expectations, and eventually remote catalog/package state across Greentic codebases.

The repository now includes the full planned PR-01 through PR-12 foundation set: a multi-crate workspace scaffold, a canonical schema/domain-model layer, a working local bootstrap path, enrichment/indexing, structured query, seeded reuse/validation policy, generated agent-facing documentation, OCI-style local packaging/sync, catalog/refresh/workflow automation, MCP/impact/change-detection helpers, seeded cross-repo Greentic adapters, and polish around examples/docs/validation. The CLI can analyze the current repository, write repo-local outputs under `.greentic-agent/`, update a user-global registry entry, surface inferred concepts and workflows from Rust sources, markdown docs, `.codex` plans, examples, and GitHub workflow files, run deterministic structured searches over that indexed data, answer owner/validation questions for seeded Greentic concepts, generate `AGENTS.md`, `CLAUDE.md`, `CODEX.md`, and `llms.txt` from indexed knowledge, package those artifacts into a local OCI-style layout, build a discovery catalog from published packages, explain whether refresh is needed, sync either a specific repo or the full discovered catalog, generate a per-repo GitHub workflow for automated index refresh/publish checks, estimate heuristic impact for a symbol or concept, detect working-tree drift relative to the indexed snapshot, validate plan files against ownership/validation hints, emit either an MCP-style tool snapshot or a request/response tool call result for agent hosts via `serve --request-file`, and apply higher-confidence curated enrichment rules for core Greentic repos such as `greentic-types`, `greentic-pack`, `greentic-bundle`, `greentic-dev`, `greentic-x`, and `greentic-sorla`.

## 2. Main Components and Functionality
- **Path:** `Cargo.toml`
  - **Role:** Workspace root manifest.
  - **Key functionality:** Declares eight workspace members under `crates/` plus shared workspace metadata and common dependencies such as `clap`, `serde`, `serde_json`, and `thiserror`.
  - **Key dependencies / integration points:** Provides the shared version and dependency baseline for the whole workspace.

- **Path:** `crates/gca-cli`
  - **Role:** Publishable binary crate named `greentic-coding-agent`.
  - **Key functionality:** Exposes scaffolded subcommands plus working `analyze`, `describe --here`, `concepts`, `workflows`, `commands`, `search --mode code|instruction|concept|reuse`, `locate-owner --concept <id>`, `required-validations --task <task>`, `generate-agent-files [--write-root]`, `package-index --tag <tag>`, `publish-index --tag <tag>`, `list-remote-repos`, `show-catalog`, `check-refresh`, `install-github-workflow`, `sync [--repo <repo>] [--tag <tag>]`, `impact --symbol <id>`, `detect-changes`, `validate-plan <plan.json>`, and `serve`. `serve` can now either emit an MCP-style tool snapshot or dispatch a request JSON file into a machine-readable response. The Clap help output now includes a fuller top-level workflow description plus per-command and per-option explanations so `--help` is usable as built-in operator documentation. `analyze` detects the repo root, scans Rust/docs/workflow/example files, writes enriched `.greentic-agent/manifest.json`, `.greentic-agent/repo-index.json`, and `.greentic-agent/fingerprints.json`, and updates a user-global registry file under `$HOME/.greentic-agent/registry.json`.
  - **Key dependencies / integration points:** This is the crate packaged by `ci/local_check.sh` and published by `.github/workflows/publish.yml`; it is the binary that future `greentic-dev` launcher integration is meant to invoke.

- **Path:** `crates/gca-agent-files/src/lib.rs`
  - **Role:** Generated agent-document rendering and writing layer.
  - **Key functionality:** Renders deterministic `AGENTS.md`, `CLAUDE.md`, `CODEX.md`, and `llms.txt` content from `RepoIndex`, includes provenance/version stamping, writes files into `.greentic-agent/generated/`, and optionally mirrors them to repo root.
  - **Key dependencies / integration points:** Uses `gca-core::RepoIndex` and `gca-query::command_catalog`; covered by golden-style content tests, missing-data fallback tests, and write-path tests.

- **Path:** `crates/gca-query/src/lib.rs`
  - **Role:** Query and command-catalog layer for indexed repo knowledge.
  - **Key functionality:** Defines structured search modes, result types, response payloads, a static command catalog, a seeded built-in policy bundle, repo-local policy loading from `.codex/policy` or `.greentic-agent/policy`, owner lookup, and required-validation matching; now includes stronger seeded owners for concept families such as bundles, wizard/launcher behavior, and shared type contracts. Provides deterministic searches over code metadata, instruction graph entries, concept graph entries, and reuse policy.
  - **Key dependencies / integration points:** Uses `gca-core::RepoIndex` as input and is covered by tests for code, instruction, concept, reuse, policy lookup, and repo-local policy loading behavior.

- **Path:** `crates/gca-core/src/config.rs`
  - **Role:** Shared config and enum contract layer.
  - **Key functionality:** Defines `AgentConfig` plus the canonical serialized enums `OutputFormat`, `LifecyclePhase`, `KnowledgeScope`, `RepoRole`, and `FreshnessStatus`, with parsing and string-stability tests.
  - **Key dependencies / integration points:** These enums are the stable machine-readable vocabulary for later indexing, policy, search, and catalog features.

- **Path:** `crates/gca-core/src/model.rs`
  - **Role:** Canonical domain-model layer.
  - **Key functionality:** Defines `RepoAgentManifest`, `RepoIndex`, `ConceptDescriptor`, `WorkflowDescriptor`, `InstructionDescriptor`, `SourceStats`, `ValidationDescriptor`, `ReuseDescriptor`, `Catalog`, and `CatalogRepo`; includes validation helpers, schema version constant `v1`, built-in concept IDs, and built-in concept descriptor generation.
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
  - **Key functionality:** Implements repo root detection, git HEAD/default-branch reading, candidate-doc discovery, Cargo manifest discovery, tracked-file fingerprint gathering, Rust/docs/workflow/example scanning, instruction-graph generation, source-stat extraction, Greentic concept/workflow enrichment, seeded/local policy loading, local output writing, and global registry updates. Returns structured `AnalyzeOutputs` and `Fingerprints`.
  - **Key dependencies / integration points:** Uses `gca-core` models/registry helpers and `gca-greentic` heuristics; covered by temp-repo tests for local output creation, instruction graph population, workflow inference, and idempotent registry updates.

- **Path:** `schemas/*.cddl`
  - **Role:** Canonical schema files for top-level formats and descriptors.
  - **Key functionality:** Define CDDL schemas for repo manifests, enriched repo indexes, catalogs, workflows, concepts, validations, and reuse descriptors using the same vocabulary as `gca-core`.
  - **Key dependencies / integration points:** Guide later serialization/validation work and now include `instruction_graph` and `source_stats`.

- **Path:** `examples/*.json`
  - **Role:** Example fixtures for the machine-readable contract layer.
  - **Key functionality:** Provide versioned example JSON for repo manifests, enriched repo indexes, catalogs, concepts, workflows, validations, and reuse descriptors. The directory also now contains a committed example installed GitHub workflow and a sample plan file for `validate-plan`.
  - **Key dependencies / integration points:** Loaded by `crates/gca-core/tests/examples.rs` and `crates/gca-oci` tests to verify that committed examples remain in sync with the Rust types and workflow renderer.

- **Path:** `crates/gca-oci/src/lib.rs`
  - **Role:** OCI-style package/export and local sync layer.
  - **Key functionality:** Packages repo-local outputs and generated agent files into a deterministic OCI-like layout with `oci-layout`, `index.json`, blob digests, and artifact payloads; supports local publish-to-remote-store, local sync-to-cache, remote repo listing, discovery catalog generation, refresh checks based on local fingerprints and versions, and GitHub workflow rendering/installation helpers.
  - **Key dependencies / integration points:** Uses `gca-core::RepoIndex` plus `gca-agent-files` rendering output; currently models GHCR-style references and artifact layout locally rather than performing real network pushes/pulls.

- **Path:** `crates/gca-mcp`
  - **Role:** MCP-style tool-surface and heuristic assistant helpers.
  - **Key functionality:** Defines a machine-readable MCP-style tool list, server snapshot payload, request/response dispatch layer, heuristic impact analysis, and plan-change validation helpers built on top of the indexed repo and seeded policy data.
  - **Key dependencies / integration points:** Uses `gca-core` and `gca-query`; covered by smoke tests for tool listing, request dispatch, impact analysis, and plan validation, but still stops short of a full transport/server implementation.

- **Path:** `templates/README.md`
  - **Role:** Future customization entrypoint for generated agent docs.
  - **Key functionality:** Documents the reserved template filenames for later customizable renderers; current generation still uses built-in deterministic renderers so crates.io dry-run packaging stays simple.
  - **Key dependencies / integration points:** Not used by the runtime yet, but now matches the implemented PR-07 direction.

- **Path:** `README.md`, `examples/README.md`
  - **Role:** Contributor-facing usage and executable documentation.
  - **Key functionality:** Document the implemented command surface, seeded adapter model, release flow, and runnable example artifacts such as `examples/plan.v1.json`, `examples/greentic-agent-index.workflow.yml`, and `examples/mcp-request.describe-repo.json`.
  - **Key dependencies / integration points:** README examples now align with the real CLI, and the committed workflow example is verified against the workflow renderer by tests.

- **Path:** `ci/local_check.sh`, `ci/check_package_contents.sh`
  - **Role:** Standard local CI wrapper and package validation helper.
  - **Key functionality:** Runs workspace format, clippy, tests, build, docs, package verification, and crates.io dry-run publication checks for every publishable crate. It currently passes with the new enrichment logic in place.
  - **Key dependencies / integration points:** Used directly by developers and by GitHub Actions workflows.

- **Path:** `.github/workflows/ci.yml`, `publish.yml`, `perf.yml`, `nightly-coverage.yml`
  - **Role:** CI, release, perf smoke, and nightly coverage workflows.
  - **Key functionality:** Validate workspace health, publish the CLI crate, build release archives, run perf smoke checks, and enforce the coverage policy.
  - **Key dependencies / integration points:** `publish.yml` still targets the CLI crate version from workspace metadata and assumes later release maturity beyond the current scaffold.

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
  - **Short description:** `analyze`, `describe --here`, `concepts`, `workflows`, `commands`, `search`, `locate-owner`, `required-validations`, `generate-agent-files`, `package-index`, `publish-index`, `list-remote-repos`, `show-catalog`, `check-refresh`, `install-github-workflow`, `sync`, `impact`, `detect-changes`, `validate-plan`, and `serve` are implemented, but later planned capabilities such as richer MCP transport, deeper semantic impact analysis, and detect-changes integration with real staged/unstaged git state remain future work.

- **Location:** `crates/gca-oci/src/lib.rs:1`
  - **Status:** Partial
  - **Short description:** OCI-style packaging, cataloging, refresh checks, and sync are implemented against a local remote-store/cache model, but live GHCR push/pull, auth handling, and richer remote catalog behavior are still future work.

- **Location:** `crates/gca-mcp/src/lib.rs:1`
  - **Status:** Partial
  - **Short description:** The crate now exposes a real MCP-style tool catalog plus request/response dispatch helpers, but it is still a lightweight in-process helper layer rather than a full long-running MCP transport server.

- **Location:** `templates/README.md:1`
  - **Status:** Placeholder
  - **Short description:** Built-in agent-file generation is implemented, but repo-customizable template overrides are still not wired into the runtime.

- **Location:** `docs/architecture.md:5`
  - **Status:** Draft
  - **Short description:** The architecture document explicitly remains planning material rather than a description of fully implemented behavior.

- **Location:** Whole repository search
  - **Status:** No inline TODO markers found
  - **Short description:** Repository-wide marker search still finds no `TODO`/`FIXME`/`XXX` comments; unfinished work is tracked mostly through scaffold crates and `.codex` planning docs.

## 4. Broken, Failing, or Conflicting Areas
- **Location:** `crates/gca-cli/src/main.rs` versus `crates/gca-core/src/config.rs`, `crates/gca-core/src/model.rs`, `crates/gca-core/src/registry.rs`, `crates/gca-greentic/src/lib.rs`, and `crates/gca-index/src/lib.rs`
  - **Evidence:** `gca-cli` still defines its own local copies of output format, manifest/index/fingerprint/registry structures, enrichment helpers, policy data, query logic, and generated-file rendering logic even though shared workspace crates now contain canonical/shared versions.
  - **Likely cause / nature of issue:** This duplication remains in place to keep the publishable CLI crate independent from unpublished workspace path dependencies during crates.io dry-run validation. It should be reconciled once the shared-crate publish strategy is decided.

- **Location:** `crates/gca-oci/src/lib.rs`, `crates/gca-cli/src/main.rs`
  - **Evidence:** The new packaging, catalog, refresh, and sync flow writes and copies a local OCI-style layout under `.greentic-agent/oci`, `~/.greentic-agent/remote-oci`, and `~/.greentic-agent/cache-oci`, but it does not authenticate to or transfer from a live OCI registry.
  - **Likely cause / nature of issue:** PR-08 and PR-09 established the artifact model, discovery shape, and automation workflow first, using a local remote-store simulation so behavior can be tested without network dependencies. Real GHCR transport remains a later follow-up.

- **Location:** External `greentic-dev` integration
  - **Evidence:** PR-01 planning requires `gtc dev coding-agent ...` delegation, but this single-repo implementation could only prepare the binary and CLI surface locally.
  - **Likely cause / nature of issue:** The launcher change belongs in another repository, so this repo cannot complete that integration by itself.

- **Location:** `docs/architecture.md` compared with the implemented repo
  - **Evidence:** The architecture doc still describes a deeper system with fuller MCP tooling, richer impact analysis, remote discovery, and OCI sync behavior than the currently implemented heuristic/local-store-based versions.
  - **Likely cause / nature of issue:** The repo now has scaffold, contract, bootstrap, enrichment, seeded adapters, query, seeded policy, generated agent-doc, catalog/refresh, and heuristic MCP/impact layers, but several higher-level product behaviors remain ahead of implementation.

## 5. Notes for Future Work
- Decide how shared crates such as `gca-core`, `gca-greentic`, and `gca-index` should be versioned and published so the CLI can consume canonical types without duplicating them.
- Replace the local remote-store simulation with a real OCI/GHCR transport layer when networked registry sync is ready to land.
- Replace the current MCP-style snapshot/helper layer with a fuller long-running transport/server implementation if external agent hosts need real session-based tool serving.
- Route CLI behavior through shared workspace crates once the publish strategy allows it, reducing the current mirrored analyze/describe/search/policy/rendering code in `gca-cli`.
- Finish PR-01 cross-repo launcher wiring in `greentic-dev` so `gtc dev coding-agent` actually delegates to this binary.
