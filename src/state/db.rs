use sqlx::{Pool, Postgres};

#[derive(Clone, Debug)]
pub struct VentDatabase {
    pub pool: Pool<Postgres>,
}

impl VentDatabase {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

//TODO: exciting caching?
