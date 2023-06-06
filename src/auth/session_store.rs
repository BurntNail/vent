use axum_login::axum_sessions::async_session::{
    serde_json::{from_str, to_string},
    Result as ASResult, Session, SessionStore,
};
use chrono::{DateTime, Local};
use sqlx::{Pool, Postgres};

//partially based off https://docs.rs/async-sqlx-session/latest/src/async_sqlx_session/pg.rs.html#270-330
#[derive(Debug, Clone)]
pub struct PostgresSessionStore {
    pool: Pool<Postgres>,
}

impl PostgresSessionStore {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SessionStore for PostgresSessionStore {
    async fn load_session(&self, cookie_value: String) -> ASResult<Option<Session>> {
        let id = Session::id_from_cookie_value(&cookie_value)?;

        let session = sqlx::query!(
            r#"
SELECT session_contents FROM public.auth_sessions
WHERE id = $1
AND (
    expiry IS NULL
    OR expiry > $2
)
"#,
            id,
            Local::now().naive_local()
        )
        .fetch_optional(&self.pool)
        .await?
        .map(|s| s.session_contents)
        .map(|s| from_str(&s));

        Ok(session.transpose()?)
    }

    async fn store_session(&self, session: Session) -> ASResult<Option<String>> {
        let session_contents = to_string(&session)?;
        sqlx::query!(
            r#"
INSERT INTO public.auth_sessions
(id, expiry, session_contents)
VALUES($1, $2, $3)
ON CONFLICT(id) DO UPDATE SET
        expiry = $2,
        session_contents = $3
        "#,
            session.id(),
            session.expiry().map(DateTime::naive_local),
            session_contents
        )
        .execute(&self.pool)
        .await?;
        Ok(session.into_cookie_value())
    }

    async fn destroy_session(&self, session: Session) -> ASResult {
        sqlx::query!(
            r#"
DELETE FROM public.auth_sessions
WHERE id = $1"#,
            session.id()
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn clear_store(&self) -> ASResult {
        sqlx::query!("DELETE FROM public.auth_sessions")
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
