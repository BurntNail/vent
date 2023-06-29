pub mod add_password;
pub mod cloudflare_turnstile;
pub mod login;
pub mod pg_session;

use crate::{error::KnotError, routes::DbPerson};
use axum_login::{
    extractors::AuthContext, secrecy::SecretVec, AuthUser, PostgresStore, RequireAuthorizationLayer,
};
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};

#[derive(
    sqlx::Type, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize, Debug,
)]
#[sqlx(type_name = "user_role", rename_all = "lowercase")]
pub enum PermissionsRole {
    Participant,
    Prefect,
    Admin,
    Dev,
}

impl AuthUser<i32, PermissionsRole> for DbPerson {
    fn get_id(&self) -> i32 {
        self.id
    }

    fn get_password_hash(&self) -> axum_login::secrecy::SecretVec<u8> {
        let Some(hp) = self.hashed_password.clone() else {
            error!(?self, "Missing Password");
            panic!("Missing Password!");
        };
        SecretVec::new(hp.into())
    }

    fn get_role(&self) -> Option<PermissionsRole> {
        Some(self.permissions)
    }
}

pub type Auth = AuthContext<i32, DbPerson, Store, PermissionsRole>;
pub type RequireAuth = RequireAuthorizationLayer<i32, DbPerson, PermissionsRole>;
pub type Store = PostgresStore<DbPerson, PermissionsRole>;

pub async fn get_secret(pool: &Pool<Postgres>) -> Result<Vec<u8>, KnotError> {
    if let Some(x) = sqlx::query!("SELECT sekrit FROM secrets")
        .fetch_optional(&mut pool.acquire().await?)
        .await?
    {
        Ok(x.sekrit)
    } else {
        let secret = {
            let mut rng = thread_rng();
            let mut v = Vec::with_capacity(64);
            v.append(&mut rng.gen::<[u8; 32]>().to_vec());
            v.append(&mut rng.gen::<[u8; 32]>().to_vec());
            v
        };

        sqlx::query!("INSERT INTO secrets (sekrit) VALUES ($1)", secret)
            .execute(&mut pool.acquire().await?)
            .await?;

        Ok(secret)
    }
}

pub fn get_auth_object(auth: Auth) -> liquid::Object {
    if let Some(user) = auth.current_user {
        let perms = liquid::object!({
            "dev_access": user.permissions >= PermissionsRole::Dev,
            "import_csv": user.permissions >= PermissionsRole::Admin,
            "run_migrations": user.permissions >= PermissionsRole::Admin,
            "edit_people": user.permissions >= PermissionsRole::Admin,
            "edit_events": user.permissions >= PermissionsRole::Prefect,
            "add_photos": user.permissions >= PermissionsRole::Prefect,
            "view_photo_adders": user.permissions >= PermissionsRole::Prefect,
            "edit_prefects_on_events": user.permissions >= PermissionsRole::Prefect,
            "edit_participants_on_events": user.permissions >= PermissionsRole::Prefect,
            "add_rm_self_to_event": user.permissions >= PermissionsRole::Participant,
            "see_photos": user.permissions >= PermissionsRole::Participant,
            "export_csv": user.permissions >= PermissionsRole::Participant,
        });

        liquid::object!({ "role": user.permissions, "permissions": perms, "user": user })
    } else {
        let perms = liquid::object!({
            "dev_access": false,
            "import_csv": false,
            "run_migrations": false,
            "edit_people": false,
            "edit_events": false,
            "add_photos": false,
            "view_photo_adders": false,
            "edit_prefects_on_events": false,
            "edit_participants_on_events": false,
            "add_rm_self_to_event": false,
            "see_photos": false,
            "export_csv": false,
        });

        liquid::object!({"role": "visitor", "permissions": perms })
    }
}
