use std::ffi::{OsStr, OsString};

use liquid::partials::{EagerCompiler, InMemorySource};
use tokio::{fs::read_to_string, sync::OnceCell};
use walkdir::{DirEntry, WalkDir};

///Struct to hold all the partials - then I can use a convenience function to easily get a [`PartialCompiler`]
#[derive(Debug)]
pub struct Partials(InMemorySource);

impl Partials {
    ///Get an [`EagerCompiler`] from the `self`
    ///
    ///NB: This will not reflect any file changes, the server needs to be restarted for that.
    pub fn to_compiler(&self) -> EagerCompiler<InMemorySource> {
        EagerCompiler::new(self.0.clone())
    }
}

///Static variable to store all of the partials
pub static PARTIALS: OnceCell<Partials> = OnceCell::const_new();

///Async function to get `Partials` - used to set [`PARTIALS`]
///
/// Looks for the Partials in `www/partials/`, and sets their `liquid` names to be in the `partials/` directory, and accepts `html`, and `liquid` extensions
pub async fn init_partials() -> Partials {
    ///The directory which contains the partials
    const PARTIALS_DIR: &str = "www/partials/";
    ///The name that the partials will have for use in embedding, eg. "www/partials/li.liquid" would be referenced as `{% include "partials/li.liquid" %}`
    const LIQUID_PARTIALS_NAME: &str = "partials/";
    ///The accepted extensions for partials
    const PARTIALS_EXTENSIONS: &[&str] = &["html", "liquid"];

    let partial_extensions = PARTIALS_EXTENSIONS
        .iter()
        .map(OsString::from) //get OsStrings from the allowed extensions
        .collect::<Vec<_>>(); //must do outside of const as this is not const

    let mut in_memory_source = InMemorySource::new(); //make a new source

    for partial in WalkDir::new(PARTIALS_DIR)
        .into_iter() //for every file in PARTIALS_DIR
        .filter_map(Result::ok) //that we can access
        .map(DirEntry::into_path) //get it as a path
        .filter(|x| {
            //and check it has one of the extensions
            x.extension().map_or(false, |x| {
                //if it doesn't have an extension, ignore
                partial_extensions.iter().any(|allowed| x == allowed)
            })
        })
    {
        match read_to_string(&partial).await {
            Ok(source) => {
                info!(?partial, "Got partial");
                if let Some(name) = partial.file_name().and_then(OsStr::to_str) {
                    in_memory_source.add(LIQUID_PARTIALS_NAME.to_string() + name, source);
                //add partial
                } else {
                    error!("Got partial, could not transform name to UTF-8");
                }
            }
            Err(e) => {
                error!(?partial, ?e, "Error reading partial");
            }
        }
    }

    Partials(in_memory_source) //return partials
}
