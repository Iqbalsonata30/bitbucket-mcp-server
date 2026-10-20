# Contributing

Read [`docs/architecture.md`](docs/architecture.md) and
[`docs/extending.md`](docs/extending.md) before changing tools or endpoints.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
```

Changes to REST behavior require mock-server tests. Changes to MCP contracts
require raw JSON-RPC or tool-schema tests. Tests must not use real credentials.

## Pull Requests

- Keep transport, application, domain, and infrastructure boundaries intact.
- Document supported Bitbucket Server/Data Center versions and endpoint sources.
- Avoid Cloud-only APIs and client-specific MCP behavior.
- Add a changelog entry for user-visible changes once release automation adopts one.
- Never commit credentials, `.env` files, or local configuration.

## Fuzzing

Install `cargo-fuzz` and use a nightly toolchain:

```bash
cargo +nightly fuzz run repository_path
```
