# PR-09 — First-run Codex/Claude bootstrap instructions

## Goal

When first run in a repo, the tool should behave like `greentic-dev coverage`: it should tell coding agents exactly how to set themselves up.

## Depends on

- PR-01 repo ID detection.
- PR-03 auth/config terminology.
- PR-06 final server command names.
- PR-08 workflow install command shape.

Keep this PR late enough that the generated instructions do not document unstable flags.

## Trigger

When the user runs:

```bash
greentic-coding-agent analyze
```

and `.greentic-agent/` does not exist yet, print a first-run bootstrap block.

Also add explicit command:

```bash
greentic-coding-agent bootstrap-instructions
```

## Template

Add:

```text
templates/CODEX_BOOTSTRAP.md.hbs
```

## Content requirements

The generated instruction must include:

- what repo was detected
- current repo ID
- how to run analyze
- how to start MCP server
- how to start HTTP server
- how to sync public indexes
- how to sync tenant/private indexes
- how to install nightly GHCR workflow
- minimum operating rules for Codex/Claude

The instruction text must be generated from the same config/defaults used by the CLI where possible. Avoid hard-coding catalog refs, ports, or server modes in the template if those values already exist in runtime config.

## Example output

```md
# Greentic Coding Agent Bootstrap

Detected repo: greenticai/greentic-coding-agent

You are working in a Greentic repo. Before editing, run:

```bash
greentic-coding-agent describe --here
greentic-coding-agent sync
greentic-coding-agent search --scope all "the task"
```

To run a long-lived local server for coding agents:

```bash
greentic-coding-agent serve --mcp --watch
```

or:

```bash
greentic-coding-agent serve --http --host 127.0.0.1 --port 7757 --watch
```

For tenant/private indexes:

```bash
greentic-coding-agent sync --tenant <tenant> --token <token>
greentic-coding-agent serve --mcp --tenant <tenant> --token <token> --watch
```

To keep this repo indexed in GHCR:

```bash
greentic-coding-agent install-github-workflow --publish-ghcr
```

Rules:
- Always call `describe --here` first.
- Search before creating new abstractions.
- Run `impact` before editing shared contracts.
- Run `required-validations` before finishing.
```

## Safety and examples

- Token examples must use placeholders only.
- Tenant examples should prefer `--token-env TENANT_GHCR_TOKEN` over inline `--token`.
- If HTTP examples are shown, bind to `127.0.0.1`.
- If `serve` defaults to MCP stdio, the examples should say that plainly and avoid redundant flags unless they improve clarity.

## Tests

- First run prints bootstrap block.
- Second run does not print unless `--show-bootstrap` is passed.
- `bootstrap-instructions --format json` returns structured guidance.
- Token examples use placeholders only.
- Bootstrap output contains detected `repo_id`.
- Bootstrap output uses current server and sync defaults from config.

## Acceptance criteria

- Codex/Claude gets useful setup instructions in one run.
- Instructions include server mode and tenant sync.
- No real token is printed.
