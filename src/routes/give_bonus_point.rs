use crate::{
    auth::{
        backend::{Auth, VentAuthBackend},
        get_auth_object, PermissionsTarget,
    },
    error::{SqlxAction, SqlxSnafu, VentError},
    routes::FormBonusPoint,
    state::VentState,
};
use axum::{
    extract::State,
    response::{IntoResponse, Redirect, Response},
    routing::get,
    Router,
};
use axum_extra::extract::Form;
use axum_login::permission_required;
use chrono::Utc;
use dotenvy::var;
use snafu::ResultExt;

#[allow(clippy::too_many_lines)]
#[axum::debug_handler]
async fn get_give_bonus_points_form(
    auth: Auth,
    State(state): State<VentState>,
) -> Result<Response, VentError> {
    if var("HIDE_BONUS_POINTS").is_ok() {
        return Ok(Redirect::to("/").into_response());
    }
    let aa = get_auth_object(auth).await?;

    let page = state
        .compile(
            "www/give_bonus_point.liquid",
            liquid::object!({"auth": aa}),
            Some("Give Bonus Point".into()),
        )
        .await?;

    Ok(page.into_response())
}

#[axum::debug_handler]
async fn post_give_bonus_points_form(
    State(state): State<VentState>,
    Form(FormBonusPoint {
        user_id,
        reason,
        quantity,
    }): Form<FormBonusPoint>,
) -> Result<impl IntoResponse, VentError> {
    let date = Utc::now().naive_utc();

    let id = sqlx::query!(
        r#"
INSERT INTO public.bonus_points (point_date, staff_member_id, num_points, reason)
VALUES ($1, $2, $3, $4)
RETURNING id
        "#,
        date,
        user_id,
        quantity,
        reason
    )
    .fetch_one(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::AddingBonusPoint,
    })?
    .id;

    state.update_events()?;

    Ok(Redirect::to(&format!("/update_bonus_point/{id}")))
}
pub fn router() -> Router<VentState> {
    Router::new()
        .route(
            "/give_bonus_point",
            get(get_give_bonus_points_form).post(post_give_bonus_points_form),
        )
        .route_layer(permission_required!(
            VentAuthBackend,
            login_url = "/login",
            PermissionsTarget::GiveBonusPoints
        ))
}
