use crate::auth::PermissionsRole;
use axum_login::AuthUser;
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

//necessary due to unability to make breaking changes with old versions - in future implentations I might store the bytes
#[derive(Clone, Debug, Serialize)]
pub struct AuthorisationBackendPerson {
    pub id: i32,
    pub first_name: String,
    pub surname: String,
    pub username: String,
    pub was_first_entry: bool,
    pub form: String,
    hashed_password_bytes: Vec<u8>,
    pub permissions: PermissionsRole,
}

impl From<DbPerson> for AuthorisationBackendPerson {
    fn from(
        DbPerson {
            id,
            first_name,
            surname,
            username,
            was_first_entry,
            form,
            hashed_password,
            permissions,
        }: DbPerson,
    ) -> Self {
        Self {
            id,
            first_name,
            surname,
            username,
            was_first_entry,
            form,
            hashed_password_bytes: hashed_password.unwrap_or_default().as_bytes().to_vec(),
            permissions,
        }
    }
}

impl AuthUser for AuthorisationBackendPerson {
    type Id = i32;

    fn id(&self) -> Self::Id {
        self.id
    }

    fn session_auth_hash(&self) -> &[u8] {
        &self.hashed_password_bytes
    }
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

#[derive(Deserialize, Clone, Debug)]
pub struct DbBonusPoint {
    pub id: i32,
    pub point_date: NaiveDateTime,
    pub staff_member_id: Option<i32>,
    pub num_points: i32,
    pub reason: String,
}
