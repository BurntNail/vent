use crate::error::{FileIdentifier, IOAction, IOSnafu, VentError};
use async_walkdir::WalkDir;
use futures::StreamExt;
use moka::future::{Cache, CacheBuilder};
use snafu::ResultExt;
use std::{
    io::{Error as IOError, ErrorKind},
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::fs::read_to_string;

#[derive(Clone, Debug)]
pub struct VentCache {
    templates_cache: Cache<PathBuf, Arc<str>>,
}

impl VentCache {
    pub fn new() -> Self {
        let templates_cache = CacheBuilder::default().name("templates_cache").build();

        Self { templates_cache }
    }

    pub async fn get(&self, path: impl AsRef<Path>) -> Result<Arc<str>, VentError> {
        let path = path.as_ref().to_path_buf();
        if !path.exists() {
            return Err(VentError::IO {
                source: IOError::from(ErrorKind::NotFound),
                action: IOAction::ReadingFile(FileIdentifier::PB(path)),
            });
        }

        if let Some(found) = self.templates_cache.get(&path).await {
            return Ok(found);
        }

        let read_in = read_to_string(path.clone())
            .await
            .map_err(|source| VentError::IO {
                source,
                action: IOAction::ReadingFile(FileIdentifier::PB(path.clone())),
            })?;
        let read_in: Arc<str> = read_in.into();
        self.templates_cache.insert(path, read_in.clone()).await;

        Ok(read_in)
    }

    pub async fn pre_populate(&self) {
        let des: Vec<_> = WalkDir::new("www/").collect().await;

        for path in des.into_iter().filter_map(Result::ok).map(|x| x.path()) {
            let contents = read_to_string(path.clone())
                .await
                .with_context(|_e| IOSnafu {
                    action: IOAction::ReadingFile(FileIdentifier::PB(path.clone())),
                });
            let contents: Arc<str> = match contents {
                Ok(c) => c.into(),
                Err(e) => {
                    warn!(?e, ?path, "Error reading file for pre-population");
                    continue;
                }
            };

            self.templates_cache.insert(path, contents).await;
        }
    }

    pub fn clear(&self) {
        self.templates_cache.invalidate_all();
    }
}
