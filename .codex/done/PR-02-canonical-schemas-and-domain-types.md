# PR-02: Canonical schemas and core domain types

## Title

feat(coding-agent): add canonical repo/index/catalog/workflow/concept schemas and Rust types

## Objective

Define the machine-readable contract layer for Greentic-coding-agent so all later commands, publishing, and discovery use a stable format.

## Why

The design depends on structured data being the source of truth. Before building indexers or catalogs, define the data model once.

## Scope

Implement in `gca-core` plus `schemas/`:
- `RepoAgentManifest`
- `RepoIndex`
- `ConceptDescriptor`
- `WorkflowDescriptor`
- `ValidationDescriptor`
- `ReuseDescriptor`
- `Catalog`
- shared enums for phase/scope/repo-role

## Deliverables

### Schema files
Add CDDL or equivalent schema definitions:
- `greentic.agent.repo-manifest.v1.cddl`
- `greentic.agent.repo-index.v1.cddl`
- `greentic.agent.catalog.v1.cddl`
- `greentic.agent.workflow.v1.cddl`
- `greentic.agent.concept.v1.cddl`
- `greentic.agent.validation.v1.cddl`
- `greentic.agent.reuse.v1.cddl`

### Rust types
Add serde-enabled Rust structs with validation helpers.

### Enums
Add:
- `LifecyclePhase`
- `KnowledgeScope`
- `RepoRole`
- `OutputFormat`
- `FreshnessStatus`

## Required content

### RepoRole enum
Must include at least:
- CoreContracts
- CliLauncher
- ComponentAuthoring
- FlowAuthoring
- PackAuthoring
- BundleAssembly
- SolutionLayer
- SorlaLayer
- ProviderFamily
- DemoApp
- CustomerSolution
- ExamplesOnly

### Concepts
Provide built-in concept IDs for:
- digital_worker
- application_pack
- extension_pack
- bundle
- flow
- component
- wizard
- setup
- start
- greentic_x
- greentic_sorla
- capability
- provider
- hook
- observer
- static_route

## Acceptance criteria

- schemas committed
- Rust types round-trip JSON successfully
- version fields present on top-level formats
- schema examples committed under `examples/`

## Test plan

- serde roundtrip tests
- example fixture validation tests
- enum string stability tests

## Out of scope

- repo scanning
- CLI commands beyond loading/printing fixtures
