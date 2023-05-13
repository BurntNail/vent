# Knot
This is a project designed for managing events with 2 classes of people - those who can only attend, and those can both attend and supervise - and these are refered to as participants and prefects respectively. Anyone with the IP/Domain can view this server and edit the events, and it also publishes an endpoint to update an ICS subscription.

## Setup/Installation

### Requirements
This project requires:
 - `cargo` and the other rust build tools like a valid `C++` compiler
 - `cargo-sqlx` installed via `cargo`
 - `devenv` - this is used for the server
 - `libssl-dev` - this is used for the server

### Environment
You need to have a `.env` file with the following variables set:
 - `DATABASE_URL` - used for the postgres database (eg. `postgres://user@127.0.0.1:1111/user`)
 - `KNOT_SERVER_IP` - the IP the server should serve to (eg. `127.0.0.1:8080`)
 - You can also optionally have a `RUST_LOG` variable set to `ERROR`, `WARN`, `INFO`, `DEBUG`, `TRACE` each of which shows all of the messages from the levels above (eg. `INFO` shows `ERROR` messages)

### Setup
I would highly reccomend a `tmux` based setup to run this without needing an active session.
1) Setup the postgres server - `devenv up`
2) Run the postgres migrations - `sqlx migrate run`
3) Run the binary - `cargo r --release` for production, or `cargo r` for development. In production to minimise downtime, I use `cargo b` rather than `cargo r` to quickly stop the old binary and launch into the new one without recompiling
4) Enjoy!


## Architecture

The project consists of an [axum](https://lib.rs/axum) app, which serves [liquid](https://shopify.github.io/liquid/) templates.

## Contributing

I'm not sure why you would, but I'll happily review any PRs/Issues that arise, but beware that this is a side project so I may be unable to look at them quickly.