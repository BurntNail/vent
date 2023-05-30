use axum_login::axum_sessions::async_session::{SessionStore, Result as ASResult, Session};
use sqlx::{Pool, Postgres};

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
        todo!()
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