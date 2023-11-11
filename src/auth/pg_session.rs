use crate::error::{VentError, SqlxAction, SqlxSnafu};
use axum_login::axum_sessions::async_session::{
    serde_json::from_value, Result as ASResult, Session, SessionStore,
};
use serde_json::to_value;
use snafu::ResultExt;
use sqlx::{pool::PoolConnection, Pool, Postgres};
use std::time::Duration;
use tokio::{
    sync::broadcast::Receiver as BroadcastReceiver,
    time::interval,
};

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
    #[instrument(level = "trace", skip(cookie_value, self), fields(id = ?Session::id_from_cookie_value(&cookie_value)))]
    async fn load_session(&self, cookie_value: String) -> ASResult<Option<Session>> {
        let id = Session::id_from_cookie_value(&cookie_value)?;

        trace!(?id, "Loading");

        let json = sqlx::query!(
            "SELECT * FROM sessions WHERE id = $1 AND (expires IS NULL OR expires > NOW())",
            id,
        )
        .fetch_optional(&mut self.pool.acquire().await?)
        .await?;

        if let Some(json) = json {
            let fv = from_value::<Session>(json.session_json)?;
            return Ok(fv.validate());
        }
        Ok(None)
    }

    #[instrument(level = "trace", skip(session, self), fields(id = ?session.id()))]
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

        trace!(id=?session.id(), "Storing");

        session.reset_data_changed();
        Ok(session.into_cookie_value())
    }

    #[instrument(level = "trace", skip(session, self), values(id = ?session.id()))]
    async fn destroy_session(&self, session: Session) -> ASResult {
        sqlx::query!(
            "DELETE FROM sessions WHERE id = $1",
            session.id().to_string()
        )
        .execute(&mut self.pool.acquire().await?)
        .await?;

        trace!(id=?session.id(), "Destroying");

        Ok(())
    }

    #[instrument(level = "info", skip(self))]
    async fn clear_store(&self) -> ASResult {
        sqlx::query!("TRUNCATE sessions")
            .execute(&mut self.pool.acquire().await?)
            .await?;

        trace!("Truncating");

        Ok(())
    }
}

pub fn clear_out_old_sessions_thread(pool: Pool<Postgres>, mut stop_rx: BroadcastReceiver<()>) {
    async fn clear_out_old(mut conn: PoolConnection<Postgres>) -> Result<(), VentError> {
        let rows_affected =
            sqlx::query!("delete FROM sessions WHERE expires < (NOW() - interval '1 day')")
                .execute(&mut conn)
                .await
                .context(SqlxSnafu {
                    action: SqlxAction::DeletingOldSessions,
                })?
                .rows_affected();

        info!(%rows_affected, "Deleted old sessions");

        Ok(())
    }

    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(60 * 60 * 24));
        loop {
            if tokio::select! {
                _stop = stop_rx.recv() => {true},
                _tick = interval.tick() => {
                    match pool.acquire().await {
                        Ok(conn) => {
                            if let Err(e) = clear_out_old(conn).await {
                                error!(?e, "Error clearing out old sessions");
                            }
                        }
                        Err(e) => error!(?e, "Error getting connection to clear out old sessions"),
                    }
                    false
                }
            } {
                return;
            }
        }
    });
}
