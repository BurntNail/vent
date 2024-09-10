use axum::{extract::State, response::IntoResponse};
use serde::Serialize;
use snafu::ResultExt;

use crate::{
    auth::{backend::Auth, get_auth_object},
    error::{SqlxAction, SqlxSnafu, VentError},
    liquid_utils::CustomFormat,
    state::{db_objects::DbEvent, VentState},
};

#[allow(clippy::too_many_lines)]
#[axum::debug_handler]
pub async fn get_index(
    auth: Auth,
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
    #[derive(Serialize, Debug)]
    struct HTMLEvent {
        pub id: i32,
        pub event_name: String,
        pub date: String,
    }

    impl<'a> From<(DbEvent, &'a str)> for HTMLEvent {
        fn from(
            (
                DbEvent {
                    id,
                    event_name,
                    date,
                    location: _,
                    teacher: _,
                    other_info: _,
                    zip_file: _,
                    is_locked: _,
                    extra_points: _,
                },
                fmt,
            ): (DbEvent, &'a str),
        ) -> Self {
            Self {
                id,
                event_name,
                date: date.to_env_string(fmt),
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

    debug!("Getting all events");

    for event in sqlx::query_as!(
        DbEvent,
        r#"
SELECT *
FROM events e
WHERE e.date > (now() - interval '12 hours')
ORDER BY e.date ASC
LIMIT 15
        "#
    )
    .fetch_all(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::FindingAllEvents,
    })? {
        let event = HTMLEvent::from((event, state.settings.niche.date_time_format.as_str()));

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
        .fetch_all(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingParticipantsOrPrefectsAtEvents {
                event_id: Some(event_id),
            },
        })?
        .len();

        let participants = sqlx::query_as!(
            PersonForm,
            r#"
SELECT p.first_name, p.surname, p.form
FROM people p
INNER JOIN events e ON e.id = $1
INNER JOIN participant_events pe ON p.id = pe.participant_id and pe.event_id = $1
    "#,
            event_id
        )
        .fetch_all(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingParticipantsOrPrefectsAtEvents {
                event_id: Some(event_id),
            },
        })?
        .len();

        let photos = sqlx::query!("SELECT FROM photos WHERE event_id = $1", event_id)
            .fetch_all(&mut *state.get_connection().await?)
            .await
            .context(SqlxSnafu {
                action: SqlxAction::FindingPhotos(event_id.into()),
            })?
            .len();

        events_to_happen.push(WholeEvent {
            event,
            participants,
            prefects,
            no_photos: photos,
        });
    }

    for event in sqlx::query_as!(
        DbEvent,
        r#"
SELECT *
FROM events e
WHERE e.date < (now() - interval '12 hours')
ORDER BY e.date DESC
LIMIT 10
        "#
    )
    .fetch_all(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::FindingAllEvents,
    })? {
        let event = HTMLEvent::from((event, state.settings.niche.date_time_format.as_str()));

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
        .fetch_all(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingParticipantsOrPrefectsAtEvents {
                event_id: Some(event_id),
            },
        })?
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
            .fetch_all(&mut *state.get_connection().await?)
            .await.context(SqlxSnafu { action: SqlxAction::FindingParticipantsOrPrefectsAtEvents {event_id: Some(event_id)} })?
            .len();

        let photos = sqlx::query!("SELECT FROM photos WHERE event_id = $1", event_id)
            .fetch_all(&mut *state.get_connection().await?)
            .await
            .context(SqlxSnafu {
                action: SqlxAction::FindingPhotos(event_id.into()),
            })?
            .len();

        happened_events.push(WholeEvent {
            event,
            participants,
            prefects,
            no_photos: photos,
        });
    }

    let aa = get_auth_object(auth).await?;

    state.compile("www/index.liquid", liquid::object!({ "events_to_happen": events_to_happen, "happened_events": happened_events, "auth": aa }), None).await
}
