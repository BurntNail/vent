use crate::{
    auth::backend::{Auth, VentAuthBackend},
    error::{
        IOAction, IOSnafu, ImageAction, ImageSnafu,
        NoImageExtensionSnafu, SqlxAction, SqlxSnafu,
        VentError,
    },
    image_format::ImageFormat,
    routes::public::{serve_static_file_from_s3},
    state::VentState,
};
use axum::{
    extract::{Multipart, Path, State},
    response::{IntoResponse, Redirect},
    routing::{get, post},
    Router,
};
use axum_login::login_required;
use rand::{random, thread_rng, Rng};
use snafu::{OptionExt, ResultExt};
use std::io::Write;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

#[axum::debug_handler]
async fn post_add_photo(
    auth: Auth,
    Path(event_id): Path<i32>,
    State(state): State<VentState>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, VentError> {
    debug!("Zeroing old zip file");
    if let Some(old_zip_file) = sqlx::query!(r#"SELECT zip_file FROM events WHERE id = $1"#, event_id)
        .fetch_optional(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::GettingEvent(event_id)
        })?
        .and_then(|x| x.zip_file) {
        state.bucket.delete_file(old_zip_file).await?;

        sqlx::query!(
        r#"
UPDATE events
SET zip_file = NULL
WHERE id = $1
"#,
        event_id
    )
            .execute(&mut *state.get_connection().await?)
            .await
            .context(SqlxSnafu {
                action: SqlxAction::UpdatingEvent(event_id),
            })?;
    }
    

    let user_id = auth.user.unwrap().id;

    while let Some(field) = multipart.next_field().await? {
        debug!("Getting bytes");
        let data = field.bytes().await?;

        debug!(data_len = %data.len(), "Getting format/ext");

        let format = ImageFormat::guess_format(&data).context(ImageSnafu {
            action: ImageAction::GuessingFormat,
        })?;
        let ext = format
            .extensions_str()
            .first()
            .context(NoImageExtensionSnafu { extension: format })?;

        debug!("Finding file name");
        
        let existing_names = state.bucket.list_files("uploads".into()).await?;
        
        info!(?existing_names);

        let file_name = loop {
            let key = format!("uploads/{:x}.{ext}", random::<u128>());
            if !existing_names.contains(&key) {
                break key;
            }
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
        
        state.bucket.write_file(file_name, data, format.to_mime_type()).await?;
    }

    Ok(Redirect::to(&format!("/update_event/{event_id}")))
}

#[axum::debug_handler]
async fn serve_image(Path(img_path): Path<String>, State(state): State<VentState>) -> Result<impl IntoResponse, VentError> {
    let short_path = format!("uploads/{img_path}");
    serve_static_file_from_s3(short_path, &state).await
}

#[axum::debug_handler]
async fn get_all_images(
    Path(event_id): Path<i32>,
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
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
        action: SqlxAction::GettingEvent(event_id),
    })?
    .zip_file
    {
        debug!(?file_name, %event_id, "Found existing zip file");
        return serve_static_file_from_s3(file_name, &state).await;
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

    let file_name: String = {
        let existing = state.bucket.list_files("zips".into()).await?;

        let mut rng = thread_rng();
        format!(
            "zips/{}",
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

    let mut buffer = vec![];
    let mut writer = ZipWriter::new(std::io::Cursor::new(&mut buffer));

    for file_path in files_to_find {
        let options = SimpleFileOptions::default().compression_level(None);
        writer.start_file(&file_path, options)?;

        let contents = state.bucket.read_file(&file_path).await?;
        writer.write_all(&contents).context(IOSnafu {
            action: IOAction::WritingToZip
        })?;
    }
    
    let cursor = writer.finish()?;
    drop(cursor);

    state.bucket.write_file(&file_name, buffer, "application/zip").await?;

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

    serve_static_file_from_s3(file_name, &state).await
}

pub fn router() -> Router<VentState> {
    Router::new()
        .route("/add_image/:id", post(post_add_photo))
        .route("/get_all_imgs/:event_id", get(get_all_images))
        .route("/uploads/:img", get(serve_image))
        .route_layer(login_required!(VentAuthBackend, login_url = "/login"))
}
