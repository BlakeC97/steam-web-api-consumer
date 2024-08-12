use std::fmt::{Display, Formatter};
use std::num::ParseIntError;
use chrono::{
    prelude::*,
    serde::ts_seconds,
};
use itertools::Itertools;
use reqwest::{
    blocking::Client,
    Url,
};
use serde::Deserialize;

#[derive(Debug, thiserror::Error)]
pub enum SteamFailure {
    #[error("Error in HTTP request: {0}")]
    Request(#[from] reqwest::Error),
    #[error("Error deserializing request: {0}")]
    Deserialize(#[from] serde_json::Error),
}

#[derive(Copy, Clone, Debug, Deserialize)]
#[serde(try_from = "&str")]
// 64-bit Steam IDs are a packed data structure, but for laziness' sake we'll leave it as an unvalidated number.
// https://developer.valvesoftware.com/wiki/SteamID
pub struct SteamId(u64);

impl Display for SteamId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<&str> for SteamId {
    type Error = ParseIntError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(SteamId(value.parse()?))
    }
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Relationship {
    All,
    Friend,
}

// https://developer.valvesoftware.com/wiki/Steam_Web_API#GetFriendList_.28v0001.29
#[derive(Debug, Deserialize)]
pub struct Friend {
    #[serde(rename = "steamid")]
    pub steam_id: SteamId,
    pub relationship: Relationship,
    #[serde(with = "ts_seconds")]
    pub friend_since: DateTime<Utc>,
}

// There's a lot more than this available, but this is enough for our purposes.
// https://developer.valvesoftware.com/wiki/Steam_Web_API#GetPlayerSummaries_.28v0002.29
#[derive(Debug, Deserialize)]
pub struct PlayerSummary {
    #[serde(rename = "steamid")]
    steam_id: SteamId,
    #[serde(rename = "personaname")]
    persona_name: String,
    #[serde(rename = "profileurl")]
    profile_url: String,
}

pub struct SteamClient<'a> {
    api_key: &'a str,
    client: Client,
}

impl<'a> SteamClient<'a> {

    pub fn new(api_key: &'a str) -> Self {
        Self {
            api_key,
            // We know this can only be invalid if the programmer messes it up, so `expect` is fine
            client: Client::builder()
                .user_agent("steam-web-api-consumer/0.1 (cjblake97@gmail.com)")
                .build()
                .expect("User-Agent on client was invalid")
        }
    }

    pub fn get_friend_list(&self, steam_id: &str) -> Result<Vec<Friend>, SteamFailure> {
        // We only need the structs to unwrap the "outer" parts of the resulting JSON, put them here
        // to keep the top-level clear
        #[derive(Debug, Deserialize)]
        struct FriendsList {
            friends: Vec<Friend>,
        }

        #[derive(Debug, Deserialize)]
        struct Response {
            #[serde(rename = "friendslist")]
            friends_list: FriendsList,
        }

        let url = Url::parse_with_params(
            "https://api.steampowered.com/ISteamUser/GetFriendList/v0001",
            &[("key", self.api_key), ("steamid", steam_id)],
        ).expect("Given an invalid URL");
        let res: Response = serde_json::from_slice(self.client.get(url).send()?.bytes()?.as_ref())?;

        Ok(res.friends_list.friends)
    }

    pub fn get_player_summaries(&self, steam_ids: &[SteamId]) -> Result<Vec<PlayerSummary>, SteamFailure> {
        #[derive(Debug, Deserialize)]
        struct Players {
            players: Vec<PlayerSummary>,
        }

        #[derive(Debug, Deserialize)]
        struct Response {
            response: Players,
        }

        let mut ret = Vec::with_capacity(steam_ids.len());
        for chunk in steam_ids.chunks(100) {
            let url = Url::parse_with_params(
                "https://api.steampowered.com/ISteamUser/GetPlayerSummaries/v0002",
                &[("key", self.api_key), ("steamids", &chunk.iter().join(","))],
            ).expect("Given an invalid const URL");

            let mut res: Response = serde_json::from_slice(self.client.get(url).send()?.bytes()?.as_ref())?;
            ret.append(&mut res.response.players);
        }

        Ok(ret)
    }
}