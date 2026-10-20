# Security Policy

## Supported Versions

Security fixes are applied to the latest released minor version. Until the
project reaches `1.0`, only the newest `0.x` release is supported.

## Reporting a Vulnerability

Do not open a public issue for a suspected vulnerability. Use the repository's
private security-advisory feature and include:

- Affected version and deployment mode
- Reproduction steps or proof of concept
- Expected impact
- Any known workaround

Maintainers should acknowledge reports within three business days and provide a
status update within seven business days. Coordinated disclosure is preferred.

## Security Boundaries

- Bitbucket credentials are read from environment variables or mounted files.
- Credentials and authorization headers must never be logged.
- Standard output is exclusively MCP JSON-RPC traffic.
- TLS verification is enabled by default.
- Mutating Bitbucket requests are not automatically retried.
- File, diff, and HTTP response sizes are bounded.
- The container runs as a non-root user.
