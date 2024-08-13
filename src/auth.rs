#![allow(clippy::match_same_arms)]

pub mod add_password;
pub mod backend;
pub mod cloudflare_turnstile;
pub mod login;
pub mod pg_session;

use crate::{auth::backend::Auth, error::VentError};
use axum_login::AuthzBackend;
use heck::AsSnakeCase;
use itertools::Itertools;
use liquid::{model::Value, Object};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use strum::IntoEnumIterator;

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
    pub fn can(self) -> HashSet<PermissionsTarget> {
        PermissionsTarget::iter().filter(|x| x.can(self)).collect()
    }
}

#[derive(strum::EnumIter, strum::IntoStaticStr, Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum PermissionsTarget {
    DevAccess,
    ImportCSV,
    ExportCSV,
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
    SeePeople,
    GiveBonusPoints,
    SeeBonusPoints
}

impl PermissionsTarget {
    pub fn can(self, role: PermissionsRole) -> bool {
        match self {
            PermissionsTarget::DevAccess => role >= PermissionsRole::Dev,
            PermissionsTarget::ImportCSV => role >= PermissionsRole::Admin,
            PermissionsTarget::ExportCSV => role >= PermissionsRole::Admin,
            PermissionsTarget::RunMigrations => role >= PermissionsRole::Admin,
            PermissionsTarget::EditPeople => role >= PermissionsRole::Admin,
            PermissionsTarget::AddRewards => role >= PermissionsRole::Admin,
            PermissionsTarget::EditEvents => role >= PermissionsRole::Prefect,
            PermissionsTarget::ViewPhotoAdders => role >= PermissionsRole::Prefect,
            PermissionsTarget::EditPrefectsOnEvents => role >= PermissionsRole::Prefect,
            PermissionsTarget::EditParticipantsOnEvents => role >= PermissionsRole::Prefect,
            PermissionsTarget::VerifyEvents => role >= PermissionsRole::Prefect,
            PermissionsTarget::AddRmSelfToEvent => role >= PermissionsRole::Participant,
            PermissionsTarget::SeePhotos => role >= PermissionsRole::Participant,
            PermissionsTarget::AddPhotos => role >= PermissionsRole::Prefect,
            PermissionsTarget::SeePeople => role >= PermissionsRole::Prefect,
            PermissionsTarget::GiveBonusPoints => role >= PermissionsRole::Admin,
            PermissionsTarget::SeeBonusPoints => role >= PermissionsRole::Participant
        }
    }
}

pub async fn get_auth_object(auth: Auth) -> Result<Object, VentError> {
    let iter = PermissionsTarget::iter().map(|x| {
        let pre_snake_case: &'static str = x.into();
        (x, AsSnakeCase(pre_snake_case).to_string().parse().expect("unable to convert string to kstring"))
    });

    match &auth.user {
        Some(x) => {
            let allowed = auth
                .backend
                .get_all_permissions(x)
                .await?
                .into_iter()
                .collect_vec();
            let mut perms = Object::new();
            for (variant, snake) in iter {
                perms.insert(snake, Value::Scalar(allowed.contains(&variant).into()));
            }

            Ok(liquid::object!({"is_logged_in": true, "permissions": perms, "user": x}))
        }
        None => {
            let mut perms = Object::new();
            for (_variant, snake) in iter {
                perms.insert(snake, Value::Scalar(false.into()));
            }

            Ok(liquid::object!({"is_logged_in": false, "permissions": perms}))
        }
    }
}
