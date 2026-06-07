//! Configuration-layer errors.

/// Errors from the config reader ([`config!`](crate::config!)).
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// An environment variable contains non-unicode bytes.
    #[error("environment variable `{0}` is not valid unicode")]
    InvalidEnv(String),

    /// The `.env` file exists but cannot be read.
    #[error("cannot read .env file `{0}`: {1}")]
    DotenvRead(String, #[source] std::io::Error),

    /// A secrets file exists but cannot be read.
    #[error("cannot read secret file `{0}`: {1}")]
    SecretRead(String, #[source] std::io::Error),

    /// The requested key was not found in *any* source (secrets, env, .env).
    #[error("required config key `{0}` not found — checked secrets/, env, .env")]
    MissingKey(String),
}
