use axum::{extract::{Form, State}, response::{IntoResponse, Redirect}, routing::{get, post}, Router, Json};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use std::collections::HashMap;
use http::StatusCode;

use crate::{
    error::{VentError, SqlxAction, SqlxSnafu},
    state::VentState,
};

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Reward {
    pub name: String,
    pub first_entry_pts: i32,
    pub second_entry_pts: i32,
    pub id: i32,
}

#[derive(Serialize, Deserialize, Debug)]
struct PersonWithRewards {
    pub id: i32,
    pub awards: Vec<Reward>,
}

#[derive(Serialize, Debug)]
struct Rewards {
    to_be_awarded: Vec<PersonWithRewards>,
    already_awarded: Vec<PersonWithRewards>
}

#[axum::debug_handler]
pub async fn get_rewards(
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
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
    SELECT   was_first_entry, id, (select count(*) from participant_events pe where pe.participant_id = id and pe.is_verified = true) as no_events
    FROM people
    "#).fetch_all(&mut *state.get_connection().await?).await.context(SqlxSnafu { action: SqlxAction::FindingPeople })? {
        let already_got_award_ids = sqlx::query!("SELECT reward_id FROM rewards_received WHERE person_id = $1", record.id).fetch_all(&mut *state.get_connection().await?).await.context(SqlxSnafu { action: SqlxAction::GettingRewardsReceived(Some(record.id.into())) })?.into_iter().map(|x| x.reward_id).collect_vec();

        if already_got_award_ids.len() == general_awards.len() {
            continue;
        }

        let mut to_be_received = vec![];
        for award in &general_awards {
            let threshold = if record.was_first_entry {
                award.first_entry_pts
            } else {
                award.second_entry_pts
            };

            if !already_got_award_ids.contains(&award.id) && (record.no_events.unwrap_or_default() as i32) >= threshold {
                to_be_received.push(award.clone());
            }
        }

        if to_be_received.is_empty() {
            continue;
        }

        to_be_awarded.push(PersonWithRewards {
            id: record.id,
            awards: to_be_received
        });
    }


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
        already_awarded.push(PersonWithRewards {
            id: person_id,
            awards,
        });
    }

    Ok(Json(Rewards { to_be_awarded, already_awarded }))
}

#[derive(Deserialize)]
pub struct AddReward {
    reward_id: i32,
    person_id: i32,
}

#[axum::debug_handler]
pub async fn post_add_reward(
    State(state): State<VentState>,
    Json(AddReward {
        reward_id,
        person_id,
    }): Json<AddReward>,
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

    Ok(StatusCode::OK)
}

pub fn router() -> Router<VentState> {
    Router::new()
        .route("/add_reward", post(post_add_reward))
        .route("/get_rewards", get(get_rewards))
}
