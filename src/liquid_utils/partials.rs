use std::ffi::{OsString, OsStr};

use liquid::partials::{EagerCompiler, InMemorySource};
use tokio::{fs::read_to_string, sync::OnceCell};
use walkdir::{WalkDir, DirEntry};

#[derive(Debug)]
pub struct Partials(InMemorySource);

impl Partials {
    pub fn to_compiler(&self) -> EagerCompiler<InMemorySource> {
        EagerCompiler::new(self.0.clone())
    }
}

pub static PARTIALS: OnceCell<Partials> = OnceCell::const_new();

pub async fn init_partials() -> Partials {
    const PARTIALS_DIR: &str = "www/partials/";
    const LIQUID_PARTIALS_NAME: &str = "partials/";
    const PARTIALS_EXTENSIONS: &[&str] = &["html", "liquid"];

    let partial_extensions = PARTIALS_EXTENSIONS.iter().map(OsString::from).collect::<Vec<_>>(); //must do outside of const as this is not const

    let mut in_memory_source = InMemorySource::new();

    for partial in WalkDir::new(PARTIALS_DIR).into_iter().filter_map(Result::ok).map(DirEntry::into_path).filter(|x| {
        x.extension().map_or(false, |x| partial_extensions.iter().any(|allowed| x == allowed))
    }) {
        match read_to_string(&partial).await {
            Ok(source) => {
                info!(?partial, "Got partial");
                if let Some(name) = partial.file_name().and_then(OsStr::to_str) {
                    in_memory_source.add(LIQUID_PARTIALS_NAME.to_string() + name, source);
                } else {
                    error!("Got partial, could not transform name to UTF-8");
                }
            }
            Err(e) => {
                error!(?partial, ?e, "Error reading partial");
            }
        }
    }

    Partials(in_memory_source)
}
