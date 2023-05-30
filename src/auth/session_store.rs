use axum_login::axum_sessions::async_session::{SessionStore, Result as ASResult, Session, serde_json::from_str};
use sqlx::{Pool, Postgres};
use chrono::Local;


//partially based off https://docs.rs/async-sqlx-session/latest/src/async_sqlx_session/pg.rs.html#270-330
#[derive(Debug, Clone)]
pub struct PostgresSessionStore {
    pool: Pool<Postgres>
}

impl PostgresSessionStore {
    pub fn new (pool: Pool<Postgres>) -> Self {
        Self {pool}
    }
}

#[async_trait]
impl SessionStore for PostgresSessionStore {
    async fn load_session(&self, cookie_value: String) -> ASResult<Option<Session>> {
        let id = Session::id_from_cookie_value(&cookie_value)?;
        
        let session = sqlx::query!(
            r#"
SELECT session_contents FROM auth_sessions
WHERE id = $1
AND (
    expiry IS NULL
    OR expiry > $2
)
"#,
id,
Local::now().naive_local()
        ).fetch_optional(&self.pool).await?.map(|s| s.session_contents).map(|s| from_str(&s));

        Ok(session.transpose()?)
    }
    async fn store_session(&self, session: Session) -> ASResult<Option<String>> {
        todo!()
    }
    async fn destroy_session(&self, session: Session) -> ASResult {
        todo!()
    }
    async fn clear_store(&self) -> ASResult {
        todo!()
    }
}