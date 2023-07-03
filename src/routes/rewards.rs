use axum::{response::{IntoResponse, Redirect}, extract::{State, Form}};
use itertools::Itertools;
use serde::{Serialize, Deserialize};

use crate::{error::KnotError, state::KnotState, liquid_utils::compile, auth::{get_auth_object, Auth}};

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Reward {
    pub name: String,
    pub first_entry_pts: i32,
    pub second_entry_pts: i32,
    pub id: i32,
}

pub async fn get_rewards (auth: Auth, State(state): State<KnotState>) -> Result<impl IntoResponse, KnotError> {
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

    let general_awards = sqlx::query_as!(Reward, "SELECT * FROM rewards").fetch_all(&mut state.get_connection().await?).await?;


    let mut final_people = vec![];
    for record in sqlx::query!(r#"
    SELECT first_name, surname, form, id, was_first_entry, (select count(*) from participant_events pe where pe.participant_id = id and pe.is_verified = true) as no_events
    FROM people
    "#).fetch_all(&mut state.get_connection().await?).await? {
        let already_got_award_ids = sqlx::query!("SELECT reward_id FROM rewards_received WHERE person_id = $1", record.id).fetch_all(&mut state.get_connection().await?).await?.into_iter().map(|x| x.reward_id).collect_vec();

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

        final_people.push(Person {
            first_name: record.first_name,
            surname: record.surname,
            form: record.form,
            id: record.id,
            n_awards: to_be_received.len(),
            awards: to_be_received
        });
    }


    final_people.sort_by_cached_key(|x| x.surname.clone());
    final_people.sort_by_cached_key(|x| x.form.clone());
    final_people.sort_by_cached_key(|x| x.awards.clone());


    compile(
        "www/rewards.liquid",
        liquid::object!({ "people": final_people, "auth": get_auth_object(auth) }),
    )
    .await
}

#[derive(Deserialize)]
pub struct AddReward {
    reward_id: i32,
    person_id: i32
}

#[axum::debug_handler]
pub async fn post_add_reward (State(state): State<KnotState>, Form(AddReward { reward_id, person_id }): Form<AddReward>) -> Result<impl IntoResponse, KnotError> {

    sqlx::query!("INSERT INTO rewards_received (reward_id, person_id) VALUES ($1, $2)", reward_id, person_id).execute(&mut state.get_connection().await?).await?;

    Ok(Redirect::to("/add_reward"))
}