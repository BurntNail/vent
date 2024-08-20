use crate::{
    auth::{
        backend::{Auth, VentAuthBackend},
        get_auth_object, PermissionsTarget,
    },
    error::{SqlxAction, SqlxSnafu, VentError},
    liquid_utils::compile_with_newtitle,
    state::VentState,
};
use axum::{
    extract::State,
    response::IntoResponse,
    routing::get,
    Router,
};
use axum_login::permission_required;
use serde::Serialize;
use snafu::ResultExt;

async fn get_show_bonus_points(
    auth: Auth,
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
    let aa = get_auth_object(auth).await?;

    #[derive(Serialize)]
    struct BonusPoint {
        bonus_point_id: i32,
        reason: String,
        num_points: i32,
        date: String,
        staff_member_username: String,
    }
    let bonus_points_vec: Vec<BonusPoint> = sqlx::query!("SELECT bonus_points.id, reason, num_points, point_date, people.username FROM bonus_points INNER JOIN people ON bonus_points.staff_member_id = people.id").fetch_all(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu { action: SqlxAction::GettingBonusPoints })?
        .into_iter().map(|row| {
        BonusPoint {
            bonus_point_id: row.id,
            reason: row.reason,
            num_points: row.num_points,
            date: row.point_date.date().format("%d/%m/%Y").to_string(),
            staff_member_username: row.username
        }
    }).collect();
    compile_with_newtitle(
        "www/show_bonus_points.liquid",
        liquid::object!({ "bonus_points": bonus_points_vec,"auth": aa }),
        &state.settings.brand.instance_name,
        Some("All Bonus Points".into()),
    )
        .await
}

pub fn router() -> Router<VentState> {
    Router::new()
        .route("/show_bonus_points", get(get_show_bonus_points))
        .route_layer(permission_required!(
            VentAuthBackend,
            login_url = "/login",
            PermissionsTarget::GiveBonusPoints
        ))
}