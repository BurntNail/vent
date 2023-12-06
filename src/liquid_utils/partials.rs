use crate::{
    auth::{backend::VentAuthBackend, PermissionsTarget},
    state::VentState,
};
use axum::{
    response::{IntoResponse, Redirect},
    routing::get,
    Router,
};
use axum_login::permission_required;
use liquid::partials::{EagerCompiler, PartialSource};
use once_cell::sync::Lazy;
use std::{
    borrow::Cow,
    collections::HashMap,
    ffi::{OsStr, OsString},
};
use tokio::{fs::read_to_string, sync::RwLock};
use walkdir::{DirEntry, WalkDir};

///Struct to hold all the partials - then I can use a convenience function to easily get a [`PartialCompiler`]
#[derive(Debug, Clone, Default)]
pub struct Partials(HashMap<String, String>);

impl Partials {
    ///Get an [`EagerCompiler`] from the `self`
    pub fn to_compiler(&self) -> EagerCompiler<Self> {
        EagerCompiler::new(self.clone())
    }

    pub async fn reload(&mut self) {
        self.0 = get_partials().await;
    }
}

impl PartialSource for Partials {
    fn contains(&self, name: &str) -> bool {
        self.0.contains_key(name)
    }

    fn names(&self) -> Vec<&str> {
        self.0.keys().map(String::as_str).collect()
    }

    fn try_get<'a>(&'a self, name: &str) -> Option<Cow<'a, str>> {
        self.0.get(name).map(|s| s.as_str().into())
    }
}

///Static variable to store all of the partials
pub static PARTIALS: Lazy<RwLock<Partials>> = Lazy::new(|| RwLock::new(Partials::default()));

#[axum::debug_handler]
async fn reload_partials() -> impl IntoResponse {
    debug!("Reloading Partials");
    PARTIALS.write().await.reload().await;
    Redirect::to("/")
}

///Async function to get `Partials` - used to set [`PARTIALS`]
///
/// Looks for the Partials in `www/partials/`, and sets their `liquid` names to be in the `partials/` directory, and accepts `html`, and `liquid` extensions
async fn get_partials() -> HashMap<String, String> {
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

    let mut in_memory_source = HashMap::new(); //make a new source

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
                info!(?partial, "Loading Partial");
                if let Some(name) = partial.file_name().and_then(OsStr::to_str) {
                    in_memory_source.insert(LIQUID_PARTIALS_NAME.to_string() + name, source);
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

    in_memory_source
}

pub fn router() -> Router<VentState> {
    Router::new()
        .route("/reload_partials", get(reload_partials))
        .route_layer(permission_required!(
            VentAuthBackend,
            PermissionsTarget::DevAccess
        ))
}
