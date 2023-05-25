# Knot
This is a project designed for managing events with 2 classes of people - those who can only attend, and those can both attend and supervise - and these are refered to as participants and prefects respectively. Anyone with the IP/Domain can view this server and edit the events, and it also publishes an endpoint to update an ICS subscription.

This was originally engineered as a replacement to a brittle spreadsheet that didn't track photos or participants - participants were manually added to a separate spreadsheet, and photos were inconsistently named in a OneDrive folder.

![excel_ugh.png](excel_ugh.png)

## Setup/Installation

### Requirements
This project requires:
 - `cargo` and the other rust build tools like a valid `C++` compiler
 - `cargo-sqlx` installed via `cargo` - this is used for postgres migrations
 - `devenv` - this is used for the postgres server
 - `libssl-dev` - this is used for the axum server

### Environment
You need to have a `.env` file with the following variables set:
 - `DATABASE_URL` - used for the postgres database (eg. `postgres://user@127.0.0.1:1111/user`)
 - `KNOT_SERVER_IP` - the IP the server should serve to (eg. `127.0.0.1:8080`)
 - You can also optionally have a `RUST_LOG` variable set to `ERROR`, `WARN`, `INFO`, `DEBUG`, `TRACE` each of which shows all of the messages from the levels above (eg. `INFO` shows `ERROR` messages)
 - You can also also optionally specify a `DATE_TIME_FORMAT` variable filled out with [appropriate format specifiers](https://docs.rs/chrono/0.4.24/chrono/format/strftime/index.html).

### Setup
I would highly reccomend a `tmux` based setup to run this without needing an active ssh session to run whilst away.
1) Setup the postgres server - `devenv up`
2) Run the postgres migrations - `sqlx migrate run`
3) Run the binary - `cargo r --release` for production, or `cargo r` for development. In production to minimise downtime, I use `cargo b` rather than `cargo r` to quickly stop the old binary and launch into the new one without recompiling. 
4) Enjoy!

### Reccomendations
I currently run this through [Caddy](https://caddyserver.com/) as a reverse proxy to get HTTPS certificates via [Let's Encrypt!](https://letsencrypt.org/).
These are the contents of the [Caddyfile](https://caddyserver.com/docs/caddyfile) on the server that this runs on, redacted appropriately. This just runs via the default systemd service which takes the config from `/etc/caddy/Caddyfile`:
```
{
    email EMAIL_ADDRESS #eg. a@b.com
}

# Refer to the Caddy docs for more information:
# https://caddyserver.com/docs/caddyfile

FULL_DOMAIN { #eg. mail.google.com
    # Another common task is to set up a reverse proxy:
    reverse_proxy 127.0.0.1:8080 # make sure this is set to a localhost address, and NOT your server's external IP
}
```


## Architecture

The project consists of an [axum](https://lib.rs/axum) app, which serves [liquid](https://shopify.github.io/liquid/) templates.

## Contributing

I'm not sure why you would, but I'll happily review any PRs/Issues that arise, but beware that this is a side project so I may be unable to look at them quickly.
