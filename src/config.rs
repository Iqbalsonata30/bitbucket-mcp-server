use std::{
    env, fmt, fs,
    path::{Path, PathBuf},
    time::Duration,
};

use serde::Deserialize;
use thiserror::Error;
use url::Url;

const DEFAULT_TIMEOUT_SECONDS: u64 = 30;
const DEFAULT_CONNECT_TIMEOUT_SECONDS: u64 = 10;
const DEFAULT_MAX_RESPONSE_BYTES: usize = 10 * 1024 * 1024;
const DEFAULT_POOL_MAX_IDLE_PER_HOST: usize = 8;
const DEFAULT_USER_AGENT: &str = concat!("bitbucket-mcp-server/", env!("CARGO_PKG_VERSION"));

#[derive(Clone)]
pub struct SecretString(String);

impl SecretString {
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(value)
    }

    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

#[derive(Clone, Debug)]
pub enum Authentication {
    Token {
        token: SecretString,
    },
    Basic {
        username: String,
        password: SecretString,
    },
}

#[derive(Clone, Debug)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub base_url: Url,
    pub authentication: Authentication,
    pub timeout: Duration,
    pub connect_timeout: Duration,
    pub tls_verify: bool,
    pub ca_file: Option<PathBuf>,
    pub retry: RetryPolicy,
    pub max_response_bytes: usize,
    pub pool_max_idle_per_host: usize,
    pub user_agent: String,
}

impl Settings {
    /// Loads an optional TOML file and overlays `BITBUCKET_*` environment variables.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when a source cannot be read or configuration is
    /// missing, malformed, or unsafe.
    pub fn load(path: Option<&Path>) -> Result<Self, ConfigError> {
        let mut raw = match path {
            Some(path) => {
                let contents = fs::read_to_string(path).map_err(|source| ConfigError::Read {
                    path: path.to_path_buf(),
                    source,
                })?;
                toml::from_str(&contents).map_err(ConfigError::Parse)?
            }
            None => RawSettings::default(),
        };
        raw.overlay_environment(env::vars())?;
        Self::try_from(raw)
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read configuration file {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse configuration file: {0}")]
    Parse(toml::de::Error),

    #[error("failed to read secret file {path}: {source}")]
    ReadSecret {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("missing configuration value: {0}")]
    Missing(&'static str),

    #[error("invalid configuration value for {field}: {message}")]
    Invalid {
        field: &'static str,
        message: String,
    },
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawSettings {
    base_url: Option<String>,
    auth: RawAuthentication,
    timeout_seconds: Option<u64>,
    connect_timeout_seconds: Option<u64>,
    tls_verify: Option<bool>,
    ca_file: Option<PathBuf>,
    retry: RawRetryPolicy,
    max_response_bytes: Option<usize>,
    pool_max_idle_per_host: Option<usize>,
    user_agent: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawAuthentication {
    r#type: Option<String>,
    token: Option<String>,
    token_file: Option<PathBuf>,
    username: Option<String>,
    password: Option<String>,
    password_file: Option<PathBuf>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawRetryPolicy {
    max_attempts: Option<u32>,
    base_delay_millis: Option<u64>,
    max_delay_millis: Option<u64>,
}

impl RawSettings {
    fn overlay_environment<I>(&mut self, variables: I) -> Result<(), ConfigError>
    where
        I: IntoIterator<Item = (String, String)>,
    {
        for (key, value) in variables {
            match key.as_str() {
                "BITBUCKET_BASE_URL" => self.base_url = Some(value),
                "BITBUCKET_AUTH_TYPE" => self.auth.r#type = Some(value),
                "BITBUCKET_TOKEN" => {
                    self.auth.token = Some(value);
                    self.auth.token_file = None;
                }
                "BITBUCKET_TOKEN_FILE" => {
                    self.auth.token_file = Some(PathBuf::from(value));
                    self.auth.token = None;
                }
                "BITBUCKET_USERNAME" => self.auth.username = Some(value),
                "BITBUCKET_PASSWORD" => {
                    self.auth.password = Some(value);
                    self.auth.password_file = None;
                }
                "BITBUCKET_PASSWORD_FILE" => {
                    self.auth.password_file = Some(PathBuf::from(value));
                    self.auth.password = None;
                }
                "BITBUCKET_TIMEOUT_SECONDS" => {
                    self.timeout_seconds = Some(parse_env(&key, &value)?);
                }
                "BITBUCKET_CONNECT_TIMEOUT_SECONDS" => {
                    self.connect_timeout_seconds = Some(parse_env(&key, &value)?);
                }
                "BITBUCKET_TLS_VERIFY" => self.tls_verify = Some(parse_env(&key, &value)?),
                "BITBUCKET_CA_FILE" => self.ca_file = Some(PathBuf::from(value)),
                "BITBUCKET_RETRY_MAX_ATTEMPTS" => {
                    self.retry.max_attempts = Some(parse_env(&key, &value)?);
                }
                "BITBUCKET_RETRY_BASE_DELAY_MILLIS" => {
                    self.retry.base_delay_millis = Some(parse_env(&key, &value)?);
                }
                "BITBUCKET_RETRY_MAX_DELAY_MILLIS" => {
                    self.retry.max_delay_millis = Some(parse_env(&key, &value)?);
                }
                "BITBUCKET_MAX_RESPONSE_BYTES" => {
                    self.max_response_bytes = Some(parse_env(&key, &value)?);
                }
                "BITBUCKET_POOL_MAX_IDLE_PER_HOST" => {
                    self.pool_max_idle_per_host = Some(parse_env(&key, &value)?);
                }
                "BITBUCKET_USER_AGENT" => self.user_agent = Some(value),
                _ => {}
            }
        }
        Ok(())
    }
}

impl TryFrom<RawSettings> for Settings {
    type Error = ConfigError;

    fn try_from(raw: RawSettings) -> Result<Self, Self::Error> {
        let raw_base_url = required(raw.base_url, "base_url")?;
        let base_url = normalize_base_url(&raw_base_url)?;
        let authentication = parse_authentication(raw.auth)?;
        let timeout_seconds = raw.timeout_seconds.unwrap_or(DEFAULT_TIMEOUT_SECONDS);
        let connect_timeout_seconds = raw
            .connect_timeout_seconds
            .unwrap_or(DEFAULT_CONNECT_TIMEOUT_SECONDS);
        let max_attempts = raw.retry.max_attempts.unwrap_or(3);
        let base_delay_millis = raw.retry.base_delay_millis.unwrap_or(100);
        let max_delay_millis = raw.retry.max_delay_millis.unwrap_or(2_000);
        let max_response_bytes = raw.max_response_bytes.unwrap_or(DEFAULT_MAX_RESPONSE_BYTES);
        let pool_max_idle_per_host = raw
            .pool_max_idle_per_host
            .unwrap_or(DEFAULT_POOL_MAX_IDLE_PER_HOST);
        let user_agent = raw
            .user_agent
            .unwrap_or_else(|| DEFAULT_USER_AGENT.to_owned());

        validate_positive("timeout_seconds", &timeout_seconds)?;
        validate_positive("connect_timeout_seconds", &connect_timeout_seconds)?;
        validate_positive("retry.max_attempts", &max_attempts)?;
        validate_positive("max_response_bytes", &max_response_bytes)?;
        validate_positive("pool_max_idle_per_host", &pool_max_idle_per_host)?;
        if base_delay_millis > max_delay_millis {
            return Err(ConfigError::Invalid {
                field: "retry.base_delay_millis",
                message: "must not exceed retry.max_delay_millis".to_owned(),
            });
        }
        if user_agent.trim().is_empty() {
            return Err(ConfigError::Invalid {
                field: "user_agent",
                message: "must not be empty".to_owned(),
            });
        }

        Ok(Self {
            base_url,
            authentication,
            timeout: Duration::from_secs(timeout_seconds),
            connect_timeout: Duration::from_secs(connect_timeout_seconds),
            tls_verify: raw.tls_verify.unwrap_or(true),
            ca_file: raw.ca_file,
            retry: RetryPolicy {
                max_attempts,
                base_delay: Duration::from_millis(base_delay_millis),
                max_delay: Duration::from_millis(max_delay_millis),
            },
            max_response_bytes,
            pool_max_idle_per_host,
            user_agent,
        })
    }
}

fn parse_authentication(raw: RawAuthentication) -> Result<Authentication, ConfigError> {
    let auth_type = required(raw.r#type, "auth.type")?;
    match auth_type.to_ascii_lowercase().as_str() {
        "token" => Ok(Authentication::Token {
            token: SecretString::new(resolve_secret(
                raw.token,
                raw.token_file,
                "auth.token",
                "auth.token_file",
            )?),
        }),
        "basic" => Ok(Authentication::Basic {
            username: non_empty(required(raw.username, "auth.username")?, "auth.username")?,
            password: SecretString::new(resolve_secret(
                raw.password,
                raw.password_file,
                "auth.password",
                "auth.password_file",
            )?),
        }),
        _ => Err(ConfigError::Invalid {
            field: "auth.type",
            message: "must be 'token' or 'basic'".to_owned(),
        }),
    }
}

fn resolve_secret(
    value: Option<String>,
    file: Option<PathBuf>,
    value_field: &'static str,
    file_field: &'static str,
) -> Result<String, ConfigError> {
    match (value, file) {
        (Some(_), Some(_)) => Err(ConfigError::Invalid {
            field: value_field,
            message: format!("must not be set together with {file_field}"),
        }),
        (Some(value), None) => non_empty(value, value_field),
        (None, Some(path)) => {
            let value = fs::read_to_string(&path)
                .map_err(|source| ConfigError::ReadSecret { path, source })?;
            non_empty(value.trim_end_matches(['\r', '\n']).to_owned(), file_field)
        }
        (None, None) => Err(ConfigError::Missing(value_field)),
    }
}

fn normalize_base_url(value: &str) -> Result<Url, ConfigError> {
    let mut url = Url::parse(value).map_err(|error| ConfigError::Invalid {
        field: "base_url",
        message: error.to_string(),
    })?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(ConfigError::Invalid {
            field: "base_url",
            message: "scheme must be http or https".to_owned(),
        });
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(ConfigError::Invalid {
            field: "base_url",
            message: "must not contain credentials, a query, or a fragment".to_owned(),
        });
    }
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    Ok(url)
}

fn required<T>(value: Option<T>, field: &'static str) -> Result<T, ConfigError> {
    value.ok_or(ConfigError::Missing(field))
}

fn non_empty(value: String, field: &'static str) -> Result<String, ConfigError> {
    if value.trim().is_empty() {
        return Err(ConfigError::Invalid {
            field,
            message: "must not be empty".to_owned(),
        });
    }
    Ok(value)
}

fn validate_positive<T>(field: &'static str, value: &T) -> Result<(), ConfigError>
where
    T: Default + PartialEq,
{
    if *value == T::default() {
        return Err(ConfigError::Invalid {
            field,
            message: "must be greater than zero".to_owned(),
        });
    }
    Ok(())
}

fn parse_env<T>(key: &str, value: &str) -> Result<T, ConfigError>
where
    T: std::str::FromStr,
    T::Err: fmt::Display,
{
    value.parse().map_err(|error: T::Err| ConfigError::Invalid {
        field: "environment",
        message: format!("{key}: {error}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_raw() -> RawSettings {
        RawSettings {
            base_url: Some("https://bitbucket.example.test/context".to_owned()),
            auth: RawAuthentication {
                r#type: Some("token".to_owned()),
                token: Some("secret".to_owned()),
                ..RawAuthentication::default()
            },
            ..RawSettings::default()
        }
    }

    #[test]
    fn secrets_are_redacted_from_debug_output() {
        let settings = Settings::try_from(valid_raw()).expect("valid settings");
        let debug = format!("{settings:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("secret"));
    }

    #[test]
    fn base_url_preserves_context_and_adds_trailing_slash() {
        let settings = Settings::try_from(valid_raw()).expect("valid settings");
        assert_eq!(
            settings.base_url.as_str(),
            "https://bitbucket.example.test/context/"
        );
    }

    #[test]
    fn environment_values_override_file_values() {
        let mut raw = valid_raw();
        raw.overlay_environment([("BITBUCKET_TIMEOUT_SECONDS".to_owned(), "45".to_owned())])
            .expect("valid environment");
        let settings = Settings::try_from(raw).expect("valid settings");
        assert_eq!(settings.timeout, Duration::from_secs(45));
    }

    #[test]
    fn token_can_be_loaded_from_a_secret_file() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("token");
        fs::write(&path, "mounted-secret\n").expect("secret is written");
        let mut raw = valid_raw();
        raw.auth.token = None;
        raw.auth.token_file = Some(path);

        let settings = Settings::try_from(raw).expect("secret file is accepted");
        let Authentication::Token { token } = settings.authentication else {
            panic!("token authentication expected");
        };
        assert_eq!(token.expose(), "mounted-secret");
    }

    #[test]
    fn inline_and_file_secrets_are_mutually_exclusive() {
        let mut raw = valid_raw();
        raw.auth.token_file = Some(PathBuf::from("/unused"));
        assert!(matches!(
            Settings::try_from(raw),
            Err(ConfigError::Invalid {
                field: "auth.token",
                ..
            })
        ));
    }
}
