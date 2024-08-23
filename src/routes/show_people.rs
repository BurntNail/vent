use crate::{
    auth::{
        backend::{Auth, VentAuthBackend},
        get_auth_object, PermissionsTarget,
    },
    error::{SqlxAction, SqlxSnafu, VentError},
    state::VentState,
};
use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
    routing::{get, post},
    Router,
};
use axum_extra::extract::Form;
use axum_login::permission_required;
use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use std::collections::HashMap;

#[axum::debug_handler]
async fn get_show_people(
    auth: Auth,
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
    #[derive(Serialize)]
    pub struct SmolPerson {
        pub first_name: String,
        pub surname: String,
        pub form: String,
        pub id: i32,
        pub pts: usize,
    }

    debug!("Getting people");

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
    let mut points_by_form: HashMap<String, usize> = HashMap::new();
    for person in people {
        let event_pts = sqlx::query!("SELECT COUNT(participant_id) FROM participant_events WHERE participant_id = $1 AND is_verified = true", person.id).fetch_one(&mut *state.get_connection().await?).await.context(SqlxSnafu { action: SqlxAction::GettingRewardsReceived(Some(person.id.into())) })?.count.unwrap_or(0) as usize;

        #[derive(Serialize)]
        struct BonusPointCount {
            num_points: i32,
        }
        let bonus_points_vec: Vec<BonusPointCount> = sqlx::query!("SELECT bonus_points.num_points FROM participant_bonus_points INNER JOIN bonus_points ON participant_bonus_points.bonus_point_id = bonus_points.id WHERE participant_id = $1;", person.id).fetch_all(&mut *state.get_connection().await?)
            .await
            .context(SqlxSnafu { action: SqlxAction::GettingRewardsReceived(Some(person.id.into())) })?
            .into_iter().map(|row| {
            BonusPointCount {
                num_points: row.num_points,
            }
        }).collect();
        let bonus_pts = bonus_points_vec.iter().map(|bp| bp.num_points).sum::<i32>() as usize;
        let pts = event_pts + bonus_pts;

        new_people.push(SmolPerson {
            first_name: person.first_name,
            surname: person.surname,
            form: person.form.clone(),
            id: person.id,
            pts,
        });
        *points_by_form.entry(person.form).or_default() += pts;
    }

    trace!("Compiling");

    let aa = get_auth_object(auth).await?;

    state
        .compile(
            "www/show_people.liquid",
            liquid::object!({ "people": new_people, "auth": aa, "points_by_form": points_by_form }),
            Some("All People".into()),
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
    State(state): State<VentState>,
    Form(RemovePerson { person_id }): Form<RemovePerson>,
) -> Result<impl IntoResponse, VentError> {
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

    Ok(Redirect::to("/show_people"))
}

#[axum::debug_handler]
#[allow(dead_code)]
async fn post_remove_event(
    State(state): State<VentState>,
    Form(RemoveEvent { event_id }): Form<RemoveEvent>,
) -> Result<impl IntoResponse, VentError> {
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

    Ok(Redirect::to("/show_people"))
}

pub fn router() -> Router<VentState> {
    Router::new()
        .route("/remove_person", post(post_remove_person))
        .route_layer(permission_required!(
            VentAuthBackend,
            login_url = "/login",
            PermissionsTarget::EditPeople
        ))
        .route("/show_people", get(get_show_people))
        .route_layer(permission_required!(
            VentAuthBackend,
            login_url = "/login",
            PermissionsTarget::SeePeople
        ))
}
