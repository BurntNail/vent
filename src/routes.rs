pub mod add_event;
pub mod add_people_to_event;
pub mod add_person;
pub mod calendar;
pub mod edit_person;
pub mod edit_user;
pub mod images;
pub mod index;
pub mod public;
pub mod show_all;
pub mod spreadsheets;
pub mod update_event_and_person;

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
struct DbPerson {
    pub first_name: String,
    pub surname: String,
    pub is_prefect: bool,
    pub id: i32,
    pub form: String,
}

#[derive(Deserialize)]
pub struct DbEvent {
    pub id: i32,
    pub event_name: String,
    pub date: NaiveDateTime,
    pub location: String,
    pub teacher: String,
    pub other_info: Option<String>,
    pub zip_file: Option<String>,
}

///Struct to hold the event that comes back from the [`add_event`] form
///
/// NB: when going into a [`DbEvent`], the ID will be -1,
#[derive(Debug, Deserialize)]
pub struct FormEvent {
    pub name: String,
    pub date: String,
    pub location: String,
    pub teacher: String,
    pub info: String,
    #[serde(rename = "cf-turnstile-response")]
    pub cf_turnstile_response: String
}
