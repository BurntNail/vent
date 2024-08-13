use crate::{auth::{
    backend::{Auth, VentAuthBackend},
    get_auth_object, PermissionsRole, PermissionsTarget,
}, error::{SqlxAction, SqlxSnafu, VentError, DatabaseIDMethod}, liquid_utils::{compile_with_newtitle, CustomFormat}, routes::{rewards::Reward, FormPerson}, state::{db_objects::DbPerson, VentState}};
use axum::{
    extract::{Path, State},
    response::{IntoResponse, Redirect},
    routing::{get, post},
    Form, Router,
};
use axum_login::permission_required;
use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use sqlx::Row;

#[axum::debug_handler]
async fn get_edit_person(
    auth: Auth,
    Path(id): Path<i32>,
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
    #[derive(Serialize)]
    struct SmolPerson {
        pub id: i32,
        pub permissions: PermissionsRole,
        pub first_name: String,
        pub surname: String,
        pub username: String,
        pub password_is_set: bool,
        pub form: String,
        pub was_first_entry: bool,
    }

    debug!("Getting relevant person");

    let person = sqlx::query_as!(
        DbPerson,
        r#"
SELECT id, first_name, surname, username, form, hashed_password, permissions as "permissions: _", was_first_entry
FROM people WHERE id = $1
        "#,
        id
    )
        .fetch_one(&mut *state.get_connection().await?)
        .await.context(SqlxSnafu { action: SqlxAction::FindingPerson(id.into()) })?;

    let person = SmolPerson {
        id: person.id,
        permissions: person.permissions,
        first_name: person.first_name,
        surname: person.surname,
        username: person.username,
        form: person.form,
        password_is_set: person.hashed_password.is_some(),
        was_first_entry: person.was_first_entry,
    };

    debug!("Getting events supervised");

    #[derive(Serialize)]
    struct Event {
        name: String,
        date: String,
        id: i32,
    }

    let events_supervised = sqlx::query!(
        r#"
SELECT date, event_name, id FROM events e 
INNER JOIN prefect_events pe
ON pe.event_id = e.id AND pe.prefect_id = $1
        "#,
        person.id
    )
        .fetch_all(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingPerson(person.id.into()),
        })?
        .into_iter()
        .map(|r| Event {
            name: r.event_name,
            date: r.date.to_env_string(&state.settings.niche.date_time_format),
            id: r.id,
        })
        .collect::<Vec<_>>();

    #[derive(Serialize)]
    struct Photo {
        event_name: String,
        path: String,
    }

    debug!("Getting events participated");

    let events_participated_records = sqlx::query!(
        r#"
SELECT date, event_name, id FROM events e
INNER JOIN participant_events pe
ON pe.event_id = e.id AND pe.participant_id = $1 AND pe.is_verified"#,
        person.id
    )
        .fetch_all(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingEventsOnPeople {
                person: person.id.into(),
            },
        })?;

    let mut events_participated = vec![];
    let mut photos = vec![];

    for record in events_participated_records {
        let name = record.event_name;
        let date = record.date.to_env_string(&state.settings.niche.date_time_format);
        let id = record.id;

        for rec in sqlx::query!("SELECT path FROM photos WHERE event_id = $1", id).fetch_all(&mut *state.get_connection().await?)
            .await
            .context(SqlxSnafu {
                action: SqlxAction::FindingPhotos(DatabaseIDMethod::Id(id))
            })? {
            photos.push(Photo {
                path: rec.path,
                event_name: name.clone(),
            });
        }

        events_participated.push(Event {
            name,
            date,
            id,
        });
    }

    #[derive(Serialize)]
    struct BonusPoint {
        bonus_point_id: i32,
        point_date: String,
        num_points: i32,
        reason: String,
        participant_first_name: String,
        participant_surname: String,
        staff_username: String,
    }

    let event_pts = sqlx::query!("SELECT COUNT(participant_id) FROM participant_events WHERE participant_id = $1 AND is_verified = true", person.id).fetch_one(&mut *state.get_connection().await?).await.context(SqlxSnafu { action: SqlxAction::GettingRewardsReceived(Some(person.id.into())) })?.count.unwrap_or(0) as usize;
    let bonus_points: Vec<BonusPoint> = sqlx::query!("SELECT bonus_point_id, bonus_points.point_date, bonus_points.num_points, bonus_points.reason, participant_people.first_name AS participant_first_name, participant_people.surname AS participant_surname, staff_people.username AS staff_username FROM participant_bonus_points INNER JOIN bonus_points ON participant_bonus_points.bonus_point_id = bonus_points.id INNER JOIN people AS participant_people ON participant_bonus_points.participant_id = participant_people.id INNER JOIN people AS staff_people ON bonus_points.staff_member_id = staff_people.id WHERE participant_bonus_points.participant_id = $1;", person.id).fetch_all(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu { action: SqlxAction::GettingRewardsReceived(Some(person.id.into())) })?
        .into_iter().map(|row| {
        BonusPoint {
            bonus_point_id: row.bonus_point_id,
            point_date: row.point_date.format(&state.settings.niche.date_time_format).to_string(),
            num_points: row.num_points,
            reason: row.reason,
            participant_first_name: row.participant_first_name,
            participant_surname: row.participant_surname,
            staff_username: row.staff_username,
        }
    }).collect();
    let bonus_pts = bonus_points.iter().map(|bp| bp.num_points).sum::<i32>() as usize;
    let pts = event_pts + bonus_pts;
    let rewards = sqlx::query_as!(Reward, "select name, first_entry_pts, second_entry_pts, id FROM rewards_received rr inner join rewards r on r.id = rr.reward_id and rr.person_id = $1", person.id).fetch_all(&mut *state.get_connection().await?).await.context(SqlxSnafu { action: SqlxAction::FindingPerson(person.id.into()) })?;

    debug!("Compiling");

    let aa = get_auth_object(auth).await?;

    compile_with_newtitle("www/edit_person.liquid", liquid::object!({ "person": person, "supervised": events_supervised, "participated": events_participated, "pts": pts, "event_pts": event_pts, "bonus_points": bonus_points, "bonus_pts": bonus_pts, "rewards": rewards, "auth": aa, "imgs": photos, "n_imgs": photos.len() }), &state.settings.brand.instance_name, Some(format!("Edit {} {}", person.first_name, person.surname))).await
}

#[axum::debug_handler]
async fn post_edit_person(
    Path(id): Path<i32>,
    State(state): State<VentState>,
    Form(FormPerson {
             first_name,
             surname,
             form,
             username,
             permissions,
         }): Form<FormPerson>,
) -> Result<impl IntoResponse, VentError> {
    debug!("Editing person");
    sqlx::query!(
        r#"
UPDATE public.people
SET permissions=$6, first_name=$2, surname=$3, form=$4, username=$5
WHERE id=$1
        "#,
        id,
        first_name,
        surname,
        form,
        username,
        permissions as _
    )
        .execute(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::UpdatingPerson(id.into()),
        })?;

    Ok(Redirect::to(&format!("/edit_person/{id}")))
}

#[derive(Deserialize)]
struct PasswordReset {
    id: i32,
}

#[axum::debug_handler]
async fn post_reset_password(
    mut auth: Auth,
    State(state): State<VentState>,
    Form(PasswordReset { id }): Form<PasswordReset>,
) -> Result<impl IntoResponse, VentError> {
    debug!("Logging out.");

    if auth.user.as_ref().is_some_and(|x| x.id == id) {
        auth.logout().await?;
    }

    debug!("Sending password reset");
    state.reset_password(id).await?;
    Ok(Redirect::to("/"))
}

pub fn router() -> Router<VentState> {
    Router::new()
        .route("/edit_person/:id", post(post_edit_person))
        .route_layer(permission_required!(
            VentAuthBackend,
            login_url = "/login",
            PermissionsTarget::SeePeople
        ))
        .route("/reset_password", post(post_reset_password))
        .route_layer(permission_required!(
            VentAuthBackend,
            login_url = "/login",
            PermissionsTarget::EditPeople
        ))
        .route("/edit_person/:id", get(get_edit_person))
        .route_layer(permission_required!(
            VentAuthBackend,
            login_url = "/login",
            PermissionsTarget::SeePeople
        ))
}
