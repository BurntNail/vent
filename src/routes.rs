pub mod add_event;
pub mod add_people_to_event;
pub mod add_person;
pub mod index;

use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct Person {
    pub person_name: String,
    pub is_prefect: bool,
    pub id: i32,
}

#[derive(Debug, Deserialize)]
pub struct FormEvent {
    pub name: String,
    pub date: String,
    pub location: String,
    pub teacher: String,
    pub info: String,
}
