use std::collections::HashSet;
use axum_login::{AuthnBackend, AuthSession, AuthzBackend, UserId};
use bcrypt::verify;
use snafu::ResultExt;
use crate::auth::login::LoginCreds;
use crate::auth::PermissionsTarget;
use crate::error::{KnotError, LoginFailureReason, SqlxAction, SqlxSnafu};
use crate::state::db_objects::DbPerson;
use crate::state::KnotState;

pub type Auth = AuthSession<KnotAuthBackend>;

#[derive(Clone)]
pub struct KnotAuthBackend {
    state: KnotState
}

impl KnotAuthBackend {
    pub fn new (state: KnotState) -> Self {
        Self {
            state
        }
    }
}

#[async_trait::async_trait]
impl AuthnBackend for KnotAuthBackend {
    type User = DbPerson;
    type Credentials = LoginCreds;
    type Error = KnotError;

    async fn authenticate(&self, LoginCreds { username, unhashed_password }: Self::Credentials) -> Result<Option<Self::User>, Self::Error> {
        let db_user = sqlx::query_as!(
            DbPerson,
            r#"
SELECT id, first_name, surname, username, form, hashed_password, permissions as "permissions: _", was_first_entry
FROM people
WHERE LOWER(username) = LOWER($1)
        "#,
            username
            )
            .fetch_optional(&mut self.state.get_connection().await?)
            .await.context(SqlxSnafu {action: SqlxAction::FindingPerson(username.into())})?;

        let Some(db_user) = db_user else {
            return Ok(None);
        };
        let Some(hashed_password) = &db_user.hashed_password else {
            self.state.reset_password(db_user.id).await?;
            return Err(KnotError::LoginFailure {reason: LoginFailureReason::PasswordIsNotSet});
        };

        if verify(unhashed_password, hashed_password)? {
            Ok(Some(db_user))
        } else {
            Err(KnotError::LoginFailure {reason: LoginFailureReason::IncorrectPassword})
        }
    }

    async fn get_user(&self, user_id: &UserId<Self>) -> Result<Option<Self::User>, Self::Error> {
        sqlx::query_as!(
            DbPerson,
            r#"
SELECT id, first_name, surname, username, form, hashed_password, permissions as "permissions: _", was_first_entry
FROM people
WHERE id = $1
        "#,
            user_id
            )
            .fetch_optional(&mut self.state.get_connection().await?)
            .await.context(SqlxSnafu {action: SqlxAction::FindingPerson(user_id.into())})
    }
}

#[async_trait::async_trait]
impl AuthzBackend for KnotAuthBackend {
    type Permission = PermissionsTarget;

    async fn get_user_permissions(&self, _user: &Self::User) -> Result<HashSet<Self::Permission>, Self::Error> {
        //TODO: individual permissions for things like photos?
        Ok(HashSet::new())
    }

    async fn get_group_permissions(&self, user: &Self::User) -> Result<HashSet<Self::Permission>, Self::Error> {
        Ok(user.permissions.can())
    }

}