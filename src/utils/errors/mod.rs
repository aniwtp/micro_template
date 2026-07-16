//! Centralised error types for `{{project-name}}`.

mod auth;

pub use auth::AuthError;
pub use simple_conf::ConfigError;
pub use db_wrapper::DbError;

// ---------------------------------------------------------------------------
// Top-level application error
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database: {0}")]
    Db(#[from] db_wrapper::DbError),

    #[error("config: {0}")]
    Config(#[from] simple_conf::ConfigError),

    #[error("auth: {0}")]
    Auth(#[from] AuthError),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("logger: {0}")]
    Logger(#[from] log::SetLoggerError),

    #[error("flatbuffer: {0}")]
    Flatbuffer(#[from] flatbuffers::InvalidFlatbuffer),

    #[error("{0}")]
    Other(String),
}
