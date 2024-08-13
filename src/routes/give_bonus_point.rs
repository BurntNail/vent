use crate::{
    auth::{
        backend::{Auth, VentAuthBackend},
        get_auth_object, PermissionsTarget,
    },
    error::{EncodeStep, ParseTimeSnafu, SqlxAction, SqlxSnafu, VentError},
    liquid_utils::compile_with_newtitle,
    routes::FormEvent,
    state::VentState,
};
use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
    routing::get,
    Router,
};
use axum_extra::extract::Form;
use axum_login::permission_required;
use chrono::NaiveDateTime;
use snafu::ResultExt;
use crate::routes::FormBonusPoint;
use chrono::Utc;

#[allow(clippy::too_many_lines)]
#[axum::debug_handler]
async fn get_give_bonus_points_form(
    auth: Auth,
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
    let aa = get_auth_object(auth).await?;

    compile_with_newtitle(
        "www/give_bonus_point.liquid",
        liquid::object!({"auth": aa}),
        &state.settings.brand.instance_name,
        Some("Give Bonus Point".into()),
    )
        .await
}

#[axum::debug_handler]
async fn post_give_bonus_points_form(
    State(state): State<VentState>,
    Form(FormBonusPoint {
             user_id,
             reason,
             quantity
         }): Form<FormBonusPoint>,
) -> Result<impl IntoResponse, VentError> {
    let date = Utc::now().naive_utc();

    let id = sqlx::query!(
        r#"
INSERT INTO public.bonus_points (point_date, staff_member_id, num_points, reason)
VALUES ($1, $2, $3, $4)
RETURNING id
        "#,
        date, user_id, quantity, reason
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
        .route("/give_bonus_point", get(get_give_bonus_points_form).post(post_give_bonus_points_form))
        .route_layer(permission_required!(
            VentAuthBackend,
            login_url = "/login",
            PermissionsTarget::GiveBonusPoints
        ))
}
