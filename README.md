# Bitbucket MCP Server

A standalone Model Context Protocol server for self-hosted Bitbucket Server and
Bitbucket Data Center installations.

## Status

Phase 7 is complete: the crate contains the architecture, reusable asynchronous
Bitbucket Data Center REST client, specification-compliant MCP stdio server,
repository/content tools, pull-request tools, and production hardening assets.
CI/CD tools remain intentionally out of scope so Bamboo can be operated
independently. See
[`docs/roadmap.md`](docs/roadmap.md) for the review gates.

## Design goals

- Interoperate with specification-compliant MCP clients without client-specific behavior.
- Keep domain and application logic independent from HTTP and MCP SDK types.
- Keep REST DTOs, domain models, and MCP response models separate.
- Make Bitbucket version and installed-application differences explicit through capabilities.
- Reuse one asynchronous HTTP client and bounded retry/pagination policies.
- Emit protocol data only on standard output and diagnostics only on standard error.

## Build

Rust 1.88 or newer is required.

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

Start the stdio server after configuring Bitbucket:

```bash
cargo run -- --config /path/to/bitbucket-mcp.toml
```

## Documentation

- [Architecture](docs/architecture.md)
- [Configuration and authentication](docs/configuration.md)
- [MCP transport and compliance](docs/mcp.md)
- [Available tools](docs/tools.md)
- [Adding endpoints and MCP tools](docs/extending.md)
- [Testing](docs/testing.md)
- [Deployment and operations](docs/deployment.md)
- [Roadmap](docs/roadmap.md)
- [Security policy](SECURITY.md)
- [Contributing](CONTRIBUTING.md)

## Terminology

The public capability is named `workspace` for a consistent MCP surface. The
Bitbucket Server/Data Center adapter maps it to Bitbucket projects. This mapping
is isolated in infrastructure and does not leak REST project DTOs to clients.

## License

Licensed under the Apache License, Version 2.0.
