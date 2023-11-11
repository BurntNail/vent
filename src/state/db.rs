use sqlx::{Pool, Postgres};

#[derive(Clone, Debug)]
pub struct VentDB {
    pub pool: Pool<Postgres>,
}

impl VentDB {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

//TODO: exciting caching?
