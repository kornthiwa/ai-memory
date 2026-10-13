use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};

use crate::error::AppResult;
use crate::state::AppState;

pub async fn health(State(state): State<AppState>) -> AppResult<Json<Value>> {
    sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.db)
        .await?;

    Ok(Json(json!({
        "status": "ok",
        "database": "up"
    })))
}
