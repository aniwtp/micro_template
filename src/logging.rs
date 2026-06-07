//! Stderr logger with timestamp, level, target, and optional file location.
//!
//! The **compile-time** maximum level is set via Cargo features:
//!
//! | Command                                                    | Included levels         |
//! |------------------------------------------------------------|-------------------------|
//! | `cargo build` _(default)_                                  | `info` + `warn` + `error` |
//! | `cargo build --no-default-features --features log-trace`   | all (`trace`+)          |
//! | `cargo build --no-default-features --features log-debug`   | `debug` + above         |
//! | `cargo build --no-default-features --features log-error`   | `error` only            |
//! | `cargo build --no-default-features --features log-off`     | (none)                  |
//!
//! Any code below the compile-time level is **stripped entirely** by the
//! compiler — zero runtime cost for disabled levels.

use log::{LevelFilter, Log, Metadata, Record, SetLoggerError};

struct StderrLogger;

impl Log for StderrLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true // actual filtering is done at compile time by `log` macros
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let ts = current_timestamp();
        let level = record.level();
        let target = record.target();
        let msg = record.args();

        // Include file & line for debug and lower
        if level <= log::Level::Debug {
            let file = record.file().unwrap_or("<unknown>");
            let line = record.line().unwrap_or(0);
            eprintln!("{ts} [{level:<5}] [{target}] {file}:{line} — {msg}");
        } else {
            eprintln!("{ts} [{level:<5}] [{target}] {msg}");
        }
    }

    fn flush(&self) {
        // stderr is unbuffered by default, but we still honour the contract.
        use std::io::Write;
        let _ = std::io::stderr().flush();
    }
}

/// Initialise the logger. Must be called once at startup.
///
/// The maximum level is locked in at **compile time**; calling `init` only
/// activates the runtime side (formatting + output).  No runtime filtering
/// overhead — use the features described in the module docs.
pub fn init() -> Result<(), SetLoggerError> {
    log::set_logger(&StderrLogger)?;
    log::set_max_level(LevelFilter::Trace); // compile-time features do the real filtering
    Ok(())
}

/// Returns UTC timestamp with milliseconds: `HH:MM:SS.mmm`
fn current_timestamp() -> String {
    let now =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let secs = now.as_secs();
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    let ms = now.subsec_millis();
    format!("{h:02}:{m:02}:{s:02}.{ms:03}")
}
