# steam-web-api-consumer
Personal tool to gather + store data from the Steam Web API.
The end goal is to insert the data into persistent storage (SQLite) for later usage.


# Setup
This program expects a Steam API Key to be supplied in the `STEAM_API_KEY` environment variable.
If you don't have one, you can [fill out the form here to get one.](https://steamcommunity.com/dev/apikey)
If a key is not provided in an environment variable, you will be interactively prompted (your key will not be echoed to the screen).

# Running
Same as you would most other Rust programs, with `cargo run`:
```shell
$ export STEAM_API_KEY=XXXXXXXXXXXXXXXX
$ cargo run
```

This will create a SQLite DB, `steam.db` with the following tables + schemas.

`player_summaries`:
```sql
steam_id INT8 PRIMARY KEY NOT NULL,
persona_name TEXT NOT NULL,
profile_url TEXT NOT NULL,
friend_since TIMESTAMP NOT NULL,
updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
removed_at TIMESTAMP
```

`name_history`:
```sql
steam_id INT8 NOT NULL,
persona_name TEXT NOT NULL,
updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
PRIMARY KEY (steam_id, persona_name)
```