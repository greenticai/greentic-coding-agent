# Org Index Rollout

`greentic-coding-agent org plan-index-rollout` creates a deterministic JSON plan for enabling the standard Greentic coding-agent index workflow across an organization.

```bash
greentic-coding-agent org plan-index-rollout \
  --org greenticai \
  --repo-list-file repos.json \
  --format json
```

The repo list can be either a JSON array or an object with a `repos` array:

```json
{
  "repos": [
    {
      "repo_id": "greenticai/greentic-pack",
      "default_branch": "main",
      "archived": false
    }
  ]
}
```

Each repo is classified as `skip`, `already_enabled`, `create_pr`, or `update_existing_workflow` by comparing `.github/workflows/greentic-agent-index.yml` with the workflow rendered by `install-github-workflow`.

Apply is explicit and supports dry runs:

```bash
greentic-coding-agent org apply-index-rollout \
  --plan rollout-plan.json \
  --dry-run \
  --format json
```

To create branches and open PRs, omit `--dry-run` and pass `--open-prs`.
