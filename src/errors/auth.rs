//! Authentication / authorisation errors.

/// Errors from the auth layer (login, token validation, etc.).
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    /// Wrong username or password.
    #[error("invalid credentials")]
    InvalidCredentials,

    /// The session / refresh token has expired.
    #[error("token expired")]
    TokenExpired,

    /// The token is malformed or tampered with.
    #[error("invalid token")]
    InvalidToken,

    /// The user does not exist in the database.
    #[error("user not found: {0}")]
    UserNotFound(String),

    /// The account exists but is not active (e.g. banned, unverified).
    #[error("account `{0}` is {1}")]
    AccountInactive(String, String),

    /// Flatbuffer request body could not be parsed.
    #[error("malformed request body: {0}")]
    MalformedRequest(String),

    /// A required field is missing from the request.
    #[error("missing required field: {0}")]
    MissingField(String),
}
