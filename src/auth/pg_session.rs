use crate::error::{KnotError, SerdeJsonAction, SerdeJsonSnafu, SqlxAction, SqlxSnafu};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use serde_json::from_slice;
use snafu::ResultExt;
use sqlx::{Pool, Postgres};
use tower_sessions::{session::Id, ExpiredDeletion, Session, SessionStore};

#[derive(Clone, Debug)]
pub struct PostgresStore {
    pool: Pool<Postgres>,
}

impl PostgresStore {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ExpiredDeletion for PostgresStore {
    async fn delete_expired(&self) -> Result<(), Self::Error> {
        sqlx::query!("DELETE FROM public.sessions where expiry_date < (now())")
            .execute(&self.pool)
            .await
            .map(|_| ())
            .context(SqlxSnafu {
                action: SqlxAction::DeletingOldSessions,
            })
    }
}

#[async_trait]
impl SessionStore for PostgresStore {
    type Error = KnotError;

    async fn save(&self, session: &Session) -> Result<(), Self::Error> {
        let session_data = serde_json::to_vec(&session).context(SerdeJsonSnafu {
            action: SerdeJsonAction::SessionSerde,
        })?;
        let session_id = &session.id().to_string();
        let session_expiry = session.expiry_date();
        let session_expiry = NaiveDateTime::new(
            NaiveDate::from_ymd_opt(
                session_expiry.year(),
                session_expiry.month() as u32,
                session_expiry.day() as u32,
            )
            .expect("poorly formatted date from OffsetDateTime"),
            NaiveTime::from_hms_opt(
                session_expiry.hour() as u32,
                session_expiry.minute() as u32,
                session_expiry.second() as u32,
            )
            .expect("poorly formatted time from OffsetDateTime"),
        );

        sqlx::query!(
            r#"
        INSERT INTO public.sessions (id, data, expiry_date)
        VALUES ($1, $2, $3)
        on conflict (id) do update
            set
              data = excluded.data,
              expiry_date = excluded.expiry_date

        "#,
            session_id,
            session_data,
            session_expiry.into()
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
        .context(SqlxSnafu {
            action: SqlxAction::AddingSession,
        })
    }

    async fn load(&self, id: &Id) -> Result<Option<Session>, Self::Error> {
        let session_id = id.to_string();
        let json = sqlx::query!(
            "SELECT * FROM sessions WHERE id = $1 and expiry_date > now()",
            session_id
        )
        .fetch_optional(&mut *self.pool.acquire().await.context(SqlxSnafu {
            action: SqlxAction::AcquiringConnection,
        })?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingSession(*id),
        })?;

        Ok(if let Some(json) = json {
            let fv = from_slice::<Session>(&json.data).context(SerdeJsonSnafu {
                action: SerdeJsonAction::SessionSerde,
            })?;
            Some(fv)
        } else {
            None
        })
    }

    async fn delete(&self, session_id: &Id) -> Result<(), Self::Error> {
        sqlx::query!(
            "DELETE FROM public.sessions WHERE id = $1",
            session_id.to_string()
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
        .context(SqlxSnafu {
            action: SqlxAction::RemovingSession(*session_id),
        })
    }
}
