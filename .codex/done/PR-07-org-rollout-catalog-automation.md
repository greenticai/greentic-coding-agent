# PR 07 — Automate organisation rollout and central catalog updates

## Position in sequence

Implement after PR 02/03 define branch-aware catalog entries and branch-specific package tags.

Current local reality before this PR:

- `org plan-index-rollout` and `org apply-index-rollout` already exist.
- Rollout planning can use GitHub or an offline `--repo-list-file`.
- Catalog commands exist for show/add/remove/enable/disable/validate/publish.
- There is no `catalog rebuild-from-ghcr`.
- `catalog publish` has no `--channel`.

## Goal

Make it easy to install index workflows across all Greentic repos and maintain a central branch-aware catalog.

## Commands

Extend existing org commands:

```bash
greentic-coding-agent org plan-index-rollout --org greenticai --format json
greentic-coding-agent org apply-index-rollout --plan rollout-plan.json --open-prs
greentic-coding-agent catalog rebuild-from-ghcr --org greenticai --channel develop
greentic-coding-agent catalog publish --channel develop --backend ghcr
```

Only add `--channel` after PR 02 has channel-aware catalog models.

## Recommended model

Prefer a central catalog repo or catalog package as source of truth.

The rollout command should be able to:

- discover repos
- check whether workflow exists
- create branch
- add generated workflow
- open PR
- report skipped/archived/private repos

Catalog automation should be able to:

- read repo list
- resolve `main` and `develop` index URIs
- write catalog v2
- validate catalog
- publish catalog package

## Acceptance criteria

- One command can generate PR plans for many repos.
- Catalog v2 can be rebuilt deterministically.
- Docs explain public and tenant catalogs.
- No destructive repo writes without explicit flags.
- Existing org rollout dry-run and offline repo-list tests remain green.

## Implementation notes

- Kept the existing `org plan-index-rollout` and `org apply-index-rollout` flow, including offline repo-list dry-run coverage.
- Added `catalog rebuild-from-ghcr --org <org> --channel <channel>` to rebuild a deterministic branch-aware catalog from published repo index packages.
- Added `catalog publish --channel <channel>` so public catalogs can be published under channel-specific tags/paths while preserving the compatibility catalog path for local sync.
- Rebuilt catalogs use `gca.catalog.v2`, set `catalog_id` and `default_channel`, and include branch entries for all published tags found for each repo.
- Updated generated catalog workflows to run on `main` and `develop` and publish with `--channel "${{ github.ref_name }}"`.
- Updated catalog/admin docs with rebuild, validate, and channel publish commands.
