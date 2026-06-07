use ntex::web;

use crate::bd::DBWrapper;
use crate::errors::AppError;

pub mod bd;
pub mod config;
pub mod errors;
pub mod generated;
pub mod logging;
pub mod logic;
pub mod routes;

#[ntex::main]
async fn main() -> Result<(), AppError> {
    // --- Init logger (compile-time level via Cargo features) ---
    logging::init()?;
    log::info!("=== micro_tamplate starting ===");

    // --- Config via `config!` macro ---
    let db_path: String = config!("DB_PATH", "test.redb");
    let bind_addr: String = config!("BIND_ADDR", "localhost:8080");

    log::info!("database path: {db_path}");
    log::info!("binding to: {bind_addr}");

    // --- Open database ---
    log::info!("opening database...");
    let bd = DBWrapper::new(&db_path)?;
    log::info!("database opened successfully");

    // --- Spawn background maintenance ---
    log::info!("spawning maintenance loop (every 24h)");
    bd.spawn_maintenance_loop();

    // --- Build app ---
    let app = async move || {
        log::trace!("building new application scope");
        web::App::new().state(bd.clone()).configure(routes::routes)
    };

    // --- Start server ---
    log::info!("starting HTTP server on {bind_addr}");
    let server = web::server(app);

    server.bind(&bind_addr)?.run().await?;
    log::info!("server shut down");

    Ok(())
}
