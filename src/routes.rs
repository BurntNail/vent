pub mod add_event;
pub mod add_people_to_event;
pub mod add_person;
pub mod calendar;
pub mod icon;
pub mod images;
pub mod index;
pub mod show_all;
pub mod update_event_and_person;
pub mod spreadsheets;
pub mod edit_person;

use crate::error::KnotError;
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
}

impl TryFrom<FormEvent> for DbEvent {
    type Error = KnotError;

    ///Get a [`DbEvent`] from a [`FormEvent`], can fail if we can't parse the date.
    ///
    /// NB: Event ID is always -1 as `try_from` cannot get a DB connection
    fn try_from(
        FormEvent {
            name,
            date,
            location,
            teacher,
            info,
        }: FormEvent,
    ) -> Result<Self, Self::Error> {
        let date = NaiveDateTime::parse_from_str(&date, "%Y-%m-%dT%H:%M")?;

        Ok(Self {
            id: -1, //no ID for events to be added
            event_name: name,
            date,
            location,
            teacher,
            other_info: Some(info),
        })
    }
}
