use sqlx::{Pool, Postgres, pool::PoolConnection};

#[derive(Clone)]
pub struct KnotState {
    postgres: Pool<Postgres>
}

impl KnotState {
    pub fn new (postgres: Pool<Postgres>) -> Self {
        Self {
            postgres
        }
    }

    pub async fn get_connection (&self) -> Result<PoolConnection<Postgres>, sqlx::Error> {
        self.postgres.acquire().await
    }
}