# {{project-name}}

Backend microservice template on **ntex** + **redb** + **FlatBuffers**.

Infrastructure in separate crates:
- **tiny-log** — stderr logger (git dep)
- **simple-conf** — config reader (git dep)
- **db-wrapper** — redb wrapper (git dep)

---

## Quick start

```sh
# 1. Generate from template
cargo generate --git <repo-url> --name my-backend

# 2. Copy env (optional — defaults work out of the box)
cp .env.example .env

# 3. Build & run
cargo build
cargo run
```

---

## Project structure

```
├── build.rs                 # flatc codegen (always rebuilds on change)
├── Cargo.toml               # deps + compile-time log level features
├── clippy.toml              # linter rules
├── rustfmt.toml             # formatter rules
├── .env.example             # config reference
├── AGENT.md                 # AI agent instructions
├── README.md                # ← this file
│
├── flatbuffers/             # IDL schemas
│   └── login.fbs            # LoginRequest / TokenResponse / RSTokens
│
└── src/
    ├── main.rs              # entrypoint (thin — wires everything)
    ├── lib.rs               # module declarations + re-exports
    │
    ├── utils/
    │   ├── convert.rs       # enum LoginReply — FB response builders
    │   ├── db/
    │   │   ├── mod.rs       # table registration
    │   │   └── team.rs      # team table + TeamDb trait
    │   └── errors/
    │       ├── mod.rs       # AppError (umbrella)
    │       └── auth.rs      # AuthError
    │
    ├── routes/
    │   ├── mod.rs           # /v1 scope
    │   └── auth/
    │       ├── mod.rs       # /v1/auth scope
    │       └── login.rs     # POST /v1/auth/login
    │
    ├── logic/mod.rs         # business logic
    └── generated/           # flatc output (gitignored)
```

---

## Stack

| Component | Crate | Why |
|-----------|-------|-----|
| Runtime | **ntex** on compio (io-uring) | Async, actor-less, no tokio |
| Database | **shodh-redb** (embedded K/V) | Single-file, TTL, no daemon |
| DB wrapper | **db-wrapper** (git) | Write buffers, helpers |
| Serialisation | **FlatBuffers** + `rust_flatbuffer_macros` | Zero-copy + macro builder |
| Logging | **tiny-log** (git) | stderr, compile-time levels |
| Config | **simple-conf** (git) | secrets → env → .env |
| Errors | **thiserror** | Derive macros, `#[from]` |

---

## Configuration

Priority (first match wins): **`/run/secrets/<KEY>` → `secrets/<KEY>` → env var → `.env`**

```rust
use simple_conf::config;

let val: Option<String> = config!("SOME_KEY");
let host: String = config!("BIND_ADDR", "localhost:8080".into());
let port: u16  = config!("PORT", 8080);
```

| Key | Default | Description |
|-----|---------|-------------|
| `DB_PATH` | `test.redb` | Database file path |
| `BIND_ADDR` | `localhost:8080` | HTTP listen address |

---

## Logging levels

Compile-time feature flags — pick exactly **one**:

```sh
cargo build                                            # info+ (default)
cargo build --no-default-features --features log-trace  # all levels
cargo build --no-default-features --features log-debug  # debug+
cargo build --no-default-features --features log-warn   # warn+
cargo build --no-default-features --features log-error  # error only
cargo build --no-default-features --features log-off    # all stripped
```

Code below the chosen level is removed by the compiler — zero runtime overhead.

---

## Endpoints

| Method | Path | Status |
|--------|------|--------|
| `POST` | `/v1/auth/login` | ✅ working (placeholder logic) |

### Login request/response

**Request** — FlatBuffer `LoginRequest`:
- `username: string`
- `password: string`

**Response** — FlatBuffer `TokenResponse`:
- `token: RSTokens { refresh: [u8; 21], session: [u8; 11] }`

Response building via `LoginReply` enum in `utils/convert.rs`:

```rust
let reply = LoginReply::Success { refresh: [0u8; 21], session: [0u8; 11] };
let body = reply.to_flatbuffer();  // → Vec<u8> ready for HTTP response
```

---

## Development commands

```sh
cargo build            # default (info logging)
cargo run              # start server
cargo clippy           # lint
cargo fmt              # format
```

---

## Conventions

- `snake_case` for modules/files, `CamelCase` for types
- `use crate::module::Type` in library code
- `use {{crate_name}}::…` in `main.rs` (replaced by cargo-generate)
- Errors via `thiserror`, `#[from]` for auto-conversion
- Responses via enums in `utils/convert.rs` with `.to_flatbuffer()`
- Always include module/endpoint name in log messages

---

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `ntex` | 3.10 | Web framework (compio) |
| `shodh-redb` | 0.5 | Embedded K/V with TTL |
| `db-wrapper` | git | redb wrapper + write buffers |
| `flatbuffers` | 25.12 | Zero-copy serialisation |
| `rust_flatbuffer_macros` | 1.1 | `build_flatbuffer!` macro |
| `paste` | 1 | Identifier concatenation |
| `log` | 0.4 | Logging facade |
| `thiserror` | 2.0 | Error derive |
| `tiny-log` | git | Stderr logger (external) |
| `simple-conf` | git | Config reader (external) |
