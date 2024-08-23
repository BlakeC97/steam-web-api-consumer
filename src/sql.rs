use rusqlite::Connection;
use crate::steam_api::{Friend, PlayerSummary};

const DB_NAME: &str = "steam.db";

pub struct DbConnection {
    conn: Connection,
}

impl DbConnection {
    /// Creates a Sqlite DB with the name `steam.db` in the current directory.
    pub fn new_with_default_name() -> Result<Self, rusqlite::Error> {
        Ok(Self {
            conn: Connection::open(DB_NAME)?,
        })
    }

    pub fn create_tables(&self) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS player_summaries (
            steam_id INT8 PRIMARY KEY NOT NULL,
            persona_name TEXT NOT NULL,
            profile_url TEXT NOT NULL,
            friend_since TIMESTAMP NOT NULL,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
            removed_at TIMESTAMP
        )",
            ()
        )?;

        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS name_history (
                steam_id INT8 NOT NULL,
                persona_name TEXT NOT NULL,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
                PRIMARY KEY (steam_id, persona_name)
            )",
            ()
        )?;

        Ok(())
    }

    /// Does the following steps, in order:
    ///     1) Updates `removed_at` for anyone not in `summaries`
    ///     2) Upserts the new players in `summaries`, updating `updated_at` to whenever this program is run.
    /// NOTE: This function will sort `friends` and `summaries`.
    pub fn update_player_summaries(&mut self, friends: &mut [Friend], summaries: &mut [PlayerSummary]) -> Result<(), rusqlite::Error> {
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
                updated_at = CURRENT_TIMESTAMP,
                removed_at = CURRENT_TIMESTAMP
            WHERE
                removed_at IS NULL
                AND steam_id NOT IN ({})
        ",
            substitution_string
        );
        self.conn.execute(&update, rusqlite::params_from_iter(curr_player_ids))?;

        friends.sort_unstable_by(|f1, f2| f1.steam_id.cmp(&f2.steam_id));
        summaries.sort_unstable_by(|s1, s2| s1.steam_id.cmp(&s2.steam_id));
        let txn = self.conn.transaction()?;
        {
            let mut summary_stmt = txn.prepare(
                "INSERT INTO player_summaries
                    (steam_id, persona_name, profile_url, friend_since)
                VALUES
                    (?, ?, ?, ?)
                ON CONFLICT (steam_id) DO
                    UPDATE SET persona_name = ?, profile_url = ?, updated_at = CURRENT_TIMESTAMP
                "
            )?;
            let mut nickname_stmt = txn.prepare(
                "INSERT INTO name_history
                    (steam_id, persona_name)
                VALUES
                    (?, ?)
                ON CONFLICT (steam_id, persona_name) DO UPDATE SET updated_at = CURRENT_TIMESTAMP
                "
            )?;

            for (friend, summary) in std::iter::zip(friends, summaries) {
                summary_stmt.execute((
                    &summary.steam_id,
                    &summary.persona_name,
                    &summary.profile_url,
                    &friend.friend_since,
                    &summary.persona_name,
                    &summary.profile_url,
                ))?;
                nickname_stmt.execute((&summary.steam_id, &summary.persona_name))?;
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
    use crate::steam_api::{Friend, Relationship};
    use crate::steam_api::SteamId;
    use super::*;

    #[derive(Debug)]
    struct PlayerSummariesRow {
        steam_id: SteamId,
        persona_name: String,
        profile_url: String,
        friend_since: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        removed_at: Option<DateTime<Utc>>,
    }

    #[derive(Debug)]
    struct NameHistoryRow {
        steam_id: SteamId,
        persona_name: String,
        updated_at: DateTime<Utc>,
    }

    #[test]
    fn test_update_player_summaries_deletion() {
        let mut db = DbConnection::new(":memory:").unwrap();
        db.create_tables().unwrap();

        db.conn.execute(
            "INSERT INTO player_summaries
                (steam_id, persona_name, profile_url, friend_since)
            VALUES
                (1, 'one', 'one_url', CURRENT_TIMESTAMP),
                (2, 'two', 'two_url', CURRENT_TIMESTAMP)
            ",
            ()
        ).unwrap();

        // Player 2 got deleted :(
        let mut friends = [Friend {
            steam_id: SteamId(1),
            relationship: Relationship::Friend,
            friend_since: Utc::now(),
        }];
        let mut players = [PlayerSummary {
            steam_id: SteamId(1),
            persona_name: "one".to_string(),
            profile_url: "one_url".to_string(),
        }];
        db.update_player_summaries(&mut friends, &mut players).unwrap();

        let deleted_person = db.conn.query_row(
            "SELECT steam_id, persona_name, profile_url, friend_since, updated_at, removed_at FROM player_summaries WHERE removed_at IS NOT NULL",
            [],
            |row| {
                Ok(PlayerSummariesRow {
                    steam_id: row.get(0).unwrap(),
                    persona_name: row.get(1).unwrap(),
                    profile_url: row.get(2).unwrap(),
                    friend_since: row.get(3).unwrap(),
                    updated_at: row.get(4).unwrap(),
                    removed_at: row.get(5).unwrap(),
                })
            }
        ).unwrap();

        assert_eq!(SteamId(2), deleted_person.steam_id);
        assert_eq!("two".to_string(), deleted_person.persona_name);
        assert_eq!("two_url".to_string(), deleted_person.profile_url);
    }

    #[test]
    fn test_update_player_summaries_deletion_idempotent() {
        let mut db = DbConnection::new(":memory:").unwrap();
        db.create_tables().unwrap();

        db.conn.execute(
            "INSERT INTO player_summaries
                (steam_id, persona_name, profile_url, friend_since)
            VALUES
                (1, 'one', 'one_url', CURRENT_TIMESTAMP),
                (2, 'two', 'two_url', CURRENT_TIMESTAMP)
            ",
            ()
        ).unwrap();

        let mut friends = [Friend {
            steam_id: SteamId(1),
            relationship: Relationship::Friend,
            friend_since: Utc::now(),
        }];
        let mut players = [PlayerSummary {
            steam_id: SteamId(1),
            persona_name: "one".to_string(),
            profile_url: "one_url".to_string(),
        }];
        db.update_player_summaries(&mut friends, &mut players).unwrap();

        let orig_removed_at = db.conn.query_row(
            "SELECT removed_at FROM player_summaries WHERE removed_at IS NOT NULL",
            [],
            |row| {
                let d: Option<DateTime<Utc>> = row.get(0).unwrap();
                Ok(d.unwrap())
            }
        ).unwrap();

        sleep(Duration::from_millis(10));
        db.update_player_summaries(&mut friends, &mut players).unwrap();

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
        db.create_tables().unwrap();

        let now = Utc::now();
        let mut friends = [
            Friend {
                steam_id: SteamId(1),
                relationship: Relationship::Friend,
                friend_since: now,
            },
            Friend {
                steam_id: SteamId(1),
                relationship: Relationship::Friend,
                friend_since: now,
            },
        ];
        let mut players = [
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
        db.update_player_summaries(&mut friends, &mut players).unwrap();

        let mut stmt = db.conn.prepare(
            "SELECT steam_id, persona_name, profile_url, friend_since, updated_at, removed_at FROM player_summaries ORDER BY 1"
        ).unwrap();
        let rows = stmt.query_map(
            [],
            |row| {
                Ok(PlayerSummariesRow {
                    steam_id: row.get(0).unwrap(),
                    persona_name: row.get(1).unwrap(),
                    profile_url: row.get(2).unwrap(),
                    friend_since: row.get(3).unwrap(),
                    updated_at: row.get(4).unwrap(),
                    removed_at: row.get(5).unwrap(),
                })
            }
        ).unwrap().filter_map(|e| e.ok()).collect::<Vec<_>>();

        assert_eq!(players[0].steam_id, rows[0].steam_id);
        assert_eq!(players[0].persona_name, rows[0].persona_name);
        assert_eq!(players[0].profile_url, rows[0].profile_url);
        assert_eq!(friends[0].friend_since, rows[0].friend_since);
        assert_eq!(None, rows[0].removed_at);

        assert_eq!(players[1].steam_id, rows[1].steam_id);
        assert_eq!(players[1].persona_name, rows[1].persona_name);
        assert_eq!(players[1].profile_url, rows[1].profile_url);
        assert_eq!(friends[1].friend_since, rows[1].friend_since);
        assert_eq!(None, rows[1].removed_at);
    }

    #[test]
    fn test_update_player_summaries_update_name() {
        let mut db = DbConnection::new(":memory:").unwrap();
        db.create_tables().unwrap();

        let mut friends = [
            Friend {
                steam_id: SteamId(1),
                relationship: Relationship::Friend,
                friend_since: Utc::now(),
            },
        ];
        let mut players = [
            PlayerSummary {
                steam_id: SteamId(1),
                persona_name: "one".to_string(),
                profile_url: "one_url".to_string(),
            },
        ];
        db.update_player_summaries(&mut friends, &mut players).unwrap();
        let (first_name, first_url): (String, String) = db.conn.query_row(
            "SELECT persona_name, profile_url FROM player_summaries",
            (),
            |row| Ok((row.get(0).unwrap(), row.get(1).unwrap()))
        ).unwrap();
        assert_eq!("one".to_string(), first_name);
        assert_eq!("one_url".to_string(), first_url);

        players[0].persona_name = "one_updated".to_string();
        players[0].profile_url = "one_url_updated".to_string();
        db.update_player_summaries(&mut friends, &mut players).unwrap();
        let (second_name, second_url): (String, String) = db.conn.query_row(
            "SELECT persona_name, profile_url FROM player_summaries",
            (),
            |row| Ok((row.get(0).unwrap(), row.get(1).unwrap()))
        ).unwrap();
        assert_eq!("one_updated".to_string(), second_name);
        assert_eq!("one_url_updated".to_string(), second_url);

        let mut stmt = db.conn.prepare(
            "SELECT steam_id, persona_name, updated_at FROM name_history ORDER BY updated_at"
        ).unwrap();
        let rows = stmt.query_map(
            [],
            |row| {
                Ok(NameHistoryRow {
                    steam_id: row.get(0).unwrap(),
                    persona_name: row.get(1).unwrap(),
                    updated_at: row.get(2).unwrap(),
                })
            }
        ).unwrap().filter_map(|e| e.ok()).collect::<Vec<_>>();

        assert_eq!(players[0].steam_id, rows[0].steam_id);
        assert_eq!("one".to_string(), rows[0].persona_name);

        assert_eq!(players[0].steam_id, rows[1].steam_id);
        assert_eq!("one_updated".to_string(), rows[1].persona_name);
    }
}