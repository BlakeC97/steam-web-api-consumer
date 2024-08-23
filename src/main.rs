mod steam_api;
mod sql;

use anyhow::Result;
use steam_api::SteamClient;
use crate::sql::DbConnection;

const MY_ID: &str = "76561197996714010";

fn main() -> Result<()> {
    let api_key = std::env::var("STEAM_API_KEY")
        .unwrap_or_else(|_| {
            rpassword::prompt_password("Enter your Steam API key: ")
                .expect("Couldn't read a Steam API key")
        });

    let client = SteamClient::new(&api_key);
    let mut friends = client.get_friend_list(MY_ID)?;
    let mut friend_details = client.get_player_summaries(&friends.iter().map(|f| f.steam_id).collect::<Vec<_>>())?;
    debug_assert_eq!(friends.len(), friend_details.len());

    let mut db = DbConnection::new_with_default_name()?;
    db.create_tables()?;
    db.update_player_summaries(&mut friends, &mut friend_details)?;
    drop(db);

    Ok(())
}
