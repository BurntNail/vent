pub mod add_event;
pub mod add_people_to_event;
pub mod add_person;
pub mod calendar;
pub mod edit_person;
pub mod edit_self;
pub mod eoy_migration;
pub mod images;
pub mod import_export;
pub mod index;
pub mod public;
pub mod rewards;
pub mod show_all;
pub mod spreadsheets;
pub mod update_events;

use serde::Deserialize;

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
