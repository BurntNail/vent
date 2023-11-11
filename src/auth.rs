pub mod add_password;
pub mod cloudflare_turnstile;
pub mod login;

pub mod backend;

use std::collections::HashSet;
use axum_login::{AuthnBackend, AuthzBackend};
use change_case::snake_case;
use itertools::Itertools;
use liquid::model::Value;
use liquid::Object;
use rand::{Rng};
use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use strum::IntoEnumIterator;
use crate::auth::backend::Auth;
use crate::error::KnotError;

#[derive(
    sqlx::Type, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize, Debug,
)]
#[sqlx(type_name = "user_role", rename_all = "lowercase")]
pub enum PermissionsRole {
    Participant,
    Prefect,
    Admin,
    Dev,
}

impl PermissionsRole {
    pub fn can (self) -> HashSet<PermissionsTarget> {
        PermissionsTarget::iter().filter(|x| x.can(&self)).collect()
    }
}

#[derive(strum::EnumIter, strum::IntoStaticStr, Copy, Clone, Debug)]
pub enum PermissionsTarget {
    DevAccess,
    ImportCSV,
    RunMigrations,
    EditPeople,
    AddRewards,
    EditEvents,
    ViewPhotoAdders,
    EditPrefectsOnEvents,
    EditParticipantsOnEvents,
    VerifyEvents,
    AddRmSelfToEvent,
    SeePhotos,
    AddPhotos,
}

impl PermissionsTarget {
    pub const fn can (&self, role: &PermissionsRole) -> bool {
        match self {
            PermissionsTarget::DevAccess => role >= &PermissionsRole::Dev,
            PermissionsTarget::ImportCSV => role >= &PermissionsRole::Admin,
            PermissionsTarget::RunMigrations => role >= &PermissionsRole::Admin,
            PermissionsTarget::EditPeople => role >= &PermissionsRole::Admin,
            PermissionsTarget::AddRewards => role >= &PermissionsRole::Admin,
            PermissionsTarget::EditEvents => role >= &PermissionsRole::Prefect,
            PermissionsTarget::ViewPhotoAdders => role >= &PermissionsRole::Prefect,
            PermissionsTarget::EditPrefectsOnEvents => role >= &PermissionsRole::Prefect,
            PermissionsTarget::EditParticipantsOnEvents => role >= &PermissionsRole::Prefect,
            PermissionsTarget::VerifyEvents => role >= &PermissionsRole::Prefect,
            PermissionsTarget::AddRmSelfToEvent => role >= &PermissionsRole::Participant,
            PermissionsTarget::SeePhotos => role >= &PermissionsRole::Participant,
            PermissionsTarget::AddPhotos => role >= &PermissionsRole::Participant,
        }
    }
}


pub async fn get_auth_object(auth: Auth) -> Result<Object, KnotError> {
    let iter = PermissionsTarget::iter().map(|x| (x, snake_case(x.into()).parse().unwrap()));

    match &auth.user {
        Some(x) => {
            let allowed = auth.backend.get_all_permissions(x).await?.into_iter().collect_vec();
            let mut perms = Object::new();
            for (variant, snake) in iter {
                perms.insert(snake, Value::Scalar(allowed.contains(&variant).into()))
            }

            Ok(liquid::object!({"is_logged_in": true, "permissions": perms, "user": x}))
        },
        None => {
            let mut perms = Object::new();
            for (_variant, snake) in iter {
                perms.insert(snake, Value::Scalar(false.into()))
            }

            Ok(liquid::object!({"is_logged_in": false, "permissions": perms}))
        }
    }
}
