# syntax=docker/dockerfile:1.7
FROM rust:1.88-bookworm AS builder

WORKDIR /source
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --locked --release

FROM gcr.io/distroless/cc-debian12:nonroot

LABEL org.opencontainers.image.title="Bitbucket MCP Server" \
      org.opencontainers.image.description="MCP server for Bitbucket Server and Data Center" \
      org.opencontainers.image.licenses="Apache-2.0"

COPY --from=builder /source/target/release/bitbucket-mcp-server /usr/local/bin/bitbucket-mcp-server
COPY LICENSE /licenses/Apache-2.0.txt
USER nonroot:nonroot
ENTRYPOINT ["/usr/local/bin/bitbucket-mcp-server"]
