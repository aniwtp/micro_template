use ntex::web;
use std::time::Duration;

use db_wrapper::DBWrapper;
use db_wrapper::counter::BUFFER_FLUSH_SECS;
use {{crate_name}}::{errors::AppError, routes, utils::{db, xor::{XorState, XorMiddleware}}};
// use {{crate_name}}::utils::db;
use simple_conf::config;

#[ntex::main]
async fn main() -> Result<(), AppError> {
    tiny_log::init()?;
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
    db::init_tables(&db);

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
            if tick.is_multiple_of(ticks_per_day) {
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
let xor_state = XorState::new(65536, Duration::from_secs(60));
    // --- Build app ---
    // Give the app factory its own clone: `db` itself must stay alive after
    // `run().await` returns, for the final flush/compact/backup below.
    let db_for_app = db.clone();
    let app = async move || {
        log::trace!("building new application scope");
        web::App::new().state(db_for_app.clone()).middleware(XorMiddleware::new(xor_state.clone())).configure(routes::routes)
    };

    // --- Start server ---
    log::info!("starting HTTP server on {bind_addr}");
    let server = web::server(app);

    // `run().await` только резолвится после того, как ntex уже поймал стоп-сигнал
    // (CTRL-C везде, SIGTERM на unix) и грациозно дождался активных запросов
    // (по умолчанию shutdown_timeout = 30s). Значит здесь уже безопасно — и нужно —
    // сделать финальный синхронный flush: фоновая maintenance-таска выше отвязана
    // от сервера и иначе будет просто убита посреди цикла, молча теряя буфер.
    server.bind(&bind_addr)?.run().await?;

    log::info!("server stopped, flushing buffers before exit...");
    if let Err(e) = db.flush_buffers() {
        log::error!("final buffer flush failed: {e}");
    }
    if let Err(e) = db.compact() {
        log::error!("final compaction failed: {e}");
    }
    if let Err(e) = db.backup() {
        log::error!("final backup failed: {e}");
    }
    log::info!("shutdown complete");
    Ok(())
}
