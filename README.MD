# Knot

This is a project designed for managing events with 2 classes of people - those who can only attend, and those can both
attend and supervise - and these are refered to as participants and prefects respectively. Anyone with the IP/Domain can
view this server and edit the events, and it also publishes an endpoint to update an ICS subscription.

This was originally engineered as a replacement to a brittle spreadsheet that didn't track photos or participants -
participants were manually added to a separate spreadsheet, and photos were inconsistently named in a OneDrive folder.

![excel_ugh.png](excel_ugh.png)

## Setup/Installation

### Requirements

This project requires:

- `cargo` and the other rust build tools like a valid `C++` compiler
- `cargo-sqlx` installed via `cargo` - this is used for postgres migrations
- `devenv` or `docker compose` - this is used for the postgres server
- `libssl-dev` - this is used for the axum server
- `clang` - this is used for the spreadsheet export

### Environment

You need to have a `.env` file with the following variables set:

##### Mandatory

| Name             | Use                                                                                             | Example Contents                      |
|------------------|-------------------------------------------------------------------------------------------------|---------------------------------------|
| `KNOT_SERVER_IP` | This is the IP the server should serve to                                                       | `127.0.0.1:8080`                      |
| `DATABASE_URL`   | This is used for the postgres database                                                          | `postgres://user@127.0.0.1:1111/user` |
| `CFT_SITEKEY`    | This is the Sitekey for [Cloudflare Turnstile](https://developers.cloudflare.com/turnstile/)    | `0x4AAAAAAAAAAAAAAAAAAAAAAAAAAA`      |
| `CFT_SECRETKEY`  | This is the secret key for [Cloudflare Turnstile](https://developers.cloudflare.com/turnstile/) | `0x4AAAAAAAAAAAAAAAAAAAAAAAAAAB`      |

##### Optional

| Name                | Use                                                          | Example Contents                                                                                                                                                                           | Default               |
|---------------------|--------------------------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|-----------------------|
| `RUST_LOG`          | Logging                                                      | `ERROR`, `WARN`, `INFO`, `DEBUG`, or `TRACE`. For more examples, see [the docs](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#example-syntax). | No logging            |
| `CONFIG_LOCATION`   | The file path to your condiguration file for below.          | `./cfg.ron`                                                                                                                                                                                | `./config/local.toml` |
| `HONEYCOMB_API_KEY` | This is the [Honeycomb](https://honeycomb.io) destination    | `abcdefghijklmopqrstuvwxyz`                                                                                                                                                                | N/A                   |
| `SENTRY_DSN`        | This is the [Sentry](https://sentry.io/welcome/) destination | `https://abc.ingest.sentry.io/123`                                                                                                                                                         | N/A                   |

### Configuration

You should also have a configuration
file ([JSON](https://www.json.org/json-en.html), [TOML](https://toml.io/en/), [YAML](https://yaml.org/), [INI](https://en.wikipedia.org/wiki/INI_file), [RON](https://lib.rs/crates/ron), [JSON5](https://json5.org/)
all supported) that follows the following schema:

```
niche: {
    date_time_format: String,
    tech_support: Url,
}
brand: {
    instance_name: String,
    domain: String,
}
mail: {
    username: String,
    password: String,
    smtp: String,
    username_domain: String,
}
```

| Name                     | Use                                                                                                                               | Example Contents                                    |
|--------------------------|-----------------------------------------------------------------------------------------------------------------------------------|-----------------------------------------------------|
| `niche.date_time_format` | For formatting dates in the UI - [appropriate format specifiers](https://docs.rs/chrono/0.4.24/chrono/format/strftime/index.html) | `"%c"`                                              |
| `niche.tech_support`     | For where user should be directed with 500-class errors.                                                                          | `"https://github.com/yourname/yourfork/issues/new"` |
| `brand.instance_name`    | For the name of the application in the UI.`"House Events Manager"`                                                                | `House Events Manager`                              |
| `brand.domain`           | The domain of the website - used for absolute links                                                                               | `http://localhost`                                  |
| `mail.username`          | This is the username for the mail account that will send password set links.                                                      | `noreply.knot@gmail.com`                            |
| `mail.password`          | This is the password for the mail account.                                                                                        | `aaaaaaaaaaaaaaab`                                  |
| `mail.smtp`              | This is the SMTP server for the mail account.                                                                                     | `smtp.zoho.eu`                                      |
| `mail.username_domain`   | This is the domain that users are registered under to send emails to.                                                             | `gmail.com`                                         |

### Setup

I would highly reccomend a `tmux` based setup to run this without needing an active ssh session to run whilst away.

1) Setup the postgres server - `devenv up` or `docker compose up`.
2) Run the postgres migrations - `sqlx migrate run`
3) Run the binary - `cargo r --release` for production, or `cargo r` for development. In production to minimise
   downtime, I use `cargo b` rather than `cargo r` to quickly stop the old binary and launch into the new one without
   recompiling.
4) Enjoy!

### Reccomendations

I currently run this through [Caddy](https://caddyserver.com/) as a reverse proxy to get HTTPS certificates
via [Let's Encrypt!](https://letsencrypt.org/).
These are the contents of the [Caddyfile](https://caddyserver.com/docs/caddyfile) on the server that this runs on,
redacted appropriately. This just runs via the default systemd service which takes the config
from `/etc/caddy/Caddyfile`:

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

The project consists of an [axum](https://lib.rs/axum) app, which serves [liquid](https://shopify.github.io/liquid/)
templates.

## Contributing

I'm not sure why you would, but I'll happily review any PRs/Issues that arise, but beware that this is a side project so
I may be unable to look at them quickly.
