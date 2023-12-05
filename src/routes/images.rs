use crate::{
    auth::backend::{Auth, KnotAuthBackend},
    error::{
        ConvertingWhatToString, DatabaseIDMethod, IOAction, IOSnafu, ImageAction, ImageSnafu,
        JoinSnafu, KnotError, MissingExtensionSnafu, NoImageExtensionSnafu, SqlxAction, SqlxSnafu,
        ThreadReason, ToStrSnafu, UnknownMIMESnafu,
    },
    routes::public::{serve_read, serve_static_file},
    state::KnotState,
};
use async_zip::{tokio::write::ZipFileWriter, Compression, ZipEntryBuilder};
use axum::{
    extract::{Multipart, Path, State},
    response::{IntoResponse, Redirect},
    routing::{get, post},
    Router,
};
use axum_login::login_required;
use new_mime_guess::MimeGuess;
use rand::{random, thread_rng, Rng};
use snafu::{OptionExt, ResultExt};
use std::{ffi::OsStr, path::PathBuf};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
};
use walkdir::WalkDir;

#[axum::debug_handler]
async fn post_add_photo(
    auth: Auth,
    Path(event_id): Path<i32>,
    State(state): State<KnotState>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, KnotError> {
    debug!("Zeroing old zip file");
    sqlx::query!(
        r#"
UPDATE events
SET zip_file = NULL
WHERE id = $1"#,
        event_id
    )
    .execute(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::UpdatingEvent(event_id),
    })?;

    let user_id = auth.user.unwrap().id;

    while let Some(field) = multipart.next_field().await? {
        debug!("Getting bytes");
        let data = field.bytes().await?;

        debug!(data_len = %data.len(), "Getting format/ext");

        let format = image::guess_format(&data).context(ImageSnafu {
            action: ImageAction::GuessingFormat,
        })?;
        let ext = format
            .extensions_str()
            .first()
            .context(NoImageExtensionSnafu { extension: format })?;

        debug!("Finding file name");

        let file_name = loop {
            let key = format!("uploads/{:x}.{ext}", random::<u128>());
            if sqlx::query!(
                r#"
    SELECT * FROM photos
    WHERE path = $1
            "#,
                &key
            )
            .fetch_optional(&mut *state.get_connection().await?)
            .await
            .with_context(|_| SqlxSnafu {
                action: SqlxAction::FindingPhotos(DatabaseIDMethod::Path(key.clone().into())),
            })?
            .is_none()
            {
                break key;
            }
            trace!(file_name=?key, "Found");
        };

        debug!(?file_name, "Adding photo to DB");

        sqlx::query!(
            r#"
INSERT INTO public.photos
("path", event_id, added_by)
VALUES($1, $2, $3)"#,
            file_name,
            event_id,
            user_id,
        )
        .execute(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::AddingPhotos,
        })?;

        debug!("Writing to file");

        let mut file = File::create(&file_name).await.context(IOSnafu {
            action: IOAction::CreatingFile(file_name.into()),
        })?;
        file.write_all(&data).await.context(IOSnafu {
            action: IOAction::WritingToFile,
        })?;
    }

    Ok(Redirect::to(&format!("/update_event/{event_id}")))
}

#[axum::debug_handler]
async fn serve_image(Path(img_path): Path<String>) -> Result<impl IntoResponse, KnotError> {
    debug!("Getting path/ext");

    let path = PathBuf::from(img_path.as_str());
    let cloned = path.clone();
    let ext = cloned.extension().context(MissingExtensionSnafu {
        was_looking_for: path.clone(),
    })?;
    let ext = ext.to_str().context(ToStrSnafu {
        what: ConvertingWhatToString::PathBuffer(path.clone()),
    })?;

    debug!("Getting body");

    let short_path = format!("uploads/{img_path}");
    let file = File::open(&short_path).await.context(IOSnafu {
        action: IOAction::OpeningFile(short_path.clone().into()),
    })?;

    serve_read(
        MimeGuess::from_ext(ext)
            .first()
            .context(UnknownMIMESnafu { path })?,
        file,
        IOAction::ReadingFile(short_path.into()),
    )
    .await
}

#[axum::debug_handler]
async fn get_all_images(
    Path(event_id): Path<i32>,
    State(state): State<KnotState>,
) -> Result<impl IntoResponse, KnotError> {
    debug!(%event_id, "Checking for existing zip");
    if let Some(file_name) = sqlx::query!(
        r#"
SELECT zip_file
FROM events
WHERE id = $1"#,
        event_id
    )
    .fetch_one(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::FindingEvent(event_id),
    })?
    .zip_file
    {
        debug!(?file_name, %event_id, "Found existing zip file");
        return serve_static_file(file_name).await;
    }
    trace!(%event_id, "Creating new zip file");

    let files_to_find = sqlx::query!(
        r#"
SELECT path FROM public.photos
WHERE event_id = $1"#,
        event_id
    )
    .fetch_all(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::FindingPhotos(event_id.into()),
    })?
    .into_iter()
    .map(|x| x.path);

    let file_name = {
        fn get_existing() -> Vec<String> {
            let zip_ext = OsStr::new("zip");
            let mut files = vec![];

            for de in WalkDir::new("uploads").into_iter().filter_map(Result::ok) {
                match de.file_name().to_str().context(ToStrSnafu {
                    what: ConvertingWhatToString::FileName(de.file_name().to_os_string()),
                }) {
                    Ok(file_name) => {
                        if de.path().extension().map_or(false, |e| e == zip_ext) {
                            files.push(file_name.to_string());
                        }
                    }
                    Err(e) => warn!("{e:?}"),
                }
            }

            files
        }

        let existing = tokio::task::spawn_blocking(get_existing)
            .await
            .context(JoinSnafu {
                title: ThreadReason::FindingExistingFilesWithWalkDir,
            })?;

        let mut rng = thread_rng();
        format!(
            "uploads/{}",
            loop {
                let key = format!("{}.zip", rng.gen::<u128>());
                if !existing.contains(&key) {
                    break key;
                }
                trace!(file_name=?key, "Failed on");
            }
        )
    };

    debug!("Creating FS stuff");

    let mut file = File::create(&file_name).await.context(IOSnafu {
        action: IOAction::CreatingFile(file_name.clone().into()),
    })?;
    let mut writer = ZipFileWriter::with_tokio(&mut file);

    let mut data = vec![];
    let mut buf = [0; 1024];

    for file_path in files_to_find {
        debug!(?file_path, "Opening file");
        let mut file = File::open(&file_path).await.context(IOSnafu {
            action: IOAction::OpeningFile(file_name.clone().into()),
        })?;

        debug!("Reading file");

        loop {
            match file.read(&mut buf).await.context(IOSnafu {
                action: IOAction::ReadingFile(file_name.clone().into()),
            })? {
                0 => break,
                n => {
                    trace!(%n, "Got bytes");
                    data.extend(&buf[0..n]);
                }
            }
        }

        debug!("Writing to zip file");

        writer
            .write_entry_whole(
                ZipEntryBuilder::new(file_path.into(), Compression::Deflate),
                &data,
            )
            .await?;
        data.clear();
    }

    debug!("Closing file");

    writer.close().await?;
    drop(file);

    debug!("Updating SQL");

    sqlx::query!(
        r#"
UPDATE events
SET zip_file = $1
WHERE id = $2"#,
        &file_name,
        event_id
    )
    .execute(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::UpdatingEvent(event_id),
    })?;

    debug!("Serving");

    serve_static_file(file_name).await
}

pub fn router() -> Router<KnotState> {
    Router::new()
        .route("/add_image/:id", post(post_add_photo))
        .route("/get_all_imgs/:event_id", get(get_all_images))
        .route("/uploads/:img", get(serve_image))
        .route_layer(login_required!(KnotAuthBackend, login_url = "/login"))
}
