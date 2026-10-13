use axum::Router;
use axum::routing::get;
use crate::state::AppState;
use super::health;

pub fn router() -> Router<AppState> {
    Router::new().route("/health", get(health::health))
}
