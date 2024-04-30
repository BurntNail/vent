use crate::error::{ComponentRangeSnafu, SerdeJsonAction, SerdeJsonSnafu, SqlxAction, SqlxSnafu};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use serde_json::from_slice;
use snafu::ResultExt;
use sqlx::{Pool, Postgres};
use time::OffsetDateTime;
use tower_sessions::{
    session::{Id, Record},
    session_store::Error as SSError,
    ExpiredDeletion, SessionStore,
};

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
    async fn delete_expired(&self) -> Result<(), SSError> {
        sqlx::query!("DELETE FROM public.sessions where expiry_date < (now())")
            .execute(&self.pool)
            .await
            .context(SqlxSnafu {
                action: SqlxAction::DeletingOldSessions,
            })?;
        Ok(())
    }
}

#[async_trait]
impl SessionStore for PostgresStore {
    async fn create(&self, session: &mut Record) -> Result<(), SSError> {
        let mut session_id = session.id;

        while sqlx::query!(
            "SELECT * FROM public.sessions WHERE id = $1",
            session_id.to_string()
        )
        .fetch_optional(&self.pool)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::AddingSession,
        })?
        .is_some()
        {
            session_id = Id::default();
        }

        session.id = session_id;

        self.save(session).await
    }

    async fn save(&self, session: &Record) -> Result<(), SSError> {
        let session_data = serde_json::to_vec(&session.data).context(SerdeJsonSnafu {
            action: SerdeJsonAction::SessionSerde,
        })?;
        let session_id = &session.id.to_string();
        let session_expiry = session.expiry_date;
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
        .context(SqlxSnafu {
            action: SqlxAction::AddingSession,
        })?;

        Ok(())
    }

    async fn load(&self, id: &Id) -> Result<Option<Record>, SSError> {
        let id = *id;

        let session_id = id.to_string();
        let rec = sqlx::query!(
            "SELECT * FROM sessions WHERE id = $1 and expiry_date > now()",
            session_id
        )
        .fetch_optional(&mut *self.pool.acquire().await.context(SqlxSnafu {
            action: SqlxAction::AcquiringConnection,
        })?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingSession(id),
        })?;

        Ok(if let Some(rec) = rec {
            let expiry_date =
                OffsetDateTime::from_unix_timestamp(rec.expiry_date.and_utc().timestamp())
                    .context(ComponentRangeSnafu {
                        naive: rec.expiry_date,
                    })?;

            let data = from_slice(&rec.data).context(SerdeJsonSnafu {
                action: SerdeJsonAction::SessionSerde,
            })?;
            Some(Record {
                id,
                data,
                expiry_date,
            })
        } else {
            None
        })
    }

    async fn delete(&self, session_id: &Id) -> Result<(), SSError> {
        sqlx::query!(
            "DELETE FROM public.sessions WHERE id = $1",
            session_id.to_string()
        )
        .execute(&self.pool)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::RemovingSession(*session_id),
        })?;

        Ok(())
    }
}
