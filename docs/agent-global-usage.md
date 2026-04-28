# Agent Global Usage

Greentic Coding Agent is designed to be installed once and used as a global knowledge source by coding agents.

## Daily Agent Setup

Sync the channel relevant to the task:

```bash
greentic-coding-agent init --channel develop --format json
greentic-coding-agent sync --channel develop --format json
greentic-coding-agent status --channel develop --format json
```

Then start the MCP-style stdio server:

```bash
greentic-coding-agent serve --stdio
```

When started inside a repository, the server uses synced global knowledge first and adds the current checkout as a local overlay. When started outside a repository, it still serves the synced organization index.

## Direct Agent Commands

Agents can call direct commands without running a server:

```bash
greentic-coding-agent agent context --task "add static route support" --format json
greentic-coding-agent agent preflight --task "add static route support" --repo greenticai/greentic-pack --format json
greentic-coding-agent agent owner --concept greentic.static-routes.v1 --format json
```

Useful supporting commands:

```bash
greentic-coding-agent search --mode instruction --scope merged "<task>" --format json
greentic-coding-agent required-validations --task "<task>" --format json
greentic-coding-agent updates --new --scope org --format json
greentic-coding-agent updates mark-seen --scope org --all --format json
```

## MCP Tool Names

Stable MCP tool names are:

```text
gca.search
gca.agent_context
gca.find_owner
gca.required_validations
gca.recent_updates
gca.branch_status
```

Older un-namespaced tool names remain available for compatibility.

## Codex Bootstrap

```toml
[mcp_servers.greentic]
command = "greentic-coding-agent"
args = ["serve", "--stdio"]
```

## Claude Code Bootstrap

```json
{
  "mcpServers": {
    "greentic": {
      "command": "greentic-coding-agent",
      "args": ["serve", "--stdio"]
    }
  }
}
```

## Watch For Changes

For a foreground update loop:

```bash
greentic-coding-agent watch --channel develop --poll 10m --format json
```

For a foreground daemon-style command:

```bash
greentic-coding-agent daemon --channel develop --poll 10m --format json
```

Both commands update the local cache and append org-level notification feed items when branch indexes change.
