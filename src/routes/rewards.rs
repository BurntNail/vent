use axum::{
    extract::{Form, State},
    response::{IntoResponse, Redirect},
    routing::get,
    Router,
};
use axum_login::permission_required;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use std::collections::HashMap;

use crate::{
    auth::{
        backend::{Auth, VentAuthBackend},
        get_auth_object, PermissionsTarget,
    },
    error::{SqlxAction, SqlxSnafu, VentError},
    liquid_utils::compile_with_newtitle,
    state::VentState,
};

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Reward {
    pub name: String,
    pub first_entry_pts: i32,
    pub second_entry_pts: i32,
    pub id: i32,
}

#[axum::debug_handler]
pub async fn get_rewards(
    auth: Auth,
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
    ///NB: these are rewards TO BE RECEIVED
    #[derive(Serialize, Deserialize)]
    struct Person {
        pub first_name: String,
        pub surname: String,
        pub form: String,
        pub id: i32,
        pub awards: Vec<Reward>,
        pub n_awards: usize,
    }

    let general_awards = sqlx::query_as!(Reward, "SELECT * FROM rewards")
        .fetch_all(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::GettingRewards,
        })?;
    let general_awards_by_id: HashMap<_, _> = general_awards
        .clone()
        .into_iter()
        .map(|x| (x.id, x))
        .collect();

    let mut to_be_awarded = vec![];

    for record in sqlx::query!(r#"
    SELECT first_name, surname, form, id, was_first_entry,
           (SELECT COUNT(*) FROM participant_events pe WHERE pe.participant_id = id AND pe.is_verified = true) AS no_events,
           (SELECT COALESCE(SUM(bonus_points.num_points), 0)
            FROM participant_bonus_points
            INNER JOIN bonus_points ON participant_bonus_points.bonus_point_id = bonus_points.id
            WHERE participant_bonus_points.participant_id = id) AS total_bonus_points
    FROM people
    "#).fetch_all(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu { action: SqlxAction::FindingPeople })?
    {
        // Fetch the already received awards for this person
        let already_got_award_ids = sqlx::query!(
        "SELECT reward_id FROM rewards_received WHERE person_id = $1",
        record.id
    ).fetch_all(&mut *state.get_connection().await?)
            .await
            .context(SqlxSnafu { action: SqlxAction::GettingRewardsReceived(Some(record.id.into())) })?
            .into_iter()
            .map(|x| x.reward_id)
            .collect_vec();

        // Check if they already have all general awards
        if already_got_award_ids.len() == general_awards.len() {
            continue;
        }

        // Calculate the total number of events and bonus points
        let no_events = record.no_events.unwrap_or_default() as i32;
        let total_points = no_events + record.total_bonus_points.unwrap_or_default() as i32;

        let mut to_be_received = vec![];
        for award in &general_awards {
            let threshold = if record.was_first_entry {
                award.first_entry_pts
            } else {
                award.second_entry_pts
            };

            // Check if the person qualifies for this award
            if !already_got_award_ids.contains(&award.id) && total_points >= threshold {
                to_be_received.push(award.clone());
            }
        }

        // If no new awards, skip
        if to_be_received.is_empty() {
            continue;
        }

        // Collect information for the person to be awarded
        to_be_awarded.push(Person {
            first_name: record.first_name,
            surname: record.surname,
            form: record.form,
            id: record.id,
            n_awards: to_be_received.len(),
            awards: to_be_received,
        });
    }

    to_be_awarded.sort_by_cached_key(|x| x.surname.clone());
    to_be_awarded.sort_by_cached_key(|x| x.form.clone());
    to_be_awarded.sort_by_cached_key(|x| x.awards.clone());

    let mut already_awarded_hm = HashMap::new();

    for record in sqlx::query!("SELECT * FROM rewards_received")
        .fetch_all(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::GettingRewardsReceived(None),
        })?
    {
        already_awarded_hm
            .entry(record.person_id)
            .or_insert(vec![])
            .push(general_awards_by_id[&record.reward_id].clone());
    }

    let mut already_awarded = vec![];
    for (person_id, awards) in already_awarded_hm {
        let record = sqlx::query!(
            "SELECT first_name, surname, form FROM people WHERE id = $1",
            person_id
        )
        .fetch_one(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingPerson(person_id.into()),
        })?;
        already_awarded.push(Person {
            first_name: record.first_name,
            surname: record.surname,
            form: record.form,
            id: person_id,
            n_awards: awards.len(),
            awards,
        });
    }

    already_awarded.sort_by_cached_key(|x| x.surname.clone());
    already_awarded.sort_by_cached_key(|x| x.form.clone());
    already_awarded.sort_by_cached_key(|x| x.awards.clone());

    let aa = get_auth_object(auth).await?;

    compile_with_newtitle(
        "www/rewards.liquid",
        liquid::object!({ "tba": to_be_awarded, "aa": already_awarded, "auth": aa }),
        &state.settings.brand.instance_name,
        Some("Rewards".into()),
    )
    .await
}

#[derive(Deserialize)]
pub struct AddReward {
    reward_id: i32,
    person_id: i32,
}

#[axum::debug_handler]
pub async fn post_add_reward(
    State(state): State<VentState>,
    Form(AddReward {
        reward_id,
        person_id,
    }): Form<AddReward>,
) -> Result<impl IntoResponse, VentError> {
    sqlx::query!(
        "INSERT INTO rewards_received (reward_id, person_id) VALUES ($1, $2)",
        reward_id,
        person_id
    )
    .execute(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::AddingReward,
    })?;

    Ok(Redirect::to("/add_reward"))
}

pub fn router() -> Router<VentState> {
    Router::new()
        .route("/add_reward", get(get_rewards).post(post_add_reward))
        .route_layer(permission_required!(
            VentAuthBackend,
            login_url = "/login",
            PermissionsTarget::AddRewards
        ))
}
