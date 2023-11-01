//! Module that publishes an iCalendar file in a GET/HEAD method

use crate::{error::KnotError, state::KnotState};
use axum::{extract::State, response::IntoResponse};

use super::public::serve_static_file;

#[instrument(level = "debug", skip(state))]
#[axum::debug_handler]
pub async fn get_calendar_feed(
    State(state): State<KnotState>,
) -> Result<impl IntoResponse, KnotError> {
    state.ensure_calendar_exists().await?;
    serve_static_file("calendar.ics").await
}
