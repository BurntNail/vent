use sqlx::{Pool, Postgres};
use axum_login::axum_sessions::async_session::{Result as ASResult, Session, SessionStore};


#[derive(Debug, Clone)]
pub struct PostgresSessionStore {
    pool: Pool<Postgres>
}

impl PostgresSessionStore {
    pub fn new (pool: Pool<Postgres>) -> Self {
        Self {pool}
    }
}

#[async_trait::async_trait]
impl SessionStore for PostgresSessionStore {
    //examples from https://docs.rs/async-session/3.0.0/src/async_session/memory_store.rs.html
    async fn load_session(&self, cookie_value: String) -> ASResult<Option<Session>> {
        // let id = Session::id_from_cookie_value(&cookie_value)?;
        // log::trace!("loading session by id `{}`", id);
        // Ok(self
        //     .inner
        //     .read()
        //     .await
        //     .get(&id)
        //     .cloned()
        //     .and_then(Session::validate))
        todo!()
    }

    async fn store_session(&self, session: Session) -> ASResult<Option<String>> {
        // log::trace!("storing session by id `{}`", session.id());
        // self.inner
        //     .write()
        //     .await
        //     .insert(session.id().to_string(), session.clone());

        // session.reset_data_changed();
        // Ok(session.into_cookie_value())
        todo!()
    }

    async fn destroy_session(&self, session: Session) -> ASResult {
        // log::trace!("destroying session by id `{}`", session.id());
        // self.inner.write().await.remove(session.id());
        // Ok(())
        todo!()
    }

    async fn clear_store(&self) -> ASResult {
        // log::trace!("clearing memory store");
        // self.inner.write().await.clear();
        // Ok(())
        todo!()
    }
}