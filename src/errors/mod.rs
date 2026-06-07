//! Centralised error types for `micro_tamplate`.
//!
//! Every subsystem returns its own error enum; the top-level [`AppError`]
//! unifies them via `#[from]` so that `?` works across layers.
//!
//! # Modules
//!
//! | Module          | Enum            | Layer            |
//! |-----------------|-----------------|------------------|
//! | [`db`]          | [`DbError`]     | database / redb  |
//! | [`config`]      | [`ConfigError`] | secrets/env/.env |
//! | [`auth`]        | [`AuthError`]   | login / tokens   |
//!
//! # Usage
//!
//! ```ignore
//! use crate::errors::{AppError, DbError, AuthError, ConfigError};
//!
//! fn do_work() -> Result<(), AppError> {
//!     let db = DBWrapper::new("test.redb")?;       // DbError → AppError
//!     let val = config!("KEY").ok_or(ConfigError::MissingKey("KEY".into()))?;
//!     Ok(())
//! }
//! ```

mod auth;
mod config;
mod db;

pub use auth::AuthError;
pub use config::ConfigError;
pub use db::DbError;

// ---------------------------------------------------------------------------
// Top-level application error
// ---------------------------------------------------------------------------

/// Umbrella error that any fallible public API can return.
///
/// Every variant implements `From<T>` so the `?` operator propagates
/// subsystem errors without manual conversion.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// Database layer error.
    #[error("database: {0}")]
    Db(#[from] DbError),

    /// Configuration layer error.
    #[error("config: {0}")]
    Config(#[from] ConfigError),

    /// Authentication / authorisation error.
    #[error("auth: {0}")]
    Auth(#[from] AuthError),

    /// I/O error (filesystem, network).
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// Logger initialisation failed.
    #[error("logger: {0}")]
    Logger(#[from] log::SetLoggerError),

    /// Flatbuffer (de)serialization error.
    #[error("flatbuffer: {0}")]
    Flatbuffer(#[from] flatbuffers::InvalidFlatbuffer),

    /// Catch-all for one-off messages.
    #[error("{0}")]
    Other(String),
}
