use axum_login::axum_sessions::async_session::{
    serde_json::from_value, Result as ASResult, Session, SessionStore,
};
use serde_json::to_value;
use sqlx::{Pool, Postgres};
use chrono::Local;

#[derive(Debug, Clone)]
pub struct PostgresSessionStore {
    pool: Pool<Postgres>,
}

impl PostgresSessionStore {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl SessionStore for PostgresSessionStore {
    #[instrument(level = "debug", skip(cookie_value, self), fields(id = ?Session::id_from_cookie_value(&cookie_value)))]
    async fn load_session(&self, cookie_value: String) -> ASResult<Option<Session>> {
        let id = Session::id_from_cookie_value(&cookie_value)?;

        debug!(?id, "Loading");

        let json = sqlx::query!("SELECT * FROM sessions WHERE id = $1 AND (expires IS NULL OR expires > $2)", id, Local::now().naive_local())
            .fetch_optional(&mut self.pool.acquire().await?)
            .await?;

        if let Some(json) = json {
            let fv = from_value::<Session>(json.session_json)?;
            return Ok(fv.validate());
        }
        Ok(None)
    }

    #[instrument(level = "debug", skip(session, self), fields(id = ?session.id()))]
    async fn store_session(&self, session: Session) -> ASResult<Option<String>> {
        if sqlx::query!(
            "SELECT id FROM sessions WHERE id = $1",
            session.id().to_string()
        )
        .fetch_optional(&mut self.pool.acquire().await?)
        .await?
        .is_some()
        {
            sqlx::query!(
                "UPDATE sessions SET session_json = $2, expires = $3 WHERE id = $1",
                session.id(),
                to_value(session.clone())?,
                session.expiry().copied().map(|x| x.naive_local())
            )
            .execute(&mut self.pool.acquire().await?)
            .await?;
        } else {
            sqlx::query!(
                "INSERT INTO sessions (id, session_json, expires) VALUES ($1, $2, $3)",
                session.id(),
                to_value(session.clone())?,
                session.expiry().copied().map(|x| x.naive_local())
            )
            .execute(&mut self.pool.acquire().await?)
            .await?;
        }

        debug!(id=?session.id(), "Storing");

        session.reset_data_changed();
        Ok(session.into_cookie_value())
    }

    #[instrument(level = "debug", skip(session, self), values(id = ?session.id()))]
    async fn destroy_session(&self, session: Session) -> ASResult {
        sqlx::query!(
            "DELETE FROM sessions WHERE id = $1",
            session.id().to_string()
        )
        .execute(&mut self.pool.acquire().await?)
        .await?;

        debug!(id=?session.id(), "Destroying");

        Ok(())
    }

    #[instrument(level = "info", skip(self))]
    async fn clear_store(&self) -> ASResult {
        sqlx::query!("TRUNCATE sessions")
            .execute(&mut self.pool.acquire().await?)
            .await?;

        debug!("Truncating");

        Ok(())
    }
}
