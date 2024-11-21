use std::env;
use s3::{Bucket, Region};
use s3::creds::Credentials;
use snafu::ResultExt;
use crate::error::{S3Action, S3Snafu, VentError};

#[derive(Clone, Debug)]
pub struct S3Bucket {
    bucket: Box<Bucket>
}

impl S3Bucket {
    pub fn new () -> Self {
        let aws_creds = {
            let access_key = env::var("AWS_ACCESS_KEY_ID").expect("expected env var AWS_ACCESS_KEY_ID");
            let secret_key =
            env::var("AWS_SECRET_ACCESS_KEY").expect("expected env var AWS_SECRET_ACCESS_KEY");

            Credentials::new(Some(&access_key), Some(&secret_key), None, None, None).unwrap()
        };
        let bucket_name = env::var("BUCKET_NAME").expect("expected env var BUCKET_NAME");
        let endpoint = env::var("AWS_ENDPOINT_URL_S3").expect("expected env var AWS_ENDPOINT_URL_S3");
        let region = Region::Custom {
            region: "auto".to_owned(),
            endpoint,
        };
        let bucket = Bucket::new(&bucket_name, region, aws_creds).unwrap();

        Self {
            bucket
        }
    }

    pub async fn write_file (&self, file_name: impl AsRef<str>, contents: impl AsRef<[u8]>, content_type: &str) -> Result<(), VentError> {
        let file_name = file_name.as_ref();
        self.bucket.put_object_with_content_type(file_name, contents.as_ref(), content_type).await.context(S3Snafu { action: S3Action::PuttingFile(file_name.to_string()) })?;
        Ok(())
    }

    pub async fn read_file (&self, file_name: impl AsRef<str>) -> Result<Vec<u8>, VentError> {
        let file_name = file_name.as_ref();
        let response = self.bucket.get_object(file_name).await.context(S3Snafu { action: S3Action::GettingFile(file_name.to_string()) })?;
        Ok(response.to_vec())
    }
    
    pub async fn delete_file(&self, file_name: impl AsRef<str>) -> Result<(), VentError> {
        let file_name = file_name.as_ref();
        self.bucket.delete_object(file_name).await.context(S3Snafu {
            action: S3Action::RemovingFile(file_name.to_string())
        })?;
        Ok(())
    }
    
    pub async fn list_files (&self, dir: String) -> Result<Vec<String>, VentError> {
        let response = self.bucket.list(dir.clone(), None).await.context(S3Snafu { action: S3Action::ListingFiles(dir) })?;
        Ok(response.into_iter().map(|x| {
            x.contents.into_iter().map(|y| {
                y.key
            })
        }).flatten().collect())
    }
}