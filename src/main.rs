mod steam_api;

use anyhow::Result;
use steam_api::SteamClient;

const MY_ID: &'static str = "76561197996714010";

fn main() -> Result<()> {
    let api_key = std::env::var("STEAM_API_KEY")
        .unwrap_or_else(|_| {
            rpassword::prompt_password("Enter your Steam API key: ")
                .expect("Couldn't read a Steam API key")
        });

    let client = SteamClient::new(&api_key);
    let friends = client.get_friend_list(MY_ID)?;
    let friend_details = dbg!(client.get_player_summaries(&friends.iter().map(|f| f.steam_id).collect::<Vec<_>>()));

    Ok(())
}
