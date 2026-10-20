# Configuration and Authentication

Configuration precedence is:

```text
command-line override > environment variable > configuration file > default
```

Select a file with `--config <path>` or `BITBUCKET_CONFIG_FILE`. Individual
`BITBUCKET_*` variables override values loaded from that file.

Secrets are accepted from environment variables, mounted secret files, or the
selected configuration file and have no defaults. Mounted files are recommended
for production. Configuration errors fail startup before MCP initialization.

## TOML

```toml
base_url = "https://bitbucket.example.com/bitbucket"
timeout_seconds = 30
connect_timeout_seconds = 10
tls_verify = true
ca_file = "/etc/ssl/certs/enterprise-ca.pem"
max_response_bytes = 10485760
pool_max_idle_per_host = 8
user_agent = "bitbucket-mcp-server/0.1.0"

[auth]
type = "token"
token_file = "/run/secrets/bitbucket_token"

[retry]
max_attempts = 3
base_delay_millis = 100
max_delay_millis = 2000
```

For Basic authentication:

```toml
[auth]
type = "basic"
username = "service-account"
password_file = "/run/secrets/bitbucket_password"
```

Unknown TOML fields are rejected to catch configuration mistakes. A base URL
may include an installation context path such as `/bitbucket`, but credentials,
queries, and fragments in the URL are rejected.

## Environment

| Setting | Environment variable | Purpose |
| --- | --- | --- |
| Base URL | `BITBUCKET_BASE_URL` | Root URL of the self-hosted installation |
| Auth mode | `BITBUCKET_AUTH_TYPE` | `token` or `basic` |
| Token | `BITBUCKET_TOKEN` | Personal access token |
| Token file | `BITBUCKET_TOKEN_FILE` | File containing a personal access token |
| Username | `BITBUCKET_USERNAME` | Basic-auth username |
| Password | `BITBUCKET_PASSWORD` | Basic-auth password |
| Password file | `BITBUCKET_PASSWORD_FILE` | File containing a Basic-auth password |
| Timeout | `BITBUCKET_TIMEOUT_SECONDS` | Whole-request timeout |
| Connect timeout | `BITBUCKET_CONNECT_TIMEOUT_SECONDS` | Connection establishment timeout |
| TLS verification | `BITBUCKET_TLS_VERIFY` | Certificate verification policy |
| CA file | `BITBUCKET_CA_FILE` | Additional enterprise CA bundle |
| Retry count | `BITBUCKET_RETRY_MAX_ATTEMPTS` | Bounded retry attempts |
| Retry base delay | `BITBUCKET_RETRY_BASE_DELAY_MILLIS` | Initial exponential backoff |
| Retry maximum delay | `BITBUCKET_RETRY_MAX_DELAY_MILLIS` | Backoff and `Retry-After` cap |
| Response limit | `BITBUCKET_MAX_RESPONSE_BYTES` | Maximum decoded response body |
| Pool idle limit | `BITBUCKET_POOL_MAX_IDLE_PER_HOST` | Connection-pool tuning |
| User agent | `BITBUCKET_USER_AGENT` | Outbound identification |
| Log filter | `RUST_LOG` | `tracing_subscriber` filter |

Exactly one authentication mode must be configured. Secret values are wrapped
in redacting types and are never included in `Debug`, errors, traces, or metrics.
TLS verification defaults to enabled. Disabling it requires an explicit setting
and emits a warning.

For each secret, configure either the inline value or its `_FILE` alternative,
never both. Environment values override file configuration and automatically
clear the conflicting source. Trailing CR/LF characters from mounted secret
files are removed; otherwise secret bytes are preserved.

Bearer token example:

```bash
export BITBUCKET_BASE_URL="https://bitbucket.example.com/bitbucket"
export BITBUCKET_AUTH_TYPE="token"
export BITBUCKET_TOKEN="<personal-access-token>"
```

Basic example:

```bash
export BITBUCKET_AUTH_TYPE="basic"
export BITBUCKET_USERNAME="<username>"
export BITBUCKET_PASSWORD="<password>"
```
