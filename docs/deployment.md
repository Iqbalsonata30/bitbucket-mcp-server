# Deployment and Operations

The server supports standard I/O for broad MCP client interoperability.
Additional official MCP transports may be enabled independently without
changing tools or use cases.

Build and run the current binary with:

```bash
cargo build --release
BITBUCKET_CONFIG_FILE=/etc/bitbucket-mcp/config.toml \
  ./target/release/bitbucket-mcp-server
```

An MCP client should launch that executable, provide configuration through its
process environment or `--config`, and communicate over the child process's
standard input/output. No client-specific protocol adaptation is required.

Container execution must keep stdin attached because stdin/stdout are the MCP
transport:

```bash
docker run --rm -i \
  --read-only \
  --mount type=bind,src=/secure/bitbucket_token,dst=/run/secrets/bitbucket_token,readonly \
  -e BITBUCKET_BASE_URL=https://bitbucket.example.com/bitbucket \
  -e BITBUCKET_AUTH_TYPE=token \
  -e BITBUCKET_TOKEN_FILE=/run/secrets/bitbucket_token \
  ghcr.io/owner/bitbucket-mcp-server:VERSION
```

The image uses a distroless runtime and an unprivileged user. It has no network
listener or container health endpoint because stdio lifecycle is owned by the
MCP client. Closing stdin, SIGINT, and SIGTERM trigger shutdown; active requests
receive cancellation through the SDK.

Operational requirements:

- Run as an unprivileged user with a read-only root filesystem where possible.
- Supply credentials through the platform secret store.
- Send MCP protocol frames only to standard output.
- Send logs and diagnostics only to standard error.
- Keep TLS verification enabled and mount enterprise CA certificates as files.
- Apply CPU/memory limits and cap file, diff, and HTTP body sizes.
- Use graceful shutdown and bounded in-flight request draining.
- Expose health and metrics only through isolated, explicitly configured endpoints.

Container images will be reproducible, use a minimal runtime base, include an
SBOM, and run the same test/lint gates as native binaries.

Tagged releases publish checksums, a CycloneDX SBOM, a Linux binary archive,
and multi-architecture OCI images with provenance and embedded SBOM metadata.
