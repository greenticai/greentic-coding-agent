GLOBAL RULE – REPO OVERVIEW, CI, AND REUSE OF GREENTIC REPOS

For this repository, always:

1. Maintain `.codex/repo_overview.md` using the repo overview maintenance routine before starting any PR-style work and after finishing it.
2. Run `ci/local_check.sh` at the end of work and ensure it passes, or explain precisely why it cannot be made to pass as part of that work.
3. Prefer existing Greentic repos/crates for shared types, interfaces, secrets, oauth, messaging, events, and similar cross-cutting concerns instead of redefining them locally.

Treat these as built-in prerequisites and finalization steps for all work in this repo.

## Workflow for Every PR
1. Pre-PR sync
   - Refresh `.codex/repo_overview.md` so it reflects the current repo before edits.
   - Show the updated overview if it changed meaningfully.
2. Implement the PR
   - Make the requested code, test, docs, and config changes.
   - Reuse existing Greentic crates before introducing new shared concepts.
   - Run appropriate validation while working.
3. Post-PR sync
   - Refresh `.codex/repo_overview.md` again based on the updated codebase.
   - Run `ci/local_check.sh`.
   - If `ci/local_check.sh` cannot be made to pass within scope, report the failing steps and key errors clearly.
   - In the final summary, explicitly mention that the repo overview was refreshed and whether `ci/local_check.sh` passed.

## Behavioural Rules
- Do not ask permission to refresh the repo overview, run `ci/local_check.sh`, or reuse existing Greentic crates.
- Do not leave `.codex/repo_overview.md` partially updated or inconsistent.
- Do not duplicate core shared types or interfaces that should come from existing Greentic crates without a documented reason.
- If the right build or CI command is unclear and cannot be inferred from the repo, ask one concise question; otherwise proceed autonomously.

The repo overview maintenance routine is defined in `.codex/repo_overview_task.md`.
