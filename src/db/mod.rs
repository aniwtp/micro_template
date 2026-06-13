//! Table declarations and init.

pub mod team;

use db_wrapper::DBWrapper;

/// Register all application tables.  Call once at startup.
pub fn init_tables(db: &DBWrapper) {
    db.init_tables(|db| {
        db.register_table(team::TEAM_PASS);
        // Add more tables here as the app grows.
    });
    log::info!("all application tables registered");
}
