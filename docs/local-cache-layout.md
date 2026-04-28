# Local Cache Layout

Greentic Coding Agent stores user-local consumer state under:

```text
~/.greentic-agent/
```

Repo-local producer output remains under the repository checkout:

```text
<repo>/.greentic-agent/
```

## User-Local Paths

Typical user-local paths include:

```text
~/.greentic-agent/config.toml
~/.greentic-agent/catalogs/public/catalog.json
~/.greentic-agent/catalogs/public/<channel>/catalog.json
~/.greentic-agent/indexes/public/<org>/<repo>/<channel>/repo-index.json
~/.greentic-agent/cache-oci/<org>/<repo>/<channel>/
~/.greentic-agent/remote-oci/
~/.greentic-agent/notifications/feed.json
~/.greentic-agent/notifications/seen.json
```

`config.toml` stores defaults such as the selected channel and catalog reference. `sync --channel <channel>` downloads catalog and repo indexes for that channel. `status --channel <channel>` reads this cache and reports branch, commit, and freshness details.

## Repo-Local Paths

Running `analyze` in a checkout writes files such as:

```text
.greentic-agent/repo-manifest.json
.greentic-agent/repo-index.json
.greentic-agent/generated/AGENTS.md
.greentic-agent/generated/CODEX.md
.greentic-agent/generated/CLAUDE.md
.greentic-agent/generated/llms.txt
```

These files are producer artifacts. They can also act as a local overlay when `serve` or `agent context` runs inside the checkout.

## Notifications

`watch`, `daemon`, and `updates --scope org` use:

```text
~/.greentic-agent/notifications/feed.json
~/.greentic-agent/notifications/seen.json
```

Feed items are keyed by repo, branch, and commit so repeated watch ticks do not create duplicate notifications for the same published index.

## Channel Selection

Use explicit channels when switching between release and integration knowledge:

```bash
greentic-coding-agent sync --channel main --format json
greentic-coding-agent sync --channel develop --format json
greentic-coding-agent status --channel develop --format json
```

If no channel is passed, the CLI uses the configured default channel and falls back to `develop` where a default is needed.
