use crate::{
    auth::{
        backend::{Auth, VentAuthBackend},
        get_auth_object, PermissionsRole, PermissionsTarget,
    },
    error::{EncodeStep, IOAction, IOSnafu, ParseTimeSnafu, SqlxAction, SqlxSnafu, VentError},
    liquid_utils::compile_with_newtitle,
    routes::FormEvent,
    state::{
        db_objects::{DbEvent, DbPerson},
        VentState,
    },
};
use axum::{
    extract::{Path, State},
    response::{IntoResponse, Redirect},
    routing::{get, post},
    Router,
};
use axum_extra::extract::Form;
use axum_login::{login_required, permission_required};
use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use std::collections::HashMap;
use tokio::fs::remove_file;
use crate::routes::add_people_to_event::AddPerson;
use crate::routes::FormBonusPoint;
use crate::state::db_objects::DbBonusPoint;

#[derive(Deserialize, Serialize, Debug, Clone)]
struct PersonPlusRelID {
    pub id: i32,
    pub first_name: String,
    pub surname: String,
    pub relation_id: i32,
    pub form: String,
}

async fn get_update_bonus_point(
    auth: Auth,
    Path(bonus_point_id): Path<i32>,
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
    debug!("Getting bonus point");
    let DbBonusPoint {
        id,
        point_date: naive_date,
        staff_member_id,
        num_points,
        reason,
    } = sqlx::query_as!(
        DbBonusPoint,
        r#"
SELECT * FROM bonus_points WHERE id = $1
"#,
        bonus_point_id
    )
        .fetch_one(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::UpdatingBonusPoint(bonus_point_id),
        })?;

    #[derive(Serialize, Clone)]
    struct RelFormGroup {
        pub form: String,
        pub people: Vec<PersonPlusRelID>,
    }
    #[derive(Serialize, Clone)]
    struct DbFormGroup {
        pub form: String,
        pub people: Vec<DbPerson>,
    }

    debug!("Getting existing participants (bonus point)");

    let mut existing_participants = HashMap::new();
    for person in sqlx::query_as!(
        PersonPlusRelID,
        r#"
SELECT p.id, p.first_name, p.surname, pbp.relation_id, p.form
FROM people p
INNER JOIN participant_bonus_points pbp ON p.id = pbp.participant_id
WHERE pbp.bonus_point_id = $1
"#,
        bonus_point_id
    )
        .fetch_all(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingPotentialParticipantsForBonusPoint(id),
        })? {
        existing_participants
            .entry(person.form.clone())
            .or_insert(RelFormGroup {
                form: person.form.clone(),
                people: vec![],
            })
            .people
            .push(person);
    }
    let mut existing_participants = existing_participants
        .into_values()
        .map(|mut rfg| {
            rfg.people.sort_by_key(|x| x.surname.clone());
            rfg
        })
        .collect::<Vec<_>>();
    existing_participants.sort_by_key(|rfg| rfg.form.clone());

    debug!("Getting possible participants (bonus point)");

    let mut possible_participants = HashMap::new();
    for person in sqlx::query_as!(
        DbPerson,
        r#"
SELECT id, first_name, surname, username, form, hashed_password, permissions as "permissions: _", was_first_entry
FROM people p
WHERE p.form != 'Gone'
"#
    )
        .fetch_all(&mut *state.get_connection().await?)
        .await.context(SqlxSnafu { action: SqlxAction::FindingPeople })?
        .into_iter()
        .filter(|p| {
            !existing_participants
                .iter()
                .any(|g| g.people.iter().any(|e| e.id == p.id))
        }) {
        possible_participants
            .entry(person.form.clone())
            .or_insert(DbFormGroup {
                form: person.form.clone(),
                people: vec![],
            })
            .people
            .push(person);
    }
    let mut possible_participants = possible_participants
        .into_values()
        .map(|mut dfg| {
            dfg.people.sort_by_key(|x| x.surname.clone());
            dfg
        })
        .collect::<Vec<_>>();
    possible_participants.sort_by_key(|dfg| dfg.form.clone());

    #[derive(Serialize)]
    struct Image {
        path: String,
        id: i32,
        added_by: Vec<String>, // len 2 if we got stuff, len 0 if not
        has_added_by: bool,
    }

    let staff_member_username = sqlx::query!(
        r#"
SELECT username FROM people WHERE id = $1
        "#,
        staff_member_id
    )
        .fetch_one(&mut *state.get_connection().await?)
        .await.context(SqlxSnafu { action: SqlxAction::FindingPerson(staff_member_id.unwrap().into()) })?.username;

    debug!("Compiling");
    let aa = get_auth_object(auth).await?;

    compile_with_newtitle(
        "www/update_bonus_point.liquid",
        liquid::object!({"bonus_point":
            liquid::object!({
                "id": bonus_point_id,
                "date": naive_date.date().format("%Y-%m-%d").to_string(),
                "staff_member": staff_member_username,
                "quantity": num_points,
                "reason": reason,
            }),
        "existing_participants": existing_participants,
        "participants": possible_participants,
        "auth": aa }),
        &state.settings.brand.instance_name,
        Some("Bonus Point".to_string()),
    )
        .await
}

async fn post_update_bonus_point(
    auth: Auth,
    Path(bonus_point_id): Path<i32>,
    State(state): State<VentState>,
    Form(FormBonusPoint {
             user_id,
             reason,
             quantity
         }): Form<FormBonusPoint>,
) -> Result<impl IntoResponse, VentError> {
    sqlx::query!(
        r#"
UPDATE public.bonus_points
SET reason=$2, num_points=$3
WHERE id=$1
        "#,
        bonus_point_id,
        reason,
        quantity
    )
        .execute(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::UpdatingBonusPoint(bonus_point_id),
        })?;

    state.update_events()?;

    Ok(Redirect::to(&format!("/update_bonus_point/{bonus_point_id}")))
}

async fn post_delete_bonus_point(
    auth: Auth,
    Path(bonus_point_id): Path<i32>,
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
    sqlx::query!(
        r#"
DELETE FROM public.bonus_points WHERE id = $1
        "#,
        bonus_point_id,
    )
        .execute(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::DeletingBonusPoint(bonus_point_id),
        })?;

    state.update_events()?;

    Ok(Redirect::to(&format!("/")))
}

#[derive(Deserialize)]
pub struct AddPeopleToBonusPoint {
    person_ids: Vec<i32>,
    bonus_point_id: i32,
}
#[axum::debug_handler]
async fn post_add_people_to_bonus_point(
    auth: Auth,
    State(state): State<VentState>,
    Form(AddPeopleToBonusPoint {
             person_ids,
             bonus_point_id
         }): Form<AddPeopleToBonusPoint>,
) -> Result<impl IntoResponse, VentError> {
    for participant_id in person_ids {
        if sqlx::query!(
            r#"
    SELECT * FROM public.participant_bonus_points
    WHERE participant_id = $1
    AND bonus_point_id = $2"#,
            participant_id,
            bonus_point_id,
        )
            .fetch_optional(&mut *state.get_connection().await?)
            .await
            .context(SqlxSnafu {
                action: SqlxAction::FindingParticipantsForBonusPoint {
                    person: participant_id.into(),
                    bonus_point_id,
                },
            })?
            .is_none()
        {
            debug!(%participant_id, %bonus_point_id, "Adding person to bonus point");
            sqlx::query!(
                r#"
    INSERT INTO public.participant_bonus_points
    (participant_id, bonus_point_id)
    VALUES($1, $2);
                "#,
                participant_id,
                bonus_point_id,
            )
                .execute(&mut *state.get_connection().await?)
                .await
                .context(SqlxSnafu {
                    action: SqlxAction::AddingParticipantToBonusPoint {
                        person: participant_id.into(),
                        bonus_point_id,
                    },
                })?;
        } else {
            warn!(%participant_id, %bonus_point_id, "Person already received this bonus point.");
        }
    }

    Ok(Redirect::to(&format!("/update_bonus_point/{bonus_point_id}"))) //then back to the update event page
}

#[derive(Deserialize)]
struct Removal {
    pub relation_id: i32,
}
#[axum::debug_handler]
async fn post_remove_person_from_bonus_point(
    auth: Auth,
    State(state): State<VentState>,
    Form(Removal {
             relation_id
         }): Form<Removal>,
) -> Result<impl IntoResponse, VentError> {
    let id = sqlx::query!(
        r#"
DELETE FROM public.participant_bonus_points WHERE relation_id = $1
RETURNING bonus_point_id
"#,
        relation_id
    )
        .fetch_one(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::RemovingParticipantFromBonusPointByRI { relation_id },
        })?
        .bonus_point_id;

    state.update_events()?;

    Ok(Redirect::to(&format!("/update_bonus_point/{id}")))
}
pub fn router() -> Router<VentState> {
    Router::new()
        .route("/update_bonus_point/:id", post(post_update_bonus_point))
        .route_layer(permission_required!(
            VentAuthBackend,
            login_url = "/login",
            PermissionsTarget::GiveBonusPoints
        ))
        .route("/bonus_point/add_people", post(post_add_people_to_bonus_point))
        .route_layer(permission_required!(
            VentAuthBackend,
            login_url = "/login",
            PermissionsTarget::GiveBonusPoints
        ))
        .route("/bonus_point/remove_person", post(post_remove_person_from_bonus_point))
        .route_layer(permission_required!(
            VentAuthBackend,
            login_url = "/login",
            PermissionsTarget::GiveBonusPoints
        ))
        .route("/delete_bonus_point/:id", post(post_delete_bonus_point))
        .route_layer(permission_required!(
            VentAuthBackend,
            login_url = "/login",
            PermissionsTarget::GiveBonusPoints
        ))
        .route("/update_bonus_point/:id", get(get_update_bonus_point))
        .route_layer(permission_required!(
            VentAuthBackend,
            login_url = "/login",
            PermissionsTarget::SeeBonusPoints
        ))
}