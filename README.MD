# Vent

Vent is a project designed to manage events, photos & rewarding participants for their participation.

This was originally engineered as a replacement to a brittle spreadsheet that didn't track photos or participants -
participants were manually added to a separate spreadsheet, and photos were inconsistently named in a OneDrive folder.

## Setup/Installation

### Environment

You need to have a `.env` file with the following variables set:

##### Mandatory

| Name             | Use                                                                                             | Example Contents                      |
|------------------|-------------------------------------------------------------------------------------------------|---------------------------------------|
| `DATABASE_URL`   | This is used for the postgres database                                                          | `postgres://user@127.0.0.1:1111/user` |
| `CFT_SITEKEY`    | This is the Sitekey for [Cloudflare Turnstile](https://developers.cloudflare.com/turnstile/)    | `0x4AAAAAAAAAAAAAAAAAAAAAAAAAAA`      |
| `CFT_SECRETKEY`  | This is the secret key for [Cloudflare Turnstile](https://developers.cloudflare.com/turnstile/) | `0x4AAAAAAAAAAAAAAAAAAAAAAAAAAB`      |

##### Optional

| Name                                  | Use                                                                                                           | Example Contents                                                                                                                                                                           | Default               |
|---------------------------------------|---------------------------------------------------------------------------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|-----------------------|
| `RUST_LOG`                            | Logging                                                                                                       | `ERROR`, `WARN`, `INFO`, `DEBUG`, or `TRACE`. For more examples, see [the docs](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#example-syntax). | No logging            |
| `HIDE_BONUS_POINTS`                   | If your application doesn't need bonus points and this variable exists, the bonus points will not be present. | `[not set]`                                                                                                                                                                                |
| `DISABLE_DIFFERENT_AWARD_THRESHOLDS`  | If your application doesn't need two reward thresholds, set this variable.| "well, um actually, we use the *other* system"                                                                                                                                             |                       |
| `VENT_SERVER_IP`						| If you'd like to specify a different IP than `0.0.0.0:8080` for the server to run on.                         | `127.0.0.1:8080` |
### Configuration

You should also have a TOML configuration file that follows this schema:

```
niche: {
    date_time_format: String,
    tech_support: Url,
},
brand: {
    instance_name: String,
    domain: String,
},
mail: {
    username: String,
    password: String,
    smtp: String,
    username_domain: String,
},
timezone_id: String,
tech_support_person: String
```

| Name                     | Use                                                                                                                               | Example Contents                                    |
|--------------------------|-----------------------------------------------------------------------------------------------------------------------------------|-----------------------------------------------------|
| `niche.date_time_format` | For formatting dates in the UI - [appropriate format specifiers](https://docs.rs/chrono/0.4.24/chrono/format/strftime/index.html) | `"%c"`                                              |
| `niche.tech_support`     | For where user should be directed with 500-class errors.                                                                          | `"https://github.com/yourname/yourfork/issues/new"` |
| `brand.instance_name`    | For the name of the application in the UI.`"House Events Manager"`                                                                | `House Events Manager`                              |
| `brand.domain`           | The domain of the website - used for absolute links                                                                               | `http://localhost`                                  |
| `mail.username`          | This is the username for the mail account that will send password set links.                                                      | `noreply.vent@gmail.com`                            |
| `mail.password`          | This is the password for the mail account.                                                                                        | `aaaaaaaaaaaaaaab`                                  |
| `mail.smtp`              | This is the SMTP server for the mail account.                                                                                     | `smtp.zoho.eu`                                      |
| `mail.username_domain`   | This is the domain that users are registered under to send emails to.                                                             | `gmail.com`                                         |
| `timezone_id`            | This is the [TZID](https://www.unicode.org/cldr/charts/43/supplemental/zone_tzid.html) for the calendar events.                   | `Europe/London`                                     |
| `tech_support_person`    | This is the name of the person to contact for password resets and will be listed on the login page.                               | `Alice Bartholomew (Alice.B@domain.com)`            |

### Setup

Previously, this project had to be manually compiled, but it now has a Docker image! 

1) You need to setup a Docker Compose environment (example file below) that creates a vent instance and a postgres instance at a minimum.
2) You need to create directories for uploads, and create marketing materials and put them all into a folder. Ensure docker has adequate permissions for them.
3) Create a config & environment file.
4) Get going!

Example docker-compose file:
```yaml
services:
  db:
    image: postgres
    restart: always
    secrets:
      - db-password
    environment:
      - POSTGRES_DB=database
      - POSTGRES_PASSWORD_FILE=/run/secrets/db-password
    ports:
      - "5432:5432"
    expose:
      - 5432
    volumes:
      - pgdata:/var/lib/postgresql/data

  vent:
    image: ghcr.io/burntnail/vent:testing
    restart: always
    ports:
      - "8080:8080"
      - "587:587"
      - "465:465"
    volumes:
      - ./uploads:/uploads
      - ./public:/public
    configs:
      - source: my-config
        target: ./config.toml
    env_file:
      - ./.env
    depends_on:
      - db
volumes:
  pgdata:

secrets:
  db-password:
    file: db_password.txt

configs:
  my-config:
    file: config/local.toml
```

Then, use whatever load balancer or reverse proxy takes your fancy - I've had success with both [Caddy](https://caddyserver.com/docs/) and [Traefik](https://doc.traefik.io/traefik/). There is no stored state outside of the postgres database and the files and efforts have been made not to hold file handles for extended periods of time so feel free to use replicated instances for high availability. This is also compatible with [watchtower](https://github.com/containrrr/watchtower) - production environments should use the *latest* tag.

## Architecture

The project consists of an [axum](https://lib.rs/axum) app, which serves [liquid](https://shopify.github.io/liquid/)
templates all based off data from a [postgres](https://www.postgresql.org/) database accessed via [sqlx](https://lib.rs/crates/sqlx).

## Contributing

I'm not sure why you would, but I'll happily review any PRs/Issues that arise, but beware that this is a side project so I may be unable to look at them quickly.
