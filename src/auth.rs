pub mod cloudflare_turnstile;

use self::cloudflare_turnstile::{verify_turnstile, GrabCFRemoteIP};
use crate::{error::KnotError, liquid_utils::compile, routes::DbPerson};
use axum::{
    extract::{State, Path},
    response::{IntoResponse, Redirect},
    Form,
};
use axum_login::{
    extractors::AuthContext, secrecy::SecretVec, AuthUser, PostgresStore, RequireAuthorizationLayer,
};
use bcrypt::{hash, verify, DEFAULT_COST};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use std::sync::Arc;

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
            panic!("Missing Password!")
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

#[derive(Deserialize)]
pub struct LoginDetails {
    pub first_name: String,
    pub surname: String,
    pub unhashed_password: String,
    #[serde(rename = "cf-turnstile-response")]
    pub cf_turnstile_response: String,
}

pub async fn get_login(auth: Auth) -> Result<impl IntoResponse, KnotError> {
    compile(
        "www/login.liquid",
        liquid::object!({ "auth": get_auth_object(auth) }),
    )
    .await
}

pub async fn get_login_failure(auth: Auth, Path(was_password_related): Path<bool>) -> Result<impl IntoResponse, KnotError> {
    compile(
        "www/failed_auth.liquid",
        liquid::object!({ "auth": get_auth_object(auth), "was_password_related": was_password_related }),
    )
    .await
}

pub async fn post_login(
    mut auth: Auth,
    State(pool): State<Arc<Pool<Postgres>>>,
    remote_ip: GrabCFRemoteIP,
    Form(LoginDetails {
        first_name,
        surname,
        unhashed_password,
        cf_turnstile_response,
    }): Form<LoginDetails>,
) -> Result<impl IntoResponse, KnotError> {
    verify_turnstile(cf_turnstile_response, remote_ip).await?;

    let db_user = sqlx::query_as!(
        DbPerson,
        r#"
SELECT id, first_name, surname, form, hashed_password, permissions as "permissions: _" 
FROM people 
WHERE first_name = $1 AND surname = $2
        "#,
        first_name,
        surname
    ) //https://github.com/launchbadge/sqlx/issues/1004
    .fetch_optional(pool.as_ref())
    .await?;

    let Some(db_user) = db_user else {
        return Ok(Redirect::to("/login_failure/false"))
    };

    Ok(match &db_user.hashed_password {
        //looks weird as otherwise borrow not living long enough
        Some(pw) => Redirect::to(if verify(unhashed_password, pw)? {
            auth.login(&db_user).await?;
            "/"
        } else {
            error!("USER FAILED TO LOGIN!!!");
            "/login_failure/true"
        }),
        None => {
            let hashed = hash(&unhashed_password, DEFAULT_COST)?;
            let person: DbPerson = sqlx::query_as!(
                DbPerson,
                r#"
UPDATE people
SET hashed_password = $1
WHERE id = $2
RETURNING id, first_name, surname, form, hashed_password, permissions as "permissions: _" 
    "#,
                hashed,
                db_user.id
            )
            .fetch_one(pool.as_ref())
            .await?;

            auth.login(&person).await?;

            Redirect::to("/")
        }
    })
}

pub async fn post_logout(mut auth: Auth) -> Result<impl IntoResponse, KnotError> {
    auth.logout().await;
    Ok(Redirect::to("/"))
}

pub fn get_auth_object(auth: Auth) -> liquid::Object {
    if let Some(user) = auth.current_user {
        let perms = liquid::object!({
            "dev_access": user.permissions >= PermissionsRole::Dev,
            "run_migrations": user.permissions >= PermissionsRole::Admin,
            "edit_people": user.permissions >= PermissionsRole::Admin,
            "edit_events": user.permissions >= PermissionsRole::Prefect,
            "add_photos": user.permissions >= PermissionsRole::Prefect,
            "edit_prefects_on_events": user.permissions >= PermissionsRole::Prefect,
            "edit_participants_on_events": user.permissions >= PermissionsRole::Participant,
            "see_photos": user.permissions >= PermissionsRole::Participant,
        });

        liquid::object!({ "role": user.permissions, "permissions": perms, "user": user })
    } else {
        let perms = liquid::object!({
            "dev_access": false,
            "run_migrations": false,
            "edit_people": false,
            "edit_events": false,
            "add_photos": false,
            "edit_prefects_on_events": false,
            "edit_participants_on_events": false,
            "see_photos": false,
        });

        liquid::object!({"role": "visitor", "permissions": perms })
    }
}
