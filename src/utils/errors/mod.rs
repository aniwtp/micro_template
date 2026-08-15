//! Centralised error types for `{{project-name}}`.

use ntex::http::StatusCode;
use ntex::web::{HttpRequest, HttpResponse, WebResponseError};

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

    #[error("auth: {0}")]
    Token(#[from] user_validate::TokenError),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("logger: {0}")]
    Logger(#[from] log::SetLoggerError),

    #[error("flatbuffer: {0}")]
    Flatbuffer(#[from] flatbuffers::InvalidFlatbuffer),

    #[error("{0}")]
    Other(String),
}

// ---------------------------------------------------------------------------
// Auto-convert AppError → HTTP response via ?
// ---------------------------------------------------------------------------

impl WebResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::Auth(e) => match e {
                AuthError::InvalidCredentials => StatusCode::UNAUTHORIZED,
                AuthError::TokenExpired => StatusCode::UNAUTHORIZED,
                AuthError::InvalidToken => StatusCode::UNAUTHORIZED,
                AuthError::MalformedRequest(_) => StatusCode::BAD_REQUEST,
                AuthError::MissingField(_) => StatusCode::BAD_REQUEST,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            },
            Self::Token(_) => StatusCode::UNAUTHORIZED,
            Self::Flatbuffer(_) => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self, _: &HttpRequest) -> HttpResponse {
        log::warn!("request error: {self}");
        HttpResponse::new(self.status_code()).set_body(ntex::http::body::Body::from(self.to_string()))
    }
}
