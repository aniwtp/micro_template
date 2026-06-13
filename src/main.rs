use ntex::web;
use std::time::Duration;

use db_wrapper::DBWrapper;
use db_wrapper::counter::BUFFER_FLUSH_SECS;
use {{crate_name}}::errors::AppError;
use {{crate_name}}::{config, logging, routes};

#[ntex::main]
async fn main() -> Result<(), AppError> {
    logging::init()?;
    log::info!("=== {{project-name}} starting ===");

    let db_path: String = config!("DB_PATH", "test.redb".to_owned());
    let bind_addr: String = config!("BIND_ADDR", "localhost:8080".to_owned());

    log::info!("database path: {db_path}");
    log::info!("binding to: {bind_addr}");

    // --- Open database ---
    log::info!("opening database...");
    let db = DBWrapper::new(&db_path)?;
    log::info!("database opened successfully");

    // --- Register tables ---
    {{crate_name}}::db::init_tables(&db);

    // --- Spawn background maintenance ---
    log::info!("spawning maintenance loop (flush every {BUFFER_FLUSH_SECS}s)");
    let db2 = db.clone();
    drop(ntex::rt::spawn(async move {
        let interval = Duration::from_secs(BUFFER_FLUSH_SECS);
        // 24h / interval = TICKS_PER_DAY
        let ticks_per_day: u32 = (24 * 60 * 60) / BUFFER_FLUSH_SECS as u32;
        let mut tick: u32 = 0;
        loop {
            ntex::time::sleep(interval).await;
            tick = tick.wrapping_add(1);

            // Flush buffers (counters synced via callback).
            if let Err(e) = db2.flush_buffers() {
                log::error!("buffer flush error: {e}");
            }

            // Daily compact + backup.
            if tick % ticks_per_day == 0 {
                log::info!("maintenance: daily compact + backup");
                if let Err(e) = db2.compact() {
                    log::error!("compaction error: {e}");
                }
                if let Err(e) = db2.backup() {
                    log::error!("backup error: {e}");
                }
            }
        }
    }));

    // --- Build app ---
    let app = async move || {
        log::trace!("building new application scope");
        web::App::new().state(db.clone()).configure(routes::routes)
    };

    // --- Start server ---
    log::info!("starting HTTP server on {bind_addr}");
    let server = web::server(app);

    server.bind(&bind_addr)?.run().await?;
    log::info!("server shut down");
    Ok(())
}
