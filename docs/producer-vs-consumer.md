# Producer Vs Consumer Workflows

Greentic Coding Agent has two normal operating modes: producer mode in repositories and consumer mode on developer or agent machines.

## Producer Mode

Producer mode runs inside a Greentic repository. Its job is to create and publish branch-aware knowledge for everyone else.

Common producer commands:

```bash
greentic-coding-agent analyze --print --format json
greentic-coding-agent package-index --tag main --tag sha-<commit> --format json
greentic-coding-agent publish-index --tag main --tag sha-<commit> --backend ghcr --token-env GHCR_TOKEN --format json
```

The generated GitHub workflow runs the same flow for the current branch. For active branches it publishes tags such as:

```text
ghcr.io/greenticai/indexes/<org>/<repo>:main
ghcr.io/greenticai/indexes/<org>/<repo>:develop
ghcr.io/greenticai/indexes/<org>/<repo>:sha-<commit>
```

Repo-local `analyze` remains useful for:

- CI publishing
- debugging index contents before a workflow runs
- bootstrapping new repositories
- providing a working-tree overlay when a server runs inside a checkout

## Consumer Mode

Consumer mode runs on a developer or coding-agent machine. Its job is to sync published indexes once and serve or query a merged local knowledge base.

Common consumer commands:

```bash
greentic-coding-agent init --channel develop --format json
greentic-coding-agent sync --channel develop --format json
greentic-coding-agent status --channel develop --format json
greentic-coding-agent search --mode instruction --scope merged "component manifest" --format json
greentic-coding-agent serve --stdio
```

Coding agents should usually start with:

```bash
greentic-coding-agent agent context --task "<task>" --format json
greentic-coding-agent updates --new --scope org --format json
greentic-coding-agent required-validations --task "<task>" --format json
```

## Catalog Responsibility

Catalogs connect producers and consumers. Producers publish repo index packages. Catalog automation rebuilds a branch-aware catalog from those packages:

```bash
greentic-coding-agent catalog rebuild-from-ghcr --org greenticai --channel develop --format json
greentic-coding-agent catalog publish --channel develop --backend ghcr --token-env GHCR_TOKEN --format json
```

Consumers then sync the catalog channel they need.

## Compatibility

Older repo-local commands still work and are covered by compatibility tests:

```bash
greentic-coding-agent analyze --print --format json
greentic-coding-agent describe --here --format json
greentic-coding-agent search --mode instruction wizard --format json
greentic-coding-agent catalog validate --format json
```
