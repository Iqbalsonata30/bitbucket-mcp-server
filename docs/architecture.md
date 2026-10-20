# Architecture

## Dependency direction

```text
MCP transport -> application -> domain
                         ^
Bitbucket REST adapter --|
```

Dependencies always point inward:

| Layer | Owns | May depend on |
| --- | --- | --- |
| `domain` | Models, identifiers, errors, pagination semantics, capabilities | Standard library, serialization/error helpers |
| `application` | Use cases and outbound ports | Domain |
| `infrastructure` | HTTP client, authentication, REST DTOs, API-version adapters | Application, domain |
| `transport` | MCP initialization, tools, MCP request/response schemas and error mapping | Application, domain |
| composition root | Configuration, construction, lifecycle, observability | All layers |

The domain never imports HTTP, MCP, environment, filesystem, or logging types.
The application layer describes what it needs through outbound traits. The
infrastructure layer implements those traits. The MCP layer performs schema
validation and mapping, then invokes one application use case.

## Module layout

```text
src/
  application/       use cases and outbound ports
  domain/            stable business vocabulary
  infrastructure/    Bitbucket REST adapters (Phase 2)
  transport/         MCP adapters (Phase 3)
  lib.rs
  main.rs            composition root
```

As capabilities are added, each receives a vertical slice:

```text
application/pull_request/create.rs
domain/pull_request.rs
infrastructure/bitbucket/pull_request.rs
infrastructure/bitbucket/dto/pull_request.rs
transport/mcp/tools/pull_request/create.rs
```

Small related modules may initially share one file. Split by operation before a
handler or use case accumulates unrelated branches.

## Model boundaries

Data crosses boundaries by explicit conversion:

```text
Bitbucket JSON <-> REST DTO -> domain model -> MCP response model -> MCP content
```

REST DTOs preserve upstream field names and optionality. Domain models enforce
server-independent invariants. MCP models preserve the public tool contract.
No layer re-exports another layer's transport models.

## Bitbucket product differences

Bitbucket Server/Data Center calls the repository owner a project, while
Bitbucket Cloud calls it a workspace. The MCP API uses `workspace`; the REST
adapter maps this to project endpoints.

Cloud-only APIs must never be silently called against a self-hosted server.
CI/CD remains outside this server so installations can continue using Bamboo or
another independently managed delivery platform.

## Tool contract

Each MCP tool owns:

- A unique, stable snake-case name prefixed with `bitbucket_`.
- A deserializable request schema with bounds and descriptions.
- A dedicated application use case.
- A serializable response schema independent of REST DTOs.
- Focused contract tests for valid input, invalid input, success, and errors.

Tool handlers contain no URL construction, authentication, retry, or REST error
inspection. Mutating tools must describe side effects in their tool metadata.
Pagination is explicit and bounded; tools never fetch every page implicitly
unless the contract specifically states that behavior.

## Error flow

Infrastructure maps status codes and Bitbucket error payloads into domain
errors. Application code may add business context but does not inspect HTTP
status codes. MCP transport performs the final conversion into protocol errors
or structured tool errors, preserving a request identifier while redacting
credentials and sensitive response data.

Expected mappings include:

| Upstream condition | Domain error |
| --- | --- |
| `401` or invalid credentials | `AuthenticationError` |
| missing project/repository | `WorkspaceNotFound` / `RepositoryNotFound` |
| duplicate branch | `BranchAlreadyExists` |
| pull request state conflict | `PullRequestConflict` |
| non-mergeable pull request | `MergeConflict` |
| timeout/connectivity failure | `NetworkError` |
| unavailable installation feature | `UnsupportedCapability` |

## Concurrency and performance

- A single configured `reqwest::Client` is cloned as a cheap pool handle.
- Use cases borrow immutable ports where practical.
- No global mutable state or `Arc<Mutex<_>>` is required for request handling.
- Retry waits are asynchronous and only safe/idempotent operations retry by default.
- Response bodies and log payloads have configurable limits.
- Pagination retains only the requested page unless a use case requires aggregation.

## Observability

The composition root installs tracing and sends diagnostics to standard error.
Each MCP call receives or generates a request identifier and an instrumented
span containing tool name, duration, outcome, and upstream attempt count.
Credentials, authorization headers, file contents, comments, and log bodies are
not recorded. Metrics are emitted through an internal hook so no metrics backend
is required by the domain or application layers.
