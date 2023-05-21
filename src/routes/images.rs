use crate::error::KnotError;
use async_zip::{tokio::write::ZipFileWriter, Compression, ZipEntryBuilder};
use axum::{
    body::StreamBody,
    extract::{Multipart, Path, State},
    http::header,
    response::{IntoResponse, Redirect},
};
use rand::{thread_rng, Rng};
use sqlx::{Pool, Postgres};
use std::{ffi::OsStr, path::PathBuf, sync::Arc};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
};
use tokio_util::io::ReaderStream;
use walkdir::WalkDir;

#[axum::debug_handler]
pub async fn post_add_photo(
    Path(event_id): Path<i32>,
    State(pool): State<Arc<Pool<Postgres>>>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, KnotError> {
    while let Some(mut field) = multipart.next_field().await? {
        let data = field.bytes().await?;

        let format = image::guess_format(&data)?;
        let ext = format
            .extensions_str()
            .first()
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
    }

    Ok(Redirect::to(&format!("/update_event/{event_id}")))
}

pub async fn serve_image(Path(img_path): Path<String>) -> Result<impl IntoResponse, KnotError> {
    let path = PathBuf::from(img_path.as_str());
    let ext = path
        .extension()
        .ok_or_else(|| KnotError::MissingFile(img_path.clone()))?
        .to_str()
        .ok_or(KnotError::InvalidUTF8)?;
    let body = StreamBody::new(ReaderStream::new(
        File::open(format!("uploads/{img_path}")).await?,
    ));

    let headers = [
        (header::CONTENT_TYPE, format!("image/{ext}")),
        (
            header::CONTENT_DISPOSITION,
            format!("filename=\"{img_path}\""),
        ),
    ];

    Ok((headers, body))
}

pub async fn get_all_images(
    Path(event_id): Path<i32>,
    State(pool): State<Arc<Pool<Postgres>>>,
) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;

    let files_to_find = sqlx::query!(
        r#"
SELECT path FROM public.photos
WHERE event_id = $1"#,
        event_id
    )
    .fetch_all(&mut conn)
    .await?
    .into_iter()
    .map(|x| x.path);

    drop(conn);

    let file_name = {
        fn get_existing() -> Vec<String> {
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

        let mut rng = thread_rng();
        format!(
            "uploads/{}",
            loop {
                let key = format!("{}.zip", rng.gen::<u128>());
                if !existing.contains(&key) {
                    break key;
                }
            }
        )
    };

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

        writer
            .write_entry_whole(
                ZipEntryBuilder::new(file_path.into(), Compression::Deflate),
                &data,
            )
            .await?;
        data.clear();
    }

    writer.close().await?;
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
