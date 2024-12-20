use crate::{
    auth::{
        backend::{Auth, VentAuthBackend},
        get_auth_object, PermissionsRole, PermissionsTarget,
    },
    error::{
        EncodeStep, EventField, MalformedCSVSnafu, ParseBoolSnafu,
        ParseTimeSnafu, PersonField, SqlxAction, SqlxSnafu, TryingToGetFromCSV, VentError,
        WhatToParse,
    },
    state::VentState,
};
use axum::{
    extract::{Multipart, State},
    response::{IntoResponse, Redirect},
    routing::{get, post},
    Router,
};
use axum_login::{login_required, permission_required};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use csv_async::{AsyncReaderBuilder};
use futures::stream::StreamExt;
use serde::Deserialize;
use snafu::{OptionExt, ResultExt};
use std::collections::HashMap;
use crate::routes::public::serve_bytes_with_mime;

#[axum::debug_handler]
pub async fn get_import_export_csv(
    auth: Auth,
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
    let aa = get_auth_object(auth).await?;

    state
        .compile(
            "www/csv.liquid",
            liquid::object!({ "auth": aa }),
            Some("Import/Export".to_string()),
        )
        .await
}

#[axum::debug_handler]
pub async fn post_import_people_from_csv(
    State(state): State<VentState>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, VentError> {
    debug!("Getting CSV file");
    let Some(field) = multipart.next_field().await? else {
        warn!("Missing import CSV file");
        return Ok(Redirect::to("/"));
    };

    debug!(name=?field.name(), "Getting text + creating reader");

    let text = field.text().await?;
    let mut csv_reader = AsyncReaderBuilder::new()
        .create_reader(text.as_bytes())
        .into_records();

    let existing_forms: HashMap<String, i32> = sqlx::query!("SELECT id, form FROM people")
        .fetch_all(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingPeople,
        })?
        .into_iter()
        .map(|r| (r.form, r.id))
        .collect();
    let existing_first_names: HashMap<String, i32> =
        sqlx::query!("SELECT id, first_name FROM people")
            .fetch_all(&mut *state.get_connection().await?)
            .await
            .context(SqlxSnafu {
                action: SqlxAction::FindingPeople,
            })?
            .into_iter()
            .map(|r| (r.first_name, r.id))
            .collect();
    let existing_surnames: HashMap<String, i32> = sqlx::query!("SELECT id, surname FROM people")
        .fetch_all(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingPeople,
        })?
        .into_iter()
        .map(|r| (r.surname, r.id))
        .collect();

    //possibility of baby data races here, but not too important

    while let Some(record) = csv_reader.next().await.transpose()? {
        debug!("Getting details from record");
        let first_name = record.get(0).context(MalformedCSVSnafu {
            was_trying_to_get: PersonField::FirstName,
        })?;
        let surname = record.get(1).context(MalformedCSVSnafu {
            was_trying_to_get: PersonField::Surname,
        })?;
        let form = record.get(2).context(MalformedCSVSnafu {
            was_trying_to_get: PersonField::Form,
        })?;
        let is_prefect: bool = record
            .get(3)
            .context(MalformedCSVSnafu {
                was_trying_to_get: PersonField::IsPrefect,
            })?
            .parse()
            .context(ParseBoolSnafu {
                trying_to_parse: WhatToParse::PartOfAPerson(PersonField::IsPrefect),
                how_got_in: EncodeStep::Decode,
            })?;
        let username = record.get(4).context(MalformedCSVSnafu {
            was_trying_to_get: PersonField::Username,
        })?;
        let was_first_entry: bool = record.get(5).and_then(|x| x.parse().ok()).unwrap_or(true);

        debug!("Checking if needs to be updated rather than created");

        let mut needs_to_update = None;

        if let Some(form_id) = existing_forms.get(form) {
            if let Some(fn_id) = existing_first_names.get(first_name) {
                if form_id == fn_id {
                    if let Some(sn_id) = existing_surnames.get(surname) {
                        if sn_id == form_id {
                            needs_to_update = Some(form_id);
                        }
                    }
                }
            }
        }

        let perms = if is_prefect {
            PermissionsRole::Prefect
        } else {
            PermissionsRole::Participant
        };

        if let Some(needs_to_update) = needs_to_update {
            debug!("Updating");
            sqlx::query!(
                "UPDATE people SET permissions = $1, username = $2 WHERE id = $3",
                perms as _,
                username,
                needs_to_update
            )
            .execute(&mut *state.get_connection().await?)
            .await
            .context(SqlxSnafu {
                action: SqlxAction::UpdatingPerson(username.to_string().into()),
            })?;
        } else {
            debug!("Creating");
            sqlx::query!(
                    r#"INSERT INTO public.people
            (first_name, surname, form, hashed_password, permissions, username, password_link_id, was_first_entry)
            VALUES($1, $2, $3, NULL, $4, $5, NULL, $6);
            "#,
                    first_name,
                    surname,
                    form,
                    perms as _,
                    username,
                    was_first_entry
                )
                .execute(&mut *state.get_connection().await?)
                .await.context(SqlxSnafu { action: SqlxAction::AddingPerson })?;
        }
    }

    Ok(Redirect::to("/"))
}

#[axum::debug_handler]
pub async fn post_import_events_from_csv(
    State(state): State<VentState>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, VentError> {
    debug!("Getting CSV");
    let Some(field) = multipart.next_field().await? else {
        warn!("Missing CSV for importing events");
        return Ok(Redirect::to("/"));
    };

    debug!(name=?field.name(), "Getting text");

    let text = field.text().await?;
    let mut csv_reader = AsyncReaderBuilder::new()
        .create_reader(text.as_bytes())
        .into_records();

    while let Some(record) = csv_reader.next().await.transpose()? {
        let location = record.get(4).context(MalformedCSVSnafu {
            was_trying_to_get: TryingToGetFromCSV::Event(EventField::Location),
        })?;
        let teacher = record.get(3).context(MalformedCSVSnafu {
            was_trying_to_get: TryingToGetFromCSV::Event(EventField::Teacher),
        })?;
        let date = {
            let str = record.get(0).context(MalformedCSVSnafu {
                was_trying_to_get: TryingToGetFromCSV::Event(EventField::Date),
            })?;
            NaiveDate::parse_from_str(str, "%d-%m-%Y").context(ParseTimeSnafu {
                original: str.to_string(),
                how_got_in: EncodeStep::Decode,
            })?
        };
        let name = record.get(1).context(MalformedCSVSnafu {
            was_trying_to_get: TryingToGetFromCSV::Event(EventField::Name),
        })?;
        let time = {
            let str = record.get(2).context(MalformedCSVSnafu {
                was_trying_to_get: TryingToGetFromCSV::Event(EventField::Time),
            })?;

            NaiveTime::parse_from_str(str, "%R").context(ParseTimeSnafu {
                original: str.to_string(),
                how_got_in: EncodeStep::Decode,
            })?
        };
        let date_time = NaiveDateTime::new(date, time);

        debug!(?name, ?date, ?location, "Creating new event");

        sqlx::query!(
            r#"
INSERT INTO events (event_name, date, location, teacher) 
VALUES ($1, $2, $3, $4)"#,
            name,
            date_time,
            location,
            teacher
        )
        .execute(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::AddingEvent,
        })?;
    }

    state.update_events()?;

    Ok(Redirect::to("/"))
}

#[axum::debug_handler]
pub async fn export_events_to_csv(
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
    let mut csv_writer = csv::Writer::from_writer(vec![]);
    csv_writer.write_record(&["date", "name", "time", "location", "teacher"]).unwrap();

    #[derive(Deserialize)]
    struct SmolEvent {
        pub event_name: String,
        pub date: NaiveDateTime,
        pub location: String,
        pub teacher: String,
    }

    for SmolEvent {
        event_name,
        date,
        location,
        teacher,
    } in sqlx::query_as!(
        SmolEvent,
        r#"SELECT event_name, date, location, teacher FROM events"#
    )
    .fetch_all(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::FindingAllEvents,
    })? {
        csv_writer.write_record(&[
            date.format("%d-%m-%Y").to_string(),
            event_name,
            date.format("%H:%M").to_string(),
            location,
            teacher,
        ]).unwrap();
    }
    
    let contents = csv_writer.into_inner().unwrap();
    serve_bytes_with_mime(contents, "text/csv").await
}
#[axum::debug_handler]
pub async fn export_people_to_csv(
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
    let mut csv_writer = csv::Writer::from_writer(vec![]);
    csv_writer.write_record(&[
        "first_name",
        "surname",
        "form",
        "is_prefect",
        "username",
        "was_first_entry",
    ]).unwrap();

    #[derive(Deserialize)]
    struct SmolPerson {
        pub first_name: String,
        pub surname: String,
        pub form: String,
        pub permissions: PermissionsRole,
        pub username: String,
        pub was_first_entry: bool,
    }

    for SmolPerson {
        first_name,
        surname,
        form,
        permissions,
        username,
        was_first_entry
    } in sqlx::query_as!(
        SmolPerson,
        r#"SELECT first_name, surname, form, permissions as "permissions: _", username, was_first_entry FROM people"#
    )
    .fetch_all(&mut *state.get_connection().await?)
    .await.context(SqlxSnafu { action: SqlxAction::FindingPeople })?
    {
        csv_writer.write_record(&[
            first_name,
            surname,
            form,
            (permissions >= PermissionsRole::Prefect).to_string(),
            username,
            was_first_entry.to_string()
        ]).unwrap();
    }

    let contents = csv_writer.into_inner().unwrap();
    serve_bytes_with_mime(contents, "text/csv").await
}

pub fn router() -> Router<VentState> {
    Router::new()
        .route("/import_people_from_csv", post(post_import_people_from_csv))
        .route("/import_events_from_csv", post(post_import_events_from_csv))
        .route_layer(permission_required!(
            VentAuthBackend,
            login_url = "/login",
            PermissionsTarget::ImportCSV
        ))
        .route("/csv", get(get_import_export_csv))
        .route("/csv_people", get(export_people_to_csv))
        .route("/csv_events", get(export_events_to_csv))
        .route_layer(login_required!(VentAuthBackend, login_url = "/login"))
}
