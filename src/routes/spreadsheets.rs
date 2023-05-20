use std::{collections::HashMap, sync::Arc};

use axum::{body::StreamBody, extract::State, http::header, response::IntoResponse};
use csv_async::AsyncWriter;
use sqlx::{Pool, Postgres};
use tokio::fs::File;
use tokio_util::io::ReaderStream;

use crate::{error::KnotError, routes::DbPerson};

pub async fn get_spreadsheet(
    State(pool): State<Arc<Pool<Postgres>>>,
) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;
    let people = sqlx::query_as!(
        DbPerson,
        r#"
SELECT * FROM people"#
    )
    .fetch_all(&mut conn)
    .await?;

    let prefect_relationships: HashMap<i32, usize> = {
        let mut map = HashMap::new();
        sqlx::query!("SELECT prefect_id FROM prefect_events")
            .fetch_all(&mut conn)
            .await?
            .into_iter()
            .for_each(|x| *map.entry(x.prefect_id).or_insert(0_usize) += 1);
        map
    };
    let participant_relationships: HashMap<i32, usize> = {
        let mut map = HashMap::new();
        sqlx::query!("SELECT participant_id FROM participant_events")
            .fetch_all(&mut conn)
            .await?
            .into_iter()
            .for_each(|x| *map.entry(x.participant_id).or_insert(0_usize) += 1);
        map
    };

    let mut writer = AsyncWriter::from_writer(File::create("student_spreadsheet.csv").await?);
    writer
        .write_record(&[
            "First Name",
            "Surname",
            "Form",
            "House Events",
            "House Events supervised",
        ])
        .await?;

    for DbPerson {
        first_name,
        surname,
        is_prefect: _,
        id,
        form,
    } in people
    {
        //NB: originally had fun stuff with SQL but way faster to cache in rust
        let tbw = &[
            first_name,
            surname,
            form,
            participant_relationships.get(&id).unwrap_or(&0).to_string(),
            prefect_relationships.get(&id).unwrap_or(&0).to_string(),
        ];
        writer.write_record(tbw).await?;
    }

    //not sure why, but need to re-read the file for this to work
    writer.flush().await?;
    drop(writer);
    let body = StreamBody::new(ReaderStream::new(
        File::open("student_spreadsheet.csv").await?,
    ));
    let headers = [
        (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
        (
            header::CONTENT_DISPOSITION,
            "filename=\"student_spreadsheet.csv\"",
        ),
    ];

    Ok((headers, body))
}
