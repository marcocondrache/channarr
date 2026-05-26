use axum::{Router, http::StatusCode, routing::any};

use crate::state::AppState;

mod health;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", any(root))
        .merge(health::router())
        .with_state(state)
}

async fn root() -> StatusCode {
    StatusCode::NO_CONTENT
}
