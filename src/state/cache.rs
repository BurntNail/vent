use crate::error::{FileIdentifier, IOAction, VentError};
use moka::future::{Cache, CacheBuilder};
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

    pub async fn get(&mut self, path: impl AsRef<Path>) -> Result<Arc<str>, VentError> {
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

    pub async fn pre_populate(&mut self) {
        todo!()
    }
}
