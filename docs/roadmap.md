# Roadmap and Review Gates

## Phase 1: Architecture

Status: complete.

Acceptance criteria:

- Inward dependency rules and model boundaries are documented.
- Domain errors, identifiers, pagination, and capability vocabulary compile.
- Workspace/repository outbound ports demonstrate dependency inversion.
- Small application use cases can be tested without HTTP or MCP.
- Product differences are handled without pretending Cloud APIs are self-hosted APIs.

## Phase 2: Reusable REST client

Status: complete.

- Async `reqwest` client with connection reuse and body/timeout limits.
- PAT and Basic authentication with redacted secrets.
- Reusable Bitbucket pagination and bounded retry policy.
- REST DTOs and centralized status/error mapping.
- Mock-server tests for authentication, retries, pagination, and malformed responses.

## Phase 3: MCP transport

Status: complete.

- Select and pin an implementation compatible with the current official MCP specification.
- Standard I/O lifecycle, initialization, capabilities, tools/list, and tools/call.
- Protocol output isolation from logs.
- Request IDs, timing spans, cancellation, and tool error mapping.
- Interoperability tests using raw protocol fixtures, not only SDK helpers.

## Phase 4: Repository tools

Status: complete.

- Workspace list/get and repository list/get.
- Branch list/create, commit get/create, and bounded file reads.
- Independent request/response schemas and tool tests.

## Phase 5: Pull request tools

Status: complete.

- Create/list/get, approve, merge, diff, comment, and list comments.
- Version-aware merge checks and conflict mapping.

## Phase 7: Production hardening

Status: complete.

- Full test matrix, fuzz/property tests for parsers, and concurrency/load tests.
- Configuration files, secret-file support, graceful shutdown, and metrics hooks.
- Container packaging, SBOM, security policy, deployment guide, and release automation.

No phase should begin until the preceding phase's public contracts and tests are
reviewed. Later implementation feedback may refine contracts through an explicit
architecture decision rather than bypassing layer boundaries.
