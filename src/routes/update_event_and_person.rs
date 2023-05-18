use std::{sync::Arc, path::PathBuf, ffi::{OsStr}};
use crate::{
    error::KnotError,
    liquid_utils::compile,
    routes::{DbEvent, Person},
};
use async_zip::{tokio::write::ZipFileWriter, ZipEntryBuilder, Compression};
use axum::{
    extract::{Multipart, Path, State},
    response::{IntoResponse, Redirect}, http::header, body::StreamBody,
};
use axum_extra::extract::Form;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use tokio::{fs::File, io::{AsyncWriteExt, AsyncReadExt}};
use tokio_util::io::ReaderStream;
use walkdir::WalkDir;

use super::FormEvent;

pub async fn get_update_event(
    Path(event_id): Path<i32>,
    State(pool): State<Arc<Pool<Postgres>>>,
) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;

    let DbEvent {
        id,
        event_name,
        date,
        location,
        teacher,
        other_info,
    } = sqlx::query_as!(
        DbEvent,
        r#"
SELECT * FROM events WHERE id = $1
"#,
        event_id
    )
    .fetch_one(&mut conn)
    .await?;
    let date = date.to_string();

    #[derive(Deserialize, Serialize, Debug)]
    struct PersonPlusRelID {
        pub id: i32,
        pub person_name: String,
        pub relation_id: i32,
    }

    let existing_prefects = sqlx::query_as!(
        PersonPlusRelID,
        r#"
SELECT p.person_name, pe.relation_id, p.id
FROM people p
INNER JOIN prefect_events pe ON pe.event_id = $1 AND pe.prefect_id = p.id"#,
        event_id
    )
    .fetch_all(&mut conn)
    .await?;

    let existing_participants = sqlx::query_as!(
        PersonPlusRelID,
        r#"
SELECT p.person_name, pe.relation_id, p.id
FROM people p
INNER JOIN participant_events pe ON pe.event_id = $1 AND pe.participant_id = p.id"#,
        event_id
    )
    .fetch_all(&mut conn)
    .await?;

    let possible_prefects = sqlx::query_as!(
        Person,
        r#"
SELECT p.id, p.person_name, p.is_prefect
FROM people p
WHERE p.is_prefect = true"#
    )
    .fetch_all(&mut conn)
    .await?
    .into_iter()
    .filter(|p| !existing_prefects.iter().any(|e| e.id == p.id))
    .collect::<Vec<_>>();
    let possible_participants = sqlx::query_as!(
        Person,
        r#"
SELECT p.id, p.person_name, p.is_prefect
FROM people p
"#
    )
    .fetch_all(&mut conn)
    .await?
    .into_iter()
    .filter(|p| !existing_participants.iter().any(|e| e.id == p.id))
    .collect::<Vec<_>>();

    let photos: Vec<String> = sqlx::query!(
        r#"
SELECT * FROM photos
WHERE event_id = $1
        "#,
        event_id
    )
    .fetch_all(&mut conn)
    .await?
    .into_iter()
    .map(|x| x.path)
    .collect();

    let globals = liquid::object!({
        "event": liquid::object!({
            "id": id,
            "event_name": event_name,
            "date": date.to_string(),
            "location": location,
            "teacher": teacher,
            "other_info": other_info.unwrap_or_default()
        }),
        "existing_prefects": existing_prefects,
        "existing_participants": existing_participants,
        "prefects": possible_prefects,
        "participants": possible_participants,
        "imgs": photos
    });

    compile("www/update_event.liquid", globals).await
}
pub async fn post_update_event(
    Path(event_id): Path<i32>,
    State(pool): State<Arc<Pool<Postgres>>>,
    Form(event): Form<FormEvent>,
) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;

    let DbEvent {
        id: _id,
        event_name,
        date,
        location,
        teacher,
        other_info,
    } = DbEvent::try_from(event)?;
    let other_info = other_info.unwrap_or_default();

    sqlx::query!(
        r#"
UPDATE public.events
SET event_name=$2, date=$3, location=$4, teacher=$5, other_info=$6
WHERE id=$1
        "#,
        event_id,
        event_name,
        date,
        location,
        teacher,
        other_info
    )
    .execute(&mut conn)
    .await?;

    Ok(Redirect::to(&format!("/update_event/{event_id}")))
}

pub async fn get_remove_prefect_from_event(
    Path(relation_id): Path<i32>,
    State(pool): State<Arc<Pool<Postgres>>>,
) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;

    let id = sqlx::query!(
        r#"
DELETE FROM prefect_events WHERE relation_id = $1 
RETURNING event_id
"#,
        relation_id
    )
    .fetch_one(&mut conn)
    .await?
    .event_id;

    Ok(Redirect::to(&format!("/update_event/{id}")))
}
pub async fn get_remove_participant_from_event(
    Path(relation_id): Path<i32>,
    State(pool): State<Arc<Pool<Postgres>>>,
) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;

    let id = sqlx::query!(
        r#"
DELETE FROM participant_events WHERE relation_id = $1 
RETURNING event_id
"#,
        relation_id
    )
    .fetch_one(&mut conn)
    .await?
    .event_id;

    Ok(Redirect::to(&format!("/update_event/{id}")))
}

#[axum::debug_handler]
pub async fn post_add_photo(
    Path(event_id): Path<i32>,
    State(pool): State<Arc<Pool<Postgres>>>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, KnotError> {
    let field = multipart
        .next_field()
        .await?
        .ok_or(KnotError::MissingFormData)?;

    let data = field.bytes().await?;

    let format = image::guess_format(&data)?;
    let ext = format
        .extensions_str().first()
        .ok_or(KnotError::NoImageExtension(format))?;

    let mut conn = pool.acquire().await?;

    let file_name = loop {
        let key = format!("uploads/{:x}.{ext}", thread_rng().gen::<u128>());
        if sqlx::query!(
            r#"
    SELECT * FROM photos
    WHERE path = $1
            "#,
            key
        )
        .fetch_optional(&mut conn)
        .await?
        .is_none()
        {
            break key;
        }
    };

    sqlx::query!(
        r#"
INSERT INTO public.photos
("path", event_id)
VALUES($1, $2)"#,
        file_name,
        event_id
    )
    .execute(&mut conn)
    .await?;

    let mut file = File::create(file_name).await?;
    file.write_all(&data).await?;

    Ok(Redirect::to(&format!("/update_event/{event_id}")))
}

pub async fn serve_image (Path(img_path): Path<String>) -> Result<impl IntoResponse, KnotError> {
    let path = PathBuf::from(img_path.as_str());
    let ext = path.extension().ok_or_else(|| KnotError::MissingFile(img_path.clone()))?.to_str().ok_or(KnotError::InvalidUTF8)?;
    let body = StreamBody::new(ReaderStream::new(File::open(format!("uploads/{img_path}")).await?));

    let headers = [
        (header::CONTENT_TYPE, format!("image/{ext}")),
        (
            header::CONTENT_DISPOSITION,
            format!("filename=\"{img_path}\""),
        ),
    ];

    Ok((headers, body))
}

#[axum::debug_handler]
pub async fn get_all_images (Path(event_id): Path<i32>, State(pool): State<Arc<Pool<Postgres>>>) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;

    let files_to_find = sqlx::query!(
        r#"
SELECT path FROM public.photos
WHERE event_id = $1"#,
        event_id
    ).fetch_all(&mut conn).await?.into_iter().map(|x| x.path);

    drop(conn);
    info!("Got ftf");

    let file_name = {
        fn get_existing () -> Vec<String> {
            let zip_ext = OsStr::new("zip");
            let mut files = vec![];
            
            for de in WalkDir::new("uploads").into_iter().filter_map(Result::ok) {
                if let Ok(file_name) = de.file_name().to_str().ok_or(KnotError::InvalidUTF8) {
                    if de.path().extension().map_or(false, |e| e == zip_ext) {
                        files.push(file_name.to_string());
                    }
                }
            }

            files
        }

        let existing = tokio::task::spawn_blocking(get_existing).await?;

        info!(?existing, "Got existing files");

        let mut rng = thread_rng();
        format!("uploads/{}", loop {
            let key = format!("{}.zip", rng.gen::<u128>());
            if !existing.contains(&key) {
                break key;
            }
        })
    };
    info!("Got file nam");

    
    let mut file = File::create(&file_name).await?;
    let mut writer = ZipFileWriter::with_tokio(&mut file);

    let mut data = vec![];
    let mut buf = [0; 1024];

    for file_path in files_to_find {
        let mut file = File::open(&file_path).await?;

        loop {
            match file.read(&mut buf).await? {
                0 => break,
                n => data.extend(&buf[0..n]),
            }
        }

        writer.write_entry_whole(ZipEntryBuilder::new(file_path.into(), Compression::Deflate), &data).await.map_err(KnotError::Zip)?;
        data.clear();
    }

    info!("Added files");


    writer.close().await.map_err(KnotError::Zip)?;
    drop(file);



    let body = StreamBody::new(ReaderStream::new(File::open(&file_name).await?));

    let headers = [
        (header::CONTENT_TYPE, "application/zip".into()),
        (
            header::CONTENT_DISPOSITION,
            format!("filename=\"{file_name}\""),
        ),
    ];

    Ok((headers, body))
}