# Repo Overview Maintenance

Use this routine whenever `.codex/repo_overview.md` must be refreshed.

## Goal
Keep `.codex/repo_overview.md` as a concise, factual snapshot of the repository's current implemented state.

## Required Output Sections
1. `# Repository Overview`
2. `## 1. High-Level Purpose`
3. `## 2. Main Components and Functionality`
4. `## 3. Work In Progress, TODOs, and Stubs`
5. `## 4. Broken, Failing, or Conflicting Areas`
6. `## 5. Notes for Future Work`

## Routine
1. Scan the repo structure and identify top-level components.
2. Inspect main entrypoints such as `Cargo.toml`, `src/main.rs`, `src/lib.rs`, workflow files, and key docs.
3. Search for `TODO`, `FIXME`, `XXX`, `HACK`, `BROKEN`, `NOTE`, `todo!`, `unimplemented!`, `NotImplemented`, `stub`, and `placeholder`.
4. Run the obvious non-destructive validation commands when they can be inferred safely.
5. Record implemented behavior, not intended behavior, while still noting important draft or planning docs.
6. Overwrite `.codex/repo_overview.md` with a fresh snapshot; do not append stale information.

## Notes
- Be explicit when documentation or plans are ahead of implementation.
- Include concrete file paths and line references where they materially help.
- If checks pass but the project has no meaningful tests, say that clearly.
