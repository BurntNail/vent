use std::sync::Arc;

use axum::{response::IntoResponse, body::StreamBody, http::header, extract::State};
use chrono::Duration;
use icalendar::{Calendar, Event, Component, EventLike};
use sqlx::{Postgres, Pool};
use tokio::{fs::File, io::AsyncWriteExt};
use tokio_util::io::ReaderStream;
use crate::{error::KnotError, routes::DbEvent};

pub const LOCATION: &str = "/ical";

pub async fn get_calendar_feed(State(pool): State<Arc<Pool<Postgres>>>) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;

    let mut calendar = Calendar::new();
    for DbEvent { id: _, event_name, date, location, teacher, other_info } in sqlx::query_as!(
        DbEvent,
        r#"SELECT * FROM events"#
    ).fetch_all(&mut conn).await? {
        let other_info = other_info.unwrap_or_default();

        calendar.push(
            Event::new()
                .summary(&event_name)
                .starts(date)
                .ends(date + Duration::minutes(45))
                .description(&format!(r#"
Location: {location}
Teacher: {teacher}
Other Information: {other_info}"#))
                .done()
        );
    }

    {
        let mut local_file = File::create("calendar.ics").await?;
        local_file.write_all(calendar.done().to_string().as_bytes()).await?;
    }



    //not sure why, but need to re-read the file for this to work
    let body = StreamBody::new(ReaderStream::new(File::open("calendar.ics").await?));
    let headers = [
        (header::CONTENT_TYPE, "text/calendar; charset=utf-8"),
        (
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"calendar.ics\"",
        ),
    ];

    Ok((headers, body))
}