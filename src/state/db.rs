use sqlx::{Pool, Postgres};

#[derive(Clone, Debug)]
pub struct KnotDatabase {
    pub pool: Pool<Postgres>,
}

impl KnotDatabase {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

//TODO: exciting caching?
