use axum_login::{AuthUser, secrecy::SecretVec, PostgresStore, RequireAuthorizationLayer, axum_sessions::async_session::{MemoryStore}, extractors::AuthContext, AuthLayer, SqlxStore};
use rand::{Rng, thread_rng};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres, FromRow};
use axum_login::axum_sessions::SessionLayer;

#[derive(Deserialize, Serialize, Debug, Clone, FromRow)]
pub struct KnotUser {
    pub id: i32,
    pub name: String,
    pub password_hash: String
}

impl AuthUser<i32, ()> for KnotUser {
    fn get_id(&self) -> i32 {
        self.id
    }

    fn get_password_hash(&self) -> SecretVec<u8> {
        SecretVec::new(self.password_hash.clone().into_bytes())
    }
}

pub type Store = PostgresStore<KnotUser>;
pub type KnotAuthContext = AuthContext<i32, KnotUser, Store>;
pub type RequireAuthLayer = RequireAuthorizationLayer<i32, KnotUser>;

pub async fn get_layers (pool: &Pool<Postgres>) -> (AuthLayer<SqlxStore<Pool<Postgres>, KnotUser, ()>, i32, KnotUser, ()>, SessionLayer<MemoryStore>) {
    let secret = {
        let mut rng = thread_rng();

        [rng.gen::<[u8; 32]>(), rng.gen::<[u8; 32]>()].concat()
    };

    let session_store = MemoryStore::new();
    let session_layer = SessionLayer::new(session_store, &secret);

    let user_store = Store::new(pool.clone());
    let auth_layer = AuthLayer::new(user_store, &secret);

    (auth_layer, session_layer)
}