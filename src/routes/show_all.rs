use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::Form;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::{
    auth::{get_auth_object, Auth},
    error::KnotError,
    liquid_utils::{compile, EnvFormatter},
    state::KnotState,
};

#[derive(Deserialize)]
pub struct SmolDbEvent {
    pub id: i32,
    pub event_name: String,
    pub date: NaiveDateTime,
}
#[derive(Serialize)]
pub struct SmolFormattedDbEvent {
    pub id: i32,
    pub event_name: String,
    pub date: String,
}

impl From<SmolDbEvent> for SmolFormattedDbEvent {
    fn from(
        SmolDbEvent {
            id,
            event_name,
            date,
        }: SmolDbEvent,
    ) -> Self {
        Self {
            id,
            event_name,
            date: date.to_env_string(),
        }
    }
}

#[instrument(level = "debug", skip(auth, state))]
pub async fn get_remove_stuff(
    auth: Auth,
    State(state): State<KnotState>,
) -> Result<impl IntoResponse, KnotError> {
	#[derive(Serialize)]
    pub struct SmolPerson {
    	pub first_name: String,
    	pub surname: String,
    	pub form: String,
    	pub id: i32,
    	pub pts: usize
      }

    debug!("Gettinng people");

    let mut people = sqlx::query!(
        r#"
SELECT first_name, surname, form, id
FROM people p
        "#
    )
    .fetch_all(&mut state.get_connection().await?)
    .await?;
    people.sort_by_key(|x| x.surname.clone());
    people.sort_by_key(|x| x.form.clone());

    let mut new_people = vec![];
    for person in people {
    	let pts = sqlx::query!("SELECT participant_id FROM participant_events WHERE participant_id = $1", person.id).fetch_all(&mut state.get_connection().await?).await?.len();
    	new_people.push(SmolPerson {
    		first_name: person.first_name,
    		surname: person.surname,	
    		form: person.form,
    		id: person.id,
    		pts
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
    .fetch_all(&mut state.get_connection().await?)
    .await?
    .into_iter()
    .map(SmolFormattedDbEvent::from)
    .collect();

    trace!("Compiling");

    compile(
        "www/show_all.liquid",
        liquid::object!({ "people": new_people, "events": events, "auth": get_auth_object(auth) }),
    )
    .await
}

#[derive(Deserialize)]
pub struct RemovePerson {
    pub person_id: Vec<i32>,
}

#[derive(Deserialize)]
pub struct RemoveEvent {
    pub event_id: Vec<i32>,
}


#[instrument(level = "info", skip(state))]
pub async fn post_remove_person(
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
        .execute(&mut state.get_connection().await?)
        .await?;
    }

    Ok(Redirect::to("/show_all"))
}

#[instrument(level = "info", skip(state))]
pub async fn post_remove_event(
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
        .execute(&mut state.get_connection().await?)
        .await?;
    }

    Ok(Redirect::to("/show_all"))
}
