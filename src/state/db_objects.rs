use crate::routes::PermissionsRole;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

//get everything `id, first_name, surname, username, form, hashed_password, permissions as "permissions: _", was_first_entry `
//https://github.com/launchbadge/sqlx/issues/1004
#[derive(Deserialize, Serialize, Clone, FromRow, Debug)]
pub struct DbPerson {
    pub id: i32,
    pub first_name: String,
    pub surname: String,
    pub username: String,
    pub was_first_entry: bool,
    pub form: String,
    pub hashed_password: Option<String>,
    pub permissions: PermissionsRole,
}

#[derive(Deserialize, Clone, Debug)]
pub struct DbEvent {
    pub id: i32,
    pub event_name: String,
    pub date: NaiveDateTime,
    pub location: String,
    pub teacher: String,
    pub other_info: Option<String>,
    pub zip_file: Option<String>,
}
