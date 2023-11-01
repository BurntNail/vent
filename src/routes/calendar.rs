//! Module that publishes an iCalendar file in a GET method

use crate::{error::KnotError, state::KnotState};
use axum::{extract::State, response::IntoResponse};
use std::time::Duration;

use super::public::serve_static_file;

#[instrument(level = "debug", skip(state))]
#[axum::debug_handler]
pub async fn get_calendar_feed(
    State(state): State<KnotState>,
) -> Result<impl IntoResponse, KnotError> {
    if !(state.ensure_calendar_exists().await?) {
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    serve_static_file("calendar.ics").await
}
