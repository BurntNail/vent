use crate::{
    auth::{login::LoginCreds, PermissionsTarget},
    error::{VentError, LoginFailureReason, SqlxAction, SqlxSnafu},
    state::{
        db_objects::{AuthorisationBackendPerson, DbPerson},
        VentState,
    },
};
use axum_login::{AuthSession, AuthnBackend, AuthzBackend, UserId};
use bcrypt::verify;
use snafu::ResultExt;
use std::collections::HashSet;

pub type Auth = AuthSession<VentAuthBackend>;

#[derive(Clone)]
pub struct VentAuthBackend {
    state: VentState,
}

impl VentAuthBackend {
    pub fn new(state: VentState) -> Self {
        Self { state }
    }
}

#[async_trait::async_trait]
impl AuthnBackend for VentAuthBackend {
    type User = AuthorisationBackendPerson;
    type Credentials = LoginCreds;
    type Error = VentError;

    async fn authenticate(
        &self,
        LoginCreds {
            username,
            unhashed_password,
        }: Self::Credentials,
    ) -> Result<Option<Self::User>, Self::Error> {
        let db_user = sqlx::query_as!(
            DbPerson,
            r#"
SELECT id, first_name, surname, username, form, hashed_password, permissions as "permissions: _", was_first_entry
FROM people
WHERE LOWER(username) = LOWER($1)
        "#,
            username
            )
            .fetch_optional(&mut *self.state.get_connection().await?)
            .await.context(SqlxSnafu {action: SqlxAction::FindingPerson(username.into())})?;

        let Some(db_user) = db_user else {
            return Ok(None);
        };
        let Some(hashed_password) = &db_user.hashed_password else {
            self.state.reset_password(db_user.id).await?;
            return Err(VentError::LoginFailure {
                reason: LoginFailureReason::PasswordIsNotSet,
            });
        };

        if verify(unhashed_password, hashed_password)? {
            Ok(Some(db_user.into()))
        } else {
            Err(VentError::LoginFailure {
                reason: LoginFailureReason::IncorrectPassword,
            })
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
            .fetch_optional(&mut *self.state.get_connection().await?)
            .await.context(SqlxSnafu {action: SqlxAction::FindingPerson((*user_id).into())})
            .map(|x| x.map(Into::into))
    }
}

#[async_trait::async_trait]
impl AuthzBackend for VentAuthBackend {
    type Permission = PermissionsTarget;

    async fn get_user_permissions(
        &self,
        _user: &Self::User,
    ) -> Result<HashSet<Self::Permission>, Self::Error> {
        //TODO: individual permissions for things like photos?
        Ok(HashSet::new())
    }

    async fn get_group_permissions(
        &self,
        user: &Self::User,
    ) -> Result<HashSet<Self::Permission>, Self::Error> {
        Ok(user.permissions.can())
    }
}
