# Testing

The test pyramid follows architecture boundaries:

| Scope | Test doubles | Verifies |
| --- | --- | --- |
| Domain unit | None | Validation, identifiers, pagination, error semantics |
| Application unit | Fake outbound ports | One use case without HTTP or MCP |
| Infrastructure integration | Mock HTTP server | Paths, auth, DTO mapping, retries, status/error mapping |
| MCP tool contract | Fake use case/port | JSON Schema, validation, response and error mapping |
| End-to-end | Disposable mock Bitbucket API | MCP initialization and tool invocation over supported transports |

Run all local checks with:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

Tests must not require real credentials or a reachable Bitbucket installation.
Live-system tests are opt-in, ignored by default, and read credentials only from
the environment.

Phase 2 mock-server coverage verifies Bearer and Basic authorization, context
paths, server-provided pagination cursors, transient retries, REST-to-domain
mapping, malformed JSON, and response-size limits.

Phase 3 launches the compiled server as a child process and feeds it raw JSONL
fixtures. This independently verifies initialization/version negotiation,
capability advertisement, `tools/list`, unknown `tools/call`, cancellation
notifications, graceful EOF shutdown, and that standard output contains only
valid JSON-RPC responses.

Phase 4 and 5 mock-server tests verify all infrastructure operations, including
multipart single-file commits, raw bounded file/diff responses, pull-request
optimistic locking, approvals, merges, and comments. Tool registry tests ensure
all 17 tools have closed object schemas; raw protocol tests verify a registered
tool returns `INVALID_PARAMS` for malformed arguments.

Phase 7 adds property tests for identifiers, pagination bounds, and secret
redaction; a 64-request concurrent client test; raw SIGTERM shutdown coverage;
and `cargo-fuzz`'s `repository_path` target. CI runs formatting, Clippy, tests,
rustdoc, dependency policy, RustSec audit, container builds, and a fuzz smoke run.
