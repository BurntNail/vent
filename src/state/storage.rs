use std::env;
use tokio::fs::File;
use s3::{Bucket, Region};
use s3::creds::Credentials;
use snafu::ResultExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::error::{IOAction, IOSnafu, S3Action, S3Snafu, VentError};

#[derive(Clone, Debug)]
pub enum VentStorage {
    S3(Box<Bucket>),
    Local
}

impl VentStorage {
    pub fn new () -> Self {
        fn get_bucket () -> Option<Box<Bucket>> {
            let aws_creds = {
                let access_key = env::var("AWS_ACCESS_KEY_ID").ok()?;
                let secret_key =
                    env::var("AWS_SECRET_ACCESS_KEY").ok()?;

                Credentials::new(Some(&access_key), Some(&secret_key), None, None, None).ok().expect("unable to create S3 credentials")
            };
            let bucket_name = env::var("BUCKET_NAME").ok()?;
            let endpoint = env::var("AWS_ENDPOINT_URL_S3").ok()?;
            let region = Region::Custom {
                region: "auto".to_owned(),
                endpoint,
            };
            let bucket = Bucket::new(&bucket_name, region, aws_creds).expect("unable to connect to S3 bucket");

            Some(bucket)
        }

        match get_bucket() {
            Some(x) => {
                info!("Using S3 storage");
                Self::S3(x)
            },
            None => {
                info!("Using local storage");
                Self::Local
            }
        }
    }

    pub async fn write_file (&self, file_name: impl AsRef<str>, contents: impl AsRef<[u8]>, content_type: &str) -> Result<(), VentError> {
        let file_name = file_name.as_ref();

        match self {
            Self::S3(bucket) => {
                bucket.put_object_with_content_type(&file_name, contents.as_ref(), content_type).await.context(S3Snafu { action: S3Action::PuttingFile(file_name.to_string()) })?;
            }
            Self::Local => {
                let mut file = File::create(&file_name).await.context(IOSnafu {
                    action: IOAction::CreatingFile(file_name.to_string().into())
                })?;
                file.write_all(contents.as_ref()).await.context(IOSnafu {
                    action: IOAction::WritingToFile(file_name.to_string().into())
                })?;
            }
        }

        Ok(())
    }

    pub async fn read_file (&self, file_name: impl AsRef<str>) -> Result<Vec<u8>, VentError> {
        let file_name = file_name.as_ref();
        match self {
            Self::S3(bucket) => {
                let response = bucket.get_object(&file_name).await.context(S3Snafu { action: S3Action::GettingFile(file_name.to_string()) })?;
                Ok(response.to_vec())
            }
            Self::Local => {
                let mut file = File::open(&file_name).await.context(IOSnafu {
                    action: IOAction::CreatingFile(file_name.to_string().into())
                })?;

                let mut output = vec![];
                let mut buf = [0_u8; 1024];

                loop {
                    match file.read(&mut buf).await.context(IOSnafu {
                        action: IOAction::ReadingFile(file_name.to_string().into())
                    })? {
                        0 => break,
                        n => output.extend_from_slice(&buf[0..n])
                    }
                }

                Ok(output)
            }
        }
    }

    pub async fn delete_file(&self, file_name: impl AsRef<str>) -> Result<(), VentError> {
        let file_name = file_name.as_ref();
        match self {
            Self::S3(bucket) => {
                bucket.delete_object(file_name).await.context(S3Snafu {
                    action: S3Action::RemovingFile(file_name.to_string())
                })?;
            }
            Self::Local => {
                tokio::fs::remove_file(file_name).await.context(IOSnafu {
                    action: IOAction::DeletingFile(file_name.to_string().into())
                })?;
            }
        }
        Ok(())
    }
    
    pub async fn list_files (&self, dir: impl AsRef<str>) -> Result<Vec<String>, VentError> {
        let dir = dir.as_ref();

        match self {
            Self::S3(bucket) => {
                let response = bucket.list(dir.to_string(), None).await.context(S3Snafu { action: S3Action::ListingFiles(dir.to_string()) })?;
                Ok(response.into_iter().map(|x| {
                    x.contents.into_iter().map(|y| {
                        y.key
                    })
                }).flatten().collect())
            },
            Self::Local => {
                let dir = dir.to_string();
                let mut response = tokio::fs::read_dir(dir.clone()).await.context(IOSnafu {
                    action: IOAction::ReadingDirectory(dir.clone().into())
                })?;

                let mut out = vec![];
                loop {
                    let next_entry = response.next_entry().await.context(IOSnafu {
                        action: IOAction::ReadingDirectory(dir.clone().into())
                    })?;
                    let Some(next_entry) = next_entry else {
                        break;
                    };

                    if let Some(file_name) = next_entry.file_name().to_str() {
                        out.push(file_name.to_string());
                    }
                }
                Ok(out)
            }
        }
    }
}