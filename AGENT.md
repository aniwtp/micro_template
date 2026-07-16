# AGENT.md — {{project-name}}

Backend microservice template: **ntex** (async web framework on compio),
**redb** (embedded K/V database), **flatbuffers** (zero-copy serialisation).

Infrastructure extracted to separate crates:
- **tiny-log** — stderr logger with compile-time levels (git dep)
- **simple-conf** — config reader: secrets → env → .env + `config!` macro (git dep)
- **db-wrapper** — redb wrapper with write buffers (git dep)

---

## Project tree

```
tamplate/
├── build.rs                 # flatc codegen (always rebuilds on change)
├── Cargo.toml               # deps + compile-time log level features
├── clippy.toml
├── rustfmt.toml
├── .env.example             # env reference (secrets/ → env → .env)
├── AGENT.md                 # ← this file
├── README.md
│
├── flatbuffers/             # IDL schemas (source of truth)
│   └── login.fbs            # LoginRequest / TokenResponse / RSTokens
│
└── src/
    ├── main.rs              # entrypoint — thin: init logger, config, DB, server
    ├── lib.rs               # module declarations + re-exports
    │
    ├── utils/
    │   ├── mod.rs
    │   ├── convert.rs       # enum LoginReply — FlatBuffer response builders
    │   ├── db/
    │   │   ├── mod.rs       # table init
    │   │   └── team.rs      # team table definition
    │   └── errors/
    │       ├── mod.rs       # AppError (umbrella) + re-exports
    │       └── auth.rs      # AuthError
    │
    ├── routes/
    │   ├── mod.rs           # /v1 scope
    │   └── auth/
    │       ├── mod.rs       # /v1/auth scope
    │       └── login.rs     # POST /v1/auth/login
    │
    ├── logic/
    │   └── mod.rs           # business logic (auth, validation, …)
    │
    └── generated/           # flatc output — DO NOT EDIT
        ├── mod.rs
        └── login_generated.rs
```

---

## Quick commands

```sh
# Dev build (info+ logging)
cargo build

# All logging levels
cargo build --no-default-features --features log-trace

# Production build (error only)
cargo build --release --no-default-features --features log-error

# Lint
cargo clippy

# Format
cargo fmt
```

---

## Architecture

### Web framework — ntex + compio

- **ntex** v3 — async actor-less web framework (like actix-web but lighter).
- Runs on **compio** (io-uring / IOCP), not tokio.
- `#[ntex::main]` macro starts the compio runtime.
- Service config uses `web::scope("/v1").service(auth::scope())`.

### Database — redb (embedded)

- **shodh-redb** v0.5 — fork of redb with TTL tables.
- **db-wrapper** (git dep) wraps `Arc<Database>` + write buffers + common operations.
- Write buffers batch inserts by **size** or **time** (auto-flush).

### Serialisation — FlatBuffers

- Single schema file: `flatbuffers/login.fbs` (everything in one file, no includes).
- `build.rs` runs `flatc --rust` on every build (no caching — always fresh).
- Generated code lands in `src/generated/` (`.gitignore`d).
- Response building via `rust_flatbuffer_macros::build_flatbuffer!` + `LoginReply` enum.

### Response converters — `utils/convert.rs`

```rust
pub enum LoginReply {
    Success { refresh: [u8; 21], session: [u8; 11] },
}

impl LoginReply {
    pub fn to_flatbuffer(&self) -> Vec<u8> { … }
}
```

Handlers just construct the enum variant and call `.to_flatbuffer()` — all serialisation
logic is encapsulated in the enum.  Add a new response variant → one arm in the match.

### Logging — `tiny-log` (external crate)

- Stderr logger with `HH:MM:SS.mmm` format and file:line for debug/trace.
- Levels controlled by **Cargo features** on the `log` crate, not runtime config.
- Features: `log-trace`, `log-debug`, `log-info` (default), `log-warn`, `log-error`, `log-off`.
- Code **below** the chosen level is stripped by the compiler (zero runtime cost).
- Usage: `tiny_log::init()?;` then standard `log::info!()` / `log::debug!()` etc.

### Config — `simple-conf` (external crate)

- `config!` macro re-exported via `pub use simple_conf::config;` in `lib.rs`.
- Priority: **secrets/ files → env vars → .env file** (first match wins).

```rust
config!("DB_PATH")              // → Option<String>
config!("BIND_ADDR", "127.0.0.1".into()) // → String with default
config!("PORT", 8080_u16)       // → u16 (parsed)
```

### Errors — thiserror hierarchy

```text
AppError (umbrella)
├── Db(#[from] db_wrapper::DbError)
├── Config(#[from] simple_conf::ConfigError)
├── Auth(#[from] AuthError)
├── Io(#[from] std::io::Error)
├── Logger(#[from] SetLoggerError)
├── Flatbuffer(#[from] InvalidFlatbuffer)
└── Other(String)
```

`main()` returns `Result<(), AppError>` — all `?` work transparently.

---

## How to add a new endpoint

1. **Define the schema** (if needed) — add tables to `flatbuffers/login.fbs`.
2. **Build** — `cargo build` regenerates `src/generated/`.
3. **Add a response variant** — extend `LoginReply` in `utils/convert.rs`.
4. **Add a handler** — create `src/routes/new_module/action.rs`.
5. **Wire the route** — register in the parent `mod.rs`.
6. **Add errors** (if needed) — extend `AuthError` or create a new error module.
7. **Add business logic** — implement in `src/logic/`.
8. **Log** at appropriate levels: `debug!` for request/response, `warn!` for bad input, `error!` for failures.

---

## Conventions

| Area | Convention |
|------|-----------|
| Naming | `snake_case` for modules/files, `CamelCase` for types |
| Imports | `use crate::module::Type` in library code |
| Main.rs | `use {{crate_name}}::…` — replaced by cargo-generate |
| Errors | `thiserror` derive, `#[from]` for auto-conversion |
| Responses | Enum per endpoint in `utils/convert.rs`, `.to_flatbuffer()` |
| Logging | Always include table/endpoint name in log messages |
| Async | `ntex::web` handlers are `async fn` returning `impl Responder` |
| State | Pass via `ntex::web::types::State<T>` |

---

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `ntex` | 3.10 | Web framework (compio runtime) |
| `shodh-redb` | 0.5 | Embedded K/V database with TTL |
| `db-wrapper` | git | redb wrapper with write buffers |
| `flatbuffers` | 25.12 | Zero-copy serialisation |
| `rust_flatbuffer_macros` | 1.1 | `build_flatbuffer!` — simplify FB construction |
| `paste` | 1 | Identifier concatenation (used by macros) |
| `log` | 0.4 | Lightweight logging facade |
| `thiserror` | 2.0 | Derive `Error` for enums |
| `tiny-log` | git | Stderr logger (external crate) |
| `simple-conf` | git | Config reader + `config!` macro (external crate) |
