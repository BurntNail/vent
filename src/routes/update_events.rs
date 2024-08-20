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

#[allow(clippy::too_many_lines)]
#[axum::debug_handler]
async fn get_update_event(
    auth: Auth,
    Path(event_id): Path<i32>,
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
    debug!("Getting event");
    let DbEvent {
        id,
        event_name,
        date: naive_date,
        location,
        teacher,
        other_info,
        zip_file: _,
        is_locked
    } = sqlx::query_as!(
        DbEvent,
        r#"
SELECT * FROM events WHERE id = $1
"#,
        event_id
    )
    .fetch_one(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::FindingEvent(event_id),
    })?;
    let date = naive_date.to_string();

    #[derive(Deserialize, Serialize, Debug, Clone)]
    struct PersonPlusRelID {
        pub id: i32,
        pub first_name: String,
        pub surname: String,
        pub form: String,
        pub relation_id: i32,
        pub is_verified: bool,
    }

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

    debug!("Getting existing prefects");

    let mut existing_prefects = HashMap::new();
    for person in sqlx::query!(
        r#"
SELECT p.first_name, p.surname, pe.relation_id, p.id, p.form
FROM people p
INNER JOIN prefect_events pe ON pe.event_id = $1 AND pe.prefect_id = p.id
"#,
        event_id
    )
    .fetch_all(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::FindingParticipantsOrPrefectsAtEvents {
            event_id: Some(event_id),
        },
    })? {
        existing_prefects
            .entry(person.form.clone())
            .or_insert(RelFormGroup {
                form: person.form.clone(),
                people: vec![],
            })
            .people
            .push(PersonPlusRelID {
                id: person.id,
                first_name: person.first_name,
                surname: person.surname,
                form: person.form,
                relation_id: person.relation_id,
                is_verified: true,
            });
    }
    let mut existing_prefects = existing_prefects
        .into_values()
        .map(|mut rfg| {
            rfg.people.sort_by_key(|x| x.surname.clone());
            rfg
        })
        .collect::<Vec<_>>();
    existing_prefects.sort_by_key(|rfg| rfg.form.clone());

    debug!("Getting existing participants");

    let mut existing_participants = HashMap::new();
    for person in sqlx::query_as!(
        PersonPlusRelID,
        r#"
SELECT p.first_name, p.surname, pe.relation_id, p.id, p.form, pe.is_verified
FROM people p
INNER JOIN participant_events pe ON pe.event_id = $1 AND pe.participant_id = p.id
"#,
        event_id
    )
    .fetch_all(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::FindingParticipantsOrPrefectsAtEvents {
            event_id: Some(event_id),
        },
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

    debug!("Getting possible prefects");

    let mut possible_prefects = HashMap::new();
    for person in sqlx::query_as!(
        DbPerson,
        r#"
SELECT id, first_name, surname, username, form, hashed_password, permissions as "permissions: _", was_first_entry
FROM people p
WHERE p.permissions != 'participant' AND p.form != 'Gone'
"#
    )
    .fetch_all(&mut *state.get_connection().await?)
    .await.context(SqlxSnafu { action: SqlxAction::FindingPeople })?
    .into_iter()
    .filter(|p| {
        !existing_prefects
            .iter()
            .any(|g| g.people.iter().any(|e| e.id == p.id))
    }) {
        possible_prefects
            .entry(person.form.clone())
            .or_insert(DbFormGroup {
                form: person.form.clone(),
                people: vec![],
            })
            .people
            .push(person);
    }
    let mut possible_prefects = possible_prefects
        .into_values()
        .map(|mut dfg| {
            dfg.people.sort_by_key(|x| x.surname.clone());
            dfg
        })
        .collect::<Vec<_>>();
    possible_prefects.sort_by_key(|dfg| dfg.form.clone());

    debug!("Getting possible participants");

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

    debug!("Getting photos");
    let mut photos = vec![];

    for raw in sqlx::query!(
        r#"
SELECT path, id, added_by FROM photos
WHERE event_id = $1
        "#,
        event_id
    )
    .fetch_all(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::FindingPhotos(event_id.into()),
    })? {
        let added_by = if let Some(added_by) = raw.added_by {
            let nf = sqlx::query!(
                "SELECT first_name, surname, form FROM people WHERE id = $1",
                added_by
            )
            .fetch_one(&mut *state.get_connection().await?)
            .await
            .context(SqlxSnafu {
                action: SqlxAction::FindingPerson(added_by.into()),
            })?;
            vec![format!("{} {}", nf.first_name, nf.surname), nf.form]
        } else {
            vec![]
        };

        photos.push(Image {
            path: raw.path,
            id: raw.id,
            has_added_by: added_by.len() == 2,
            added_by,
        });
    }

    debug!("Compiling");

    #[derive(Serialize)]
    pub struct AlreadyIn {
        pub is_in: bool,
        pub past_date: bool,
        pub rel_id: i32,
    }

    let already_in = auth.user.clone().map_or(
        AlreadyIn {
            is_in: false,
            past_date: naive_date < Utc::now().naive_local(),
            rel_id: -1,
        },
        |target_id| {
            if let Some(relation) = existing_participants
                .iter()
                .find(|rel_id| rel_id.people.iter().any(|rel_id| rel_id.id == target_id.id))
            {
                AlreadyIn {
                    is_in: true,
                    past_date: naive_date < Utc::now().naive_local(),
                    rel_id: relation
                        .people
                        .iter()
                        .find(|rel_id| rel_id.id == target_id.id)
                        .expect("but I just found one :(")
                        .relation_id,
                }
            } else {
                AlreadyIn {
                    is_in: false,
                    past_date: naive_date < Utc::now().naive_local(),
                    rel_id: -1,
                }
            }
        },
    );

    let aa = get_auth_object(auth).await?;

    compile_with_newtitle(
        "www/update_event.liquid",
        liquid::object!({"event": 
            liquid::object!({
                "id": id,
                "event_name": event_name.clone(),
                "date": date.to_string(),
                "location": location,
                "teacher": teacher,
                "other_info": other_info.unwrap_or_default(),
                "is_locked": is_locked
            }),
        "existing_prefects": existing_prefects,
        "existing_participants": existing_participants,
        "prefects": possible_prefects,
        "participants": possible_participants,
        "n_imgs": photos.len(),
        "imgs": photos,
        "auth": aa, "already_in": already_in }),
        &state.settings.brand.instance_name,
        Some(event_name),
    )
    .await
}
#[axum::debug_handler]
async fn post_update_event(
    Path(event_id): Path<i32>,
    State(state): State<VentState>,
    Form(FormEvent {
        name,
        date,
        location,
        teacher,
        info,
        is_locked
    }): Form<FormEvent>,
) -> Result<impl IntoResponse, VentError> {
    let date = NaiveDateTime::parse_from_str(&date, "%Y-%m-%dT%H:%M").context(ParseTimeSnafu {
        original: date.clone(),
        how_got_in: EncodeStep::Decode,
    })?;

    sqlx::query!(
        r#"
UPDATE public.events
SET event_name=$2, date=$3, location=$4, teacher=$5, other_info=$6, is_locked=$7
WHERE id=$1
        "#,
        event_id,
        name,
        date,
        location,
        teacher,
        info,
        is_locked
    )
    .execute(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::UpdatingEvent(event_id),
    })?;

    state.update_events()?;

    Ok(Redirect::to(&format!("/update_event/{event_id}")))
}

#[derive(Deserialize)]
struct Removal {
    relation_id: i32,
}

#[axum::debug_handler]
async fn post_remove_prefect_from_event(
    State(state): State<VentState>,
    Form(Removal { relation_id }): Form<Removal>,
) -> Result<impl IntoResponse, VentError> {
    let id = sqlx::query!(
        r#"
DELETE FROM prefect_events WHERE relation_id = $1 
RETURNING event_id
"#,
        relation_id
    )
    .fetch_one(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::RemovingPrefectOrPrefectFromEventByRI { relation_id },
    })?
    .event_id;

    state.update_events()?;

    Ok(Redirect::to(&format!("/update_event/{id}")))
}
#[axum::debug_handler]
async fn get_remove_participant_from_event(
    auth: Auth,
    State(state): State<VentState>,
    Form(Removal { relation_id }): Form<Removal>,
) -> Result<impl IntoResponse, VentError> {
    let current_user = auth.user.expect("need to be logged in to add participants");

    let event_details = sqlx::query!(
        "SELECT * FROM participant_events WHERE relation_id = $1",
        relation_id
    )
    .fetch_one(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::FindingParticipantOrPrefectByRI { relation_id },
    })?;

    if current_user.permissions < PermissionsRole::Prefect
        && current_user.id != event_details.participant_id
    {
        warn!(participant_id=?event_details.participant_id, perp=?current_user.id, "Participant did POST magic to get other participant, but failed.");
    } else {
        sqlx::query!(
            r#"
    DELETE FROM participant_events WHERE relation_id = $1 
    "#,
            relation_id
        )
        .execute(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::RemovingPrefectOrPrefectFromEventByRI { relation_id },
        })?;
    }
    Ok(Redirect::to(&format!(
        "/update_event/{}",
        event_details.event_id
    )))
}

#[axum::debug_handler]
async fn delete_image(
    Path(img_id): Path<i32>,
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
    let event = sqlx::query!(
        r#"
DELETE FROM public.photos
WHERE id=$1
RETURNING path, event_id"#,
        img_id
    )
    .fetch_one(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::RemovingPhoto(img_id),
    })?;

    if let Some(existing_zip_file) = sqlx::query!(
        r#"
SELECT zip_file
FROM events
WHERE id = $1"#,
        event.event_id
    )
    .fetch_one(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::FindingEvent(event.event_id),
    })?
    .zip_file
    {
        sqlx::query!(
            r#"
    UPDATE events
    SET zip_file = NULL
    WHERE id = $1"#,
            event.event_id
        )
        .execute(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::UpdatingEvent(event.event_id),
        })?;

        remove_file(&existing_zip_file).await.context(IOSnafu {
            action: IOAction::DeletingFile(existing_zip_file.into()),
        })?;
    }

    remove_file(&event.path).await.context(IOSnafu {
        action: IOAction::DeletingFile(event.path.into()),
    })?;

    Ok(Redirect::to(&format!("/update_event/{}", event.event_id)))
}

#[derive(Deserialize)]
struct VerifyPerson {
    event_id: i32,
    person_id: i32,
}

#[axum::debug_handler]
async fn post_verify_person(
    State(state): State<VentState>,
    Form(VerifyPerson {
        event_id,
        person_id,
    }): Form<VerifyPerson>,
) -> Result<impl IntoResponse, VentError> {
    sqlx::query!("UPDATE participant_events SET is_verified = true WHERE event_id = $1 AND participant_id = $2", event_id, person_id).execute(&mut *state.get_connection().await?).await.context(SqlxSnafu { action: SqlxAction::UpdatingParticipantOrPrefect {person: person_id.into(), event_id} })?;

    Ok(Redirect::to(&format!("/update_event/{event_id}")))
}

#[axum::debug_handler]
async fn post_unverify_person(
    State(state): State<VentState>,
    Form(VerifyPerson {
        event_id,
        person_id,
    }): Form<VerifyPerson>,
) -> Result<impl IntoResponse, VentError> {
    sqlx::query!("UPDATE participant_events SET is_verified = false WHERE event_id = $1 AND participant_id = $2", event_id, person_id).execute(&mut *state.get_connection().await?).await.context(SqlxSnafu { action: SqlxAction::UpdatingParticipantOrPrefect {person: person_id.into(), event_id} })?;

    Ok(Redirect::to(&format!("/update_event/{event_id}")))
}

#[derive(Deserialize)]
struct VerifyEveryone {
    event_id: i32,
}

#[axum::debug_handler]
async fn post_verify_everyone(
    State(state): State<VentState>,
    Form(VerifyEveryone { event_id }): Form<VerifyEveryone>,
) -> Result<impl IntoResponse, VentError> {
    sqlx::query!(
        "UPDATE participant_events SET is_verified = true WHERE event_id = $1",
        event_id
    )
    .execute(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::MassVerifying { event_id },
    })?;
    Ok(Redirect::to(&format!("/update_event/{event_id}")))
}

pub fn router() -> Router<VentState> {
    Router::new()
        .route("/update_event/:id", post(post_update_event))
        .route_layer(permission_required!(
            VentAuthBackend,
            login_url = "/login",
            PermissionsTarget::EditEvents
        ))
        .route("/verify_all", post(post_verify_everyone))
        .route("/verify_participant", post(post_verify_person))
        .route("/unverify_participant", post(post_unverify_person))
        .route_layer(permission_required!(
            VentAuthBackend,
            login_url = "/login",
            PermissionsTarget::VerifyEvents
        ))
        .route(
            "/remove_prefect_from_event",
            post(post_remove_prefect_from_event),
        )
        .route_layer(permission_required!(
            VentAuthBackend,
            login_url = "/login",
            PermissionsTarget::EditPrefectsOnEvents
        ))
        .route(
            "/remove_participant_from_event",
            post(get_remove_participant_from_event),
        )
        .route_layer(permission_required!(
            VentAuthBackend,
            login_url = "/login",
            PermissionsTarget::EditParticipantsOnEvents
        ))
        .route("/remove_img/:id", get(delete_image))
        .route_layer(login_required!(VentAuthBackend, login_url = "/login"))
        .route("/update_event/:id", get(get_update_event))
}
