//! Team table: declaration + methods.

use shodh_redb::TableDefinition;
use db_wrapper::DBWrapper;

/// Team row: (name, slug, alt_names, links, is_avif).
pub type TeamRow = (String, String, Vec<String>, Vec<(u8, String)>, bool);

/// Team data: id → TeamRow.
pub const TEAM_PASS: TableDefinition<u64, TeamRow> =
    TableDefinition::new("team_data");

/// Extension methods for team-related DB operations.
pub trait TeamDb {
    fn save_title(
        &self,
        team_name: &str,
        slug: &str,
        alt_names: Vec<String>,
        links: Vec<(u8, String)>,
        is_avif: bool,
    ) -> Result<u64, db_wrapper::DbError>;
}

impl TeamDb for DBWrapper {
    fn save_title(
        &self,
        team_name: &str,
        slug: &str,
        alt_names: Vec<String>,
        links: Vec<(u8, String)>,
        is_avif: bool,
    ) -> Result<u64, db_wrapper::DbError> {
        let id = self.next(0);
        let row: TeamRow = (
            team_name.to_owned(),
            slug.to_owned(),
            alt_names,
            links,
            is_avif,
        );
        self.write(TEAM_PASS, id, row)?;
        Ok(id)
    }
}
