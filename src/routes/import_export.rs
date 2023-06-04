use crate::{
    auth::{get_auth_object, Auth, PermissionsRole},
    error::KnotError,
    liquid_utils::compile,
    state::KnotState, routes::public::serve_static_file,
};
use axum::{
    extract::State,
    response::{IntoResponse},
};
use chrono::NaiveDateTime;
use csv_async::AsyncWriterBuilder;
use serde::Deserialize;
use tokio::fs::File;

pub async fn get_import_export_csv(auth: Auth) -> Result<impl IntoResponse, KnotError> {
    compile(
        "www/csv.liquid",
        liquid::object!({ "auth": get_auth_object(auth) }),
    )
    .await
}

pub async fn export_events_to_csv(
    State(state): State<KnotState>,
) -> Result<impl IntoResponse, KnotError> {
    let mut asw = AsyncWriterBuilder::new().create_writer(File::create("public/events.csv").await?);
    asw.write_record(&["name", "date_time", "location", "teacher", "string"])
        .await?;

    #[derive(Deserialize)]
    struct SmolEvent {
        pub event_name: String,
        pub date: NaiveDateTime,
        pub location: String,
        pub teacher: String,
        pub other_info: Option<String>
    }

    for SmolEvent { event_name, date, location, teacher, other_info } in sqlx::query_as!(
        SmolEvent,
        r#"SELECT event_name, date, location, teacher, other_info FROM events"#
    )
    .fetch_all(&mut state.get_connection().await?)
    .await?
    {
        asw.write_record(&[event_name, date.format("%Y-%m-%dT%H:%M").to_string(), location, teacher, other_info.unwrap_or_default()]).await?;
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
        asw.write_record(&[first_name, surname, form, (permissions >= PermissionsRole::Prefect).to_string(), username]).await?;
    }

    asw.flush().await?; //flush here to ensure we get the errors
    drop(asw);

    serve_static_file("public/people.csv").await
}
