use crate::{
    auth::{get_auth_object, Auth, PermissionsRole},
    error::KnotError,
    liquid_utils::compile,
    routes::public::serve_static_file,
    state::KnotState,
};
use axum::{
    extract::{Multipart, State},
    response::{IntoResponse, Redirect},
};
use chrono::NaiveDateTime;
use csv_async::{AsyncReaderBuilder, AsyncWriterBuilder};
use futures::stream::StreamExt;
use serde::Deserialize;
use std::collections::HashMap;
use tokio::fs::File;

pub async fn get_import_export_csv(auth: Auth) -> Result<impl IntoResponse, KnotError> {
    compile(
        "www/csv.liquid",
        liquid::object!({ "auth": get_auth_object(auth) }),
    )
    .await
}

pub async fn post_import_people_from_csv(
    State(state): State<KnotState>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, KnotError> {
    let Some(field) = multipart.next_field().await? else {
        return Ok(Redirect::to("/"))
    };

    let text = field.text().await?;
    let mut csv_reader = AsyncReaderBuilder::new()
        .create_reader(text.as_bytes())
        .into_records();

    let existing_forms: HashMap<String, i32> = sqlx::query!("SELECT id, form FROM people")
        .fetch_all(&mut state.get_connection().await?)
        .await?
        .into_iter()
        .map(|r| (r.form, r.id))
        .collect();
    let existing_first_names: HashMap<String, i32> = sqlx::query!("SELECT id, surname FROM people")
        .fetch_all(&mut state.get_connection().await?)
        .await?
        .into_iter()
        .map(|r| (r.surname, r.id))
        .collect();
    let existing_surnames: HashMap<String, i32> = sqlx::query!("SELECT id, first_name FROM people")
        .fetch_all(&mut state.get_connection().await?)
        .await?
        .into_iter()
        .map(|r| (r.first_name, r.id))
        .collect();

    //possibility of baby data races here, but not too important

    while let Some(record) = csv_reader.next().await.transpose()? {
        let first_name = record.get(0).ok_or(KnotError::MalformedCSV)?;
        let surname = record.get(1).ok_or(KnotError::MalformedCSV)?;
        let form = record.get(2).ok_or(KnotError::MalformedCSV)?;
        let is_prefect: bool = record.get(3).ok_or(KnotError::MalformedCSV)?.parse()?;
        let username = record.get(4).ok_or(KnotError::MalformedCSV)?;

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
            sqlx::query!(
                "UPDATE people SET permissions = $1, username = $2 WHERE id = $3",
                perms as _,
                username,
                needs_to_update
            )
            .execute(&mut state.get_connection().await?)
            .await?;
        } else {
            sqlx::query!(
                r#"INSERT INTO public.people
            (first_name, surname, form, hashed_password, permissions, username, password_link_id)
            VALUES($1, $2, $3, NULL, $4, $5, NULL);
            "#,
                first_name,
                surname,
                form,
                perms as _,
                username
            )
            .execute(&mut state.get_connection().await?)
            .await?;
        }
    }

    Ok(Redirect::to("/"))
}

pub async fn post_import_events_from_csv(
    State(state): State<KnotState>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, KnotError> {
    let Some(field) = multipart.next_field().await? else {
        return Ok(Redirect::to("/"))
    };

    let text = field.text().await?;
    let mut csv_reader = AsyncReaderBuilder::new()
        .create_reader(text.as_bytes())
        .into_records();

    while let Some(record) = csv_reader.next().await.transpose()? {
        let name = record.get(0).ok_or(KnotError::MalformedCSV)?;
        let date_time = NaiveDateTime::parse_from_str(
            record.get(1).ok_or(KnotError::MalformedCSV)?,
            "%Y-%m-%dT%H:%M",
        )?;
        let location = record.get(2).ok_or(KnotError::MalformedCSV)?;
        let teacher = record.get(3).ok_or(KnotError::MalformedCSV)?;
        let other_info = record.get(4);

        sqlx::query!(
            r#"
INSERT INTO events (event_name, date, location, teacher, other_info) 
VALUES ($1, $2, $3, $4, $5)"#,
            name,
            date_time,
            location,
            teacher,
            other_info
        )
        .execute(&mut state.get_connection().await?)
        .await?;
    }

    Ok(Redirect::to("/"))
}

pub async fn export_events_to_csv(
    State(state): State<KnotState>,
) -> Result<impl IntoResponse, KnotError> {
    let mut asw = AsyncWriterBuilder::new().create_writer(File::create("public/events.csv").await?);
    asw.write_record(&["name", "date_time", "location", "teacher", "other_info"])
        .await?;

    #[derive(Deserialize)]
    struct SmolEvent {
        pub event_name: String,
        pub date: NaiveDateTime,
        pub location: String,
        pub teacher: String,
        pub other_info: Option<String>,
    }

    for SmolEvent {
        event_name,
        date,
        location,
        teacher,
        other_info,
    } in sqlx::query_as!(
        SmolEvent,
        r#"SELECT event_name, date, location, teacher, other_info FROM events"#
    )
    .fetch_all(&mut state.get_connection().await?)
    .await?
    {
        asw.write_record(&[
            event_name,
            date.format("%Y-%m-%dT%H:%M").to_string(),
            location,
            teacher,
            other_info.unwrap_or_default(),
        ])
        .await?;
    }

    asw.flush().await?; //flush here to ensure we get the errors
    drop(asw);

    serve_static_file("public/events.csv").await
}
pub async fn export_people_to_csv(
    State(state): State<KnotState>,
) -> Result<impl IntoResponse, KnotError> {
    let mut asw = AsyncWriterBuilder::new().create_writer(File::create("public/people.csv").await?);
    asw.write_record(&["first_name", "surname", "form", "is_prefect", "username"])
        .await?;

    #[derive(Deserialize)]
    struct SmolPerson {
        pub first_name: String,
        pub surname: String,
        pub form: String,
        pub permissions: PermissionsRole,
        pub username: String,
    }

    for SmolPerson {
        first_name,
        surname,
        form,
        permissions,
        username,
    } in sqlx::query_as!(
        SmolPerson,
        r#"SELECT first_name, surname, form, permissions as "permissions: _", username FROM people"#
    )
    .fetch_all(&mut state.get_connection().await?)
    .await?
    {
        asw.write_record(&[
            first_name,
            surname,
            form,
            (permissions >= PermissionsRole::Prefect).to_string(),
            username,
        ])
        .await?;
    }

    asw.flush().await?; //flush here to ensure we get the errors
    drop(asw);

    serve_static_file("public/people.csv").await
}
