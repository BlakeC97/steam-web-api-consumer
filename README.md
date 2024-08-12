# steam-web-api-consumer
Personal tool to gather + store data from the Steam Web API.
The end goal is to insert the data into persistent storage (SQLite) for later usage.


# Running
This program expects a Steam API Key to be supplied in the `STEAM_API_KEY` environment variable.
If you don't have one, you can [fill out the form here to get one.](https://steamcommunity.com/dev/apikey)
If a key is not provided in an environment variable, you will be interactively prompted (your key will not be echoed to the screen).

Same as you would most other Rust programs, with `cargo`:
```shell
$ export STEAM_API_KEY=XXXXXXXXXXXXXXXX
$ cargo run
```

As of right now, nothing is stored yet, and it will print my friends list + their details (Display Name, URL, Steam ID) to `stdout`.