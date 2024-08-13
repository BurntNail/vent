use crate::{auth::{
    backend::{Auth, VentAuthBackend},
    get_auth_object, PermissionsRole, PermissionsTarget,
}, error::{DatabaseIDMethod, SqlxAction, SqlxSnafu, VentError}, liquid_utils::{compile_with_newtitle, CustomFormat}, routes::{rewards::Reward, FormPerson}, state::{db_objects::DbPerson, VentState}};
use axum::{
    extract::{Path, State},
    response::{IntoResponse, Redirect},
    routing::{get, post},
    Form, Router,
};
use axum_login::login_required;
use bcrypt::{hash, DEFAULT_COST};
use serde::Serialize;
use serde::Deserialize;
use snafu::ResultExt;

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

#[derive(Serialize)]
struct Event {
    name: String,
    date: String,
    id: i32,
}

#[derive(Serialize)]
struct Photo {
    event_name: String,
    path: String,
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

#[axum::debug_handler]
pub async fn get_edit_user(
    auth: Auth,
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
    let aa = get_auth_object(auth.clone()).await?;
    let current_id = auth.user.unwrap().id;
    debug!("Getting relevant person");

    let person = sqlx::query_as!(
        DbPerson,
        r#"
SELECT id, first_name, surname, username, form, hashed_password, permissions as "permissions: _", was_first_entry
FROM people WHERE id = $1
        "#,
        current_id
    )
        .fetch_one(&mut *state.get_connection().await?)
        .await.context(SqlxSnafu { action: SqlxAction::FindingPerson(current_id.into()) })?;

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

    let events_supervised = sqlx::query!(
        r#"
SELECT date, event_name, id FROM events e
INNER JOIN prefect_events pe
ON pe.event_id = e.id AND pe.prefect_id = $1
        "#,
        current_id
    )
        .fetch_all(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingPerson(current_id.into()),
        })?
        .into_iter()
        .map(|r| Event {
            name: r.event_name,
            date: r.date.to_env_string(&state.settings.niche.date_time_format),
            id: r.id,
        })
        .collect::<Vec<_>>();

    debug!("Getting events participated");

    let events_participated_records = sqlx::query!(
        r#"
SELECT date, event_name, id FROM events e
INNER JOIN participant_events pe
ON pe.event_id = e.id AND pe.participant_id = $1 AND pe.is_verified"#,
        current_id
    )
        .fetch_all(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingEventsOnPeople {
                person: current_id.into(),
            },
        })?;

    let mut events_participated = vec![];
    let mut photos = vec![];

    for record in events_participated_records {
        let name = record.event_name;
        let date = record.date.format(&state.settings.niche.date_time_format).to_string();
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

    let event_pts = sqlx::query!("SELECT COUNT(participant_id) FROM participant_events WHERE participant_id = $1 AND is_verified = true", current_id).fetch_one(&mut *state.get_connection().await?).await.context(SqlxSnafu { action: SqlxAction::GettingRewardsReceived(Some(person.id.into())) })?.count.unwrap_or(0) as usize;
    let bonus_points: Vec<BonusPoint> = sqlx::query!("SELECT bonus_point_id, bonus_points.point_date, bonus_points.num_points, bonus_points.reason, participant_people.first_name AS participant_first_name, participant_people.surname AS participant_surname, staff_people.username AS staff_username FROM participant_bonus_points INNER JOIN bonus_points ON participant_bonus_points.bonus_point_id = bonus_points.id INNER JOIN people AS participant_people ON participant_bonus_points.participant_id = participant_people.id INNER JOIN people AS staff_people ON bonus_points.staff_member_id = staff_people.id WHERE participant_bonus_points.participant_id = $1;", current_id).fetch_all(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu { action: SqlxAction::GettingRewardsReceived(Some(person.id.into())) })?
        .into_iter().map(|row| {
        BonusPoint {
            bonus_point_id: row.bonus_point_id,
            point_date: row.point_date.to_env_string(&state.settings.niche.date_time_format),
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

    compile_with_newtitle("www/edit_self.liquid", liquid::object!({ "person": person, "supervised": events_supervised, "participated": events_participated, "pts": pts, "event_pts": event_pts, "bonus_points": bonus_points, "bonus_pts": bonus_pts, "rewards": rewards, "auth": aa, "imgs": photos, "n_imgs": photos.len() }), &state.settings.brand.instance_name, Some(format!("Edit {} {}", person.first_name, person.surname))).await
}

#[derive(Deserialize)]
pub struct LoginDetails {
    pub first_name: String,
    pub surname: String,
    pub unhashed_password: String,
}
#[axum::debug_handler]
pub async fn post_edit_user(
    auth: Auth,
    State(state): State<VentState>,
    Form(LoginDetails {
             first_name,
             surname,
             unhashed_password,
         }): Form<LoginDetails>,
) -> Result<impl IntoResponse, VentError> {
    let current_id = auth.user.unwrap().id;

    debug!(%current_id, "Hashing password");

    let hashed = hash(&unhashed_password, DEFAULT_COST)?;

    debug!("Updating in DB");

    sqlx::query!(
        r#"
UPDATE people
SET first_name=$1, surname = $2, hashed_password=$3
WHERE id=$4;
        "#,
        first_name,
        surname,
        hashed,
        current_id
    )
        .execute(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::UpdatingPerson(current_id.into()),
        })?;

    Ok(Redirect::to("/"))
}

pub fn router() -> Router<VentState> {
    Router::new()
        .route("/edit_user", get(get_edit_user).post(post_edit_user))
        .route_layer(login_required!(VentAuthBackend, login_url = "/login"))
}
