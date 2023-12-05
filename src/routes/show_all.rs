use crate::{
    auth::{
        backend::{Auth, KnotAuthBackend},
        get_auth_object, PermissionsTarget,
    },
    error::{KnotError, SqlxAction, SqlxSnafu},
    liquid_utils::{compile_with_newtitle, CustomFormat},
    state::KnotState,
};
use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
    routing::{get, post},
    Router,
};
use axum_extra::extract::Form;
use axum_login::permission_required;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use snafu::ResultExt;

#[derive(Deserialize)]
struct SmolDbEvent {
    pub id: i32,
    pub event_name: String,
    pub date: NaiveDateTime,
}
#[derive(Serialize)]
struct SmolFormattedDbEvent {
    pub id: i32,
    pub event_name: String,
    pub date: String,
}

impl<'a> From<(SmolDbEvent, &'a str)> for SmolFormattedDbEvent {
    fn from(
        (
            SmolDbEvent {
                id,
                event_name,
                date,
            },
            fmt,
        ): (SmolDbEvent, &'a str),
    ) -> Self {
        Self {
            id,
            event_name,
            date: date.to_env_string(fmt),
        }
    }
}

#[axum::debug_handler]
async fn get_show_all(
    auth: Auth,
    State(state): State<KnotState>,
) -> Result<impl IntoResponse, KnotError> {
    #[derive(Serialize)]
    pub struct SmolPerson {
        pub first_name: String,
        pub surname: String,
        pub form: String,
        pub id: i32,
        pub pts: usize,
    }

    debug!("Gettinng people");

    let mut people = sqlx::query!(
        r#"
SELECT first_name, surname, form, id
FROM people p
        "#
    )
    .fetch_all(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::FindingPeople,
    })?;
    people.sort_by_key(|x| x.surname.clone());
    people.sort_by_key(|x| x.form.clone());

    let mut new_people = vec![];
    for person in people {
        let pts = sqlx::query!("SELECT COUNT(participant_id) FROM participant_events WHERE participant_id = $1 AND is_verified = true", person.id).fetch_one(&mut *state.get_connection().await?).await.context(SqlxSnafu { action: SqlxAction::GettingRewardsReceived(Some(person.id.into())) })?.count.unwrap_or(0) as usize;
        new_people.push(SmolPerson {
            first_name: person.first_name,
            surname: person.surname,
            form: person.form,
            id: person.id,
            pts,
        });
    }

    trace!("Getting events");

    let events: Vec<SmolFormattedDbEvent> = sqlx::query_as!(
        SmolDbEvent,
        r#"
SELECT id, event_name, date
FROM events e
ORDER BY e.date DESC
        "#
    )
    .fetch_all(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::FindingAllEvents,
    })?
    .into_iter()
    .map(|event| {
        SmolFormattedDbEvent::from((event, state.settings.niche.date_time_format.as_str()))
    })
    .collect();

    trace!("Compiling");

    let aa = get_auth_object(auth).await?;

    compile_with_newtitle(
        "www/show_all.liquid",
        liquid::object!({ "people": new_people, "events": events, "auth": aa }),
        &state.settings.brand.instance_name,
        Some("All People/Events".into()),
    )
    .await
}

#[derive(Deserialize)]
struct RemovePerson {
    pub person_id: Vec<i32>,
}

#[derive(Deserialize)]
struct RemoveEvent {
    pub event_id: Vec<i32>,
}

#[axum::debug_handler]
async fn post_remove_person(
    State(state): State<KnotState>,
    Form(RemovePerson { person_id }): Form<RemovePerson>,
) -> Result<impl IntoResponse, KnotError> {
    for person_id in person_id {
        trace!(?person_id, "Removing");
        sqlx::query!(
            r#"
DELETE FROM public.people
WHERE id=$1
            "#,
            person_id
        )
        .execute(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::RemovingPerson(person_id.into()),
        })?;
    }

    Ok(Redirect::to("/show_all"))
}

#[axum::debug_handler]
async fn post_remove_event(
    State(state): State<KnotState>,
    Form(RemoveEvent { event_id }): Form<RemoveEvent>,
) -> Result<impl IntoResponse, KnotError> {
    for event_id in event_id {
        trace!(?event_id, "Removing");
        sqlx::query!(
            r#"
    DELETE FROM public.events
    WHERE id=$1
            "#,
            event_id
        )
        .execute(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::RemovingEvent(event_id),
        })?;
    }

    Ok(Redirect::to("/show_all"))
}

pub fn router() -> Router<KnotState> {
    Router::new()
        .route("/remove_person", post(post_remove_person))
        .route_layer(permission_required!(
            KnotAuthBackend,
            login_url = "/login",
            PermissionsTarget::EditPeople
        ))
        .route("/remove_event", post(post_remove_event))
        .route_layer(permission_required!(
            KnotAuthBackend,
            login_url = "/login",
            PermissionsTarget::EditEvents
        ))
        .route("/show_all", get(get_show_all))
}
