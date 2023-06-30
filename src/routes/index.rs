use axum::{extract::State, response::IntoResponse};
use chrono::Utc;
use serde::Serialize;

use crate::{
    auth::{get_auth_object, Auth},
    error::KnotError,
    liquid_utils::{compile, EnvFormatter},
    routes::DbEvent,
    state::KnotState,
};

#[allow(clippy::too_many_lines)]
#[instrument(level = "debug", skip(auth, state))]
pub async fn get_index(
    auth: Auth,
    State(state): State<KnotState>,
) -> Result<impl IntoResponse, KnotError> {
    #[derive(Serialize, Debug)]
    struct HTMLEvent {
        pub id: i32,
        pub event_name: String,
        pub date: String,
        pub location: String,
        pub teacher: String,
        pub other_info: String,
    }

    impl From<DbEvent> for HTMLEvent {
        fn from(
            DbEvent {
                id,
                event_name,
                date,
                location,
                teacher,
                other_info,
                zip_file: _,
            }: DbEvent,
        ) -> Self {
            Self {
                id,
                event_name,
                date: date.to_env_string(),
                location,
                teacher,
                other_info: other_info.unwrap_or_default(),
            }
        }
    }

    #[derive(Serialize)]
    struct PersonForm {
        pub first_name: String,
        pub surname: String,
        pub form: String,
    }

    #[derive(Serialize, Debug)]
    struct WholeEvent {
        event: HTMLEvent,
        participants: usize,
        prefects: usize,
        no_photos: usize,
    }

    let mut happened_events = vec![];
    let mut events_to_happen = vec![];

    let now = Utc::now().naive_local();

    debug!("Getting all events");

    for event in sqlx::query_as!(
        DbEvent,
        r#"
SELECT *
FROM events
ORDER BY events.date DESC
LIMIT 25
        "#
    )
    .fetch_all(&mut state.get_connection().await?)
    .await?
    {
            //TODO: cache using HashMaps etc
            let date = event.date;
            let event = HTMLEvent::from(event);

            let event_id = event.id;
            let prefects = sqlx::query_as!(
                PersonForm,
                r#"
SELECT p.first_name, p.surname, p.form
FROM people p
INNER JOIN events e ON e.id = $1
INNER JOIN prefect_events pe ON p.id = pe.prefect_id and pe.event_id = $1
    "#,
                event_id
            )
            .fetch_all(&mut state.get_connection().await?)
            .await?
            .len();

            let participants = sqlx::query_as!(
                PersonForm,
                r#"
SELECT p.first_name, p.surname, p.form
FROM people p
INNER JOIN events e ON e.id = $1
INNER JOIN participant_events pe ON p.id = pe.participant_id and pe.event_id = $1 AND pe.is_verified = true
    "#,
                event_id
            )
            .fetch_all(&mut state.get_connection().await?)
            .await?
            .len();

            let photos = sqlx::query!("SELECT FROM photos WHERE event_id = $1", event_id)
                .fetch_all(&mut state.get_connection().await?)
                .await?
                .len();

            if date < now {
                trace!(?event, "Adding to old events");
                happened_events.push(WholeEvent {
                    event,
                    participants,
                    prefects,
                    no_photos: photos,
                });
            } else {
                trace!(?event, "Adding to new events");
                events_to_happen.push(WholeEvent {
                    event,
                    participants,
                    prefects,
                    no_photos: photos,
                });
            }
    }
    events_to_happen.reverse();

    debug!("Compiling");

    compile("www/index.liquid", liquid::object!({ "events_to_happen": events_to_happen, "happened_events": happened_events, "auth": get_auth_object(auth) })).await
}
