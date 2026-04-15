# Templates

PR-07 added generated agent-file support in code.

This directory is reserved for future customizable templates such as:
- `AGENTS.md.hbs`
- `CLAUDE.md.hbs`
- `CODEX.md.hbs`
- `llms.txt.hbs`

The current implementation uses built-in deterministic renderers so generated files stay available during crates.io packaging checks.
