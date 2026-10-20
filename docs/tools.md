# Available Tools

All names and schemas are client-independent MCP contracts. Pagination uses
`start` and `limit`; responses return Bitbucket's server-provided `next_start`.

## Workspace and Repository

| Tool | Purpose |
| --- | --- |
| `bitbucket_list_workspaces` | List Bitbucket projects as workspaces |
| `bitbucket_get_workspace` | Get one workspace/project |
| `bitbucket_list_repositories` | List repositories in a workspace |
| `bitbucket_get_repository` | Get one repository |
| `bitbucket_get_branches` | List repository branches |
| `bitbucket_create_branch` | Create a branch from a commit or ref |
| `bitbucket_get_commit` | Get one commit by hash |
| `bitbucket_create_commit` | Create or replace one file in one commit |
| `bitbucket_read_file` | Read bounded raw file content |

Bitbucket Data Center's core REST API has no generic atomic multi-file commit
endpoint. `bitbucket_create_commit` therefore maps to the official single-file
`PUT /browse/{path}` operation. It accepts complete file content and optionally
`source_commit` for optimistic concurrency. Use Git push for atomic multi-file
commits, deletes, renames, merges, or arbitrary parent selection.

Raw file reads return UTF-8 when valid and base64 otherwise. `max_bytes` is
bounded to 10 MiB and may be further constrained by server configuration.

## Pull Requests

| Tool | Purpose |
| --- | --- |
| `bitbucket_create_pull_request` | Create a same-repository or hierarchy-compatible PR |
| `bitbucket_list_pull_requests` | List PRs with optional state filter |
| `bitbucket_get_pull_request` | Get one PR including its current version |
| `bitbucket_approve_pull_request` | Approve as the authenticated user |
| `bitbucket_merge_pull_request` | Merge using a required optimistic-lock version |
| `bitbucket_get_pull_request_diff` | Retrieve a bounded raw unified diff |
| `bitbucket_comment_pull_request` | Add a general comment or reply |
| `bitbucket_list_pull_request_comments` | List comments, optionally for a path |

Fetch a pull request immediately before merge and pass its current `version`.
A stale version, content conflict, merge-check veto, closed PR, or archived
repository can produce a typed merge conflict. Mutating operations are never
automatically retried.
