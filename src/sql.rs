use std::path::Path;
use chrono::{DateTime, Utc};
use rusqlite::Connection;
use crate::steam_api::{PlayerSummary, SteamId};

const DB_NAME: &str = "steam.db";

pub struct DbConnection {
    conn: Connection,
}

struct Row {
    steam_id: SteamId,
    persona_name: String,
    profile_url: String,
    created_at: DateTime<Utc>,
    last_checked_at: DateTime<Utc>,
    removed_at: Option<DateTime<Utc>>,
}

impl DbConnection {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, rusqlite::Error> {
        Ok(Self {
            conn: Connection::open(path)?,
        })
    }

    /// Creates a Sqlite DB with the name `steam.db` in the current directory.
    pub fn new_with_default_name() -> Result<Self, rusqlite::Error> {
        Ok(Self {
            conn: Connection::open(DB_NAME)?,
        })
    }

    pub fn create_table(&self) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS player_summaries (
            steam_id INT8 PRIMARY KEY NOT NULL,
            persona_name TEXT NOT NULL,
            profile_url TEXT NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
            last_checked_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
            removed_at TIMESTAMP
        )",
            ()
        )?;

        Ok(())
    }

    /// Does the following steps, in order:
    /// 1) Updates `removed_at` for anyone not in `summaries`
    /// 2) Upserts the new players in `summaries`, updating `last_checked_at` to whenever this program is run.
    pub fn update_player_summaries(&mut self, summaries: &[PlayerSummary]) -> Result<(), rusqlite::Error> {
        let curr_player_ids = summaries.iter().map(|s| s.steam_id).collect::<Vec<_>>();

        let substitution_string = {
            let mut s = "?,".repeat(summaries.len());
            // Get rid of the trailing comma
            s.pop();
            s
        };
        let update = format!(
            "UPDATE
                player_summaries
            SET
                last_checked_at = CURRENT_TIMESTAMP,
                removed_at = CURRENT_TIMESTAMP
            WHERE
                removed_at IS NULL
                AND steam_id NOT IN ({})
        ",
            substitution_string
        );
        self.conn.execute(&update, rusqlite::params_from_iter(curr_player_ids))?;

        let txn = self.conn.transaction()?;
        {
            let mut stmt = txn.prepare(
                "INSERT INTO player_summaries
                    (steam_id, persona_name, profile_url)
                VALUES
                    (?, ?, ?)
                ON CONFLICT (steam_id) DO UPDATE SET last_checked_at = CURRENT_TIMESTAMP
                "
            )?;
            for summary in summaries {
                stmt.execute((&summary.steam_id, &summary.persona_name, &summary.profile_url))?;
            }
        }
        txn.commit()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::thread::sleep;
    use std::time::Duration;
    use chrono::{DateTime, Utc};
    use crate::steam_api::SteamId;
    use super::*;

    #[test]
    fn test_update_player_summaries_deletion() {
        let mut db = DbConnection::new(":memory:").unwrap();
        db.create_table().unwrap();

        db.conn.execute(
            "INSERT INTO player_summaries
                    (steam_id, persona_name, profile_url)
                VALUES
                    (1, 'one', 'one_url'),
                    (2, 'two', 'two_url')
                ",
            ()
        ).unwrap();

        // Player 2 got deleted :(
        let players = [PlayerSummary {
            steam_id: SteamId(1),
            persona_name: "one".to_string(),
            profile_url: "one_url".to_string(),
        }];
        db.update_player_summaries(&players).unwrap();

        let deleted_person = db.conn.query_row(
            "SELECT steam_id, persona_name, profile_url FROM player_summaries WHERE removed_at IS NOT NULL",
            [],
            |row| {
                Ok(PlayerSummary {
                    steam_id: SteamId(row.get(0).unwrap()),
                    persona_name: row.get(1).unwrap(),
                    profile_url: row.get(2).unwrap(),
                })
            }
        ).unwrap();

        assert_eq!(
            PlayerSummary {
                steam_id: SteamId(2),
                persona_name: "two".to_string(),
                profile_url: "two_url".to_string(),
            },
            deleted_person
        );
    }

    #[test]
    fn test_update_player_summaries_deletion_idempotent() {
        let mut db = DbConnection::new(":memory:").unwrap();
        db.create_table().unwrap();

        db.conn.execute(
            "INSERT INTO player_summaries
                    (steam_id, persona_name, profile_url)
                VALUES
                    (1, 'one', 'one_url'),
                    (2, 'two', 'two_url')
                ",
            ()
        ).unwrap();

        let players = [PlayerSummary {
            steam_id: SteamId(1),
            persona_name: "one".to_string(),
            profile_url: "one_url".to_string(),
        }];
        db.update_player_summaries(&players).unwrap();

        let orig_removed_at = db.conn.query_row(
            "SELECT removed_at FROM player_summaries WHERE removed_at IS NOT NULL",
            [],
            |row| {
                let d: Option<DateTime<Utc>> = row.get(0).unwrap();
                Ok(d.unwrap())
            }
        ).unwrap();

        sleep(Duration::from_secs(1));
        db.update_player_summaries(&players).unwrap();

        let new_removed_at = db.conn.query_row(
            "SELECT removed_at FROM player_summaries WHERE removed_at IS NOT NULL",
            [],
            |row| {
                let d: Option<DateTime<Utc>> = row.get(0).unwrap();
                Ok(d.unwrap())
            }
        ).unwrap();

        assert_eq!(orig_removed_at, new_removed_at);
    }

    #[test]
    fn test_update_player_summaries_from_empty() {
        let mut db = DbConnection::new(":memory:").unwrap();
        db.create_table().unwrap();

        let players = [
            PlayerSummary {
                steam_id: SteamId(1),
                persona_name: "one".to_string(),
                profile_url: "one_url".to_string(),
            },
            PlayerSummary {
                steam_id: SteamId(2),
                persona_name: "two".to_string(),
                profile_url: "two_url".to_string(),
            },
        ];
        db.update_player_summaries(&players).unwrap();

        // TODO: Don't just check the count
        db.conn.query_row(
            "SELECT count(*) FROM player_summaries WHERE removed_at IS NULL",
            [],
            |row| {
                let n: i64 = row.get(0).unwrap();
                assert_eq!(2, n);
                Ok(())
            }
        ).unwrap();
    }
}