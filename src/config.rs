//! Configuration reader with priority: secrets files → env vars → `.env` file.
//!
//! The [`config!`] macro takes a key and returns `Option<String>`, or you can
//! provide a default value.
//!
//! # Search order (first match wins)
//!
//! 1. `/run/secrets/<key>`        — Docker / K8s secrets mount
//! 2. `secrets/<key>`             — local secrets directory
//! 3. `std::env::var(<key>)`      — process environment
//! 4. `<key>=...` in `.env` file  — dotenv file at project root
//!
//! # Examples
//!
//! ```ignore
//! // Returns Option<String>
//! let db_path: Option<String> = config!("DB_PATH");
//!
//! // With default value
//! let host: String = config!("HOST", "localhost");
//! let port: u16  = config!("PORT", 8080);
//! ```

use std::{collections::HashMap, path::Path, sync::OnceLock};

// ---------------------------------------------------------------------------
// .env cache — parsed once, reused forever
// ---------------------------------------------------------------------------

static DOTENV: OnceLock<HashMap<String, String>> = OnceLock::new();

fn dotenv_map() -> &'static HashMap<String, String> {
    DOTENV.get_or_init(|| parse_dotenv(find_dotenv().as_deref()))
}

/// Look for `.env` relative to the current working directory, then walk up.
fn find_dotenv() -> Option<std::path::PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let candidate = dir.join(".env");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

/// Minimal `.env` parser — handles `KEY=VALUE`, ignores comments and blank
/// lines.  Does **not** support quoting, escaping, or interpolation
/// (intentionally — this is for simple configs, not a full dotenv engine).
fn parse_dotenv(path: Option<&Path>) -> HashMap<String, String> {
    let Some(path) = path else {
        log::debug!(".env file not found — dotenv source is empty");
        return HashMap::new();
    };

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("cannot read .env at {}: {e}", path.display());
            return HashMap::new();
        },
    };

    log::debug!("loading .env from {}", path.display());

    let mut map = HashMap::new();
    for (line_no, raw) in content.lines().enumerate() {
        let line = raw.trim();

        // Skip blanks and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Split on first `=`
        let Some((key, val)) = line.split_once('=') else {
            log::warn!(".env:{} — skipping malformed line: {raw}", line_no + 1);
            continue;
        };

        let key = key.trim().to_owned();
        let val = val.trim().to_owned();

        if key.is_empty() {
            log::warn!(".env:{} — empty key, skipping", line_no + 1);
            continue;
        }

        log::trace!(".env: {key} => {val}");
        map.insert(key, val);
    }

    log::debug!("parsed {} keys from .env", map.len());
    map
}

// ---------------------------------------------------------------------------
// Secrets files — two locations
// ---------------------------------------------------------------------------

/// Read `/run/secrets/<key>` (Docker/K8s convention) or `secrets/<key>`.
fn read_secret_file(key: &str) -> Option<String> {
    for root in &["/run/secrets", "secrets"] {
        let path = Path::new(root).join(key);
        if path.is_file() {
            match std::fs::read_to_string(&path) {
                Ok(val) => {
                    let trimmed = val.trim().to_owned();
                    log::debug!("config: found {key} in secret file {}", path.display());
                    return Some(trimmed);
                },
                Err(e) => {
                    log::warn!("config: cannot read secret file {}: {e}", path.display());
                },
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Look up `key` in order: secrets → env → `.env`.
///
/// Returns the **first** value found, or `None`.
pub fn get_config(key: &str) -> Option<String> {
    // 1. Secrets files
    if let Some(val) = read_secret_file(key) {
        return Some(val);
    }

    // 2. Environment variables
    match std::env::var(key) {
        Ok(val) => {
            log::trace!("config: found {key} in environment");
            return Some(val);
        },
        Err(std::env::VarError::NotPresent) => { /* fall through */ },
        Err(e) => {
            log::warn!("config: env var {key} is invalid: {e}");
        },
    }

    // 3. .env file
    if let Some(val) = dotenv_map().get(key) {
        log::trace!("config: found {key} in .env");
        return Some(val.clone());
    }

    log::trace!("config: {key} not found in any source");
    None
}

// ---------------------------------------------------------------------------
// Convenience macro
// ---------------------------------------------------------------------------

/// Look up a configuration value.
///
/// ```ignore
/// config!("KEY")           → Option<String>
/// config!("KEY", "default") → String        (with default)
/// config!("KEY", 8080_u16)  → u16           (parsed from string)
/// ```
///
/// For the default form, the default **must** implement `Into<String>`,
/// and for numeric types it additionally implements `FromStr`.
#[macro_export]
macro_rules! config {
    // No default — returns Option<String>
    ($key:expr) => {
        $crate::config::get_config($key)
    };
    // With default — panics if the default cannot be parsed (shouldn't happen
    // for literals).
    ($key:expr, $default:expr) => {{
        let default_str: String = ($default).into();
        $crate::config::get_config($key).unwrap_or(default_str)
    }};
}
